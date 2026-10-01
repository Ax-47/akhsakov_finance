//! Storage ports for named connector credentials and paper portfolios.

use crate::shared::RepositoryError;
use dtos::mcp::{McpAccessPreset, McpAuditEvent, McpConnection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ConnectionRecord {
    pub connection: McpConnection,
    pub token_hash: String,
}

pub trait ConnectionRepository: Send + Sync {
    fn list(&self) -> Result<Vec<McpConnection>, RepositoryError>;
    fn find(&self, id: Uuid) -> Result<Option<ConnectionRecord>, RepositoryError>;
    fn create(&self, record: &ConnectionRecord) -> Result<(), RepositoryError>;
    fn update(&self, id: Uuid, name: &str, preset: McpAccessPreset, enabled: bool, portfolios: &[Uuid]) -> Result<(), RepositoryError>;
    fn rotate(&self, id: Uuid, token_hash: &str) -> Result<(), RepositoryError>;
    fn delete(&self, id: Uuid) -> Result<(), RepositoryError>;
    fn touch(&self, id: Uuid) -> Result<(), RepositoryError>;
    fn audit(&self, id: Uuid, method: &str, tool: Option<&str>, portfolios: &[Uuid], success: bool, error: Option<&str>) -> Result<(), RepositoryError>;
    fn events(&self, id: Uuid, limit: usize) -> Result<Vec<McpAuditEvent>, RepositoryError>;
    /// Whether the connection drives a contestant in a running or paused race.
    fn in_active_race(&self, id: Uuid) -> Result<bool, RepositoryError>;
}

/// Which portfolios an AI assistant manages.
pub trait AiPortfolioRepository: Send + Sync {
    /// Oldest first.
    fn ai_portfolios(&self) -> Result<Vec<uuid::Uuid>, RepositoryError>;
    fn add_ai_portfolio(&self, id: uuid::Uuid) -> Result<(), RepositoryError>;
    /// Hands the portfolio back to you; it and its trades stay.
    fn remove_ai_portfolio(&self, id: uuid::Uuid) -> Result<(), RepositoryError>;
}

/// A live price to trade at, in the instrument's own currency.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveQuote {
    pub price: rust_decimal::Decimal,
    pub previous_close: rust_decimal::Decimal,
    pub currency: String,
    /// USD per unit of `currency`, now.
    pub usd_per_unit: rust_decimal::Decimal,
    /// Unix seconds.
    pub timestamp: i64,
    /// The provider couldn't be reached; this is the last saved price.
    pub stale: bool,
}

/// Market prices port, so the trading tools can be tested without Yahoo.
#[async_trait::async_trait]
pub trait LivePrices: Send + Sync {
    async fn quote(&self, ticker: &types::ticker_symbol::TickerSymbol) -> Result<LiveQuote, String>;
}

/// Market research the read tools pass on, so they can be tested without
/// the network. Answers are JSON for the model to read.
#[async_trait::async_trait]
pub trait MarketData: Send + Sync {
    /// Closes in USD (stocks and funds) as `(YYYY-MM-DD, close)`, oldest first.
    async fn closes(
        &self,
        ticker: &types::ticker_symbol::TickerSymbol,
        range: types::range::Range,
        interval: types::interval::Interval,
    ) -> Result<Vec<(String, f64)>, String>;
    /// Valuation, profitability, dividends and analyst targets.
    async fn fundamentals(&self, ticker: &types::ticker_symbol::TickerSymbol) -> Result<serde_json::Value, String>;
    /// Upcoming earnings and ex-dividend dates, soonest first.
    async fn calendar(&self, tickers: Vec<types::ticker_symbol::TickerSymbol>) -> Result<serde_json::Value, String>;
}
