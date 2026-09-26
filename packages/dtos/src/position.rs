use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};

use crate::{portfolio::GetDashBoardResponse, transaction::Transaction};

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

    for tx in data.transactions.iter().filter(|tx| !tx.is_cash()) {
        let e = map.entry(tx.ticker.clone()).or_default();
        match tx.transaction_type {
            TransactionType::Buy => {
                e.0 += tx.shares * tx.price + tx.fee;
                e.1 += tx.shares;
            }
            TransactionType::Sell if e.1 > Decimal::ZERO => {
                let avg = e.0 / e.1;
                let sold = tx.shares.min(e.1);
                e.0 -= sold * avg;
                e.1 -= sold;
            }
            // `shares` holds the split ratio, e.g. 4 for a 4-for-1 split.
            TransactionType::Split if tx.shares > Decimal::ZERO => e.1 *= tx.shares,
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

/// Realized gain: sale proceeds above average cost (net of fees), plus
/// dividends (whose `price` is the cash amount received).
pub fn realized_pnl(transactions: &[Transaction]) -> Decimal {
    let mut book: HashMap<&TickerSymbol, (Decimal, Decimal)> = HashMap::new(); // (cost, shares)
    let mut realized = Decimal::ZERO;
    for tx in transactions.iter().filter(|tx| !tx.is_cash()) {
        let e = book.entry(&tx.ticker).or_default();
        match tx.transaction_type {
            TransactionType::Buy => {
                e.0 += tx.shares * tx.price + tx.fee;
                e.1 += tx.shares;
            }
            TransactionType::Sell if e.1 > Decimal::ZERO => {
                let avg = e.0 / e.1;
                let sold = tx.shares.min(e.1);
                realized += sold * (tx.price - avg) - tx.fee;
                e.0 -= sold * avg;
                e.1 -= sold;
            }
            TransactionType::Split if tx.shares > Decimal::ZERO => e.1 *= tx.shares,
            TransactionType::Dividend => realized += tx.price - tx.fee,
            _ => {}
        }
    }
    realized
}

/// Uninvested cash, or `None` when no deposits / withdrawals are recorded
/// (cash isn't being tracked).
pub fn cash_balance(transactions: &[Transaction]) -> Option<Decimal> {
    if !transactions.iter().any(|tx| tx.is_cash()) {
        return None;
    }
    Some(transactions.iter().fold(Decimal::ZERO, |cash, tx| {
        let amount = tx.shares * tx.price;
        cash + match tx.transaction_type {
            TransactionType::Deposit => amount - tx.fee,
            TransactionType::Withdrawal => -amount - tx.fee,
            TransactionType::Buy => -amount - tx.fee,
            TransactionType::Sell => amount - tx.fee,
            TransactionType::Dividend => tx.price - tx.fee,
            _ => Decimal::ZERO,
        }
    }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::CASH_TICKER;
    use uuid::Uuid;

    fn tx(ticker: &str, kind: TransactionType, shares: Decimal, price: Decimal, fee: Decimal) -> Transaction {
        Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new(ticker).unwrap(),
            transaction_type: kind,
            shares,
            price,
            date: "2026-01-01".into(),
            fee,
        }
    }

    #[test]
    fn fees_splits_realized_and_cash() {
        use TransactionType::*;
        let txs = vec![
            tx(CASH_TICKER, Deposit, dec!(1000), dec!(1), dec!(0)),
            tx("AAA", Buy, dec!(10), dec!(50), dec!(5)), // cost 505
            tx("AAA", Split, dec!(2), dec!(0), dec!(0)), // 20 shares
            tx("AAA", Sell, dec!(5), dec!(30), dec!(1)), // avg 25.25
            tx("AAA", Dividend, dec!(0), dec!(12), dec!(0)),
        ];
        let data = GetDashBoardResponse { portfolios: vec![], transactions: txs.clone() };
        let pos = compute_positions(&data, &HashMap::new());
        assert_eq!(pos.len(), 1, "cash is not a position");
        assert_eq!(pos[0].shares, dec!(15));
        assert_eq!(pos[0].avg_cost, dec!(25.25));
        // (30 − 25.25) × 5 − 1 fee + 12 dividend
        assert_eq!(realized_pnl(&txs), dec!(34.75));
        // 1000 − 505 + 149 + 12
        assert_eq!(cash_balance(&txs), Some(dec!(656)));
        assert_eq!(cash_balance(&txs[1..]), None);
    }
}
