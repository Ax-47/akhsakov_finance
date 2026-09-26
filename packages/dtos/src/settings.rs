//! User settings: display currency and analysis assumptions.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use types::ticker_symbol::TickerSymbol;

/// Currencies offered for display, as (code, symbol).
pub const CURRENCIES: [(&str, &str); 13] = [
    ("USD", "$"),
    ("THB", "฿"),
    ("EUR", "€"),
    ("GBP", "£"),
    ("JPY", "¥"),
    ("CNY", "CN¥"),
    ("HKD", "HK$"),
    ("SGD", "S$"),
    ("AUD", "A$"),
    ("CAD", "C$"),
    ("CHF", "CHF "),
    ("KRW", "₩"),
    ("INR", "₹"),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Display currency (ISO code). Stored amounts stay in USD.
    pub currency: String,
    /// Annual risk-free rate, percent.
    pub risk_free: Decimal,
    /// Benchmark ticker for performance and risk, e.g. `^GSPC`.
    pub benchmark: TickerSymbol,
    /// Annual return assumed for goal projections, percent.
    pub assumed_return: Decimal,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            currency: "USD".into(),
            risk_free: dec!(4),
            benchmark: TickerSymbol::new("^GSPC").expect("valid benchmark"),
            assumed_return: dec!(7),
        }
    }
}

impl Settings {
    /// Rejects unsupported currencies and out-of-range percentages.
    pub fn validate(&self) -> Result<(), String> {
        if !CURRENCIES.iter().any(|(code, _)| *code == self.currency) {
            return Err(format!("Unsupported currency {}", self.currency));
        }
        if !(Decimal::ZERO..=dec!(20)).contains(&self.risk_free) {
            return Err("Risk-free rate must be between 0% and 20%".into());
        }
        if !(Decimal::ZERO..=dec!(30)).contains(&self.assumed_return) {
            return Err("Assumed return must be between 0% and 30%".into());
        }
        Ok(())
    }

    pub fn currency_symbol(&self) -> &'static str {
        CURRENCIES
            .iter()
            .find(|(code, _)| *code == self.currency)
            .map_or("$", |(_, symbol)| symbol)
    }

    /// Yahoo FX pair giving the display rate, e.g. `USDTHB=X`; `None` for USD.
    pub fn fx_ticker(&self) -> Option<TickerSymbol> {
        (self.currency != "USD")
            .then(|| TickerSymbol::new(&format!("USD{}=X", self.currency)).ok())
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_checked() {
        let s = Settings::default();
        assert!(s.validate().is_ok());
        assert_eq!(s.fx_ticker(), None);
        let thb = Settings {
            currency: "THB".into(),
            ..s.clone()
        };
        assert_eq!(thb.currency_symbol(), "฿");
        assert_eq!(thb.fx_ticker().unwrap().as_str(), "USDTHB=X");
        assert!(
            Settings {
                currency: "XYZ".into(),
                ..s.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Settings {
                risk_free: dec!(25),
                ..s
            }
            .validate()
            .is_err()
        );
    }
}
