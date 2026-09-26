//! Yahoo Finance adapter for [`ResearchGateway`], via yfinance-rs.

mod fundamentals;

use crate::research::repositories::{RawFundamentals, ResearchGateway};
use async_trait::async_trait;
use dtos::{
    research::{
        humanize, CorporateAction, Holders, InsiderTrade, Institution, NewsItem, OptionChainView,
        OptionQuote, RatingChange,
    },
};
use rust_decimal::prelude::ToPrimitive;
use types::ticker_symbol::TickerSymbol;
use yfinance_rs::{Action, Range, Ticker, YfClient};

/// Holds one client so Yahoo's cookie / crumb is reused across requests.
pub struct YahooResearchGateway {
    client: YfClient,
}

impl YahooResearchGateway {
    pub fn new() -> Self {
        Self {
            client: crate::shared::yahoo_client(),
        }
    }

    fn ticker(&self, t: &TickerSymbol) -> Ticker {
        Ticker::new(&self.client, t.as_str())
    }
}

fn dec(d: rust_decimal::Decimal) -> Option<f64> {
    d.to_f64()
}

fn debug_name(v: impl std::fmt::Debug) -> String {
    humanize(&format!("{v:?}"))
}

#[async_trait]
impl ResearchGateway for YahooResearchGateway {
    async fn fundamentals(&self, ticker: &TickerSymbol) -> Result<RawFundamentals, String> {
        fundamentals::fetch(&self.client, ticker.as_str()).await
    }

    async fn news(&self, ticker: &TickerSymbol) -> Result<Vec<NewsItem>, String> {
        let articles = self
            .ticker(ticker)
            .news()
            .await
            .map_err(|e| e.to_string())?;
        Ok(articles
            .into_iter()
            .map(|a| NewsItem {
                title: a.title,
                publisher: a.publisher,
                link: a.link,
                published_at: a.published_at.format("%Y-%m-%d %H:%M").to_string(),
            })
            .collect())
    }

    async fn rating_changes(&self, ticker: &TickerSymbol) -> Result<Vec<RatingChange>, String> {
        let rows = self
            .ticker(ticker)
            .upgrades_downgrades()
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| RatingChange {
                date: r.ts.date_naive().to_string(),
                firm: r.firm,
                from: r.from_grade.map(debug_name),
                to: r.to_grade.map(debug_name),
                action: r.action.map(debug_name),
            })
            .collect())
    }

    async fn holders(&self, ticker: &TickerSymbol) -> Result<Holders, String> {
        let t = self.ticker(ticker);
        let (major, institutions, insiders) = tokio::join!(
            t.major_holders(),
            t.institutional_holders(),
            t.insider_transactions()
        );
        if let (Err(e), Err(_), Err(_)) = (&major, &institutions, &insiders) {
            return Err(e.to_string());
        }
        Ok(Holders {
            breakdown: major
                .unwrap_or_default()
                .into_iter()
                .filter_map(|m| Some((m.category, dec(m.value.into_inner())?)))
                .collect(),
            institutions: institutions
                .unwrap_or_default()
                .into_iter()
                .map(|h| Institution {
                    name: h.holder,
                    shares: h.shares.map(|s| s as f64),
                    pct_held: h.pct_held.and_then(|p| dec(p.into_inner())),
                    value: h.value.and_then(|v| dec(v.amount())),
                    date: h.date_reported.to_string(),
                })
                .collect(),
            insider_trades: insiders
                .unwrap_or_default()
                .into_iter()
                .map(|i| InsiderTrade {
                    name: i.insider,
                    position: debug_name(i.position),
                    kind: debug_name(i.transaction_type),
                    shares: i.shares.map(|s| s as f64),
                    value: i.value.and_then(|v| dec(v.amount())),
                    date: i.transaction_date.to_string(),
                })
                .collect(),
        })
    }

    async fn corporate_actions(
        &self,
        ticker: &TickerSymbol,
    ) -> Result<Vec<CorporateAction>, String> {
        let actions = self
            .ticker(ticker)
            .actions(Some(Range::Max))
            .await
            .map_err(|e| e.to_string())?;
        Ok(actions
            .into_iter()
            .filter_map(|a| match a {
                Action::Dividend { date, amount } => Some(CorporateAction::Dividend {
                    date: date.to_string(),
                    amount: dec(amount.amount())?,
                }),
                Action::Split {
                    date,
                    numerator,
                    denominator,
                } => Some(CorporateAction::Split {
                    date: date.to_string(),
                    numerator: numerator.get(),
                    denominator: denominator.get(),
                }),
                Action::CapitalGain { date, gain } => Some(CorporateAction::CapitalGain {
                    date: date.to_string(),
                    amount: dec(gain.amount())?,
                }),
                #[allow(unreachable_patterns)]
                _ => None,
            })
            .collect())
    }

    async fn option_chain(
        &self,
        ticker: &TickerSymbol,
        expiration: Option<i64>,
    ) -> Result<OptionChainView, String> {
        let t = self.ticker(ticker);
        let expirations = t.options().await.map_err(|e| e.to_string())?;
        let chosen = expiration
            .filter(|e| expirations.contains(e))
            .or_else(|| expirations.first().copied());
        let Some(date) = chosen else {
            return Ok(OptionChainView::default());
        };
        let chain = t
            .option_chain(Some(date))
            .await
            .map_err(|e| e.to_string())?;
        let (mut calls, mut puts) = (Vec::new(), Vec::new());
        for c in chain.contracts {
            let quote = OptionQuote {
                strike: dec(c.key.strike.amount()).unwrap_or_default(),
                last: c.price.and_then(|p| dec(p.into_inner())),
                bid: c.bid.and_then(|p| dec(p.into_inner())),
                ask: c.ask.and_then(|p| dec(p.into_inner())),
                volume: c.volume,
                open_interest: c.open_interest,
                implied_volatility: c.implied_volatility.and_then(|v| dec(v.into_inner())),
                in_the_money: c.in_the_money.unwrap_or(false),
            };
            if format!("{:?}", c.key.side).eq_ignore_ascii_case("call") {
                calls.push(quote);
            } else {
                puts.push(quote);
            }
        }
        calls.sort_by(|a, b| a.strike.total_cmp(&b.strike));
        puts.sort_by(|a, b| a.strike.total_cmp(&b.strike));
        Ok(OptionChainView {
            expirations,
            expiration: Some(date),
            calls,
            puts,
        })
    }
}
