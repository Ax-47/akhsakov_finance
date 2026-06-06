use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::asset::entities::asset::Asset;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub id: Uuid,
    pub name: String,
    pub assets: Vec<Asset>,
}

impl Portfolio {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            assets: vec![],
        }
    }
}
