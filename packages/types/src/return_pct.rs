use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ReturnPct(f64);

impl ReturnPct {
    pub fn from_cost_and_value(cost: f64, value: f64) -> Self {
        if cost == 0.0 {
            return Self(0.0);
        }
        Self(((value - cost) / cost) * 100.0)
    }
    pub fn value(&self) -> f64 {
        self.0
    }
    pub fn is_positive(&self) -> bool {
        self.0 >= 0.0
    }
    pub fn sign_str(&self) -> &'static str {
        if self.is_positive() { "+" } else { "" }
    }
}

impl fmt::Display for ReturnPct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:.2}%", self.sign_str(), self.0)
    }
}
