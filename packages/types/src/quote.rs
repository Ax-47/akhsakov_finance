use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub current_price: Decimal,
    pub previous_close_price: Decimal,
    pub timestamp: i64,
}
