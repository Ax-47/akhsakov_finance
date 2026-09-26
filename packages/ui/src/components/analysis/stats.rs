//! Return-series statistics shared by the relations graph and risk report.
//!
//! Prices are daily closes keyed by day number (days since the Unix epoch).
//! Series are lined up on every day any of them traded, carrying the last
//! close over a market's holidays, so a stock's move across another
//! market's holiday still counts (on its next trading day) instead of
//! being dropped. Annual figures scale by the time the window actually
//! covers, not by an assumed 252 days.
//!
//! Markets that close at different times react to each other a day apart:
//! Bangkok closes before New York opens, so a Thai stock's return on day t
//! reflects New York's day t−1. Comparing same-day returns alone makes such
//! pairs look unrelated, so covariances and betas between them add the
//! one-day-lagged term (Dimson's correction).

use rust_decimal::prelude::ToPrimitive;
use std::collections::{BTreeMap, BTreeSet};
use types::candle::Candle;

/// Fewer overlapping days than this and a statistic is not meaningful.
pub const MIN_SAMPLES: usize = 10;
/// Fewer days than this and a 95% VaR rests on a handful of bad days.
pub const RELIABLE_VAR_DAYS: usize = 100;
/// Sessions opening this far apart (seconds) trade at different times.
const ASYNC_SECS: i64 = 3 * 3600;
/// RiskMetrics decay for the EWMA volatility.
pub const EWMA_LAMBDA: f64 = 0.94;
/// Days used to seed the EWMA before its forecasts are tested.
const EWMA_WARMUP: usize = 20;
/// One-sided 95% quantile of the standard normal.
const Z_95: f64 = 1.644_853_626_951_472_2;
/// A holding whose history starts later than this share of the window is
/// left out, rather than cutting the window short for every holding.
const MAX_LATE_START: f64 = 0.25;
/// Sessions averaged for a stock's typical daily volume.
const VOLUME_SESSIONS: usize = 20;
const DAYS_PER_YEAR: f64 = 365.25;

// ─── Price series ─────────────────────────────────────────────────────────────

/// Daily closes of one instrument, in one currency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PriceSeries {
    /// Close per day number.
    pub closes: BTreeMap<i64, f64>,
    /// When the session opens, in seconds after UTC midnight (median).
    pub session: Option<i64>,
    /// Average shares traded per session, over the last 20.
    pub avg_volume: Option<f64>,
}

impl PriceSeries {
    /// Closes of stock or index candles, keyed by the UTC day the session
    /// opens on.
    pub fn from_candles(candles: &[Candle]) -> Self {
        Self::build(candles, 0)
    }

    /// Closes of FX candles. An FX day starts at midnight London time
    /// (23:00 UTC in summer), so the UTC date would be a day early.
    pub fn from_fx_candles(candles: &[Candle]) -> Self {
        Self::build(candles, 6 * 3600)
    }

    fn build(candles: &[Candle], shift_secs: i64) -> Self {
        let mut sorted: Vec<&Candle> = candles.iter().collect();
        sorted.sort_by_key(|c| c.ts);
        let closes: BTreeMap<i64, f64> = sorted
            .iter()
            .filter_map(|c| {
                let day = (c.ts.timestamp() + shift_secs).div_euclid(86_400);
                Some((day, c.close.to_f64()?))
            })
            .filter(|(_, close)| close.is_finite() && *close > 0.0)
            .collect();
        let mut sessions: Vec<i64> = sorted
            .iter()
            .map(|c| c.ts.timestamp().rem_euclid(86_400))
            .collect();
        sessions.sort_unstable();
        let volumes: Vec<f64> = sorted
            .iter()
            .rev()
            .filter_map(|c| c.volume)
            .take(VOLUME_SESSIONS)
            .map(|v| v as f64)
            .collect();
        Self {
            closes,
            session: sessions.get(sessions.len() / 2).copied(),
            avg_volume: (!volumes.is_empty()).then(|| mean(&volumes)),
        }
    }

    pub fn first_day(&self) -> Option<i64> {
        self.closes.keys().next().copied()
    }

