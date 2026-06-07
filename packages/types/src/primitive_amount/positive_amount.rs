use super::error;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PositiveAmount(Decimal);

impl PositiveAmount {
    pub fn new(value: Decimal) -> Result<Self, error::PrimitiveError> {
        if value <= Decimal::ZERO {
            return Err(error::PrimitiveError::NonPositive(value));
        }
        Ok(Self(value))
    }
    pub fn value(&self) -> Decimal {
        self.0
    }
}
