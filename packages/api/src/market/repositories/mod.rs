//! Ports for the market overview.

use async_trait::async_trait;
use dtos::market::{MarketIndex, ScreenFilter};

/// A company in an index.
#[derive(Debug, Clone, PartialEq)]
pub struct Constituent {
    /// Yahoo symbol, e.g. `BRK-B` or `PTT.BK`.
    pub ticker: String,
    pub name: String,
    pub sector: String,
}

/// Which companies make up an index. Errors are provider messages.
#[async_trait]
pub trait IndexRepository: Send + Sync {
    async fn constituents(&self, index: MarketIndex) -> Result<Vec<Constituent>, String>;
}

/// Latest quote with the figures heatmaps, screens and calendars need, in
/// the instrument's own currency.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarketQuote {
    pub ticker: String,
    pub name: Option<String>,
    /// ISO code of every money field below.
    pub currency: String,
    pub price: f64,
    pub change_pct: Option<f64>,
    pub market_cap: Option<f64>,
    pub pe: Option<f64>,
    pub forward_pe: Option<f64>,
    /// Percent, e.g. 1.2.
    pub dividend_yield_pct: Option<f64>,
    /// Forecast annual dividend per share.
    pub dividend_rate: Option<f64>,
    pub volume: Option<f64>,
    pub high_52w: Option<f64>,
    pub low_52w: Option<f64>,
    /// Next (or latest) earnings date(s), `YYYY-MM-DD`.
    pub earnings_dates: Vec<String>,
    pub earnings_estimated: bool,
    /// Next dividend payment date.
    pub dividend_date: Option<String>,
}

/// Dividend and earnings dates for one stock.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TickerCalendar {
    pub earnings: Vec<String>,
    pub ex_dividend: Option<String>,
    pub payment: Option<String>,
}

/// Errors are provider messages, e.g. rate limiting.
#[async_trait]
pub trait MarketDataGateway: Send + Sync {
    /// Quotes for many tickers in as few requests as possible. Unknown
    /// tickers are left out.
    async fn quotes(&self, tickers: &[String]) -> Result<Vec<MarketQuote>, String>;
    /// One page of matches, and how many there are in total. Money bounds
    /// in `filter` are in the region's currency.
    async fn screen(&self, filter: &ScreenFilter) -> Result<(u32, Vec<MarketQuote>), String>;
    async fn calendar(&self, ticker: &str) -> Result<TickerCalendar, String>;
}
