use super::primitive_amount::error;
use crate::primitive_amount::PositiveAmount;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Quantity(PositiveAmount);

impl Quantity {
    pub fn new(qty: f64) -> Result<Self, QuantityError> {
        Ok(Self(PositiveAmount::new(qty)?))
    }
    pub fn value(&self) -> f64 {
        self.0.value()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QuantityError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(#[from] error::PrimitiveError),
}
