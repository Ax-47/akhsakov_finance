use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ticker(String);

impl Ticker {
    pub fn new(raw: &str) -> Result<Self, TickerError> {
        let s = raw.trim().to_uppercase();
        if s.is_empty() || s.len() > 10 {
            return Err(TickerError::InvalidTicker(raw.to_string()));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Ticker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TickerError {
    #[error("Invalid ticker symbol: '{0}'")]
    InvalidTicker(String),
}
