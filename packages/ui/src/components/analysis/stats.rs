//! Return-series statistics shared by the relations graph and risk report.
//!
//! Everything works on daily simple returns keyed by day number
//! (days since the Unix epoch), so series from different tickers line up.

use rust_decimal::prelude::ToPrimitive;
use std::collections::BTreeMap;
use types::candle::Candle;

pub type Returns = BTreeMap<i64, f64>;

pub const TRADING_DAYS: f64 = 252.0;
/// Fewer overlapping days than this and a statistic is not meaningful.
pub const MIN_SAMPLES: usize = 10;

/// Simple returns between consecutive daily closes.
pub fn daily_returns(candles: &[Candle]) -> Returns {
    let closes: BTreeMap<i64, f64> = candles
        .iter()
        .filter_map(|c| Some((c.ts.timestamp() / 86_400, c.close.to_f64()?)))
        .filter(|(_, close)| *close > 0.0)
        .collect();
    closes
        .iter()
        .zip(closes.iter().skip(1))
        .map(|((_, prev), (day, close))| (*day, close / prev - 1.0))
        .collect()
}

/// Pearson correlation over the days both series have a return for.
pub fn correlation(a: &Returns, b: &Returns) -> Option<f64> {
    let (xs, ys): (Vec<f64>, Vec<f64>) = a
        .iter()
        .filter_map(|(day, x)| Some((*x, *b.get(day)?)))
        .unzip();
    if xs.len() < MIN_SAMPLES {
        return None;
    }
    let denom = (variance(&xs) * variance(&ys)).sqrt();
    (denom > 0.0).then(|| (covariance(&xs, &ys) / denom).clamp(-1.0, 1.0))
}

/// Symmetric matrix of pairwise correlations (unknown pairs count as 0).
pub fn correlation_matrix(returns: &[Returns]) -> Vec<Vec<f64>> {
    let n = returns.len();
    let mut corr = vec![vec![1.0; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let c = correlation(&returns[i], &returns[j]).unwrap_or(0.0);
            corr[i][j] = c;
            corr[j][i] = c;
        }
    }
    corr
}

// ─── Risk report ──────────────────────────────────────────────────────────────

/// Risk statistics for one holding.
#[derive(Clone, Debug, PartialEq)]
pub struct HoldingRisk {
    /// Portfolio weight (0–1).
    pub weight: f64,
    /// Share of portfolio variance this holding is responsible for (0–1).
    pub risk_share: f64,
    pub volatility: f64,
    pub beta: f64,
    pub max_drawdown: f64,
}

/// Portfolio risk over a window, assuming today's weights were held
/// throughout (rebalanced daily).
#[derive(Clone, Debug, PartialEq)]
pub struct RiskReport {
    pub days: usize,
    pub first_day: i64,
    pub last_day: i64,

    pub annual_return: f64,
    pub volatility: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub max_drawdown: f64,
    /// 1-day 95% value at risk, as a positive loss fraction.
    pub var_95: f64,
    /// Average loss on days beyond the VaR (expected shortfall).
    pub cvar_95: f64,

    pub beta: f64,
    pub alpha: f64,
    pub market_correlation: f64,
    pub market_return: f64,
    pub market_volatility: f64,
    pub market_max_drawdown: f64,

    /// Weighted average of holding volatilities / portfolio volatility.
    /// 1.0 = no diversification benefit.
    pub diversification_ratio: f64,
    /// Annual risk-free rate used for Sharpe, Sortino and alpha (fraction).
    pub risk_free: f64,
    /// (day, return) of the worst single day and worst 5-day stretch.
    pub worst_day: (i64, f64),
    pub worst_week: (i64, f64),

    /// Growth of 1.0 for the portfolio and the market, per aligned day.
    pub growth: Vec<(i64, f64, f64)>,
    pub holdings: Vec<HoldingRisk>,
}

