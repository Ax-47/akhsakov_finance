use crate::{Transaction, asset::get_asset_response::GetAssetResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GetPortfolioResponse {
    pub id: Uuid,
    pub name: String,
    pub assets: Vec<GetAssetResponse>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, Deserialize)]
pub struct GetDashBoardResponse {
    pub portfolios: Vec<GetPortfolioResponse>,
    pub transactions: Vec<Transaction>,
}
