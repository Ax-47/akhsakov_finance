use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref};
/// Longest ticker accepted, in bytes.
pub const MAX_LEN: usize = 20;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TickerSymbol(String);

impl TickerSymbol {
    pub fn new(raw: &str) -> Result<Self, TickerSymbolError> {
        let s = raw.trim().to_uppercase();
        // Long enough for Thai fund codes like K-CHANGE-SSF or SCBRMS&P500.
        if s.is_empty() || s.len() > MAX_LEN {
            return Err(TickerSymbolError::InvalidTicker(raw.to_string()));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Deref for TickerSymbol {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Into<String> for TickerSymbol {
    fn into(self) -> String {
        self.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_long_fund_codes() {
        assert_eq!(TickerSymbol::new(" k-change-ssf ").unwrap().as_str(), "K-CHANGE-SSF");
        assert!(TickerSymbol::new("SCBRMS&P500").is_ok());
        assert!(TickerSymbol::new("").is_err());
        assert!(TickerSymbol::new(&"X".repeat(MAX_LEN + 1)).is_err());
    }
}
