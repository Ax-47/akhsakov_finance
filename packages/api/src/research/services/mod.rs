//! Research use cases: they own ordering, limits and conversion to USD (the
//! app's base currency) so every client sees the same view.

use crate::{
    research::repositories::{RawFundamentals, ResearchGateway},
    shared::{FxRates, ServiceError},
};
use dtos::{
    fundamentals::{valuation_history, StockFundamentals},
    research::{CorporateAction, Holders, NewsItem, OptionChainView, RatingChange},
};
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;

const MAX_NEWS: usize = 20;
const MAX_RATING_CHANGES: usize = 30;

#[derive(Clone)]
pub struct ResearchService {
    gateway: Arc<dyn ResearchGateway>,
    fx: Arc<dyn FxRates>,
}

impl ResearchService {
    pub fn new(gateway: Arc<dyn ResearchGateway>, fx: Arc<dyn FxRates>) -> Self {
        Self { gateway, fx }
    }

    /// USD per unit of `currency` now, as a float.
    async fn rate(&self, currency: &str) -> Result<f64, ServiceError> {
        if currency == "USD" {
            return Ok(1.0);
        }
        self.fx
            .usd_per_unit_now(currency)
            .await
            .map_err(ServiceError::Upstream)?
            .to_f64()
            .ok_or_else(|| ServiceError::Upstream(format!("bad {currency} rate")))
    }

    /// USD per unit of the currency `t` trades in.
    async fn rate_for(&self, t: &TickerSymbol) -> Result<f64, ServiceError> {
        let currency = self.fx.currency_of(t).await.map_err(ServiceError::Upstream)?;
        self.rate(&currency).await
    }

    /// Everything in USD; quarter-end valuations use the converted prices
    /// and statements, so ratios hold even when the two currencies differ.
    pub async fn fundamentals(&self, t: &TickerSymbol) -> Result<StockFundamentals, ServiceError> {
        let RawFundamentals { mut data, closes } = self
            .gateway
            .fundamentals(t)
            .await
            .map_err(ServiceError::Upstream)?;
        let trading = self.rate(&data.currency).await?;
        let financial = if data.financial_currency == data.currency {
            trading
        } else {
            self.rate(&data.financial_currency).await?
        };
        data.to_usd(trading, financial);
        let closes: Vec<(String, f64)> = closes.into_iter().map(|(d, c)| (d, c * trading)).collect();
        data.valuation = valuation_history(&data.quarterly, &closes);
        Ok(data)
    }

    /// Newest first.
    pub async fn news(&self, t: &TickerSymbol) -> Result<Vec<NewsItem>, ServiceError> {
        let mut news = self.gateway.news(t).await.map_err(ServiceError::Upstream)?;
        news.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        news.truncate(MAX_NEWS);
        Ok(news)
    }

    /// Newest first.
    pub async fn rating_changes(
        &self,
        t: &TickerSymbol,
    ) -> Result<Vec<RatingChange>, ServiceError> {
        let mut rows = self
            .gateway
            .rating_changes(t)
            .await
            .map_err(ServiceError::Upstream)?;
        rows.sort_by(|a, b| b.date.cmp(&a.date));
        rows.truncate(MAX_RATING_CHANGES);
        Ok(rows)
    }

    pub async fn holders(&self, t: &TickerSymbol) -> Result<Holders, ServiceError> {
        let mut h = self
            .gateway
            .holders(t)
            .await
            .map_err(ServiceError::Upstream)?;
        h.insider_trades.sort_by(|a, b| b.date.cmp(&a.date));
        h.scale(self.rate_for(t).await?);
        Ok(h)
    }

    /// Newest first.
    pub async fn corporate_actions(
        &self,
        t: &TickerSymbol,
    ) -> Result<Vec<CorporateAction>, ServiceError> {
        let mut actions = self
            .gateway
            .corporate_actions(t)
            .await
            .map_err(ServiceError::Upstream)?;
        actions.sort_by(|a, b| b.date().cmp(a.date()));
        let rate = self.rate_for(t).await?;
        actions.iter_mut().for_each(|a| a.scale(rate));
        Ok(actions)
    }

