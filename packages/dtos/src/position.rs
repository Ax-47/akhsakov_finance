use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};

use crate::{portfolio::GetDashBoardResponse, transaction::AppData};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub ticker: TickerSymbol,
    pub shares: Decimal,
    pub avg_cost: Decimal,
    pub current_price: Decimal,
    pub daily_change_pct: Decimal,
}

impl Position {
    pub fn market_value(&self) -> Decimal {
        self.shares * self.current_price
    }

    pub fn cost_basis(&self) -> Decimal {
        self.shares * self.avg_cost
    }

    pub fn unrealized_pnl(&self) -> Decimal {
        self.market_value() - self.cost_basis()
    }

    pub fn unrealized_pnl_pct(&self) -> Decimal {
        let basis = self.cost_basis();
        if basis > Decimal::ZERO {
            self.unrealized_pnl() / basis * dec!(100)
        } else {
            Decimal::ZERO
        }
    }
}

/// Build positions from transactions and live price data.
/// `prices` maps ticker → (current_price, daily_change_pct).
pub fn compute_positions(
    data: &GetDashBoardResponse,
    prices: &HashMap<TickerSymbol, (Decimal, Decimal)>,
) -> Vec<Position> {
    let mut map: HashMap<TickerSymbol, (Decimal, Decimal)> = HashMap::new(); // (cost_basis, shares)

    for tx in &data.transactions {
        let e = map.entry(tx.ticker.clone()).or_default();
        match tx.transaction_type {
            TransactionType::Buy => {
                e.0 += tx.shares * tx.price;
                e.1 += tx.shares;
            }
            TransactionType::Sell if e.1 > Decimal::ZERO => {
                let avg = e.0 / e.1;
                e.0 -= tx.shares * avg;
                e.1 -= tx.shares;
            }
            _ => {}
        }
    }

    let mut positions: Vec<Position> = map
        .into_iter()
        .filter(|(_, (_, shares))| *shares > dec!(0.0001))
        .map(|(ticker, (cost, shares))| {
            let (price, day_pct) = prices
                .get(&ticker)
                .copied()
                .unwrap_or((Decimal::ZERO, Decimal::ZERO));
            Position {
                avg_cost: if shares > Decimal::ZERO {
                    cost / shares
                } else {
                    Decimal::ZERO
                },
                ticker,
                shares,
                current_price: price,
                daily_change_pct: day_pct,
            }
        })
        .collect();

    positions.sort_by(|a, b| b.cost_basis().cmp(&a.cost_basis()));
    positions
}

// ── portfolio_summary ─────────────────────────────────────────────────────────

pub fn portfolio_summary(positions: &[Position]) -> (Decimal, Decimal, Decimal, Decimal) {
    let total_value: Decimal = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| p.market_value())
        .sum();

    let total_cost: Decimal = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| p.cost_basis())
        .sum();

    let total_pnl = total_value - total_cost;

    let day_change: Decimal = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| {
            let prev = p.current_price / (Decimal::ONE + p.daily_change_pct / dec!(100));
            p.shares * (p.current_price - prev)
        })
        .sum();

    (total_value, total_cost, total_pnl, day_change)
}
