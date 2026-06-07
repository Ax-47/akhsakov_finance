use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::transaction::{AppData, TransactionType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub ticker: String,
    pub shares: f64,
    pub avg_cost: f64,
    pub current_price: f64,
    pub daily_change_pct: f64,
}

impl Position {
    pub fn market_value(&self) -> f64 {
        self.shares * self.current_price
    }

    pub fn cost_basis(&self) -> f64 {
        self.shares * self.avg_cost
    }

    pub fn unrealized_pnl(&self) -> f64 {
        self.market_value() - self.cost_basis()
    }

    pub fn unrealized_pnl_pct(&self) -> f64 {
        let basis = self.cost_basis();
        if basis > 0.0 {
            self.unrealized_pnl() / basis * 100.0
        } else {
            0.0
        }
    }
}

/// Build positions from transactions and live price data.
/// `prices` maps ticker → (current_price, daily_change_pct).
pub fn compute_positions(data: &AppData, prices: &HashMap<String, (f64, f64)>) -> Vec<Position> {
    // ticker → (total_cost_basis, total_shares)
    let mut map: HashMap<String, (f64, f64)> = HashMap::new();

    for tx in &data.transactions {
        let e = map.entry(tx.ticker.clone()).or_default();
        match tx.transaction_type {
            TransactionType::Buy => {
                e.0 += tx.shares * tx.price + tx.fees;
                e.1 += tx.shares;
            }
            TransactionType::Sell => {
                if e.1 > 1e-9 {
                    let avg = e.0 / e.1;
                    e.0 -= tx.shares * avg;
                    e.1 -= tx.shares;
                }
            }
            TransactionType::Dividend => {}
        }
    }

    let mut positions: Vec<Position> = map
        .into_iter()
        .filter(|(_, (_, shares))| *shares > 1e-4)
        .map(|(ticker, (cost, shares))| {
            let (price, day_pct) = prices.get(&ticker).copied().unwrap_or((0.0, 0.0));
            Position {
                ticker,
                shares,
                avg_cost: if shares > 0.0 { cost / shares } else { 0.0 },
                current_price: price,
                daily_change_pct: day_pct,
            }
        })
        .collect();

    positions.sort_by(|a, b| {
        b.cost_basis()
            .partial_cmp(&a.cost_basis())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    positions
}

/// Aggregate portfolio-level stats from a position slice.
/// Returns `(total_value, total_cost, unrealized_pnl, day_dollar_change)`.
pub fn portfolio_summary(positions: &[Position]) -> (f64, f64, f64, f64) {
    let total_value: f64 = positions
        .iter()
        .map(|p| {
            if p.current_price > 0.0 {
                p.market_value()
            } else {
                p.cost_basis()
            }
        })
        .sum();

    let total_cost: f64 = positions.iter().map(|p| p.cost_basis()).sum();
    let total_pnl = total_value - total_cost;

    let day_change: f64 = positions
        .iter()
        .filter(|p| p.current_price > 0.0 && p.daily_change_pct.abs() > 1e-9)
        .map(|p| {
            let prev = p.current_price / (1.0 + p.daily_change_pct / 100.0);
            p.shares * (p.current_price - prev)
        })
        .sum();

    (total_value, total_cost, total_pnl, day_change)
}
