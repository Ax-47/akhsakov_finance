use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::ticker_symbol::TickerSymbol;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub ticker_symbol: TickerSymbol,
    pub current_price: Decimal,
    pub previous_close_price: Decimal,
    pub timestamp: i64,
    /// ISO code of the prices. The API converts stocks to USD; FX pairs
    /// and indices keep their own units.
    #[serde(default = "usd")]
    pub currency: String,
    /// Served from the saved copy because the provider couldn't be reached.
    #[serde(default)]
    pub stale: bool,
}

fn usd() -> String {
    "USD".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteUpdate {
    pub ticker_symbol: TickerSymbol,
    pub current_price: Decimal,
    pub timestamp: i64,
}