impl RiskReport {
    /// `holdings` and `weights` are index-aligned; weights needn't sum to 1.
    /// Only days where every holding and the market traded are used.
    /// `risk_free` is the annual rate as a fraction, e.g. 0.04.
    pub fn compute(
        holdings: &[Returns],
        weights: &[f64],
        market: &Returns,
        risk_free: f64,
    ) -> Option<Self> {
        let total_weight: f64 = weights.iter().sum();
        if holdings.is_empty() || holdings.len() != weights.len() || total_weight <= 0.0 {
            return None;
        }
        let w: Vec<f64> = weights.iter().map(|x| x / total_weight).collect();

        let days: Vec<i64> = market
            .keys()
            .copied()
            .filter(|d| holdings.iter().all(|h| h.contains_key(d)))
            .collect();
        if days.len() < MIN_SAMPLES * 2 {
            return None;
        }

        let series: Vec<Vec<f64>> = holdings
            .iter()
            .map(|h| days.iter().map(|d| h[d]).collect())
            .collect();
        let bench: Vec<f64> = days.iter().map(|d| market[d]).collect();
        let port: Vec<f64> = (0..days.len())
            .map(|t| series.iter().zip(&w).map(|(s, wi)| s[t] * wi).sum())
            .collect();

        let volatility = annual_vol(&port);
        let annual_return = annualised(&port);
        let market_return = annualised(&bench);
        let beta = beta(&port, &bench);

        // Risk contribution: w_i (Σw)_i / wᵀΣw.
        let n = series.len();
        let cov: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| covariance(&series[i], &series[j])).collect())
            .collect();
        let sigma_w: Vec<f64> = (0..n)
            .map(|i| (0..n).map(|j| cov[i][j] * w[j]).sum())
            .collect();
        let port_var: f64 = (0..n).map(|i| w[i] * sigma_w[i]).sum();

        let holdings_risk: Vec<HoldingRisk> = (0..n)
            .map(|i| HoldingRisk {
                weight: w[i],
                risk_share: if port_var > 0.0 {
                    w[i] * sigma_w[i] / port_var
                } else {
                    0.0
                },
                volatility: annual_vol(&series[i]),
                beta: self::beta(&series[i], &bench),
                max_drawdown: max_drawdown(&series[i]),
            })
            .collect();

        let weighted_vol: f64 = holdings_risk.iter().map(|h| h.weight * h.volatility).sum();
        let (var_95, cvar_95) = value_at_risk(&port, 0.95);

        let worst_day = days
            .iter()
            .zip(&port)
            .map(|(d, r)| (*d, *r))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or_default();
        let worst_week = port
            .windows(5)
            .enumerate()
            .map(|(i, win)| {
                (
                    days[i + 4],
                    win.iter().fold(1.0, |acc, r| acc * (1.0 + r)) - 1.0,
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or(worst_day);

        let mut growth = Vec::with_capacity(days.len());
        let (mut gp, mut gm) = (1.0, 1.0);
        for (t, day) in days.iter().enumerate() {
            gp *= 1.0 + port[t];
            gm *= 1.0 + bench[t];
            growth.push((*day, gp, gm));
        }

        Some(Self {
            days: days.len(),
            first_day: days[0],
            last_day: days[days.len() - 1],
            annual_return,
            volatility,
            sharpe: ratio(annual_return - risk_free, volatility),
            sortino: ratio(
                annual_return - risk_free,
                downside_deviation(&port, risk_free),
            ),
            max_drawdown: max_drawdown(&port),
            var_95,
            cvar_95,
            beta,
            alpha: annual_return - (risk_free + beta * (market_return - risk_free)),
            market_correlation: ratio(
                covariance(&port, &bench),
                (variance(&port) * variance(&bench)).sqrt(),
            ),
            market_return,
            market_volatility: annual_vol(&bench),
            market_max_drawdown: max_drawdown(&bench),
            diversification_ratio: ratio(weighted_vol, volatility),
            risk_free,
            worst_day,
            worst_week,
            growth,
            holdings: holdings_risk,
        })
    }
}

// ─── Primitives ───────────────────────────────────────────────────────────────

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

/// Sample covariance.
fn covariance(xs: &[f64], ys: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let (mx, my) = (mean(xs), mean(ys));
    xs.iter()
        .zip(ys)
        .map(|(x, y)| (x - mx) * (y - my))
        .sum::<f64>()
        / (xs.len() - 1) as f64
}

fn variance(xs: &[f64]) -> f64 {
    covariance(xs, xs)
}

fn ratio(num: f64, den: f64) -> f64 {
    if den.abs() > f64::EPSILON {
        num / den
    } else {
        0.0
    }
}

fn annual_vol(xs: &[f64]) -> f64 {
    variance(xs).sqrt() * TRADING_DAYS.sqrt()
}

/// Compound annual growth rate of a daily return series.
fn annualised(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let growth: f64 = xs.iter().map(|r| 1.0 + r).product();
    growth.max(0.0).powf(TRADING_DAYS / xs.len() as f64) - 1.0
}

fn beta(xs: &[f64], market: &[f64]) -> f64 {
    ratio(covariance(xs, market), variance(market))
}

/// Annualised deviation of returns below the daily risk-free rate.
fn downside_deviation(xs: &[f64], risk_free: f64) -> f64 {
    let rf = risk_free / TRADING_DAYS;
    let squares: Vec<f64> = xs.iter().map(|r| (r - rf).min(0.0).powi(2)).collect();
    mean(&squares).sqrt() * TRADING_DAYS.sqrt()
}

/// Largest peak-to-trough fall, as a positive fraction.
pub fn max_drawdown(xs: &[f64]) -> f64 {
    let (mut value, mut peak, mut worst) = (1.0_f64, 1.0_f64, 0.0_f64);
    for r in xs {
        value *= 1.0 + r;
        peak = peak.max(value);
        worst = worst.max(1.0 - value / peak);
    }
    worst
}

/// Historical VaR and expected shortfall at `confidence`, as positive losses.
fn value_at_risk(xs: &[f64], confidence: f64) -> (f64, f64) {
    let mut sorted = xs.to_vec();
    sorted.sort_by(f64::total_cmp);
    let cut = (((1.0 - confidence) * sorted.len() as f64).floor() as usize).max(1);
    let tail = &sorted[..cut.min(sorted.len())];
    let var = -tail.last().copied().unwrap_or(0.0);
    (var.max(0.0), (-mean(tail)).max(0.0))
}

pub const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `(year, month 1–12, day 1–31)` from days since the Unix epoch
/// (Howard Hinnant's `civil_from_days`).
pub fn civil_from_days(day: i64) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m as u32, d as u32)
}

