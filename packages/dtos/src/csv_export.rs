//! CSV export of transactions and holdings. The transactions format is the
//! one `csv_import` reads, so an export can be imported again.

use crate::{position::Position, transaction::Transaction};

/// `date,symbol,type,quantity,price,fee,portfolio`, oldest first.
/// `portfolio_name` maps a transaction to its portfolio's name.
pub fn transactions_csv(
    transactions: &[Transaction],
    portfolio_name: impl Fn(&Transaction) -> String,
) -> String {
    let mut rows: Vec<&Transaction> = transactions.iter().collect();
    rows.sort_by(|a, b| a.date.cmp(&b.date));
    let mut out = String::from("date,symbol,type,quantity,price,fee,portfolio\n");
    for t in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            t.date,
            t.ticker,
            t.transaction_type,
            t.shares.normalize(),
            t.price.normalize(),
            t.fee.normalize(),
            quote(&portfolio_name(t)),
        ));
    }
    out
}

/// `symbol,shares,avg_cost,price,market_value,unrealized_gain,unrealized_pct`.
pub fn holdings_csv(positions: &[Position]) -> String {
    let mut out =
        String::from("symbol,shares,avg_cost,price,market_value,unrealized_gain,unrealized_pct\n");
    for p in positions {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            p.ticker,
            p.shares.normalize(),
            p.avg_cost.round_dp(4).normalize(),
            p.current_price.normalize(),
            p.market_value().round_dp(2),
            p.unrealized_pnl().round_dp(2),
            p.unrealized_pnl_pct().round_dp(2),
        ));
    }
    out
}

/// Quotes a field if it contains a comma, quote or newline.
fn quote(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_import::parse_transactions_csv;
    use rust_decimal_macros::dec;
    use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
    use uuid::Uuid;

    #[test]
    fn export_then_import_round_trips() {
        let portfolio = Uuid::new_v4();
        let txs = vec![
            Transaction {
                id: Uuid::new_v4(),
                portfolio_id: portfolio,
                ticker: TickerSymbol::new("NVDA").unwrap(),
                transaction_type: TransactionType::Buy,
                shares: dec!(1.5),
                price: dec!(215.87),
                date: "2026-05-20".into(),
                fee: dec!(1),
            },
            Transaction {
                id: Uuid::new_v4(),
                portfolio_id: portfolio,
                ticker: TickerSymbol::new("NVDA").unwrap(),
                transaction_type: TransactionType::Sell,
                shares: dec!(0.5),
                price: dec!(250),
                date: "2026-06-01".into(),
                fee: dec!(0),
            },
        ];
        let csv = transactions_csv(&txs, |_| "Growth, long term".into());
        assert!(
            csv.contains("\"Growth, long term\""),
            "names with commas are quoted"
        );
        let (parsed, errors) = parse_transactions_csv(&csv, portfolio);
        assert!(errors.is_empty(), "{errors:?}");
        let strip = |t: &Transaction| {
            (
                t.date.clone(),
                t.ticker.clone(),
                t.transaction_type.clone(),
                t.shares,
                t.price,
                t.fee,
            )
        };
        assert_eq!(
            parsed.iter().map(strip).collect::<Vec<_>>(),
            txs.iter().map(strip).collect::<Vec<_>>()
        );
    }
}
