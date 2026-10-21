//! Portfolio insights: dividend income (received and forecast), what drove
//! your return, and a backtest / regular-investing (DCA) simulator. All
//! amounts are USD.

use crate::{planning::xirr, position::Position, transaction::Transaction};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use types::transaction_type::TransactionType;

// ─── Dividends ────────────────────────────────────────────────────────────────

/// A stock's dividend, from the market (USD).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DividendInfo {
    pub ticker: String,
    /// Forecast dividends per share over the next year.
    pub annual_per_share: f64,
    /// At today's price, as a fraction.
    pub current_yield: Option<f64>,
    pub next_ex_date: Option<String>,
    pub next_payment: Option<String>,
}

/// Dividends received per month (`YYYY-MM`, net of tax withheld), oldest
/// first, with empty months between the first and last filled in.
pub fn dividends_by_month(transactions: &[Transaction]) -> Vec<(String, f64)> {
    let mut months: BTreeMap<String, f64> = BTreeMap::new();
    for t in transactions {
        if t.transaction_type == TransactionType::Dividend {
            let amount = (t.usd_price() - t.usd_fee()).to_f64().unwrap_or(0.0);
            *months.entry(t.date.get(..7).unwrap_or_default().to_string()).or_default() += amount;
        }
    }
    let (Some(first), Some(last)) = (months.keys().next().cloned(), months.keys().last().cloned())
    else {
        return vec![];
    };
    let mut out = Vec::new();
    let mut month = first;
    loop {
        out.push((month.clone(), months.get(&month).copied().unwrap_or(0.0)));
        if month >= last {
            break;
        }
        month = next_month(&month);
    }
    out
}

fn next_month(ym: &str) -> String {
    let (y, m): (i32, u32) = (ym[..4].parse().unwrap_or(0), ym[5..7].parse().unwrap_or(1));
    if m == 12 {
        format!("{:04}-01", y + 1)
    } else {
        format!("{y:04}-{:02}", m + 1)
    }
}

/// Forecast income from one holding.
#[derive(Debug, Clone, PartialEq)]
pub struct IncomeRow {
    pub ticker: String,
    pub shares: f64,
    pub annual_income: f64,
    /// Annual income over what you paid, as a fraction.
    pub yield_on_cost: Option<f64>,
    pub current_yield: Option<f64>,
    pub next_payment: Option<String>,
}

