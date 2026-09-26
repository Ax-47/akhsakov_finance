//! Planning maths: rebalancing to target weights, FIFO tax lots and
//! realized gains, money-weighted return (XIRR) and goal projections.

use crate::transaction::Transaction;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

// ─── Rebalancing ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetWeight {
    pub ticker: TickerSymbol,
    /// Percent of the portfolio, 0–100.
    pub weight: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RebalanceTrade {
    pub ticker: TickerSymbol,
    /// Positive = buy, negative = sell, in money.
    pub amount: f64,
    /// Shares at the current price, when known.
    pub shares: Option<f64>,
    pub current_pct: f64,
    pub target_pct: f64,
}

/// Trades that move holdings to `targets`, after adding `new_cash`.
/// `holdings` is `(ticker, market value, price)`. Holdings without a target
/// are treated as target 0 (sell). Trades under `min_trade` are skipped.
pub fn rebalance(
    holdings: &[(TickerSymbol, f64, f64)],
    targets: &[TargetWeight],
    new_cash: f64,
    min_trade: f64,
) -> Vec<RebalanceTrade> {
    let total_now: f64 = holdings.iter().map(|h| h.1).sum();
    let total_after = total_now + new_cash;
    if total_after <= 0.0 {
        return vec![];
    }
    let mut tickers: Vec<&TickerSymbol> = holdings.iter().map(|h| &h.0).collect();
    for t in targets {
        if !tickers.contains(&&t.ticker) {
            tickers.push(&t.ticker);
        }
    }
    tickers
        .into_iter()
        .filter_map(|t| {
            let (value, price) = holdings
                .iter()
                .find(|h| &h.0 == t)
                .map_or((0.0, 0.0), |h| (h.1, h.2));
            let target_pct = targets
                .iter()
                .find(|x| &x.ticker == t)
                .and_then(|x| x.weight.to_f64())
                .unwrap_or(0.0);
            let amount = total_after * target_pct / 100.0 - value;
            (amount.abs() >= min_trade).then(|| RebalanceTrade {
                ticker: t.clone(),
                amount,
                shares: (price > 0.0).then(|| amount / price),
                current_pct: if total_now > 0.0 {
                    value / total_now * 100.0
                } else {
                    0.0
                },
                target_pct,
            })
        })
        .collect()
}

// ─── Tax lots ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Lot {
    pub ticker: TickerSymbol,
    pub date: String,
    pub shares: Decimal,
    /// Per share, including the buy fee.
    pub cost: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RealizedGain {
    pub ticker: TickerSymbol,
    pub bought: String,
    pub sold: String,
    pub shares: Decimal,
    pub proceeds: Decimal,
    pub cost: Decimal,
    /// Held more than a year.
    pub long_term: bool,
}

impl RealizedGain {
    pub fn gain(&self) -> Decimal {
        self.proceeds - self.cost
    }
}

/// Matches sales to the oldest purchases first (FIFO). Returns the lots
/// still open and every realized gain, oldest first.
pub fn fifo_lots(transactions: &[Transaction]) -> (Vec<Lot>, Vec<RealizedGain>) {
    let mut txs: Vec<&Transaction> = transactions.iter().filter(|t| !t.is_cash()).collect();
    txs.sort_by(|a, b| a.date.cmp(&b.date));
    let mut open: HashMap<&TickerSymbol, VecDeque<Lot>> = HashMap::new();
    let mut realized = Vec::new();

    for tx in txs {
        let lots = open.entry(&tx.ticker).or_default();
        match tx.transaction_type {
            TransactionType::Buy if tx.shares > Decimal::ZERO => lots.push_back(Lot {
                ticker: tx.ticker.clone(),
                date: tx.date.clone(),
                shares: tx.shares,
                cost: (tx.shares * tx.price + tx.fee) / tx.shares,
            }),
            TransactionType::Split if tx.shares > Decimal::ZERO => {
                for lot in lots.iter_mut() {
                    lot.shares *= tx.shares;
                    lot.cost /= tx.shares;
                }
            }
            TransactionType::Sell if tx.shares > Decimal::ZERO => {
                let mut remaining = tx.shares;
                let fee_per_share = tx.fee / tx.shares;
                while remaining > Decimal::ZERO {
                    let Some(lot) = lots.front_mut() else { break };
                    let take = remaining.min(lot.shares);
                    realized.push(RealizedGain {
                        ticker: tx.ticker.clone(),
                        bought: lot.date.clone(),
                        sold: tx.date.clone(),
                        shares: take,
                        proceeds: take * (tx.price - fee_per_share),
                        cost: take * lot.cost,
                        long_term: days_between(&lot.date, &tx.date).is_some_and(|d| d > 365),
                    });
                    lot.shares -= take;
                    remaining -= take;
                    if lot.shares.is_zero() {
                        lots.pop_front();
                    }
                }
            }
            _ => {}
        }
    }
    let mut lots: Vec<Lot> = open.into_values().flatten().collect();
    lots.sort_by(|a, b| {
        a.ticker
            .as_str()
            .cmp(b.ticker.as_str())
            .then(a.date.cmp(&b.date))
    });
    (lots, realized)
}

