use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionType {
    Buy,
    Sell,
    Dividend,
    Split,
    Transfer,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionType::Buy => write!(f, "Buy"),
            TransactionType::Sell => write!(f, "Sell"),
            TransactionType::Dividend => write!(f, "Dividend"),
            TransactionType::Split => write!(f, "Split"),
            TransactionType::Transfer => write!(f, "Transfer"),
        }
    }
}

impl std::str::FromStr for TransactionType {
    type Err = TransactionTypeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "buy" => Ok(TransactionType::Buy),
            "sell" => Ok(TransactionType::Sell),
            "dividend" => Ok(TransactionType::Dividend),
            "split" => Ok(TransactionType::Split),
            "transfer" => Ok(TransactionType::Transfer),
            _ => Err(TransactionTypeError::UnknownTransactionType(s.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionTypeError {
    #[error("Unknown transaction type: '{0}'")]
    UnknownTransactionType(String),
}