/// Days since the Unix epoch from a civil date (`days_from_civil`).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `Mar 5, 2026` from a day number.
pub fn day_label(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{} {d}, {y}", MONTHS[(m - 1) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(values: &[f64]) -> Returns {
        values
            .iter()
            .enumerate()
            .map(|(i, v)| (i as i64, *v))
            .collect()
    }

    fn wave(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| (((i * 7) % 11) as f64 - 5.0) / 500.0)
            .collect()
    }

    #[test]
    fn correlation_detects_same_and_opposite_moves() {
        let a = wave(30);
        let neg: Vec<f64> = a.iter().map(|v| -v).collect();
        assert!((correlation(&series(&a), &series(&a)).unwrap() - 1.0).abs() < 1e-9);
        assert!((correlation(&series(&a), &series(&neg)).unwrap() + 1.0).abs() < 1e-9);
        assert!(correlation(&series(&a[..5]), &series(&a[..5])).is_none());
    }

    #[test]
    fn drawdown_and_var() {
        // +10%, -50%, +20%: peak 1.1 → trough 0.55.
        assert!((max_drawdown(&[0.1, -0.5, 0.2]) - 0.5).abs() < 1e-9);
        let xs: Vec<f64> = (1..=100).map(|i| (i as f64 - 50.0) / 1000.0).collect();
        let (var, cvar) = value_at_risk(&xs, 0.95);
        assert!((var - 0.045).abs() < 1e-9);
        assert!(cvar >= var);
    }

    #[test]
    fn report_for_market_clone_has_beta_one() {
        let m = wave(60);
        let report = RiskReport::compute(&[series(&m)], &[1.0], &series(&m), 0.04).unwrap();
        assert!((report.beta - 1.0).abs() < 1e-9);
        assert!(report.alpha.abs() < 1e-9);
        assert!((report.holdings[0].risk_share - 1.0).abs() < 1e-9);
        assert_eq!(report.days, 60);
    }

    #[test]
    fn risk_shares_sum_to_one() {
        let a = wave(60);
        let b: Vec<f64> = a.iter().rev().map(|v| v * 2.0).collect();
        let report =
            RiskReport::compute(&[series(&a), series(&b)], &[3.0, 1.0], &series(&a), 0.04).unwrap();
        let total: f64 = report.holdings.iter().map(|h| h.risk_share).sum();
        assert!((total - 1.0).abs() < 1e-9);
        assert!((report.holdings[0].weight - 0.75).abs() < 1e-9);
    }

    #[test]
    fn labels_days() {
        assert_eq!(day_label(0), "Jan 1, 1970");
        assert_eq!(day_label(20_454), "Jan 1, 2026");
        for day in [-1, 0, 59, 20_454, 20_700] {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day);
        }
    }
}
