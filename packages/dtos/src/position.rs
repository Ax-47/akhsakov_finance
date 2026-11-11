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
                e.0 += tx.shares * tx.usd_price() + tx.usd_fee();
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
                e.0 += tx.shares * tx.usd_price() + tx.usd_fee();
                e.1 += tx.shares;
            }
            TransactionType::Sell if e.1 > Decimal::ZERO => {
                let avg = e.0 / e.1;
                let sold = tx.shares.min(e.1);
                realized += sold * (tx.usd_price() - avg) - tx.usd_fee();
                e.0 -= sold * avg;
                e.1 -= sold;
            }
            TransactionType::Split if tx.shares > Decimal::ZERO => e.1 *= tx.shares,
            TransactionType::Dividend => realized += tx.usd_price() - tx.usd_fee(),
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
        let (amount, fee) = (tx.shares * tx.usd_price(), tx.usd_fee());
        cash + match tx.transaction_type {
            TransactionType::Deposit => amount - fee,
            TransactionType::Withdrawal => -amount - fee,
            TransactionType::Buy => -amount - fee,
            TransactionType::Sell => amount - fee,
            TransactionType::Dividend => tx.usd_price() - fee,
            _ => Decimal::ZERO,
        }
    }))
}

// ── What can be sold ──────────────────────────────────────────────────────────

/// Shares too few to count as a holding; a sale may overshoot by this much
/// (brokers round fractional shares differently).
pub const DUST: Decimal = dec!(0.0001);

/// What one portfolio has of a ticker to sell on a date.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sellable {
    /// Shares held at the end of the date.
    pub held: Decimal,
    /// The part of `held` that sales recorded on later dates don't need.
    pub free: Decimal,
}

/// What `transactions` (one portfolio's, without the sale being checked)
/// leave of `ticker` to sell on `date`. Everything recorded on `date`
/// counts, as the day's buys are replayed before its sales.
pub fn sellable_shares(transactions: &[Transaction], ticker: &TickerSymbol, date: &str) -> Sellable {
    let mut ledger: Vec<&Transaction> = transactions.iter().filter(|tx| &tx.ticker == ticker).collect();
    ledger.sort_by(|a, b| a.date.cmp(&b.date));
    let mut held = Decimal::ZERO;
    let mut later = ledger.iter().peekable();
    while let Some(tx) = later.next_if(|tx| tx.date.as_str() <= date) {
        apply_shares(&mut held, tx);
    }
    // A sale today of `n` shares takes `n × splits since` from every later
    // balance, none of which may go below zero.
    let (mut free, mut splits) = (held, Decimal::ONE);
    let mut balance = held;
    for tx in later {
        if tx.transaction_type == TransactionType::Split && tx.shares > Decimal::ZERO {
            splits *= tx.shares;
        }
        apply_shares(&mut balance, tx);
        free = free.min(balance / splits);
    }
    Sellable { held, free }
}

/// Why a sale can't be recorded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SaleProblem {
    /// The portfolio has none of the ticker on the sale's date.
    NotHeld,
    /// It has fewer shares than the sale, namely `held`.
    TooMany { held: Decimal },
    /// It has the shares, but later sales need all but `free` of them.
    NeededLater { free: Decimal },
}

/// Checks `sale` against the other transactions of its portfolio: a sale
/// of shares the portfolio doesn't have would be ignored in its holdings
/// and still counted as cash received.
pub fn check_sale(others: &[Transaction], sale: &Transaction) -> Result<(), SaleProblem> {
    let Sellable { held, free } = sellable_shares(others, &sale.ticker, &sale.date);
    if held <= DUST {
        Err(SaleProblem::NotHeld)
    } else if sale.shares > held + DUST {
        Err(SaleProblem::TooMany { held })
    } else if sale.shares > free + DUST {
        Err(SaleProblem::NeededLater { free: free.max(Decimal::ZERO) })
    } else {
        Ok(())
    }
}

/// Sales of more shares than their portfolio held at the time, each with
/// the shares it had to sell. [`compute_positions`] counts only that part,
/// so the rest of such a sale changes nothing.
pub fn uncovered_sales(transactions: &[Transaction]) -> HashMap<uuid::Uuid, Decimal> {
    let mut held: HashMap<(uuid::Uuid, &TickerSymbol), Decimal> = HashMap::new();
    let mut uncovered = HashMap::new();
    for tx in transactions.iter().filter(|tx| !tx.is_cash()) {
        let balance = held.entry((tx.portfolio_id, &tx.ticker)).or_default();
        if tx.transaction_type == TransactionType::Sell && tx.shares > *balance + DUST {
            uncovered.insert(tx.id, *balance);
        }
        apply_shares(balance, tx);
    }
    uncovered
}

