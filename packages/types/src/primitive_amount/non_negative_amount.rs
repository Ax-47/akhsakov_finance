use super::error;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NonNegativeAmount(f64);

impl NonNegativeAmount {
    pub fn new(value: f64) -> Result<Self, error::PrimitiveError> {
        match value {
            v if v < 0.0 => Err(error::PrimitiveError::Negative(v)),
            v if v.is_nan() => Err(error::PrimitiveError::NotANumber),
            v if v.is_infinite() => Err(error::PrimitiveError::Infinite),
            v => Ok(Self(v)),
        }
    }

    pub fn zero() -> Self {
        Self(0.0)
    }
    pub fn value(&self) -> f64 {
        self.0
    }
}
