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
    /// 14-day RSI rises to `value` or above (overbought).
    RsiAbove,
    /// 14-day RSI falls to `value` or below (oversold).
    RsiBelow,
    /// Price crosses above its `value`-day simple moving average.
    CrossAboveSma,
    /// Price crosses below its `value`-day simple moving average.
    CrossBelowSma,
    /// Price comes within `value` percent of its 52-week high.
    Near52WeekHigh,
    /// Price comes within `value` percent of its 52-week low.
    Near52WeekLow,
    /// Earnings are due within `value` days.
    EarningsWithin,
    /// The ex-dividend date is within `value` days.
    ExDividendWithin,
}

impl AlertKind {
    pub const ALL: [AlertKind; 14] = [
        Self::PriceAbove,
        Self::PriceBelow,
        Self::DayMove,
        Self::WeightAbove,
        Self::PortfolioDayDrop,
        Self::PortfolioDrawdown,
        Self::RsiAbove,
        Self::RsiBelow,
        Self::CrossAboveSma,
        Self::CrossBelowSma,
        Self::Near52WeekHigh,
        Self::Near52WeekLow,
        Self::EarningsWithin,
        Self::ExDividendWithin,
    ];

    /// Whether the alert watches all holdings rather than one ticker.
    pub fn is_portfolio(self) -> bool {
        matches!(self, Self::PortfolioDayDrop | Self::PortfolioDrawdown)
    }

    /// Whether the alert needs daily price history (indicators, 52 weeks).
    pub fn is_technical(self) -> bool {
        matches!(
            self,
            Self::RsiAbove | Self::RsiBelow | Self::CrossAboveSma | Self::CrossBelowSma | Self::Near52WeekHigh | Self::Near52WeekLow
        )
    }

    /// Whether the alert watches the earnings / dividend calendar.
    pub fn is_event(self) -> bool {
        matches!(self, Self::EarningsWithin | Self::ExDividendWithin)
    }

