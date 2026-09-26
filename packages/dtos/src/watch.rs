//! Ticker search results, the watchlist and price alerts.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub symbol: String,
    pub name: Option<String>,
    pub exchange: Option<String>,
    /// e.g. `Equity`, `Etf`, `Crypto`.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchItem {
    pub ticker: TickerSymbol,
    pub added_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertKind {
    /// Price rises to or above `value`.
    PriceAbove,
    /// Price falls to or below `value`.
    PriceBelow,
    /// Moves at least `value` percent today, either way.
    DayMove,
    /// The holding grows to at least `value` percent of all holdings.
    WeightAbove,
}

impl AlertKind {
    pub const ALL: [AlertKind; 4] = [
        Self::PriceAbove,
        Self::PriceBelow,
        Self::DayMove,
        Self::WeightAbove,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::PriceAbove => "Price rises above",
            Self::PriceBelow => "Price falls below",
            Self::DayMove => "Moves more than (% today)",
            Self::WeightAbove => "Weight in portfolio above (%)",
        }
    }
}

impl fmt::Display for AlertKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PriceAbove => "price_above",
            Self::PriceBelow => "price_below",
            Self::DayMove => "day_move",
            Self::WeightAbove => "weight_above",
        })
    }
}

impl FromStr for AlertKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.to_string() == s)
            .ok_or_else(|| format!("unknown alert kind \"{s}\""))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub ticker: TickerSymbol,
    pub kind: AlertKind,
    pub value: Decimal,
    pub created_at: String,
    /// When it fired; `None` while still active.
    pub triggered_at: Option<String>,
}

impl Alert {
    pub fn is_active(&self) -> bool {
        self.triggered_at.is_none()
    }

    /// Whether the condition holds for the latest price, today's change (%)
    /// and portfolio weight (%). Missing inputs never trigger.
    pub fn is_met(
        &self,
        price: Option<Decimal>,
        day_pct: Option<Decimal>,
        weight_pct: Option<Decimal>,
    ) -> bool {
        match self.kind {
            AlertKind::PriceAbove => price.is_some_and(|p| p >= self.value),
            AlertKind::PriceBelow => price.is_some_and(|p| p > Decimal::ZERO && p <= self.value),
            AlertKind::DayMove => day_pct.is_some_and(|d| d.abs() >= self.value),
            AlertKind::WeightAbove => weight_pct.is_some_and(|w| w >= self.value),
        }
    }

    /// e.g. `NVDA price rose above $250.00`.
    pub fn describe(&self) -> String {
        let v = self.value.normalize();
        match self.kind {
            AlertKind::PriceAbove => format!("{} price above ${v}", self.ticker),
            AlertKind::PriceBelow => format!("{} price below ${v}", self.ticker),
            AlertKind::DayMove => format!("{} moves more than {v}% in a day", self.ticker),
            AlertKind::WeightAbove => format!("{} over {v}% of your holdings", self.ticker),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn alert(kind: AlertKind, value: Decimal) -> Alert {
        Alert {
            id: Uuid::nil(),
            ticker: TickerSymbol::new("NVDA").unwrap(),
            kind,
            value,
            created_at: "2026-01-01".into(),
            triggered_at: None,
        }
    }

    #[test]
    fn conditions() {
        let above = alert(AlertKind::PriceAbove, dec!(250));
        assert!(above.is_met(Some(dec!(250)), None, None));
        assert!(!above.is_met(Some(dec!(249.99)), None, None));
        assert!(!above.is_met(None, None, None));

        let below = alert(AlertKind::PriceBelow, dec!(200));
        assert!(below.is_met(Some(dec!(199)), None, None));
        assert!(!below.is_met(Some(dec!(0)), None, None), "no price yet");

        let moved = alert(AlertKind::DayMove, dec!(5));
        assert!(moved.is_met(None, Some(dec!(-5.2)), None));
        assert!(!moved.is_met(None, Some(dec!(4.9)), None));

        let weight = alert(AlertKind::WeightAbove, dec!(25));
        assert!(weight.is_met(None, None, Some(dec!(30))));
    }

    #[test]
    fn kind_round_trips_as_text() {
        for k in AlertKind::ALL {
            assert_eq!(k.to_string().parse::<AlertKind>().unwrap(), k);
        }
    }
}