/// Next year's dividends from current holdings, largest first; holdings
/// that pay nothing are left out.
pub fn income_forecast(positions: &[Position], infos: &[DividendInfo]) -> Vec<IncomeRow> {
    let mut rows: Vec<IncomeRow> = positions
        .iter()
        .filter_map(|p| {
            let info = infos.iter().find(|i| i.ticker == p.ticker.as_str())?;
            let shares = p.shares.to_f64()?;
            let annual_income = info.annual_per_share * shares;
            if annual_income <= 0.0 {
                return None;
            }
            let cost = p.cost_basis().to_f64()?;
            Some(IncomeRow {
                ticker: info.ticker.clone(),
                shares,
                annual_income,
                yield_on_cost: (cost > 0.0).then(|| annual_income / cost),
                current_yield: info.current_yield,
                next_payment: info.next_payment.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| b.annual_income.total_cmp(&a.annual_income));
    rows
}

// ─── Return attribution ───────────────────────────────────────────────────────

/// Everything one stock has earned you.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TickerGain {
    pub ticker: String,
    /// On shares still held, at the given price.
    pub unrealized: f64,
    /// From sales, net of fees, at average cost.
    pub realized: f64,
    /// Net of tax withheld.
    pub dividends: f64,
    /// Still held.
    pub shares: f64,
}

impl TickerGain {
    pub fn total(&self) -> f64 {
        self.unrealized + self.realized + self.dividends
    }
}

/// Gains per stock, largest contribution first. `prices` are USD; a held
/// stock without a price has no unrealized gain.
pub fn gains_by_ticker(transactions: &[Transaction], prices: &HashMap<String, f64>) -> Vec<TickerGain> {
    // (cost, shares, gain) per ticker, average-cost basis.
    let mut book: BTreeMap<String, (f64, f64, TickerGain)> = BTreeMap::new();
    for t in transactions.iter().filter(|t| !t.is_cash()) {
        let (Some(shares), Some(price), Some(fee)) =
            (t.shares.to_f64(), t.usd_price().to_f64(), t.usd_fee().to_f64())
        else {
            continue;
        };
        let e = book.entry(t.ticker.to_string()).or_insert_with(|| {
            (0.0, 0.0, TickerGain { ticker: t.ticker.to_string(), ..Default::default() })
        });
        match t.transaction_type {
            TransactionType::Buy => {
                e.0 += shares * price + fee;
                e.1 += shares;
            }
            TransactionType::Sell if e.1 > 0.0 => {
                let avg = e.0 / e.1;
                let sold = shares.min(e.1);
                e.2.realized += sold * (price - avg) - fee;
                e.0 -= sold * avg;
                e.1 -= sold;
            }
            TransactionType::Split if shares > 0.0 => e.1 *= shares,
            TransactionType::Dividend => e.2.dividends += price - fee,
            _ => {}
        }
    }
    let mut gains: Vec<TickerGain> = book
        .into_iter()
        .map(|(ticker, (cost, shares, mut g))| {
            if shares > 1e-9 {
                g.shares = shares;
                if let Some(p) = prices.get(&ticker) {
                    g.unrealized = shares * p - cost;
                }
            }
            g
        })
        .collect();
    gains.sort_by(|a, b| b.total().total_cmp(&a.total()));
    gains
}

// ─── Backtest ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BacktestPlan {
    /// Target weight per asset (fractions, normalised if they don't sum to 1).
    pub weights: Vec<f64>,
    pub initial: f64,
    /// Added at every step after the first.
    pub contribution: f64,
    pub rebalance: Rebalance,
    pub fees: Fees,
}

/// When holdings are reset to the target weights.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Rebalance {
    #[default]
    Never,
    /// Every this many steps: 12 is yearly and 3 quarterly for monthly
    /// prices.
    Every(usize),
    /// Whenever an asset's share is further than this from its target, in
    /// fractions (0.05 = 5 percentage points).
    Drift(f64),
}

/// What investing costs, in the same money as the plan's amounts.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fees {
    /// Fraction of every amount bought or sold (0.001 = 0.1%).
    pub rate: f64,
    /// Charged for each asset bought or sold in a step.
    pub per_trade: f64,
    /// Fraction of the value taken each year, a twelfth every step.
    pub yearly: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BacktestResult {
    /// (date, value, total invested) per step.
    pub points: Vec<(String, f64, f64)>,
    pub final_value: f64,
    pub invested: f64,
    /// Money-weighted annual return of your cash flows.
    pub irr: Option<f64>,
    /// Time-weighted annual growth, ignoring when money went in.
    pub cagr: Option<f64>,
    /// Largest peak-to-trough fall of the time-weighted index, as a
    /// negative fraction.
    pub max_drawdown: f64,
    /// Steps (months, for monthly prices) from the peak before that fall
    /// until the index was back at the peak; `None` if it never fell or
    /// hasn't got back yet.
    pub recovery_months: Option<usize>,
    /// Annualised standard deviation of the monthly time-weighted returns;
    /// `None` with fewer than three months.
    pub volatility: Option<f64>,
    /// Best and worst full calendar years, as `(year, return)`.
    pub best_year: Option<(String, f64)>,
    pub worst_year: Option<(String, f64)>,
    /// Everything [`Fees`] took, in the same money as the plan's amounts.
    pub fees_paid: f64,
    /// How many times holdings were reset to the target weights.
    pub rebalances: usize,
}

impl BacktestResult {
    /// Return per unit of risk: (growth − `risk_free`) ÷ volatility, all
    /// annual fractions.
    pub fn sharpe(&self, risk_free: f64) -> Option<f64> {
        let vol = self.volatility.filter(|v| *v > 0.0)?;
        Some((self.cagr? - risk_free) / vol)
    }
}

