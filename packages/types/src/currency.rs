use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Currency {
    #[default]
    Usd,
    Thb,
}
impl Currency {
    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::Usd => "$",
            Currency::Thb => "฿",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Currency::Usd => "USD",
            Currency::Thb => "THB",
        }
    }
}
