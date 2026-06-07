use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::asset::get_asset_response::GetAssetResponse;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetPortfolioResponse {
    pub id: Uuid,
    pub name: String,
    pub assets: Vec<GetAssetResponse>,
}