    /// What `value` means, for the input label.
    pub fn unit(self) -> &'static str {
        match self {
            Self::PriceAbove | Self::PriceBelow => "Price ($)",
            Self::RsiAbove | Self::RsiBelow => "RSI (0–100)",
            Self::CrossAboveSma | Self::CrossBelowSma => "Moving average (days)",
            Self::EarningsWithin | Self::ExDividendWithin => "Days before",
            _ => "Percent (%)",
        }
    }

    /// Checks a value for this kind; the message says what's wrong.
    pub fn validate(self, value: Decimal) -> Result<(), &'static str> {
        use rust_decimal_macros::dec;
        let ok = match self {
            Self::RsiAbove | Self::RsiBelow => value > Decimal::ZERO && value < Decimal::ONE_HUNDRED,
            Self::CrossAboveSma | Self::CrossBelowSma => value.fract().is_zero() && (dec!(2)..=dec!(250)).contains(&value),
            Self::Near52WeekHigh | Self::Near52WeekLow => value >= Decimal::ZERO && value <= dec!(50),
            Self::EarningsWithin | Self::ExDividendWithin => value.fract().is_zero() && (Decimal::ZERO..=dec!(90)).contains(&value),
            _ => true,
        };
        if ok {
            return Ok(());
        }
        Err(match self {
            Self::RsiAbove | Self::RsiBelow => "RSI levels are between 0 and 100",
            Self::CrossAboveSma | Self::CrossBelowSma => "Use a whole number of days from 2 to 250, e.g. 50 or 200",
            Self::Near52WeekHigh | Self::Near52WeekLow => "Use a distance from 0% to 50%",
            _ => "Use a whole number of days from 0 to 90",
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PriceAbove => "Price rises above",
            Self::PriceBelow => "Price falls below",
            Self::DayMove => "Moves more than (% today)",
            Self::WeightAbove => "Weight in portfolio above (%)",
            Self::PortfolioDayDrop => "All holdings fall more than (% today)",
            Self::PortfolioDrawdown => "All holdings fall from their peak by (%)",
            Self::RsiAbove => "RSI (14) rises above — overbought",
            Self::RsiBelow => "RSI (14) falls below — oversold",
            Self::CrossAboveSma => "Price crosses above moving average (days)",
            Self::CrossBelowSma => "Price crosses below moving average (days)",
            Self::Near52WeekHigh => "Within (%) of its 52-week high",
            Self::Near52WeekLow => "Within (%) of its 52-week low",
            Self::EarningsWithin => "Earnings in (days)",
            Self::ExDividendWithin => "Ex-dividend date in (days)",
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
            Self::RsiAbove => "rsi_above",
            Self::RsiBelow => "rsi_below",
            Self::CrossAboveSma => "cross_above_sma",
            Self::CrossBelowSma => "cross_below_sma",
            Self::Near52WeekHigh => "near_52w_high",
            Self::Near52WeekLow => "near_52w_low",
            Self::EarningsWithin => "earnings_within",
            Self::ExDividendWithin => "ex_dividend_within",
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
            _ => false,
        }
    }

    /// For technical alerts: whether the condition holds for daily closes,
    /// oldest first, whose last entry is the latest price.
    pub fn is_met_by_history(&self, closes: &[f64]) -> bool {
        use crate::technicals::{rsi, sma};
        use rust_decimal::prelude::ToPrimitive;
        let v = self.value.to_f64().unwrap_or(0.0);
        let Some(&last) = closes.last() else { return false };
        match self.kind {
            AlertKind::RsiAbove => rsi(closes, 14).last().copied().flatten().is_some_and(|r| r >= v),
            AlertKind::RsiBelow => rsi(closes, 14).last().copied().flatten().is_some_and(|r| r <= v),
            AlertKind::CrossAboveSma | AlertKind::CrossBelowSma => {
                let avg = sma(closes, v as usize);
                let n = closes.len();
                let (Some(Some(now)), Some(Some(before))) = (avg.last(), n.checked_sub(2).and_then(|i| avg.get(i))) else {
                    return false;
                };
                let prev = closes[n - 2];
                if self.kind == AlertKind::CrossAboveSma {
                    prev < *before && last >= *now
                } else {
                    prev > *before && last <= *now
                }
            }
            AlertKind::Near52WeekHigh | AlertKind::Near52WeekLow => {
                // About 252 trading days in a year.
                let year = &closes[closes.len().saturating_sub(252)..];
                if year.len() < 20 {
                    return false;
                }
                if self.kind == AlertKind::Near52WeekHigh {
                    let high = year.iter().copied().fold(f64::MIN, f64::max);
                    last >= high * (1.0 - v / 100.0)
                } else {
                    let low = year.iter().copied().fold(f64::MAX, f64::min);
                    low > 0.0 && last <= low * (1.0 + v / 100.0)
                }
            }
            _ => false,
        }
    }

    /// For calendar alerts: whether the next event is `days_until` away or
    /// sooner (and not past).
    pub fn is_met_by_event(&self, days_until: Option<i64>) -> bool {
        self.kind.is_event()
            && days_until.is_some_and(|d| d >= 0 && Decimal::from(d) <= self.value)
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
            AlertKind::RsiAbove => format!("{} RSI at or above {v} (overbought)", self.ticker),
            AlertKind::RsiBelow => format!("{} RSI at or below {v} (oversold)", self.ticker),
            AlertKind::CrossAboveSma => format!("{} crossed above its {v}-day average", self.ticker),
            AlertKind::CrossBelowSma => format!("{} crossed below its {v}-day average", self.ticker),
            AlertKind::Near52WeekHigh => format!("{} within {v}% of its 52-week high", self.ticker),
            AlertKind::Near52WeekLow => format!("{} within {v}% of its 52-week low", self.ticker),
            AlertKind::EarningsWithin => format!("{} reports earnings within {v} days", self.ticker),
            AlertKind::ExDividendWithin => format!("{} goes ex-dividend within {v} days", self.ticker),
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
    fn technical_conditions() {
        // 30 days rising 1 a day, then a sharp drop.
        let mut closes: Vec<f64> = (1..=30).map(|i| 100.0 + i as f64).collect();
        assert!(alert(AlertKind::RsiAbove, dec!(70)).is_met_by_history(&closes));
        assert!(!alert(AlertKind::RsiBelow, dec!(30)).is_met_by_history(&closes));
        assert!(alert(AlertKind::Near52WeekHigh, dec!(1)).is_met_by_history(&closes));
        assert!(!alert(AlertKind::Near52WeekLow, dec!(5)).is_met_by_history(&closes));
        assert!(!alert(AlertKind::CrossBelowSma, dec!(10)).is_met_by_history(&closes), "still above");
        closes.push(110.0);
        assert!(alert(AlertKind::CrossBelowSma, dec!(10)).is_met_by_history(&closes), "fell through the average");
        assert!(!alert(AlertKind::CrossAboveSma, dec!(10)).is_met_by_history(&closes));
        assert!(!alert(AlertKind::RsiAbove, dec!(70)).is_met_by_history(&[]), "no history");

        let earnings = alert(AlertKind::EarningsWithin, dec!(3));
        assert!(earnings.is_met_by_event(Some(2)));
        assert!(!earnings.is_met_by_event(Some(5)) && !earnings.is_met_by_event(Some(-1)) && !earnings.is_met_by_event(None));
        assert!(!earnings.is_met(Some(dec!(1)), Some(dec!(50)), Some(dec!(50))), "not a price alert");

        assert!(AlertKind::CrossAboveSma.validate(dec!(50)).is_ok());
        assert!(AlertKind::CrossAboveSma.validate(dec!(50.5)).is_err());
        assert!(AlertKind::RsiBelow.validate(dec!(100)).is_err());
        assert!(AlertKind::EarningsWithin.validate(dec!(0)).is_ok());
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