    pub fn last_day(&self) -> Option<i64> {
        self.closes.keys().next_back().copied()
    }

    /// The last close on or before `day`, or the first close for a day
    /// before the history starts.
    pub fn close_on(&self, day: i64) -> Option<f64> {
        self.closes
            .range(..=day)
            .next_back()
            .or_else(|| self.closes.iter().next())
            .map(|(_, c)| *c)
    }

    /// The last close on or before `day`, if one falls within `tolerance`
    /// days of it.
    pub fn close_near(&self, day: i64, tolerance: i64) -> Option<f64> {
        self.closes
            .range(day - tolerance..=day)
            .next_back()
            .map(|(_, c)| *c)
    }

    /// The series in another currency. `from` and `to` are the USD value
    /// of one unit of this series' currency and of the target currency
    /// (`None` for USD itself); each close converts at its day's rates.
    pub fn converted(&self, from: Option<&PriceSeries>, to: Option<&PriceSeries>) -> Self {
        let rate = |fx: Option<&PriceSeries>, day: i64| fx.map_or(Some(1.0), |s| s.close_on(day));
        Self {
            closes: self
                .closes
                .iter()
                .filter_map(|(day, close)| Some((*day, close * rate(from, *day)? / rate(to, *day)?)))
                .collect(),
            ..self.clone()
        }
    }

    /// Price change from the close on `from` to the close on `to` (each
    /// the last close within `tolerance` days before).
    pub fn change_between(&self, from: i64, to: i64, tolerance: i64) -> Option<f64> {
        Some(self.close_near(to, tolerance)? / self.close_near(from, tolerance)? - 1.0)
    }
}

/// Whether `a`'s market opens well before `b`'s on the same (UTC) day, so
/// `a` only sees `b`'s day the next session.
fn trades_earlier(a: Option<i64>, b: Option<i64>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if b - a >= ASYNC_SECS)
}

/// Series lined up on a shared calendar.
struct Aligned {
    /// Day of the starting closes.
    start: i64,
    /// Day of each step.
    days: Vec<i64>,
    /// Simple return per series per step (0 on days a series didn't trade).
    returns: Vec<Vec<f64>>,
}

/// Returns of every series from the day `start` on, over the days any of
/// them traded. `None` if a series has no close on or before `start`.
fn align(series: &[&PriceSeries], start: i64) -> Option<Aligned> {
    let mut prev: Vec<f64> = series
        .iter()
        .map(|s| s.closes.range(..=start).next_back().map(|(_, c)| *c))
        .collect::<Option<_>>()?;
    let days: Vec<i64> = series
        .iter()
        .flat_map(|s| s.closes.range(start + 1..).map(|(d, _)| *d))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut returns = vec![Vec::with_capacity(days.len()); series.len()];
    for day in &days {
        for (i, s) in series.iter().enumerate() {
            let close = s.closes.get(day).copied().unwrap_or(prev[i]);
            returns[i].push(close / prev[i] - 1.0);
            prev[i] = close;
        }
    }
    Some(Aligned { start, days, returns })
}

/// Covariance of two aligned return series, adding the lagged term when
/// one market trades earlier in the day than the other.
fn cross_cov(a: &[f64], b: &[f64], a_first: bool, b_first: bool) -> f64 {
    let n = a.len().min(b.len());
    let mut c = covariance(&a[..n], &b[..n]);
    if n > 2 {
        if a_first {
            // a on day t sees b's day t−1.
            c += covariance(&a[1..n], &b[..n - 1]);
        }
        if b_first {
            c += covariance(&b[1..n], &a[..n - 1]);
        }
    }
    c
}

/// Pearson correlation, with the lag correction for markets that trade at
/// different times. `None` with too little shared history.
pub fn correlation(a: &PriceSeries, b: &PriceSeries) -> Option<f64> {
    let start = a.first_day()?.max(b.first_day()?);
    let aligned = align(&[a, b], start)?;
    let (x, y) = (&aligned.returns[0], &aligned.returns[1]);
    if x.len() < MIN_SAMPLES {
        return None;
    }
    let cov = cross_cov(
        x,
        y,
        trades_earlier(a.session, b.session),
        trades_earlier(b.session, a.session),
    );
    let denom = (variance(x) * variance(y)).sqrt();
    (denom > 0.0).then(|| (cov / denom).clamp(-1.0, 1.0))
}

