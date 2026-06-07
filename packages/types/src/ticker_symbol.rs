use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TickerSymbol(String);

impl TickerSymbol {
    pub fn new(raw: &str) -> Result<Self, TickerSymbolError> {
        let s = raw.trim().to_uppercase();
        if s.is_empty() || s.len() > 10 {
            return Err(TickerSymbolError::InvalidTicker(raw.to_string()));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TickerSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TickerSymbolError {
    #[error("Invalid ticker symbol: '{0}'")]
    InvalidTicker(String),
}