/// Annualised volatility of monthly growth factors, and the best and worst
/// full calendar years, from the time-weighted index at each month
/// (`YYYY-MM-…` dates).
fn index_stats(index: &[(String, f64)]) -> (Option<f64>, Option<(String, f64)>, Option<(String, f64)>) {
    let returns: Vec<f64> = index.windows(2).map(|w| w[1].1 / w[0].1 - 1.0).collect();
    let volatility = (returns.len() >= 3).then(|| {
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
        var.sqrt() * 12f64.sqrt()
    });
    // December's index closes each year; a year counts once the one
    // before it has a December too.
    let decembers: Vec<(&str, f64)> = index
        .iter()
        .filter(|(d, _)| d.get(5..7) == Some("12"))
        .filter_map(|(d, v)| Some((d.get(..4)?, *v)))
        .collect();
    let years: Vec<(String, f64)> = decembers
        .windows(2)
        .map(|w| (w[1].0.to_string(), w[1].1 / w[0].1 - 1.0))
        .collect();
    let best = years.iter().cloned().max_by(|a, b| a.1.total_cmp(&b.1));
    let worst = years.iter().cloned().min_by(|a, b| a.1.total_cmp(&b.1));
    (volatility, best, worst)
}

/// Steps from the peak before the deepest fall of `index` until it first
/// reached that peak again.
fn recovery_steps(index: &[(String, f64)]) -> Option<usize> {
    let mut peak = 0;
    // (fall, peak position, trough position) of the deepest fall so far.
    let mut worst = (0.0, 0, 0);
    for (t, (_, v)) in index.iter().enumerate() {
        if *v >= index[peak].1 {
            peak = t;
        }
        let fall = v / index[peak].1 - 1.0;
        if fall < worst.0 {
            worst = (fall, peak, t);
        }
    }
    let (fall, peak, trough) = worst;
    if fall >= 0.0 {
        return None;
    }
    index[trough..]
        .iter()
        .position(|(_, v)| *v >= index[peak].1)
        .map(|i| trough + i - peak)
}

