use super::currency::{Thb, Usd};
use super::primitive_amount::NonNegativeAmount;
use super::primitive_amount::error;
use crate::currency::CurrencyInfo;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Money<T> {
    amount: NonNegativeAmount,
    _currency: PhantomData<T>,
}
impl<T> Money<T> {
    pub fn new(amount: f64) -> Result<Self, MoneyError> {
        Ok(Self {
            amount: NonNegativeAmount::new(amount)?,
            _currency: PhantomData,
        })
    }
    pub fn zero() -> Self {
        Self {
            amount: NonNegativeAmount::zero(),
            _currency: PhantomData,
        }
    }
    pub fn amount(&self) -> f64 {
        self.amount.value()
    }
}
pub type MoneyTHB = Money<Thb>;
pub type MoneyUSD = Money<Usd>;
impl<T: CurrencyInfo> fmt::Display for Money<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:.2} {}", T::symbol(), self.amount.value(), T::code())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum MoneyError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(#[from] error::PrimitiveError),
}