    pub async fn option_chain(
        &self,
        t: &TickerSymbol,
        expiration: Option<i64>,
    ) -> Result<OptionChainView, ServiceError> {
        let mut chain = self
            .gateway
            .option_chain(t, expiration)
            .await
            .map_err(ServiceError::Upstream)?;
        chain.scale(self.rate_for(t).await?);
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// Fake provider returning news out of order.
    struct FakeGateway;

    #[async_trait]
    impl ResearchGateway for FakeGateway {
        async fn fundamentals(&self, t: &TickerSymbol) -> Result<RawFundamentals, String> {
            if t.as_str() != "PTT.BK" {
                return Err("rate limited".into());
            }
            let mut data = StockFundamentals::default();
            data.currency = "THB".into();
            data.financial_currency = "THB".into();
            data.stats.price = Some(35.0);
            data.quarterly = vec![dtos::fundamentals::PeriodFinancials {
                period: "2026Q1".into(),
                end_date: Some("2026-03-31".into()),
                revenue: Some(1000.0),
                shares: Some(10.0),
                ..Default::default()
            }];
            Ok(RawFundamentals {
                data,
                closes: vec![("2026-03-31".into(), 30.0)],
            })
        }
        async fn news(&self, _: &TickerSymbol) -> Result<Vec<NewsItem>, String> {
            Ok((0..30)
                .map(|i| NewsItem {
                    title: format!("n{i}"),
                    publisher: None,
                    link: None,
                    published_at: format!("2026-01-{:02} 10:00", i % 28 + 1),
                })
                .collect())
        }
        async fn rating_changes(&self, _: &TickerSymbol) -> Result<Vec<RatingChange>, String> {
            Ok(vec![])
        }
        async fn holders(&self, _: &TickerSymbol) -> Result<Holders, String> {
            Ok(Holders::default())
        }
        async fn corporate_actions(
            &self,
            _: &TickerSymbol,
        ) -> Result<Vec<CorporateAction>, String> {
            Ok(vec![
                CorporateAction::Dividend {
                    date: "2024-01-01".into(),
                    amount: 0.1,
                },
                CorporateAction::Split {
                    date: "2025-06-10".into(),
                    numerator: 10,
                    denominator: 1,
                },
            ])
        }
        async fn option_chain(
            &self,
            _: &TickerSymbol,
            _: Option<i64>,
        ) -> Result<OptionChainView, String> {
            Ok(OptionChainView::default())
        }
    }

    /// THB is 0.03 USD; every stock trades in THB.
    struct FakeFx;

    #[async_trait]
    impl FxRates for FakeFx {
        async fn usd_per_unit(&self, c: &str, _: &str) -> Result<rust_decimal::Decimal, String> {
            self.usd_per_unit_now(c).await
        }
        async fn usd_per_unit_now(&self, c: &str) -> Result<rust_decimal::Decimal, String> {
            match c {
                "THB" => Ok(rust_decimal::Decimal::new(3, 2)),
                _ => Ok(rust_decimal::Decimal::ONE),
            }
        }
        async fn currency_of(&self, _: &TickerSymbol) -> Result<String, String> {
            Ok("THB".into())
        }
    }

    #[tokio::test]
    async fn converts_to_usd() {
        let s = ResearchService::new(Arc::new(FakeGateway), Arc::new(FakeFx));
        let f = s.fundamentals(&TickerSymbol::new("PTT.BK").unwrap()).await.unwrap();
        assert_eq!(f.currency, "USD");
        assert!((f.stats.price.unwrap() - 1.05).abs() < 1e-9);
        assert!((f.quarterly[0].revenue.unwrap() - 30.0).abs() < 1e-9);
        // Quarter-end price 30 THB = $0.90; 10 shares → $9 market cap.
        assert!((f.valuation[0].market_cap.unwrap() - 9.0).abs() < 1e-9);
        let actions = s.corporate_actions(&TickerSymbol::new("PTT.BK").unwrap()).await.unwrap();
        let dividend = actions.iter().find_map(|a| match a {
            CorporateAction::Dividend { amount, .. } => Some(*amount),
            _ => None,
        });
        assert!((dividend.unwrap() - 0.003).abs() < 1e-12);
    }

    #[tokio::test]
    async fn orders_limits_and_maps_errors() {
        let s = ResearchService::new(Arc::new(FakeGateway), Arc::new(FakeFx));
        let t = TickerSymbol::new("NVDA").unwrap();
        let news = s.news(&t).await.unwrap();
        assert_eq!(news.len(), MAX_NEWS);
        assert!(news
            .windows(2)
            .all(|w| w[0].published_at >= w[1].published_at));
        assert_eq!(
            s.corporate_actions(&t).await.unwrap()[0].date(),
            "2025-06-10"
        );
        assert!(
            matches!(s.fundamentals(&t).await, Err(ServiceError::Upstream(m)) if m == "rate limited")
        );
    }
}
