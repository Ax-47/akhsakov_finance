use types::{
    asset_class::AssetClass, money::Money, quantity::Quantity, ticker_symbol::TickerSymbol,
};
use uuid::Uuid;

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetAssetResponse {
    pub portfolio_id: Uuid,
    pub ticker_symbol: TickerSymbol,
    pub asset_class: AssetClass,
    pub quantity: Quantity, // shares / units
    pub cost: Money,        // per share in portfolio currency
}
