use crate::{Transaction, asset::get_asset_response::GetAssetResponse};
use rust_decimal::Decimal;
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
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PortfolioHistoryPoint {
    pub label: String,
    pub value: Decimal,
    pub pct: Decimal,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GetPortfolioHistoryResponse {
    pub points: Vec<(String, Vec<PortfolioHistoryPoint>)>,
}