/// Simulates investing `plan` at each date. `prices[a][t]` is asset `a`'s
/// price at `dates[t]`; `None` before it existed (its share goes to the
/// others until then).
pub fn backtest(dates: &[String], prices: &[Vec<Option<f64>>], plan: &BacktestPlan) -> BacktestResult {
    let n = dates.len();
    if n == 0 || prices.is_empty() || prices.len() != plan.weights.len() {
        return BacktestResult::default();
    }
    let mut units = vec![0.0; prices.len()];
    let value_at = |units: &[f64], t: usize| -> f64 {
        units
            .iter()
            .zip(prices)
            .map(|(u, p)| p[t].map_or(0.0, |px| u * px))
            .sum()
    };
    let fees = plan.fees;
    // The assets with a price at `t` and a weight, and their total weight.
    let live = |t: usize| -> (Vec<usize>, f64) {
        let live: Vec<usize> = (0..prices.len())
            .filter(|&a| prices[a][t].is_some_and(|p| p > 0.0) && plan.weights[a] > 0.0)
            .collect();
        let total_w = live.iter().map(|&a| plan.weights[a]).sum();
        (live, total_w)
    };
    // Spends `cash` across available assets at their target weights, after
    // trading fees; returns the fees, or `None` if nothing can be bought.
    let buy = |units: &mut [f64], cash: f64, t: usize| -> Option<f64> {
        let (live, total_w) = live(t);
        if total_w <= 0.0 {
            return None;
        }
        let net = (cash - fees.per_trade * live.len() as f64).max(0.0) / (1.0 + fees.rate);
        for &a in &live {
            units[a] += net * plan.weights[a] / total_w / prices[a][t].unwrap();
        }
        Some(cash - net)
    };
    // Resets holdings to the target weights; returns the trading fees, worked
    // out on the trades before fees (close enough at realistic rates).
    let rebalance = |units: &mut [f64], t: usize| -> f64 {
        let (live, total_w) = live(t);
        if total_w <= 0.0 {
            return 0.0;
        }
        let total = value_at(units, t);
        let trades: Vec<f64> = (0..units.len())
            .map(|a| {
                let target = if live.contains(&a) { total * plan.weights[a] / total_w } else { 0.0 };
                (target - prices[a][t].map_or(0.0, |p| units[a] * p)).abs()
            })
            .collect();
        let traded = trades.iter().filter(|x| **x > 1e-9).count();
        let fee = (fees.rate * trades.iter().sum::<f64>() + fees.per_trade * traded as f64).min(total);
        units.iter_mut().for_each(|u| *u = 0.0);
        for &a in &live {
            units[a] = (total - fee) * plan.weights[a] / total_w / prices[a][t].unwrap();
        }
        fee
    };

    let mut invested = 0.0;
    let mut fees_paid = 0.0;
    let mut rebalances = 0;
    // Whether any asset's share is more than `band` from its target.
    let drifted = |units: &[f64], t: usize, band: f64| -> bool {
        let (live, total_w) = live(t);
        let total = value_at(units, t);
        total > 0.0
            && live.iter().any(|&a| {
                let share = units[a] * prices[a][t].unwrap() / total;
                (share - plan.weights[a] / total_w).abs() > band
            })
    };
    let mut flows: Vec<(String, f64)> = Vec::new();
    let mut points = Vec::with_capacity(n);
    let (mut index, mut peak, mut max_dd) = (1.0_f64, 1.0_f64, 0.0_f64);
    let mut started: Option<usize> = None;
    let mut index_points: Vec<(String, f64)> = Vec::new();
    for t in 0..n {
        if started.is_some_and(|s| t > s) && fees.yearly > 0.0 {
            let share = fees.yearly / 12.0;
            fees_paid += value_at(&units, t) * share;
            units.iter_mut().for_each(|u| *u *= 1.0 - share);
        }
        let before = value_at(&units, t);
        if let Some(s) = started {
            if t > s {
                // Time-weighted growth since the last step, before new money.
                let prev_after = points.last().map_or(0.0, |p: &(String, f64, f64)| p.1);
                if prev_after > 0.0 {
                    index *= before / prev_after;
                    peak = peak.max(index);
                    max_dd = max_dd.min(index / peak - 1.0);
                }
            }
        }
        let cash = if started.is_none() { plan.initial } else { plan.contribution };
        if let Some(fee) = (cash > 0.0).then(|| buy(&mut units, cash, t)).flatten() {
            fees_paid += fee;
            invested += cash;
            flows.push((dates[t].clone(), -cash));
            started.get_or_insert(t);
        }
        if started.is_none() {
            continue;
        }
        index_points.push((dates[t].clone(), index));
        let since = t - started.unwrap();
        let due = since > 0
            && match plan.rebalance {
                Rebalance::Never => false,
                Rebalance::Every(steps) => steps > 0 && since % steps == 0,
                Rebalance::Drift(band) => drifted(&units, t, band),
            };
        if due {
            fees_paid += rebalance(&mut units, t);
            rebalances += 1;
        }
        points.push((dates[t].clone(), value_at(&units, t), invested));
    }

    let final_value = points.last().map_or(0.0, |p| p.1);
    let years = match (started, points.last()) {
        (Some(s), Some(last)) => crate::planning::days_between(&dates[s], &last.0).map(|d| d as f64 / 365.25),
        _ => None,
    };
    if let Some(last) = points.last() {
        flows.push((last.0.clone(), final_value));
    }
    let (volatility, best_year, worst_year) = index_stats(&index_points);
    BacktestResult {
        recovery_months: recovery_steps(&index_points),
        fees_paid,
        rebalances,
        volatility,
        best_year,
        worst_year,
        final_value,
        invested,
        irr: xirr(&flows),
        cagr: years.filter(|y| *y > 0.0).map(|y| index.powf(1.0 / y) - 1.0),
        max_drawdown: max_dd,
        points,
    }
}

#[cfg(test)]
mod backtest_stats_tests {
    use super::*;

    #[test]
    fn volatility_and_calendar_years() {
        // One asset: flat through 2024, +10% over 2025 in one step in May,
        // then −20% in 2026 (a full year once December 2026 is in).
        let mut dates = vec![];
        let mut prices = vec![];
        let mut p = 100.0;
        for year in 2024..=2026 {
            for month in 1..=12 {
                if year == 2025 && month == 5 {
                    p *= 1.1;
                }
                if year == 2026 && month == 3 {
                    p *= 0.8;
                }
                dates.push(format!("{year}-{month:02}-01"));
                prices.push(Some(p));
            }
        }
        let plan = BacktestPlan { weights: vec![1.0], initial: 1000.0, contribution: 0.0, ..Default::default() };
        let r = backtest(&dates, &[prices], &plan);
        let (best, worst) = (r.best_year.clone().unwrap(), r.worst_year.clone().unwrap());
        assert_eq!(best.0, "2025");
        assert!((best.1 - 0.1).abs() < 1e-9, "{best:?}");
        assert_eq!(worst.0, "2026");
        assert!((worst.1 + 0.2).abs() < 1e-9, "{worst:?}");
        let vol = r.volatility.unwrap();
        assert!(vol > 0.0 && vol < 1.0, "{vol}");
        // Growth below the risk-free rate gives a negative Sharpe.
        assert!(r.sharpe(0.05).unwrap() < 0.0);
    }

