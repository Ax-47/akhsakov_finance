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
    /// Simple interest a year, in percent, for deposits and bonds priced by
    /// hand: the value grows by it from the latest price you entered.
    #[serde(default)]
    pub yearly_rate: Option<Decimal>,
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
            yearly_rate: None,
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
        if self.yearly_rate.is_some_and(|r| r < Decimal::ZERO || r > Decimal::from(50)) {
            return Err("Interest must be between 0% and 50% a year".into());
        }
        self.yearly_rate = self.yearly_rate.filter(|r| !r.is_zero());
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

/// `price`, entered on `from`, with `yearly_rate` percent simple interest
/// up to `on` (dates `YYYY-MM-DD`; nothing earns before `from`).
pub fn accrued(price: Decimal, from: &str, on: &str, yearly_rate: Decimal) -> Decimal {
    let days = crate::planning::days_between(from, on).unwrap_or(0).max(0);
    price * (Decimal::ONE + yearly_rate / Decimal::ONE_HUNDRED * Decimal::from(days) / Decimal::from(365))
}

/// The value on `day` (`YYYY-MM-DD`): the latest price entered on or before
/// it, plus interest since then when there's a `yearly_rate`.
pub fn value_on(prices: &[ManualPrice], yearly_rate: Option<Decimal>, day: &str) -> Option<Decimal> {
    let entry = prices.iter().filter(|p| p.date.as_str() <= day).max_by(|a, b| a.date.cmp(&b.date))?;
    Some(match yearly_rate {
        Some(rate) => accrued(entry.price, &entry.date, day, rate),
        None => entry.price,
    })
}

/// Current and previous price, as `(current, previous)`. Without interest
/// these are the latest two entries ([`latest_two`]); with it, the value
/// `today` and the day before.
pub fn current_and_previous(prices: &[ManualPrice], yearly_rate: Option<Decimal>, today: &str) -> Option<(Decimal, Decimal)> {
    if yearly_rate.is_none() {
        return latest_two(prices);
    }
    let current = value_on(prices, yearly_rate, today).or_else(|| latest_two(prices).map(|p| p.0))?;
    let yesterday = crate::planning::add_days(today, -1).unwrap_or_default();
    Some((current, value_on(prices, yearly_rate, &yesterday).unwrap_or(current)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn deposits_earn_simple_interest_from_the_latest_entry() {
        let p = |d: &str, v| ManualPrice { date: d.into(), price: v };
        let prices = [p("2026-01-01", dec!(1)), p("2026-07-01", dec!(1.01))];
        // 1.5% a year: 73 days after the July entry is 0.3% more.
        assert_eq!(value_on(&prices, Some(dec!(1.5)), "2026-09-12"), Some(dec!(1.01) * dec!(1.003)));
        assert_eq!(value_on(&prices, None, "2026-09-12"), Some(dec!(1.01)));
        // Before the first entry there's no value.
        assert_eq!(value_on(&prices, Some(dec!(1.5)), "2025-12-31"), None);
        let (now, before) = current_and_previous(&prices, Some(dec!(36.5)), "2026-07-03").unwrap();
        assert_eq!((now, before), (dec!(1.01) * dec!(1.002), dec!(1.01) * dec!(1.001)));
        // Without interest, the last two entries as before.
        assert_eq!(current_and_previous(&prices, None, "2026-07-03"), Some((dec!(1.01), dec!(1))));
        assert_eq!(accrued(dec!(100), "2026-01-02", "2026-01-01", dec!(5)), dec!(100));
    }

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
        let rate = |r| AssetInfo { yearly_rate: Some(r), ..AssetInfo::new(TickerSymbol::new("KBANK-FD").unwrap()) }.validated();
        assert_eq!(rate(dec!(1.5)).unwrap().yearly_rate, Some(dec!(1.5)));
        assert_eq!(rate(dec!(0)).unwrap().yearly_rate, None);
        assert!(rate(dec!(-1)).is_err() && rate(dec!(51)).is_err());
        for w in TaxWrapper::ALL {
            assert_eq!(TaxWrapper::from_key(w.key()), Some(w));
        }
    }
}
