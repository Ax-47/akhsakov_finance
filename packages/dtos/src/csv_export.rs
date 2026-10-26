//! CSV export of transactions, holdings, backtests and a stock's financial
//! statements. The transactions format is the one `csv_import` reads, so an
//! export can be imported again.

use crate::{
    fundamentals::PeriodFinancials, insights::BacktestResult, position::Position,
    statements::Statement, transaction::Transaction,
};
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

/// `statement,line`, then one column per period (its end date), newest
/// first. Amounts are whole units of the statements' currency (USD); a line
/// the period didn't report is left empty.
pub fn statements_csv(periods: &[PeriodFinancials]) -> String {
    let mut out = String::from("statement,line");
    for p in periods {
        out.push(',');
        out.push_str(&quote(p.end_date.as_deref().unwrap_or(&p.period)));
    }
    out.push('\n');
    for statement in Statement::ALL {
        for line in statement.lines() {
            out.push_str(&format!("{},{}", quote(statement.title()), quote(line.label)));
            for p in periods {
                out.push(',');
                if let Some(v) = (line.get)(p) {
                    out.push_str(&format!("{v:.0}"));
                }
            }
            out.push('\n');
        }
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
    fn statements_one_row_per_line_and_column_per_period() {
        let periods = [
            PeriodFinancials { period: "2026Q2".into(), end_date: Some("2026-06-30".into()), revenue: Some(1.5e9), ..Default::default() },
            PeriodFinancials { period: "2026Q1".into(), revenue: Some(1.25e9), ..Default::default() },
        ];
        let csv = statements_csv(&periods);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "statement,line,2026-06-30,2026Q1");
        assert_eq!(lines[1], "Income statement,Revenue,1500000000,1250000000");
        assert!(lines.contains(&"Balance sheet,\"Property, plant & equipment\",,"));
        assert_eq!(lines.len(), 1 + crate::statements::LINES.len());
    }

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
