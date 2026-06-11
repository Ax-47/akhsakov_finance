use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::ticker_symbol::TickerSymbol;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub ticker_symbol: TickerSymbol,
    pub current_price: Decimal,
    pub previous_close_price: Decimal,
    pub timestamp: i64,
}
