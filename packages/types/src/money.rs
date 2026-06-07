use super::primitive_amount::NonNegativeAmount;
use super::primitive_amount::error;
use crate::currency::Currency;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Add;
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Money {
    amount: NonNegativeAmount,
    currency: Currency,
}
impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Result<Self, MoneyError> {
        Ok(Self {
            amount: NonNegativeAmount::new(amount)?,
            currency,
        })
    }
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: NonNegativeAmount::zero(),
            currency,
        }
    }
    pub fn amount(&self) -> Decimal {
        self.amount.value()
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }
}
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.currency.symbol(), self.amount.value())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum MoneyError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(#[from] error::PrimitiveError),
}