/// `(year, short-term gain, long-term gain, dividends)` per year, newest first.
pub fn realized_by_year(transactions: &[Transaction]) -> Vec<(i32, Decimal, Decimal, Decimal)> {
    let mut years: BTreeMap<i32, (Decimal, Decimal, Decimal)> = BTreeMap::new();
    let (_, realized) = fifo_lots(transactions);
    for r in realized {
        if let Some(y) = year(&r.sold) {
            let e = years.entry(y).or_default();
            if r.long_term {
                e.1 += r.gain();
            } else {
                e.0 += r.gain();
            }
        }
    }
    for tx in transactions
        .iter()
        .filter(|t| t.transaction_type == TransactionType::Dividend)
    {
        if let Some(y) = year(&tx.date) {
            years.entry(y).or_default().2 += tx.price - tx.fee;
        }
    }
    years
        .into_iter()
        .rev()
        .map(|(y, (s, l, d))| (y, s, l, d))
        .collect()
}

fn year(date: &str) -> Option<i32> {
    date.get(..4)?.parse().ok()
}

// ─── Money-weighted return ────────────────────────────────────────────────────

/// Cash flows from the investor's point of view: buys and deposits are
/// money in (negative), sells, dividends and withdrawals money out
/// (positive), plus today's value as the final inflow.
pub fn investor_cash_flows(
    transactions: &[Transaction],
    value_today: f64,
    today: &str,
) -> Vec<(String, f64)> {
    let tracks_cash = transactions.iter().any(|t| t.is_cash());
    let mut flows: Vec<(String, f64)> = transactions
        .iter()
        .filter_map(|t| {
            let gross = (t.shares * t.price).to_f64()?;
            let fee = t.fee.to_f64()?;
            let amount = match t.transaction_type {
                // With cash tracked, only deposits / withdrawals cross the
                // account boundary; trades are internal.
                TransactionType::Deposit if tracks_cash => -(gross + fee),
                TransactionType::Withdrawal if tracks_cash => gross - fee,
                TransactionType::Buy if !tracks_cash => -(gross + fee),
                TransactionType::Sell if !tracks_cash => gross - fee,
                TransactionType::Dividend if !tracks_cash => t.price.to_f64()? - fee,
                _ => return None,
            };
            Some((t.date.clone(), amount))
        })
        .collect();
    flows.push((today.to_string(), value_today));
    flows
}

/// Annualised internal rate of return for dated cash flows, or `None` if it
/// can't be solved (e.g. all flows the same sign).
pub fn xirr(flows: &[(String, f64)]) -> Option<f64> {
    let first = flows.iter().map(|f| f.0.as_str()).min()?;
    let points: Vec<(f64, f64)> = flows
        .iter()
        .map(|(d, v)| Some((days_between(first, d)? as f64 / 365.0, *v)))
        .collect::<Option<_>>()?;
    if !(points.iter().any(|p| p.1 > 0.0) && points.iter().any(|p| p.1 < 0.0)) {
        return None;
    }
    let npv = |r: f64| {
        points
            .iter()
            .map(|(t, v)| v / (1.0 + r).powf(*t))
            .sum::<f64>()
    };
    // Bisection on a wide bracket: robust, and plenty fast for our sizes.
    let (mut lo, mut hi) = (-0.99, 10.0);
    let (f_lo, f_hi) = (npv(lo), npv(hi));
    if f_lo.signum() == f_hi.signum() {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        let f_mid = npv(mid);
        if f_mid.abs() < 1e-9 {
            return Some(mid);
        }
        if f_mid.signum() == f_lo.signum() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo + hi) / 2.0)
}

// ─── Goals ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub id: Uuid,
    pub name: String,
    pub target: Decimal,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Planned contribution per month.
    pub monthly: Decimal,
    /// `None` = all holdings.
    pub portfolio_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoalProjection {
    pub months: u32,
    /// Value at the date, growing at `assumed_return`.
    pub projected: f64,
    pub on_track: bool,
    /// Annual return needed to reach the target; `None` if out of reach
    /// or already reached.
    pub required_return: Option<f64>,
}

