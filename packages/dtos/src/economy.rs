//! Economic data: market gauges, macro indicators and the yield curve.

use serde::{Deserialize, Serialize};

/// A live market level, e.g. the VIX or oil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gauge {
    pub label: String,
    /// Yahoo symbol, e.g. `^VIX`.
    pub symbol: String,
    pub value: f64,
    pub change_pct: Option<f64>,
    /// How to show `value`: "%", "$" or "" for points.
    pub unit: String,
}

/// A macro series from FRED, e.g. inflation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Indicator {
    /// FRED id, e.g. `UNRATE`.
    pub id: String,
    pub label: String,
    /// One line on what it is.
    pub description: String,
    /// "%" for rates.
    pub unit: String,
    /// `(YYYY-MM-DD, value)`, oldest first; about five years.
    pub history: Vec<(String, f64)>,
}

impl Indicator {
    pub fn latest(&self) -> Option<&(String, f64)> {
        self.history.last()
    }

    /// The reading `n` observations before the latest.
    pub fn previous(&self, n: usize) -> Option<&(String, f64)> {
        self.history.len().checked_sub(n + 1).map(|i| &self.history[i])
    }
}

/// Treasury yields by maturity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub label: String,
    /// Maturity in years, for the x axis.
    pub years: f64,
    /// Yield, percent.
    pub now: Option<f64>,
    pub year_ago: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EconomySnapshot {
    pub gauges: Vec<Gauge>,
    pub indicators: Vec<Indicator>,
    pub yield_curve: Vec<CurvePoint>,
}

/// Year-over-year % change of a monthly index series (e.g. CPI → inflation),
/// oldest first. Skips months without a value twelve months earlier.
pub fn year_over_year(monthly: &[(String, f64)]) -> Vec<(String, f64)> {
    monthly
        .iter()
        .enumerate()
        .skip(12)
        .filter_map(|(i, (date, v))| {
            let before = monthly[i - 12].1;
            (before != 0.0).then(|| (date.clone(), (v / before - 1.0) * 100.0))
        })
        .collect()
}

/// The last observation of each month, oldest first (daily or weekly
/// series → monthly).
pub fn monthly_last(series: &[(String, f64)]) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    for (date, v) in series {
        match out.last_mut() {
            Some((last, lv)) if last.get(..7) == date.get(..7) => {
                *last = date.clone();
                *lv = *v;
            }
            _ => out.push((date.clone(), *v)),
        }
    }
    out
}

/// Value on or before `date` in a series sorted oldest first.
pub fn value_on_or_before(series: &[(String, f64)], date: &str) -> Option<f64> {
    let i = series.partition_point(|(d, _)| d.as_str() <= date);
    i.checked_sub(1).map(|i| series[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflation_from_index_levels() {
        let monthly: Vec<(String, f64)> = (0..14)
            .map(|m| (format!("2025-{:02}-01", m % 12 + 1), 100.0 + m as f64))
            .collect();
        let yoy = year_over_year(&monthly);
        assert_eq!(yoy.len(), 2);
        assert!((yoy[0].1 - 12.0).abs() < 1e-9); // 112 vs 100
        assert_eq!(value_on_or_before(&monthly[..3], "2025-02-15"), Some(101.0));
        assert_eq!(value_on_or_before(&monthly[..3], "2024-12-31"), None);
    }

    #[test]
    fn samples_the_last_value_of_each_month() {
        let daily = [("2026-01-02", 1.0), ("2026-01-30", 2.0), ("2026-02-02", 3.0)]
            .map(|(d, v)| (d.to_string(), v));
        assert_eq!(
            monthly_last(&daily),
            vec![("2026-01-30".to_string(), 2.0), ("2026-02-02".to_string(), 3.0)]
        );
    }
}
