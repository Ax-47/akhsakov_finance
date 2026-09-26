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

/// A named list of watched stocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Watchlist {
    pub id: Uuid,
    pub name: String,
    /// Oldest first.
    pub items: Vec<WatchItem>,
}

/// Your notes and tags on a stock.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Note {
    pub ticker: String,
    pub text: String,
    /// Lowercase, e.g. `["dividend", "long term"]`.
    pub tags: Vec<String>,
    pub updated_at: String,
}

pub const MAX_TAGS: usize = 10;
pub const MAX_TAG_LEN: usize = 24;
pub const MAX_NOTE_LEN: usize = 5000;

/// Tags as stored: trimmed, lowercase, without `#`, unique, in order.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().trim_start_matches('#').trim().to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t.chars().take(MAX_TAG_LEN).collect());
        }
    }
    out.truncate(MAX_TAGS);
    out
}

/// Ticker stored on alerts about all holdings together.
pub const PORTFOLIO_TICKER: &str = "$PORTFOLIO";

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
    /// All holdings together fall at least `value` percent today.
    PortfolioDayDrop,
    /// All holdings together fall `value` percent or more below their
    /// highest value since the alert was set.
    PortfolioDrawdown,
}

impl AlertKind {
    pub const ALL: [AlertKind; 6] = [
        Self::PriceAbove,
        Self::PriceBelow,
        Self::DayMove,
        Self::WeightAbove,
        Self::PortfolioDayDrop,
        Self::PortfolioDrawdown,
    ];

    /// Whether the alert watches all holdings rather than one ticker.
    pub fn is_portfolio(self) -> bool {
        matches!(self, Self::PortfolioDayDrop | Self::PortfolioDrawdown)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PriceAbove => "Price rises above",
            Self::PriceBelow => "Price falls below",
            Self::DayMove => "Moves more than (% today)",
            Self::WeightAbove => "Weight in portfolio above (%)",
            Self::PortfolioDayDrop => "All holdings fall more than (% today)",
            Self::PortfolioDrawdown => "All holdings fall from their peak by (%)",
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
            Self::PortfolioDayDrop => "portfolio_day_drop",
            Self::PortfolioDrawdown => "portfolio_drawdown",
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
    /// Drawdown alerts: highest value of all holdings seen since it was set.
    #[serde(default)]
    pub peak: Option<Decimal>,
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
            AlertKind::PortfolioDayDrop | AlertKind::PortfolioDrawdown => false,
        }
    }

    /// For alerts on all holdings: whether the condition holds for today's
    /// change (%) of their total value and that value, measured against
    /// [`Self::peak`].
    pub fn is_met_by_portfolio(&self, day_pct: Option<Decimal>, value: Option<Decimal>) -> bool {
        match self.kind {
            AlertKind::PortfolioDayDrop => day_pct.is_some_and(|d| -d >= self.value),
            AlertKind::PortfolioDrawdown => match (value, self.peak) {
                (Some(v), Some(peak)) if peak > Decimal::ZERO => {
                    (peak - v) / peak * Decimal::ONE_HUNDRED >= self.value
                }
                _ => false,
            },
            _ => false,
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
            AlertKind::PortfolioDayDrop => format!("All holdings down {v}% or more in a day"),
            AlertKind::PortfolioDrawdown => format!("All holdings {v}% or more below their peak"),
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
            peak: None,
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
    fn portfolio_conditions() {
        let drop = alert(AlertKind::PortfolioDayDrop, dec!(2));
        assert!(drop.is_met_by_portfolio(Some(dec!(-2.5)), None));
        assert!(!drop.is_met_by_portfolio(Some(dec!(2.5)), None), "a rise isn't a drop");
        assert!(!drop.is_met(None, Some(dec!(-5)), None), "not a ticker alert");

        let mut dd = alert(AlertKind::PortfolioDrawdown, dec!(10));
        assert!(!dd.is_met_by_portfolio(None, Some(dec!(80))), "no peak yet");
        dd.peak = Some(dec!(100));
        assert!(!dd.is_met_by_portfolio(None, Some(dec!(91))));
        assert!(dd.is_met_by_portfolio(None, Some(dec!(90))));
        assert!(AlertKind::PortfolioDrawdown.is_portfolio() && !AlertKind::DayMove.is_portfolio());
    }

    #[test]
    fn kind_round_trips_as_text() {
        for k in AlertKind::ALL {
            assert_eq!(k.to_string().parse::<AlertKind>().unwrap(), k);
        }
    }

    #[test]
    fn tags_are_normalized() {
        let raw = ["  Dividend ", "#growth", "dividend", "", "AI"].map(String::from);
        assert_eq!(normalize_tags(&raw), vec!["dividend", "growth", "ai"]);
        let many: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
        assert_eq!(normalize_tags(&many).len(), MAX_TAGS);
    }
}
