//! Storage port for the connector's key.

use crate::shared::RepositoryError;

pub trait KeyRepository: Send + Sync {
    /// `None` while the connector is off.
    fn key(&self) -> Result<Option<String>, RepositoryError>;
    /// Replaces the key; `None` turns the connector off.
    fn set_key(&self, key: Option<&str>) -> Result<(), RepositoryError>;
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