/// Symmetric matrix of pairwise correlations (0 where there's too little
/// shared history).
pub fn correlation_matrix(series: &[PriceSeries]) -> Vec<Vec<f64>> {
    let n = series.len();
    let mut corr = vec![vec![1.0; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let c = correlation(&series[i], &series[j]).unwrap_or(0.0);
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
    /// Position in the `holdings` passed to [`RiskReport::compute`].
    pub index: usize,
    /// Weight in the analysed portfolio (0–1).
    pub weight: f64,
    /// Share of portfolio variance this holding is responsible for (0–1).
    pub risk_share: f64,
    pub volatility: f64,
    pub beta: f64,
    pub max_drawdown: f64,
}

/// Why a holding was left out of the analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exclusion {
    /// No usable price history.
    NoHistory,
    /// History starts too late in the window (e.g. a recent listing).
    TooNew,
}

/// How often the EWMA 95% VaR was breached in the window, as a check of
/// how well it would have worked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VarBacktest {
    /// Days forecast (after a short warm-up).
    pub days: usize,
    /// Days that lost more than the forecast.
    pub breaches: usize,
    /// Kupiec test: chance of a breach count this far from 5% if the model
    /// were right. Below 0.05 means the model mis-sizes risk.
    pub p_value: f64,
}

impl VarBacktest {
    pub fn breach_rate(&self) -> f64 {
        ratio(self.breaches as f64, self.days as f64)
    }
}

/// Portfolio risk over a window, assuming today's weights were held
/// throughout (rebalanced daily).
#[derive(Clone, Debug, PartialEq)]
pub struct RiskReport {
    /// Return days in the window.
    pub days: usize,
    /// Day of the starting prices and of the last close.
    pub first_day: i64,
    pub last_day: i64,
    /// Length of the window in years.
    pub years: f64,

    /// Total return over the window.
    pub period_return: f64,
    /// Compound annual growth rate (only meaningful for a year or more).
    pub annual_return: f64,
    /// Annualised, including the lag between markets.
    pub volatility: f64,
    /// Annualised EWMA volatility: how much it swings lately.
    pub volatility_now: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub max_drawdown: f64,
    /// 1-day 95% value at risk from the window's days, as a positive loss.
    pub var_95: f64,
    /// Average loss on days beyond the VaR (expected shortfall).
    pub cvar_95: f64,
    /// 1-day 95% VaR from the EWMA volatility (for tomorrow).
    pub var_95_now: f64,
    pub var_test: VarBacktest,

    pub beta: f64,
    /// Jensen's alpha: annual return beyond what beta earned.
    pub alpha: f64,
    pub market_correlation: f64,
    pub market_period_return: f64,
    pub market_return: f64,
    pub market_volatility: f64,
    pub market_max_drawdown: f64,

    /// Weighted average of holding volatilities / portfolio volatility.
    /// 1.0 = no diversification benefit.
    pub diversification_ratio: f64,
    /// Diversification ratio squared: how many independent bets the
    /// holdings amount to once correlation is counted.
    pub independent_bets: f64,
    /// Annual risk-free rate used for Sharpe, Sortino and alpha (fraction).
    pub risk_free: f64,
    /// Share of the analysed money held as cash (earning the risk-free rate).
    pub cash_weight: f64,
    /// Share of the money (holdings given, plus cash) the analysis covers.
    pub coverage: f64,
    /// (day, return) of the worst single day and worst calendar week.
    pub worst_day: (i64, f64),
    pub worst_week: (i64, f64),

    /// Growth of 1.0 for the portfolio and the market, from the start.
    pub growth: Vec<(i64, f64, f64)>,
    pub holdings: Vec<HoldingRisk>,
    /// Holdings left out, by position in the input.
    pub excluded: Vec<(usize, Exclusion)>,
}

