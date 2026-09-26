//! Monthly investment plans (dollar-cost averaging): buy a fixed amount of
//! one asset on the same day each month. The app reminds you on the day
//! and records the purchase in one click.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

/// Plans run on days 1–28, so every month has the day.
pub const MAX_DAY: u32 = 28;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcaPlan {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub ticker: TickerSymbol,
    /// Money to invest each time, in `currency`.
    pub amount: Decimal,
    /// The asset's trading currency, e.g. `USD` or `THB`.
    pub currency: String,
    /// Day of the month, 1–28.
    pub day: u32,
    pub active: bool,
    /// `YYYY-MM-DD` the plan starts; months before don't count.
    pub start: String,
    /// `YYYY-MM` of the last recorded purchase.
    #[serde(default)]
    pub last_done: Option<String>,
}

impl DcaPlan {
    pub fn validate(&self) -> Result<(), String> {
        if self.amount <= Decimal::ZERO {
            return Err("The amount must be above zero".into());
        }
        if !(1..=MAX_DAY).contains(&self.day) {
            return Err(format!("Pick a day from 1 to {MAX_DAY}"));
        }
        if !(self.currency.len() == 3 && self.currency.chars().all(|c| c.is_ascii_uppercase())) {
            return Err("Currency must be a 3-letter code, e.g. USD".into());
        }
        if crate::planning::days_between("1970-01-01", &self.start).is_none() {
            return Err("Start date must be YYYY-MM-DD".into());
        }
        Ok(())
    }

    /// This month's purchase day for `today` (`YYYY-MM-DD`).
    fn due_in(&self, month: &str) -> String {
        format!("{month}-{:02}", self.day)
    }

    /// Whether a purchase is due on or before `today` and not yet recorded
    /// this month.
    pub fn is_due(&self, today: &str) -> bool {
        let Some(month) = today.get(..7) else { return false };
        let due = self.due_in(month);
        self.active
            && due.as_str() <= today
            && due >= self.start
            && self.last_done.as_deref() != Some(month)
    }

    /// The next purchase day on or after `today`, or today's overdue one.
    pub fn next_due(&self, today: &str) -> Option<String> {
        if !self.active {
            return None;
        }
        let month = today.get(..7)?;
        if self.is_due(today) {
            return Some(self.due_in(month));
        }
        let this = self.due_in(month);
        if this.as_str() > today && this >= self.start && self.last_done.as_deref() != Some(month) {
            return Some(this);
        }
        // Next month (and the start month if the plan starts later).
        let mut next = next_month(month)?;
        while self.due_in(&next) < self.start {
            next = next_month(&next)?;
        }
        Some(self.due_in(&next))
    }

    /// Shares the amount buys at `price` (fractional, 6 decimals).
    pub fn shares_at(&self, price: Decimal) -> Option<Decimal> {
        (price > Decimal::ZERO).then(|| (self.amount / price).round_dp(6))
    }
}

/// `YYYY-MM` after `month`.
fn next_month(month: &str) -> Option<String> {
    let y: i32 = month.get(..4)?.parse().ok()?;
    let m: u32 = month.get(5..7)?.parse().ok()?;
    Some(if m >= 12 { format!("{:04}-01", y + 1) } else { format!("{y:04}-{:02}", m + 1) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn plan(day: u32, start: &str, last_done: Option<&str>) -> DcaPlan {
        DcaPlan {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new("VOO").unwrap(),
            amount: dec!(500),
            currency: "USD".into(),
            day,
            active: true,
            start: start.into(),
            last_done: last_done.map(String::from),
        }
    }

    #[test]
    fn due_once_a_month() {
        let p = plan(5, "2026-01-01", None);
        assert!(!p.is_due("2026-03-04"));
        assert!(p.is_due("2026-03-05"));
        assert!(p.is_due("2026-03-20"), "overdue until recorded");
        assert_eq!(p.next_due("2026-03-04").as_deref(), Some("2026-03-05"));
        assert_eq!(p.next_due("2026-03-20").as_deref(), Some("2026-03-05"));

        let done = plan(5, "2026-01-01", Some("2026-03"));
        assert!(!done.is_due("2026-03-20"));
        assert_eq!(done.next_due("2026-03-20").as_deref(), Some("2026-04-05"));
        assert_eq!(plan(5, "2026-01-01", Some("2026-12")).next_due("2026-12-06").as_deref(), Some("2027-01-05"));

        // Started after this month's day: first due next month.
        let late = plan(5, "2026-03-10", None);
        assert!(!late.is_due("2026-03-20"));
        assert_eq!(late.next_due("2026-03-20").as_deref(), Some("2026-04-05"));
        // Starts in the future.
        assert_eq!(plan(5, "2026-06-01", None).next_due("2026-03-01").as_deref(), Some("2026-06-05"));

        let paused = DcaPlan { active: false, ..p };
        assert!(!paused.is_due("2026-03-05") && paused.next_due("2026-03-05").is_none());
    }

    #[test]
    fn validates_and_sizes() {
        let p = plan(5, "2026-01-01", None);
        assert!(p.validate().is_ok());
        assert!(plan(29, "2026-01-01", None).validate().is_err());
        assert!(DcaPlan { amount: dec!(0), ..p.clone() }.validate().is_err());
        assert_eq!(p.shares_at(dec!(400)), Some(dec!(1.25)));
        assert_eq!(p.shares_at(dec!(0)), None);
    }
}