    #[test]
    fn time_to_recover_from_the_worst_fall() {
        let dates: Vec<String> = (1..=6).map(|m| format!("2026-{m:02}-01")).collect();
        let plan = BacktestPlan { weights: vec![1.0], initial: 100.0, contribution: 0.0, ..Default::default() };
        let run = |p: &[f64]| backtest(&dates[..p.len()], &[p.iter().copied().map(Some).collect()], &plan);
        // Halves, then is back at the old high three months after it.
        let r = run(&[10.0, 5.0, 8.0, 10.0, 12.0, 12.0]);
        assert!((r.max_drawdown + 0.5).abs() < 1e-9);
        assert_eq!(r.recovery_months, Some(3));
        // A smaller, later dip doesn't count: only the deepest fall does.
        let r = run(&[10.0, 6.0, 11.0, 10.0, 11.0, 12.0]);
        assert_eq!(r.recovery_months, Some(2));
        // Still below the old high at the end.
        let r = run(&[10.0, 5.0, 8.0]);
        assert_eq!(r.recovery_months, None);
        assert!(r.max_drawdown < 0.0);
        // Never fell.
        let r = run(&[10.0, 11.0, 12.0]);
        assert_eq!((r.max_drawdown, r.recovery_months), (0.0, None));
    }

    #[test]
    fn too_short_for_stats() {
        let dates: Vec<String> = (1..=2).map(|m| format!("2026-{m:02}-01")).collect();
        let plan = BacktestPlan { weights: vec![1.0], initial: 100.0, contribution: 0.0, ..Default::default() };
        let r = backtest(&dates, &[vec![Some(1.0), Some(1.1)]], &plan);
        assert_eq!((r.volatility, r.best_year, r.worst_year), (None, None, None));
    }
}

#[cfg(test)]
mod fee_tests {
    use super::*;

    fn months(n: usize) -> Vec<String> {
        (1..=n).map(|m| format!("2026-{m:02}-01")).collect()
    }

    #[test]
    fn trading_fees_come_off_each_purchase() {
        let dates = months(2);
        let prices = vec![vec![Some(10.0); 2], vec![Some(20.0); 2]];
        let plan = BacktestPlan {
            weights: vec![0.5, 0.5],
            initial: 1000.0,
            contribution: 100.0,
            fees: Fees { rate: 0.01, per_trade: 2.0, yearly: 0.0 },
            ..Default::default()
        };
        let r = backtest(&dates, &prices, &plan);
        // 1000 − 2 × 2 = 996 to trade, 996 / 1.01 bought; then 100 − 4 = 96.
        let bought = 996.0 / 1.01 + 96.0 / 1.01;
        assert!((r.final_value - bought).abs() < 1e-9, "{}", r.final_value);
        assert!((r.fees_paid - (1100.0 - bought)).abs() < 1e-9, "{}", r.fees_paid);
        assert_eq!(r.invested, 1100.0);
        // Flat prices, so the fees are the whole loss.
        assert!(r.irr.unwrap() < 0.0);
    }

    #[test]
    fn a_yearly_fee_is_taken_monthly_and_shows_in_growth() {
        let dates = months(13);
        let prices = vec![vec![Some(10.0); 13]];
        let plan = BacktestPlan {
            weights: vec![1.0],
            initial: 1200.0,
            fees: Fees { yearly: 0.012, ..Default::default() },
            ..Default::default()
        };
        let r = backtest(&dates, &prices, &plan);
        let left = 1200.0 * (1.0 - 0.001f64).powi(12);
        assert!((r.final_value - left).abs() < 1e-9, "{}", r.final_value);
        assert!((r.fees_paid - (1200.0 - left)).abs() < 1e-9);
        assert!((r.cagr.unwrap() + 0.012).abs() < 0.001, "{:?}", r.cagr);
        assert!(r.max_drawdown < 0.0);
    }

