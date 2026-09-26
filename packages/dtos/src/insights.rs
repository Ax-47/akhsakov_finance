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

#[derive(Debug, Clone, PartialEq)]
pub struct BacktestPlan {
    /// Target weight per asset (fractions, normalised if they don't sum to 1).
    pub weights: Vec<f64>,
    pub initial: f64,
    /// Added at every step after the first.
    pub contribution: f64,
    /// Reset to the target weights every 12 steps (once a year for monthly
    /// prices).
    pub rebalance_yearly: bool,
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
    // Spends `cash` across available assets at their target weights.
    let buy = |units: &mut [f64], cash: f64, t: usize| {
        let live: Vec<usize> = (0..prices.len())
            .filter(|&a| prices[a][t].is_some_and(|p| p > 0.0))
            .collect();
        let total_w: f64 = live.iter().map(|&a| plan.weights[a].max(0.0)).sum();
        if total_w <= 0.0 {
            return false;
        }
        for &a in &live {
            units[a] += cash * plan.weights[a].max(0.0) / total_w / prices[a][t].unwrap();
        }
        true
    };

    let mut invested = 0.0;
    let mut flows: Vec<(String, f64)> = Vec::new();
    let mut points = Vec::with_capacity(n);
    let (mut index, mut peak, mut max_dd) = (1.0_f64, 1.0_f64, 0.0_f64);
    let mut started: Option<usize> = None;
    for t in 0..n {
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
        if cash > 0.0 && buy(&mut units, cash, t) {
            invested += cash;
            flows.push((dates[t].clone(), -cash));
            started.get_or_insert(t);
        }
        if started.is_none() {
            continue;
        }
        if plan.rebalance_yearly && t > started.unwrap() && (t - started.unwrap()) % 12 == 0 {
            let total = value_at(&units, t);
            units.iter_mut().for_each(|u| *u = 0.0);
            buy(&mut units, total, t);
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
    BacktestResult {
        final_value,
        invested,
        irr: xirr(&flows),
        cagr: years.filter(|y| *y > 0.0).map(|y| index.powf(1.0 / y) - 1.0),
        max_drawdown: max_dd,
        points,
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
        let plan = BacktestPlan { weights: vec![1.0], initial: 1000.0, contribution: 0.0, rebalance_yearly: false };
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
        let plan = BacktestPlan { weights: vec![0.5, 0.5], initial: 100.0, contribution: 100.0, rebalance_yearly: false };
        let r = backtest(&dates, &prices, &plan);
        assert_eq!(r.invested, 400.0);
        assert!((r.max_drawdown + 0.5).abs() < 1e-9, "{}", r.max_drawdown);
        // Month 0: 10 units A. Month 1: +20 A. Month 2: +5 A, +2.5 B.
        // Month 3: +5 A, +2.5 B → 40 A × 10 + 5 B × 20 = 500.
        assert!((r.final_value - 500.0).abs() < 1e-9, "{}", r.final_value);
    }

    #[test]
    fn yearly_rebalance_restores_weights() {
        let dates = months(13);
        let prices = vec![
            (0..13).map(|i| Some(if i == 12 { 30.0 } else { 10.0 })).collect(),
            vec![Some(10.0); 13],
        ];
        let plan = BacktestPlan { weights: vec![0.5, 0.5], initial: 100.0, contribution: 0.0, rebalance_yearly: true };
        let r = backtest(&dates, &prices, &plan);
        // Before: 5 A × 30 + 5 B × 10 = 200, rebalanced 100 / 100.
        assert!((r.final_value - 200.0).abs() < 1e-9);
    }
}