impl RiskReport {
    /// `holdings` and `weights` are index-aligned; weights needn't sum to 1.
    /// `cash` is uninvested money in the same units as `weights` (0 for
    /// none). `risk_free` is the annual rate as a fraction, e.g. 0.04. All
    /// series must be in the same currency.
    pub fn compute(
        holdings: &[PriceSeries],
        weights: &[f64],
        cash: f64,
        market: &PriceSeries,
        risk_free: f64,
    ) -> Option<Self> {
        if holdings.len() != weights.len() {
            return None;
        }
        let (m_first, m_last) = (market.first_day()?, market.last_day()?);
        let latest_start = m_first + ((m_last - m_first) as f64 * MAX_LATE_START) as i64;

        let mut excluded = Vec::new();
        let mut included = Vec::new();
        for (i, h) in holdings.iter().enumerate() {
            if weights[i] <= 0.0 {
                continue;
            }
            match h.first_day() {
                _ if h.closes.len() < 2 => excluded.push((i, Exclusion::NoHistory)),
                Some(first) if first > latest_start => excluded.push((i, Exclusion::TooNew)),
                _ => included.push(i),
            }
        }
        let cash = cash.max(0.0);
        let included_weight: f64 = included.iter().map(|&i| weights[i]).sum();
        let all_weight: f64 = weights.iter().filter(|w| **w > 0.0).sum::<f64>() + cash;
        if included.is_empty() || included_weight <= 0.0 {
            return None;
        }
        let total = included_weight + cash;
        let w: Vec<f64> = included.iter().map(|&i| weights[i] / total).collect();
        let cash_weight = cash / total;

        let start = included
            .iter()
            .filter_map(|&i| holdings[i].first_day())
            .fold(m_first, i64::max);
        let mut refs: Vec<&PriceSeries> = included.iter().map(|&i| &holdings[i]).collect();
        refs.push(market);
        let aligned = align(&refs, start)?;
        let n = aligned.days.len();
        if n < MIN_SAMPLES * 2 {
            return None;
        }
        let (series, bench) = aligned.returns.split_at(included.len());
        let bench = &bench[0];
        let days = &aligned.days;
        let last_day = days[n - 1];
        let years = (last_day - aligned.start) as f64 / DAYS_PER_YEAR;
        if years <= 0.0 {
            return None;
        }
        let per_year = n as f64 / years;

        // Cash earns the risk-free rate for the calendar days of each step.
        let cash_returns: Vec<f64> = days
            .iter()
            .scan(aligned.start, |prev, day| {
                let r = risk_free * (day - *prev) as f64 / DAYS_PER_YEAR;
                *prev = *day;
                Some(r)
            })
            .collect();
        let port: Vec<f64> = (0..n)
            .map(|t| {
                series.iter().zip(&w).map(|(s, wi)| s[t] * wi).sum::<f64>()
                    + cash_weight * cash_returns[t]
            })
            .collect();

        // Covariances with the lag correction between markets.
        let k = series.len();
        let sessions: Vec<Option<i64>> = included.iter().map(|&i| holdings[i].session).collect();
        let cov: Vec<Vec<f64>> = (0..k)
            .map(|i| {
                (0..k)
                    .map(|j| {
                        if i == j {
                            variance(&series[i])
                        } else {
                            cross_cov(
                                &series[i],
                                &series[j],
                                trades_earlier(sessions[i], sessions[j]),
                                trades_earlier(sessions[j], sessions[i]),
                            )
                        }
                    })
                    .collect()
            })
            .collect();
        let sigma_w: Vec<f64> = (0..k).map(|i| (0..k).map(|j| cov[i][j] * w[j]).sum()).collect();
        let lagged_var: f64 = (0..k).map(|i| w[i] * sigma_w[i]).sum();
        // The lag terms can, rarely, make the estimate implausible.
        let port_var = if lagged_var > 0.0 { lagged_var } else { variance(&port) };

        let market_var = variance(bench);
        let betas: Vec<f64> = (0..k)
            .map(|i| {
                ratio(
                    cross_cov(
                        &series[i],
                        bench,
                        trades_earlier(sessions[i], market.session),
                        trades_earlier(market.session, sessions[i]),
                    ),
                    market_var,
                )
            })
            .collect();
        let beta: f64 = w.iter().zip(&betas).map(|(wi, b)| wi * b).sum();

        let holdings_risk: Vec<HoldingRisk> = (0..k)
            .map(|i| HoldingRisk {
                index: included[i],
                weight: w[i],
                risk_share: ratio(w[i] * sigma_w[i], lagged_var.max(0.0)),
                volatility: (cov[i][i] * per_year).sqrt(),
                beta: betas[i],
                max_drawdown: max_drawdown(&series[i]),
            })
            .collect();

        let volatility = (port_var * per_year).sqrt();
        let weighted_vol: f64 = holdings_risk.iter().map(|h| h.weight * h.volatility).sum();
        let diversification_ratio = ratio(weighted_vol, volatility);

        let growth_of = |xs: &[f64]| xs.iter().map(|r| 1.0 + r).product::<f64>();
        let period_return = growth_of(&port) - 1.0;
        let market_period_return = growth_of(bench) - 1.0;
        let annualise = |g: f64| (1.0 + g).max(0.0).powf(1.0 / years) - 1.0;
        let mean_annual = |xs: &[f64]| mean(xs) * per_year;
        let excess = mean_annual(&port) - risk_free;

        let (var_95, cvar_95) = value_at_risk(&port, 0.95);
        let (ewma_daily, var_test) = ewma_backtest(&port);

        let worst_day = days
            .iter()
            .zip(&port)
            .map(|(d, r)| (*d, *r))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or_default();
        let worst_week = worst_window(days, &port, 7).unwrap_or(worst_day);

        let mut growth = Vec::with_capacity(n + 1);
        let (mut gp, mut gm) = (1.0, 1.0);
        growth.push((aligned.start, gp, gm));
        for (t, day) in days.iter().enumerate() {
            gp *= 1.0 + port[t];
            gm *= 1.0 + bench[t];
            growth.push((*day, gp, gm));
        }

        Some(Self {
            days: n,
            first_day: aligned.start,
            last_day,
            years,
            period_return,
            annual_return: annualise(period_return),
            volatility,
            volatility_now: ewma_daily * per_year.sqrt(),
            sharpe: ratio(excess, volatility),
            sortino: ratio(excess, downside_deviation(&port, risk_free / per_year) * per_year.sqrt()),
            max_drawdown: max_drawdown(&port),
            var_95,
            cvar_95,
            var_95_now: Z_95 * ewma_daily,
            var_test,
            beta,
            alpha: excess - beta * (mean_annual(bench) - risk_free),
            market_correlation: ratio(beta * market_var, (port_var * market_var).sqrt()).clamp(-1.0, 1.0),
            market_period_return,
            market_return: annualise(market_period_return),
            market_volatility: (market_var * per_year).sqrt(),
            market_max_drawdown: max_drawdown(bench),
            diversification_ratio,
            independent_bets: diversification_ratio.powi(2),
            risk_free,
            cash_weight,
            coverage: ratio(total, all_weight),
            worst_day,
            worst_week,
            growth,
            holdings: holdings_risk,
            excluded,
        })
    }