    #[test]
    fn rebalancing_pays_on_what_it_trades() {
        let dates = months(13);
        let prices = vec![
            (0..13).map(|i| Some(if i == 12 { 30.0 } else { 10.0 })).collect(),
            vec![Some(10.0); 13],
        ];
        let plan = BacktestPlan {
            weights: vec![0.5, 0.5],
            initial: 100.0,
            rebalance: Rebalance::Every(12),
            fees: Fees { rate: 0.01, ..Default::default() },
            ..Default::default()
        };
        let r = backtest(&dates, &prices, &plan);
        // Bought 100 / 1.01 at the start, so 4.95 units of each. A triples:
        // 148.5 + 49.5 = 198, rebalanced by moving 49.5 from A to B, which
        // costs 1% of the 99 traded.
        let start = 100.0 / 1.01;
        let before = start / 2.0 * 3.0 + start / 2.0;
        let fee = 0.01 * (start / 2.0 * 3.0 - before / 2.0) * 2.0;
        assert!((r.final_value - (before - fee)).abs() < 1e-9, "{}", r.final_value);
        assert!((r.fees_paid - (100.0 - start + fee)).abs() < 1e-9);
    }

    #[test]
    fn no_fees_by_default() {
        let dates = months(3);
        let plan = BacktestPlan { weights: vec![1.0], initial: 100.0, contribution: 10.0, ..Default::default() };
        let r = backtest(&dates, &[vec![Some(10.0); 3]], &plan);
        assert_eq!((r.final_value, r.fees_paid), (120.0, 0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use types::ticker_symbol::TickerSymbol;
    use uuid::Uuid;

    fn tx(ticker: &str, kind: TransactionType, shares: rust_decimal::Decimal, price: rust_decimal::Decimal, date: &str) -> Transaction {
        Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new(ticker).unwrap(),
            transaction_type: kind,
            shares,
            price,
            date: date.into(),
            fee: dec!(0),
            currency: "USD".into(),
            fx_to_usd: dec!(1),
        }
    }

    #[test]
    fn dividends_fill_empty_months() {
        use TransactionType::*;
        let txs = [
            tx("KO", Dividend, dec!(0), dec!(10), "2025-11-15"),
            tx("KO", Dividend, dec!(0), dec!(12), "2026-02-15"),
            tx("PG", Dividend, dec!(0), dec!(3), "2026-02-20"),
        ];
        let months = dividends_by_month(&txs);
        let labels: Vec<&str> = months.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(labels, ["2025-11", "2025-12", "2026-01", "2026-02"]);
        assert_eq!(months[3].1, 15.0);
        assert_eq!(months[1].1, 0.0);
    }

    #[test]
    fn attributes_gains_per_stock() {
        use TransactionType::*;
        let txs = [
            tx("A", Buy, dec!(10), dec!(10), "2026-01-01"),
            tx("A", Sell, dec!(5), dec!(14), "2026-02-01"), // +20
            tx("A", Dividend, dec!(0), dec!(3), "2026-03-01"),
            tx("B", Buy, dec!(2), dec!(50), "2026-01-01"),
        ];
        let prices = HashMap::from([("A".to_string(), 12.0), ("B".to_string(), 40.0)]);
        let g = gains_by_ticker(&txs, &prices);
        assert_eq!(g[0].ticker, "A");
        assert_eq!((g[0].realized, g[0].dividends, g[0].unrealized), (20.0, 3.0, 10.0));
        assert_eq!(g[1].total(), -20.0);
    }

    #[test]
    fn income_forecast_uses_holdings_and_cost() {
        let p = Position {
            ticker: TickerSymbol::new("KO").unwrap(),
            shares: dec!(10),
            avg_cost: dec!(50),
            current_price: dec!(60),
            daily_change_pct: dec!(0),
        };
        let info = DividendInfo {
            ticker: "KO".into(),
            annual_per_share: 2.0,
            current_yield: Some(0.033),
            next_ex_date: None,
            next_payment: Some("2026-10-01".into()),
        };
        let rows = income_forecast(&[p], &[info]);
        assert_eq!(rows[0].annual_income, 20.0);
        assert_eq!(rows[0].yield_on_cost, Some(0.04));
    }

    fn months(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("{}-{:02}-01", 2020 + i / 12, i % 12 + 1)).collect()
    }

    #[test]
    fn lump_sum_in_a_steady_riser() {
        // +1% a month for two years.
        let dates = months(25);
        let prices = vec![(0..25).map(|i| Some(100.0 * 1.01f64.powi(i))).collect()];
        let plan = BacktestPlan { weights: vec![1.0], initial: 1000.0, contribution: 0.0, ..Default::default() };
        let r = backtest(&dates, &prices, &plan);
        assert!((r.final_value - 1000.0 * 1.01f64.powi(24)).abs() < 1e-6);
        assert_eq!(r.invested, 1000.0);
        assert!((r.cagr.unwrap() - 0.1268).abs() < 0.002, "{:?}", r.cagr);
        assert!((r.irr.unwrap() - r.cagr.unwrap()).abs() < 0.002);
        assert_eq!(r.max_drawdown, 0.0);
    }

    #[test]
    fn contributions_drawdown_and_late_assets() {
        let dates = months(4);
        // B doesn't exist yet at the start; A halves then recovers.
        let prices = vec![
            vec![Some(10.0), Some(5.0), Some(10.0), Some(10.0)],
            vec![None, None, Some(20.0), Some(20.0)],
        ];
        let plan = BacktestPlan { weights: vec![0.5, 0.5], initial: 100.0, contribution: 100.0, ..Default::default() };
        let r = backtest(&dates, &prices, &plan);
        assert_eq!(r.invested, 400.0);
        assert!((r.max_drawdown + 0.5).abs() < 1e-9, "{}", r.max_drawdown);
        // Month 0: 10 units A. Month 1: +20 A. Month 2: +5 A, +2.5 B.
        // Month 3: +5 A, +2.5 B → 40 A × 10 + 5 B × 20 = 500.
        assert!((r.final_value - 500.0).abs() < 1e-9, "{}", r.final_value);
    }

    #[test]
    fn quarterly_and_drift_rebalancing() {
        // A rises 10% a month; B is flat.
        let dates = months(13);
        let prices = vec![(0..13).map(|i| Some(10.0 * 1.1f64.powi(i))).collect(), vec![Some(10.0); 13]];
        let plan = |rebalance| BacktestPlan { weights: vec![0.5, 0.5], initial: 100.0, rebalance, ..Default::default() };
        let quarterly = backtest(&dates, &prices, &plan(Rebalance::Every(3)));
        assert_eq!(quarterly.rebalances, 4);
        let yearly = backtest(&dates, &prices, &plan(Rebalance::Every(12)));
        assert_eq!(yearly.rebalances, 1);
        // Selling the riser to top up the flat one costs growth here.
        assert!(quarterly.final_value < yearly.final_value);
        // A's share passes 55% in month 3 (57.1%), so a 5-point band trades
        // then, and again each time it drifts that far: months 6, 9 and 12.
        let drift = backtest(&dates, &prices, &plan(Rebalance::Drift(0.05)));
        assert_eq!(drift.rebalances, 4);
        let never = backtest(&dates, &prices, &plan(Rebalance::Never));
        assert_eq!(never.rebalances, 0);
        let wide = backtest(&dates, &prices, &plan(Rebalance::Drift(0.5)));
        assert_eq!(wide.rebalances, 0);
        assert_eq!(wide.final_value, never.final_value);
    }

    #[test]
    fn yearly_rebalance_restores_weights() {
        let dates = months(13);
        let prices = vec![
            (0..13).map(|i| Some(if i == 12 { 30.0 } else { 10.0 })).collect(),
            vec![Some(10.0); 13],
        ];
        let plan = BacktestPlan { weights: vec![0.5, 0.5], initial: 100.0, contribution: 0.0, rebalance: Rebalance::Every(12), ..Default::default() };
        let r = backtest(&dates, &prices, &plan);
        // Before: 5 A × 30 + 5 B × 10 = 200, rebalanced 100 / 100.
        assert!((r.final_value - 200.0).abs() < 1e-9);
    }
}
