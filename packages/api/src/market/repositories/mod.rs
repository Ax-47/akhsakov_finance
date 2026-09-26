//! Ports for the market overview.

use async_trait::async_trait;
use dtos::market::MarketIndex;
use types::ticker_symbol::TickerSymbol;

/// A company in an index.
#[derive(Debug, Clone, PartialEq)]
pub struct Constituent {
    pub ticker: &'static str,
    pub name: &'static str,
    pub sector: &'static str,
    /// Shares outstanding, billions; market cap is this times the live price.
    pub shares_bn: f64,
}

/// Which companies make up an index.
pub trait IndexRepository: Send + Sync {
    fn constituents(&self, index: MarketIndex) -> Vec<Constituent>;
}

/// Latest price for one instrument.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketQuote {
    pub ticker: TickerSymbol,
    pub name: Option<String>,
    pub price: f64,
    pub previous_close: Option<f64>,
}

/// Errors are provider messages, e.g. rate limiting.
#[async_trait]
pub trait MarketDataGateway: Send + Sync {
    /// Quotes for many tickers in as few requests as possible. Unknown
    /// tickers are left out.
    async fn quotes(&self, tickers: &[TickerSymbol]) -> Result<Vec<MarketQuote>, String>;
    /// Market capitalisation, USD.
    async fn market_cap(&self, ticker: &TickerSymbol) -> Result<Option<f64>, String>;
}
