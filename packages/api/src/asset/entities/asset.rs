use serde::{Deserialize, Serialize};
use types::{money::Money, quantity::Quantity, ticker_symbol::TickerSymbol};
use uuid::Uuid;

use super::asset_class::AssetClass;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub portfolio_id: Uuid,
    pub ticker_symbol: TickerSymbol,
    pub asset_class: AssetClass,
    pub quantity: Quantity, // shares / units
    pub cost: Money,        // per share in portfolio currency
}

impl Asset {
    pub fn new(
        portfolio_id: Uuid,
        ticker_symbol: TickerSymbol,
        asset_class: AssetClass,
        quantity: Quantity,
        cost: Money,
    ) -> Self {
        Self {
            portfolio_id,
            ticker_symbol,
            asset_class,
            quantity,
            cost,
        }
    }
}