/// Projects `current` plus `monthly` contributions to the goal date.
pub fn project_goal(goal: &Goal, current: f64, today: &str, assumed_return: f64) -> GoalProjection {
    let months =
        days_between(today, &goal.date).map_or(0, |d| (d.max(0) as f64 / 30.44).round() as u32);
    let (target, monthly) = (
        goal.target.to_f64().unwrap_or(0.0),
        goal.monthly.to_f64().unwrap_or(0.0),
    );
    let future = |annual: f64| {
        let r = (1.0 + annual).powf(1.0 / 12.0) - 1.0;
        let growth = (1.0 + r).powi(months as i32);
        let contributions = if r.abs() < 1e-12 {
            monthly * months as f64
        } else {
            monthly * (growth - 1.0) / r
        };
        current * growth + contributions
    };
    let projected = future(assumed_return);
    let required_return = if current + monthly * months as f64 >= target {
        None
    } else {
        // Bisection: future() rises with the rate.
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        (future(hi) >= target).then(|| {
            for _ in 0..100 {
                let mid = (lo + hi) / 2.0;
                if future(mid) >= target {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            hi
        })
    };
    GoalProjection {
        months,
        projected,
        on_track: projected >= target,
        required_return,
    }
}

// ─── Dates ────────────────────────────────────────────────────────────────────

/// Days from `a` to `b` (`YYYY-MM-DD`), negative if `b` is earlier.
pub fn days_between(a: &str, b: &str) -> Option<i64> {
    Some(day_number(b)? - day_number(a)?)
}

fn day_number(date: &str) -> Option<i64> {
    let y: i64 = date.get(..4)?.parse().ok()?;
    let m: i64 = date.get(5..7)?.parse().ok()?;
    let d: i64 = date.get(8..10)?.parse().ok()?;
    // days_from_civil
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    Some(era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sym(s: &str) -> TickerSymbol {
        TickerSymbol::new(s).unwrap()
    }

    fn tx(
        ticker: &str,
        kind: TransactionType,
        date: &str,
        shares: Decimal,
        price: Decimal,
        fee: Decimal,
    ) -> Transaction {
        Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: sym(ticker),
            transaction_type: kind,
            shares,
            price,
            date: date.into(),
            fee,
        }
    }

    #[test]
    fn rebalances_with_new_cash() {
        let holdings = vec![(sym("AAA"), 700.0, 10.0), (sym("BBB"), 300.0, 30.0)];
        let targets = vec![
            TargetWeight {
                ticker: sym("AAA"),
                weight: dec!(50),
            },
            TargetWeight {
                ticker: sym("BBB"),
                weight: dec!(30),
            },
            TargetWeight {
                ticker: sym("CCC"),
                weight: dec!(20),
            },
        ];
        let trades = rebalance(&holdings, &targets, 1000.0, 1.0);
        let get = |t: &str| trades.iter().find(|x| x.ticker.as_str() == t).unwrap();
        // Total after = 2000: AAA 1000 (+300), BBB 600 (+300), CCC 400 (+400).
        assert!((get("AAA").amount - 300.0).abs() < 1e-9);
        assert!((get("AAA").shares.unwrap() - 30.0).abs() < 1e-9);
        assert!((get("BBB").amount - 300.0).abs() < 1e-9);
        assert!((get("CCC").amount - 400.0).abs() < 1e-9);
        assert_eq!(get("CCC").shares, None, "no price for a new ticker");
        assert!((get("AAA").current_pct - 70.0).abs() < 1e-9);
    }

    #[test]
    fn fifo_matches_oldest_lots_and_tracks_holding_period() {
        use TransactionType::*;
        let txs = vec![
            tx("AAA", Buy, "2024-01-10", dec!(10), dec!(10), dec!(0)),
            tx("AAA", Buy, "2025-06-01", dec!(10), dec!(20), dec!(0)),
            tx("AAA", Sell, "2025-07-01", dec!(15), dec!(30), dec!(15)), // fee 1/share
            tx("AAA", Dividend, "2025-08-01", dec!(0), dec!(5), dec!(0)),
        ];
        let (lots, realized) = fifo_lots(&txs);
        assert_eq!(realized.len(), 2);
        // 10 from the 2024 lot: long-term, (29 − 10) × 10.
        assert!(realized[0].long_term);
        assert_eq!(realized[0].gain(), dec!(190));
        // 5 from the 2025 lot: short-term, (29 − 20) × 5.
        assert!(!realized[1].long_term);
        assert_eq!(realized[1].gain(), dec!(45));
        assert_eq!(lots.len(), 1);
        assert_eq!(lots[0].shares, dec!(5));
        assert_eq!(
            realized_by_year(&txs),
            vec![(2025, dec!(45), dec!(190), dec!(5))]
        );
    }

    #[test]
    fn xirr_of_a_simple_doubling() {
        // 1000 in, 1210 out two years later: 10% a year.
        let flows = vec![
            ("2024-01-01".into(), -1000.0),
            ("2026-01-01".into(), 1210.0),
        ];
        assert!((xirr(&flows).unwrap() - 0.10).abs() < 1e-3);
        assert_eq!(xirr(&[("2024-01-01".into(), 100.0)]), None);
    }

    #[test]
    fn goal_projection() {
        let goal = Goal {
            id: Uuid::nil(),
            name: "House".into(),
            target: dec!(10000),
            date: "2027-01-01".into(),
            monthly: dec!(0),
            portfolio_id: None,
        };
        // 12 months at 0%: stays at 5000, needs ~100%/yr.
        let p = project_goal(&goal, 5000.0, "2026-01-01", 0.0);
        assert_eq!(p.months, 12);
        assert!(!p.on_track);
        assert!((p.required_return.unwrap() - 1.0).abs() < 0.01);
        // Contributions alone reach it: no return needed.
        let saving = Goal {
            monthly: dec!(500),
            ..goal
        };
        assert_eq!(
            project_goal(&saving, 5000.0, "2026-01-01", 0.0).required_return,
            None
        );
    }
}
