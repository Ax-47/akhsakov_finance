use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionType {
    Buy,
    Sell,
    Dividend,
    Split,
    Transfer,
    /// Cash paid into the account (use the `$CASH` ticker).
    Deposit,
    /// Cash taken out of the account (use the `$CASH` ticker).
    Withdrawal,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionType::Buy => write!(f, "Buy"),
            TransactionType::Sell => write!(f, "Sell"),
            TransactionType::Dividend => write!(f, "Dividend"),
            TransactionType::Split => write!(f, "Split"),
            TransactionType::Transfer => write!(f, "Transfer"),
            TransactionType::Deposit => write!(f, "Deposit"),
            TransactionType::Withdrawal => write!(f, "Withdrawal"),
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
            "deposit" => Ok(TransactionType::Deposit),
            "withdrawal" | "withdraw" => Ok(TransactionType::Withdrawal),
            _ => Err(TransactionTypeError::UnknownTransactionType(s.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionTypeError {
    #[error("Unknown transaction type: '{0}'")]
    UnknownTransactionType(String),
}