/// The effect of `tx` on the number of shares held, as [`compute_positions`]
/// counts it.
fn apply_shares(held: &mut Decimal, tx: &Transaction) {
    match tx.transaction_type {
        TransactionType::Buy => *held += tx.shares,
        TransactionType::Sell => *held -= tx.shares.min(*held),
        TransactionType::Split if tx.shares > Decimal::ZERO => *held *= tx.shares,
        _ => {}
    }
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
            currency: "USD".into(),
            fx_to_usd: Decimal::ONE,
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

    fn on(date: &str, ticker: &str, kind: TransactionType, shares: Decimal) -> Transaction {
        Transaction { id: Uuid::new_v4(), date: date.into(), ..tx(ticker, kind, shares, dec!(10), dec!(0)) }
    }

    #[test]
    fn only_shares_held_on_the_day_can_be_sold() {
        use TransactionType::*;
        let amd = TickerSymbol::new("AMD").unwrap();
        let txs = vec![
            on("2026-11-02", "AMD", Buy, dec!(0.06032)),
            on("2026-11-02", "CAT", Buy, dec!(0.02384)),
            on("2026-11-10", "AMD", Buy, dec!(1)),
        ];
        let sellable = |date| sellable_shares(&txs, &amd, date);
        assert_eq!(sellable("2026-11-01"), Sellable::default(), "not bought yet");
        assert_eq!(sellable("2026-11-04").held, dec!(0.06032), "the later buy doesn't count");
        assert_eq!(sellable("2026-11-10").free, dec!(1.06032), "the day's own buys do");
        // A mistyped ticker has nothing to sell.
        assert_eq!(sellable_shares(&txs, &TickerSymbol::new("AND").unwrap(), "2026-11-04"), Sellable::default());
    }

    #[test]
    fn shares_sold_later_are_not_free_to_sell_earlier() {
        use TransactionType::*;
        let aaa = TickerSymbol::new("AAA").unwrap();
        let txs = vec![
            on("2026-01-05", "AAA", Buy, dec!(10)),
            on("2026-02-05", "AAA", Split, dec!(2)), // 20 shares
            on("2026-03-05", "AAA", Sell, dec!(14)), // 6 left
            on("2026-04-05", "AAA", Buy, dec!(5)),
        ];
        // 7 pre-split shares became the 14 sold in March.
        assert_eq!(sellable_shares(&txs, &aaa, "2026-01-20"), Sellable { held: dec!(10), free: dec!(3) });
        assert_eq!(sellable_shares(&txs, &aaa, "2026-02-20"), Sellable { held: dec!(20), free: dec!(6) });
        assert_eq!(sellable_shares(&txs, &aaa, "2026-04-05"), Sellable { held: dec!(11), free: dec!(11) });
    }

    #[test]
    fn a_sale_is_checked_against_its_portfolio() {
        use TransactionType::*;
        let txs = vec![
            on("2026-11-02", "AMD", Buy, dec!(0.06032)),
            on("2026-11-20", "AMD", Sell, dec!(0.05)),
        ];
        let sale = |date, ticker, shares| check_sale(&txs, &on(date, ticker, Sell, shares));
        assert_eq!(sale("2026-11-04", "AND", dec!(0.03082)), Err(SaleProblem::NotHeld));
        assert_eq!(sale("2026-11-01", "AMD", dec!(0.01)), Err(SaleProblem::NotHeld));
        assert_eq!(sale("2026-11-04", "AMD", dec!(1)), Err(SaleProblem::TooMany { held: dec!(0.06032) }));
        assert_eq!(sale("2026-11-04", "AMD", dec!(0.03082)), Err(SaleProblem::NeededLater { free: dec!(0.01032) }));
        assert_eq!(sale("2026-11-04", "AMD", dec!(0.01032)), Ok(()));
        assert_eq!(sale("2026-11-21", "AMD", dec!(0.01032)), Ok(()));
    }

    #[test]
    fn sales_of_shares_not_held_are_found() {
        use TransactionType::*;
        let other = Uuid::new_v4();
        let typo = on("2026-11-04", "AND", Sell, dec!(0.03082));
        let too_many = on("2026-11-05", "AMD", Sell, dec!(1));
        let elsewhere = Transaction { portfolio_id: other, ..on("2026-11-06", "CAT", Sell, dec!(0.02)) };
        let txs = vec![
            on("2026-11-02", "AMD", Buy, dec!(0.06032)),
            on("2026-11-02", "CAT", Buy, dec!(0.02384)),
            on("2026-11-03", "CAT", Sell, dec!(0.02)),
            typo.clone(),
            too_many.clone(),
            elsewhere.clone(),
            // Rounded a hair over what's left: still a full sale.
            on("2026-11-07", "CAT", Sell, dec!(0.00389)),
        ];
        let uncovered = uncovered_sales(&txs);
        assert_eq!(uncovered.len(), 3);
        assert_eq!(uncovered[&typo.id], dec!(0));
        assert_eq!(uncovered[&too_many.id], dec!(0.06032));
        assert_eq!(uncovered[&elsewhere.id], dec!(0), "another portfolio's shares don't cover it");
    }

    #[test]
    fn foreign_trades_are_costed_in_usd_at_the_trade_rate() {
        use TransactionType::*;
        let thb = |kind, shares, price, fee, fx| Transaction {
            currency: "THB".into(),
            fx_to_usd: fx,
            ..tx("PTT.BK", kind, shares, price, fee)
        };
        let txs = vec![
            thb(Buy, dec!(100), dec!(35), dec!(20), dec!(0.03)), // $105 + $0.60
            thb(Sell, dec!(50), dec!(40), dec!(0), dec!(0.025)), // $1 × 50
        ];
        let data = GetDashBoardResponse { portfolios: vec![], transactions: txs.clone() };
        let pos = compute_positions(&data, &HashMap::new());
        assert_eq!(pos[0].avg_cost, dec!(1.056));
        // 50 × ($1.00 − $1.056)
        assert_eq!(realized_pnl(&txs), dec!(-2.8));
    }
}
