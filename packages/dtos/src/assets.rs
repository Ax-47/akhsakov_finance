//! What an asset is beyond its ticker: its class (fund, gold, deposit …),
//! a name, whether you price it yourself, and whether it's held in a Thai
//! tax-saving wrapper (SSF, RMF, Thai ESG).
//!
//! Assets without a market price on Yahoo (most Thai mutual funds, bank
//! deposits, bonds, physical gold) are priced from [`ManualPrice`]s you
//! enter; the quote service then treats them like any other ticker.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use types::{asset_class::AssetClass, ticker_symbol::TickerSymbol};

/// Thai tax-saving funds: buying them lowers taxable income, if held long
/// enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaxWrapper {
    /// Super Savings Fund: hold 10 years from each purchase.
    Ssf,
    /// Retirement Mutual Fund: hold 5 years and until age 55.
    Rmf,
    /// Thai ESG fund: hold 5 years from each purchase.
    ThaiEsg,
}

impl TaxWrapper {
    pub const ALL: [TaxWrapper; 3] = [Self::Ssf, Self::Rmf, Self::ThaiEsg];

    pub fn key(self) -> &'static str {
        match self {
            Self::Ssf => "ssf",
            Self::Rmf => "rmf",
            Self::ThaiEsg => "thaiesg",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.key() == key)
    }
}

impl fmt::Display for TaxWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ssf => "SSF",
            Self::Rmf => "RMF",
            Self::ThaiEsg => "Thai ESG",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetInfo {
    pub ticker: TickerSymbol,
    pub class: AssetClass,
    /// Shown next to the ticker; may be empty.
    pub name: String,
    /// Currency of manual prices (ISO code).
    pub currency: String,
    /// Priced from your own entries instead of Yahoo.
    pub manual: bool,
    #[serde(default)]
    pub wrapper: Option<TaxWrapper>,
}

impl AssetInfo {
    pub fn new(ticker: TickerSymbol) -> Self {
        Self {
            ticker,
            class: AssetClass::Stock,
            name: String::new(),
            currency: "USD".into(),
            manual: false,
            wrapper: None,
        }
    }

    /// Trims the name and checks the fields.
    pub fn validated(mut self) -> Result<Self, String> {
        self.name = self.name.trim().chars().take(80).collect();
        self.currency = self.currency.trim().to_uppercase();
        if !(self.currency.len() == 3 && self.currency.chars().all(|c| c.is_ascii_uppercase())) {
            return Err("Currency must be a 3-letter code, e.g. THB".into());
        }
        if self.ticker.as_str().starts_with('$') {
            return Err("Tickers starting with $ are reserved".into());
        }
        Ok(self)
    }
}

/// A price you entered, in the asset's currency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualPrice {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub price: Decimal,
}

/// Latest and previous price from manual entries (any order), as
/// `(latest, previous)`; the previous equals the latest with one entry.
pub fn latest_two(prices: &[ManualPrice]) -> Option<(Decimal, Decimal)> {
    let mut sorted: Vec<&ManualPrice> = prices.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));
    let latest = sorted.first()?.price;
    Some((latest, sorted.get(1).map_or(latest, |p| p.price)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn latest_two_prices() {
        let p = |d: &str, v| ManualPrice { date: d.into(), price: v };
        assert_eq!(latest_two(&[]), None);
        assert_eq!(latest_two(&[p("2026-01-01", dec!(10))]), Some((dec!(10), dec!(10))));
        let many = [p("2026-01-01", dec!(10)), p("2026-03-01", dec!(12)), p("2026-02-01", dec!(11))];
        assert_eq!(latest_two(&many), Some((dec!(12), dec!(11))));
    }

    #[test]
    fn validation() {
        let t = TickerSymbol::new("KFSSF").unwrap();
        let ok = AssetInfo { name: "  Krungsri SSF ".into(), currency: "thb".into(), ..AssetInfo::new(t.clone()) };
        let v = ok.validated().unwrap();
        assert_eq!((v.name.as_str(), v.currency.as_str()), ("Krungsri SSF", "THB"));
        assert!(AssetInfo { currency: "baht".into(), ..AssetInfo::new(t) }.validated().is_err());
        assert!(AssetInfo::new(TickerSymbol::new("$CASH").unwrap()).validated().is_err());
        for w in TaxWrapper::ALL {
            assert_eq!(TaxWrapper::from_key(w.key()), Some(w));
        }
    }
}
