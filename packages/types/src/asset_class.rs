use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Default, Eq, Hash, Serialize, Deserialize)]
pub enum AssetClass {
    #[default]
    Stock,
    Etf,
    /// Mutual fund (e.g. Thai SSF / RMF funds).
    Fund,
    Crypto,
    Gold,
    Bond,
    /// Savings or fixed deposit.
    Deposit,
    Cash,
    Other(String),
}

impl AssetClass {
    /// Every class offered when describing an asset.
    pub const CHOICES: [AssetClass; 8] = [
        Self::Stock,
        Self::Etf,
        Self::Fund,
        Self::Crypto,
        Self::Gold,
        Self::Bond,
        Self::Deposit,
        Self::Cash,
    ];

    /// Stable text for storage, e.g. `fund`.
    pub fn key(&self) -> &str {
        match self {
            Self::Stock => "stock",
            Self::Etf => "etf",
            Self::Fund => "fund",
            Self::Crypto => "crypto",
            Self::Gold => "gold",
            Self::Bond => "bond",
            Self::Deposit => "deposit",
            Self::Cash => "cash",
            Self::Other(s) => s,
        }
    }

    /// Inverse of [`Self::key`]; unknown text becomes `Other`.
    pub fn from_key(key: &str) -> Self {
        Self::CHOICES
            .into_iter()
            .find(|c| c.key() == key)
            .unwrap_or_else(|| Self::Other(key.to_string()))
    }
}

impl std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetClass::Stock => write!(f, "Stock"),
            AssetClass::Etf => write!(f, "ETF"),
            AssetClass::Fund => write!(f, "Fund"),
            AssetClass::Crypto => write!(f, "Crypto"),
            AssetClass::Gold => write!(f, "Gold"),
            AssetClass::Bond => write!(f, "Bond"),
            AssetClass::Deposit => write!(f, "Deposit"),
            AssetClass::Cash => write!(f, "Cash"),
            AssetClass::Other(s) => write!(f, "{s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for c in AssetClass::CHOICES {
            assert_eq!(AssetClass::from_key(c.key()), c);
        }
        assert_eq!(AssetClass::from_key("art"), AssetClass::Other("art".into()));
    }
}
