use std::fmt;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TradeDate(NaiveDate);

impl TradeDate {
    pub fn new(date: NaiveDate, today: NaiveDate) -> Result<Self, TradeDateError> {
        if date > today {
            return Err(TradeDateError::FutureTradeDate(date));
        }
        Ok(Self(date))
    }
    pub fn new_today() -> Self {
        Self(chrono::Local::now().date_naive())
    }

    pub fn from_str_date(s: &str, today: NaiveDate) -> Result<Self, TradeDateError> {
        let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| TradeDateError::InvalidFormat(s.to_string()))?;
        Self::new(date, today)
    }
    pub fn from_ymd(
        year: i32,
        month: u32,
        day: u32,
        today: NaiveDate,
    ) -> Result<Self, TradeDateError> {
        let date = NaiveDate::from_ymd_opt(year, month, day)
            .ok_or(TradeDateError::InvalidDate(year, month, day))?;
        Self::new(date, today)
    }

    pub fn value(&self) -> NaiveDate {
        self.0
    }
}

impl fmt::Display for TradeDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%d"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TradeDateError {
    #[error("Trade date cannot be in the future: {0}")]
    FutureTradeDate(NaiveDate),

    #[error("Invalid date format: '{0}' expected YYYY-MM-DD")]
    InvalidFormat(String),

    #[error("Invalid date: {0}-{1:02}-{2:02}")]
    InvalidDate(i32, u32, u32),
}
