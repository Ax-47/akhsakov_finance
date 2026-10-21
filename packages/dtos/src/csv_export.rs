//! CSV export of transactions, holdings and backtests. The transactions
//! format is the one `csv_import` reads, so an export can be imported again.

use crate::{insights::BacktestResult, position::Position, transaction::Transaction};
use std::collections::BTreeSet;

/// `date,symbol,type,quantity,price,fee,currency,fx_to_usd,portfolio`,
/// oldest first. Price and fee are in the trade's currency.
/// `portfolio_name` maps a transaction to its portfolio's name.
pub fn transactions_csv(
    transactions: &[Transaction],
    portfolio_name: impl Fn(&Transaction) -> String,
) -> String {
    let mut rows: Vec<&Transaction> = transactions.iter().collect();
    rows.sort_by(|a, b| a.date.cmp(&b.date));
    let mut out = String::from("date,symbol,type,quantity,price,fee,currency,fx_to_usd,portfolio\n");
    for t in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            t.date,
            t.ticker,
            t.transaction_type,
            t.shares.normalize(),
            t.price.normalize(),
            t.fee.normalize(),
            t.currency,
            t.fx_to_usd.normalize(),
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

/// One row per month: `month`, then `<name> invested (USD)` and
/// `<name> value (USD)` for each named result. A result with no point that
/// month (it started later) leaves its cells empty.
pub fn backtest_csv(results: &[(String, BacktestResult)]) -> String {
    let mut out = String::from("month");
    for (name, _) in results {
        out.push_str(&format!(",{},{}", quote(&format!("{name} invested (USD)")), quote(&format!("{name} value (USD)"))));
    }
    out.push('\n');
    let months: BTreeSet<&str> = results.iter().flat_map(|(_, r)| r.points.iter().map(|p| p.0.as_str())).collect();
    for month in months {
        out.push_str(month.get(..7).unwrap_or(month));
        for (_, r) in results {
            match r.points.iter().find(|p| p.0 == month) {
                Some((_, value, invested)) => out.push_str(&format!(",{invested:.2},{value:.2}")),
                None => out.push_str(",,"),
            }
        }
        out.push('\n');
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

    #[test]
    fn backtest_rows_by_month() {
        let result = |points: &[(&str, f64, f64)]| BacktestResult {
            points: points.iter().map(|(d, v, i)| (d.to_string(), *v, *i)).collect(),
            ..Default::default()
        };
        let csv = backtest_csv(&[
            ("Mix 1".into(), result(&[("2026-01-01", 100.0, 100.0), ("2026-02-01", 212.346, 200.0)])),
            ("Nasdaq, 100".into(), result(&[("2026-02-01", 101.0, 100.0)])),
        ]);
        assert_eq!(
            csv,
            "month,Mix 1 invested (USD),Mix 1 value (USD),\"Nasdaq, 100 invested (USD)\",\"Nasdaq, 100 value (USD)\"\n\
             2026-01,100.00,100.00,,\n\
             2026-02,200.00,212.35,100.00,101.00\n"
        );
    }
    use crate::csv_import::parse_transactions_csv;
    use rust_decimal::Decimal;
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
                currency: "USD".into(),
                fx_to_usd: Decimal::ONE,
            },
            Transaction {
                id: Uuid::new_v4(),
                portfolio_id: portfolio,
                ticker: TickerSymbol::new("PTT.BK").unwrap(),
                transaction_type: TransactionType::Sell,
                shares: dec!(100),
                price: dec!(34.25),
                date: "2026-06-01".into(),
                fee: dec!(0),
                currency: "THB".into(),
                fx_to_usd: dec!(0.0305),
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
                t.currency.clone(),
                t.fx_to_usd,
            )
        };
        assert_eq!(
            parsed.iter().map(strip).collect::<Vec<_>>(),
            txs.iter().map(strip).collect::<Vec<_>>()
        );
    }
}
