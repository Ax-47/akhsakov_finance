//! Market-data port for research.

use async_trait::async_trait;
use dtos::{
    fundamentals::StockFundamentals,
    research::{CorporateAction, Holders, NewsItem, OptionChainView, RatingChange},
};
use types::ticker_symbol::TickerSymbol;

/// Errors are provider messages, e.g. rate limiting.
#[async_trait]
pub trait ResearchGateway: Send + Sync {
    async fn fundamentals(&self, ticker: &TickerSymbol) -> Result<StockFundamentals, String>;
    async fn news(&self, ticker: &TickerSymbol) -> Result<Vec<NewsItem>, String>;
    async fn rating_changes(&self, ticker: &TickerSymbol) -> Result<Vec<RatingChange>, String>;
    async fn holders(&self, ticker: &TickerSymbol) -> Result<Holders, String>;
    async fn corporate_actions(
        &self,
        ticker: &TickerSymbol,
    ) -> Result<Vec<CorporateAction>, String>;
    /// `expiration` in unix seconds; `None` for the nearest.
    async fn option_chain(
        &self,
        ticker: &TickerSymbol,
        expiration: Option<i64>,
    ) -> Result<OptionChainView, String>;
}
