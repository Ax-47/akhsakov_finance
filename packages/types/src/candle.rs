use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub ts: DateTime<Utc>,
    /// Prices are adjusted for splits and dividends (total return).
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<u64>,
    /// Adjusted close ÷ the close actually traded that day (1.0 on and
    /// after the latest dividend, below 1 before it). Multiply a real
    /// trade price by this to compare it with the adjusted prices; divide
    /// the prices by it to get the chart a broker shows. `None` if unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adj_factor: Option<f64>,
}

impl Candle {
    /// [`Self::adj_factor`], or 1.0 when unknown.
    pub fn adjustment(&self) -> f64 {
        self.adj_factor
            .filter(|f| f.is_finite() && *f > 0.0)
            .unwrap_or(1.0)
    }
}
