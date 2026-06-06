use std::ops::Mul;

use super::primitive_amount::error;
use crate::{money::Money, primitive_amount::PositiveAmount};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Quantity(PositiveAmount);

impl Quantity {
    pub fn new(qty: Decimal) -> Result<Self, QuantityError> {
        Ok(Self(PositiveAmount::new(qty)?))
    }
    pub fn value(&self) -> Decimal {
        self.0.value()
    }
}

impl Mul<Money> for Quantity {
    type Output = Money;
    fn mul(self, rhs: Money) -> Money {
        Money::new(self.value() * rhs.amount(), rhs.currency()).expect("invariant violation")
    }
}
#[derive(Debug, thiserror::Error)]
pub enum QuantityError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(#[from] error::PrimitiveError),
}