    /// Whether the window is long enough to quote annual returns (returns
    /// over less than a year shouldn't be annualised).
    pub fn full_year(&self) -> bool {
        self.years >= 0.95
    }

    /// Whether there are enough days for the historical VaR to mean much.
    pub fn var_reliable(&self) -> bool {
        self.days >= RELIABLE_VAR_DAYS
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
    let n = xs.len().min(ys.len());
    if n < 2 {
        return 0.0;
    }
    let (mx, my) = (mean(&xs[..n]), mean(&ys[..n]));
    xs.iter()
        .zip(ys)
        .map(|(x, y)| (x - mx) * (y - my))
        .sum::<f64>()
        / (n - 1) as f64
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

/// Root mean square of returns below `target` (per step).
fn downside_deviation(xs: &[f64], target: f64) -> f64 {
    let squares: Vec<f64> = xs.iter().map(|r| (r - target).min(0.0).powi(2)).collect();
    mean(&squares).sqrt()
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

/// Worst compounded return over any `span`-calendar-day stretch, and the
/// day it ended.
fn worst_window(days: &[i64], xs: &[f64], span: i64) -> Option<(i64, f64)> {
    let mut from = 0;
    (0..xs.len())
        .map(|t| {
            while days[t] - days[from] >= span {
                from += 1;
            }
            let g: f64 = xs[from..=t].iter().map(|r| 1.0 + r).product();
            (days[t], g - 1.0)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Daily EWMA volatility after the last day, and how its 95% VaR held up:
/// each day is forecast only from the days before it.
fn ewma_backtest(xs: &[f64]) -> (f64, VarBacktest) {
    let seed = xs.len().min(EWMA_WARMUP);
    if seed == 0 {
        return (0.0, VarBacktest::default());
    }
    let mut var = xs[..seed].iter().map(|r| r * r).sum::<f64>() / seed as f64;
    let (mut days, mut breaches) = (0, 0);
    for r in &xs[seed..] {
        let forecast = Z_95 * var.sqrt();
        if forecast > 0.0 {
            days += 1;
            breaches += usize::from(-r > forecast);
        }
        var = EWMA_LAMBDA * var + (1.0 - EWMA_LAMBDA) * r * r;
    }
    let test = VarBacktest {
        days,
        breaches,
        p_value: kupiec_p_value(days, breaches, 0.05),
    };
    (var.sqrt(), test)
}

/// Kupiec's proportion-of-failures test: the chance of seeing a breach
/// rate this far from `p` over `days` if the true rate were `p`.
fn kupiec_p_value(days: usize, breaches: usize, p: f64) -> f64 {
    if days == 0 {
        return 1.0;
    }
    let (t, x) = (days as f64, breaches as f64);
    let log_lik = |q: f64| {
        let term = |count: f64, prob: f64| if count > 0.0 { count * prob.ln() } else { 0.0 };
        term(t - x, 1.0 - q) + term(x, q)
    };
    let lr = (2.0 * (log_lik(x / t) - log_lik(p))).max(0.0);
    // Chi-square with one degree of freedom.
    erfc((lr / 2.0).sqrt())
}

/// Complementary error function (Abramowitz & Stegun 7.1.26, |ε| < 1.5e-7).
fn erfc(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let y = poly * (-x * x).exp();
    if x >= 0.0 {
        y
    } else {
        2.0 - y
    }
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
    format!("{} {d}, {y}", crate::i18n::tr_str(MONTHS[(m - 1) as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const US_OPEN: i64 = 13 * 3600 + 1800;
    const BKK_OPEN: i64 = 3 * 3600;

    /// Prices from returns, one per day starting at day 1000.
    fn series_on(days: &[i64], returns: &[f64], session: i64) -> PriceSeries {
        let mut price = 100.0;
        let mut closes = BTreeMap::from([(days[0] - 1, price)]);
        for (d, r) in days.iter().zip(returns) {
            price *= 1.0 + r;
            closes.insert(*d, price);
        }
        PriceSeries { closes, session: Some(session), avg_volume: None }
    }

    fn series(returns: &[f64]) -> PriceSeries {
        let days: Vec<i64> = (1000..1000 + returns.len() as i64).collect();
        series_on(&days, returns, US_OPEN)
    }

    /// A deterministic, roughly uncorrelated return sequence.
    fn noise(n: usize, seed: u64) -> Vec<f64> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                ((x >> 33) as f64 / (1u64 << 31) as f64 - 0.5) / 25.0
            })
            .collect()
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn correlation_detects_same_and_opposite_moves() {
        let a = noise(60, 1);
        let neg: Vec<f64> = a.iter().map(|v| -v).collect();
        assert!(close(correlation(&series(&a), &series(&a)).unwrap(), 1.0, 1e-9));
        assert!(close(correlation(&series(&a), &series(&neg)).unwrap(), -1.0, 1e-9));
        assert!(correlation(&series(&a[..5]), &series(&a[..5])).is_none());
    }

    #[test]
    fn drawdown_and_var() {
        // +10%, -50%, +20%: peak 1.1 → trough 0.55.
        assert!(close(max_drawdown(&[0.1, -0.5, 0.2]), 0.5, 1e-9));
        let xs: Vec<f64> = (1..=100).map(|i| (i as f64 - 50.0) / 1000.0).collect();
        let (var, cvar) = value_at_risk(&xs, 0.95);
        assert!(close(var, 0.045, 1e-9));
        assert!(cvar >= var);
    }

    #[test]
    fn report_for_market_clone_has_beta_one_and_no_alpha() {
        let m = noise(260, 2);
        let report = RiskReport::compute(&[series(&m)], &[1.0], 0.0, &series(&m), 0.04).unwrap();
        assert!(close(report.beta, 1.0, 1e-9));
        assert!(report.alpha.abs() < 1e-9);
        assert!(close(report.holdings[0].risk_share, 1.0, 1e-9));
        assert_eq!(report.days, 260);
        assert_eq!(report.growth.len(), 261, "starts at the first close");
        assert!(close(report.market_correlation, 1.0, 1e-9));
    }

    #[test]
    fn leveraged_market_has_no_alpha() {
        // Funded 1.5× leverage earns nothing beyond its beta; a CAGR-based
        // alpha reported ±several % here.
        let m = noise(260, 3);
        let rf_day = 0.04 / (260.0 / (260.0 / 365.25));
        let p: Vec<f64> = m.iter().map(|r| rf_day + 1.5 * (r - rf_day)).collect();
        let report = RiskReport::compute(&[series(&p)], &[1.0], 0.0, &series(&m), 0.04).unwrap();
        assert!(close(report.beta, 1.5, 1e-9));
        assert!(report.alpha.abs() < 1e-3, "{}", report.alpha);
    }

    #[test]
    fn risk_shares_sum_to_one() {
        let a = noise(60, 4);
        let b: Vec<f64> = noise(60, 5).iter().map(|v| v * 2.0).collect();
        let report =
            RiskReport::compute(&[series(&a), series(&b)], &[3.0, 1.0], 0.0, &series(&a), 0.04).unwrap();
        let total: f64 = report.holdings.iter().map(|h| h.risk_share).sum();
        assert!(close(total, 1.0, 1e-9));
        assert!(close(report.holdings[0].weight, 0.75, 1e-9));
    }

    #[test]
    fn a_later_market_is_measured_against_the_previous_us_day() {
        // A Bangkok stock that follows New York's previous day: same-day
        // returns look unrelated, the lagged beta finds the link.
        let n = 300;
        let us = noise(n + 1, 6);
        let idio = noise(n + 1, 7);
        let days: Vec<i64> = (2000..2000 + n as i64).collect();
        let thai: Vec<f64> = (0..n).map(|t| 0.8 * us[t] + 0.3 * idio[t + 1]).collect();
        let market = series_on(&days, &us[1..], US_OPEN);
        let stock = series_on(&days, &thai, BKK_OPEN);
        let report = RiskReport::compute(std::slice::from_ref(&stock), &[1.0], 0.0, &market, 0.0).unwrap();
        assert!(close(report.beta, 0.8, 0.12), "{}", report.beta);

        let same_session = PriceSeries { session: Some(US_OPEN), ..stock };
        let naive = RiskReport::compute(&[same_session], &[1.0], 0.0, &market, 0.0).unwrap();
        assert!(naive.beta.abs() < 0.15, "{}", naive.beta);
    }

    #[test]
    fn holidays_keep_every_move() {
        // The stock rises 10% on a day the market is shut: the move must
        // survive (it used to be dropped with the day).
        let market_days: Vec<i64> = (0..30).map(|d| 3000 + d).filter(|d| *d != 3010).collect();
        let stock_days: Vec<i64> = (3000..3030).collect();
        let mut stock_returns = vec![0.0; 30];
        stock_returns[10] = 0.10;
        let m = noise(29, 8);
        let market = series_on(&market_days, &m, US_OPEN);
        let stock = series_on(&stock_days, &stock_returns, US_OPEN);
        let report = RiskReport::compute(&[stock], &[1.0], 0.0, &market, 0.0).unwrap();
        assert!(close(report.period_return, 0.10, 1e-9));
        assert_eq!(report.days, 30);
    }

    #[test]
    fn short_windows_scale_by_calendar_time() {
        // 63 trading days over ~3 months, +5%: 91 calendar days.
        let days: Vec<i64> = (0..91).map(|d| 4000 + d).filter(|d| (d - 4000) % 7 < 5).take(63).collect();
        let mut rs = vec![0.0; days.len()];
        rs[0] = 0.05;
        let s = series_on(&days, &rs, US_OPEN);
        let report = RiskReport::compute(std::slice::from_ref(&s), &[1.0], 0.0, &s, 0.0).unwrap();
        assert!(!report.full_year());
        assert!(close(report.period_return, 0.05, 1e-9));
        assert!(report.years < 0.26);
    }

    #[test]
    fn missing_and_new_holdings_are_left_out_not_fatal() {
        let m = noise(200, 9);
        let market = series(&m);
        let recent_days: Vec<i64> = (1150..1200).collect();
        let recent = series_on(&recent_days, &noise(50, 10), US_OPEN);
        let report = RiskReport::compute(
            &[series(&m), PriceSeries::default(), recent],
            &[50.0, 30.0, 20.0],
            0.0,
            &market,
            0.0,
        )
        .unwrap();
        assert_eq!(report.excluded, vec![(1, Exclusion::NoHistory), (2, Exclusion::TooNew)]);
        assert_eq!(report.days, 200, "the window isn't cut to the new listing");
        assert!(close(report.coverage, 0.5, 1e-9));
        assert_eq!(report.holdings[0].index, 0);
    }

    #[test]
    fn cash_dampens_risk_and_earns_the_risk_free_rate() {
        let m = noise(260, 11);
        let all_in = RiskReport::compute(&[series(&m)], &[1.0], 0.0, &series(&m), 0.04).unwrap();
        let half = RiskReport::compute(&[series(&m)], &[1.0], 1.0, &series(&m), 0.04).unwrap();
        assert!(close(half.volatility, all_in.volatility / 2.0, 1e-9));
        assert!(close(half.beta, 0.5, 1e-9));
        assert!(half.alpha.abs() < 1e-9);
        assert!(close(half.cash_weight, 0.5, 1e-12));
    }

    #[test]
    fn var_backtest_counts_breaches() {
        let calm = vec![0.001; 40];
        let (vol, test) = ewma_backtest(&[calm.clone(), vec![-0.05], calm].concat());
        assert!(vol > 0.0);
        assert_eq!(test.breaches, 1);
        assert_eq!(test.days, 61);
        // 5 breaches in 100 is right on target; 20 in 100 is not.
        assert!(kupiec_p_value(100, 5, 0.05) > 0.99);
        assert!(kupiec_p_value(100, 20, 0.05) < 0.001);
        assert!(close(erfc(0.0), 1.0, 1e-7) && close(erfc(1.0), 0.157_299_2, 1e-6));
    }

    #[test]
    fn converts_currency_day_by_day() {
        let s = PriceSeries { closes: BTreeMap::from([(1, 30.0), (2, 33.0)]), ..PriceSeries::default() };
        let thb = PriceSeries { closes: BTreeMap::from([(1, 0.030), (2, 0.025)]), ..PriceSeries::default() };
        // A USD price of 30 then 33 in baht at 1/0.030 and 1/0.025.
        let baht = s.converted(None, Some(&thb));
        assert!(close(baht.closes[&1], 1000.0, 1e-9) && close(baht.closes[&2], 1320.0, 1e-9));
        assert!(close(s.change_between(1, 5, 7).unwrap(), 0.1, 1e-12));
        assert_eq!(s.change_between(1, 20, 7), None, "no close near day 20");
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
