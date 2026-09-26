//! Research use cases: thin, but they own ordering and limits so every
//! client sees the same view.

use crate::{research::repositories::ResearchGateway, shared::ServiceError};
use dtos::{
    fundamentals::StockFundamentals,
    research::{CorporateAction, Holders, NewsItem, OptionChainView, RatingChange},
};
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;

const MAX_NEWS: usize = 20;
const MAX_RATING_CHANGES: usize = 30;

#[derive(Clone)]
pub struct ResearchService {
    gateway: Arc<dyn ResearchGateway>,
}

impl ResearchService {
    pub fn new(gateway: Arc<dyn ResearchGateway>) -> Self {
        Self { gateway }
    }

    pub async fn fundamentals(&self, t: &TickerSymbol) -> Result<StockFundamentals, ServiceError> {
        self.gateway
            .fundamentals(t)
            .await
            .map_err(ServiceError::Upstream)
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
        Ok(actions)
    }

    pub async fn option_chain(
        &self,
        t: &TickerSymbol,
        expiration: Option<i64>,
    ) -> Result<OptionChainView, ServiceError> {
        self.gateway
            .option_chain(t, expiration)
            .await
            .map_err(ServiceError::Upstream)
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
        async fn fundamentals(&self, _: &TickerSymbol) -> Result<StockFundamentals, String> {
            Err("rate limited".into())
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

    #[tokio::test]
    async fn orders_limits_and_maps_errors() {
        let s = ResearchService::new(Arc::new(FakeGateway));
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
