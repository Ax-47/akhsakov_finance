use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Default, Eq, Serialize, Deserialize)]
pub enum AssetClass {
    #[default]
    Stock,
    Etf,
    Crypto,
    Bond,
    Cash,
    Other(String),
}

impl std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetClass::Stock => write!(f, "Stock"),
            AssetClass::Etf => write!(f, "ETF"),
            AssetClass::Crypto => write!(f, "Crypto"),
            AssetClass::Bond => write!(f, "Bond"),
            AssetClass::Cash => write!(f, "Cash"),
            AssetClass::Other(s) => write!(f, "{s}"),
        }
    }
}
