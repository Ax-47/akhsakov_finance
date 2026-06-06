use super::error;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NonNegativeAmount(Decimal);

impl NonNegativeAmount {
    pub fn new(value: Decimal) -> Result<Self, error::PrimitiveError> {
        if value < Decimal::ZERO {
            return Err(error::PrimitiveError::Negative(value));
        }
        Ok(Self(value))
    }

    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }
    pub fn value(&self) -> Decimal {
        self.0
    }
}
