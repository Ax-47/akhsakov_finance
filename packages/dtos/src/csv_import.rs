//! Transaction import from broker CSV exports.
//!
//! Columns are matched by header name (case-insensitive), so most broker
//! exports work as-is: date, ticker/symbol, type/action/side,
//! shares/quantity, price, fee/commission. `,`, `;` and tab separators and
//! quoted fields are supported.

use crate::transaction::Transaction;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    /// One message per skipped line, e.g. `line 4: unknown ticker ""`.
    pub errors: Vec<String>,
}

const DATE: &[&str] = &["date", "trade date", "transaction date", "time", "datetime"];
const TICKER: &[&str] = &["ticker", "symbol", "instrument", "stock", "security"];
const KIND: &[&str] = &["type", "action", "side", "transaction type", "activity"];
const SHARES: &[&str] = &["shares", "quantity", "qty", "units", "amount"];
const PRICE: &[&str] = &["price", "price per share", "avg price", "execution price", "fill price"];
const FEE: &[&str] = &["fee", "fees", "commission", "commissions"];

/// Parses CSV text into transactions for `portfolio_id`; bad lines are
/// reported and skipped rather than failing the whole import.
pub fn parse_transactions_csv(text: &str, portfolio_id: Uuid) -> (Vec<Transaction>, Vec<String>) {
    let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty());
    let Some((_, header)) = lines.next() else {
        return (vec![], vec!["the file is empty".into()]);
    };
    let sep = [',', ';', '\t']
        .into_iter()
        .max_by_key(|c| header.matches(*c).count())
        .unwrap_or(',');
    let headers: Vec<String> = split(header, sep).iter().map(|h| h.trim().to_lowercase()).collect();
    let col = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));

    let (Some(date_i), Some(ticker_i), Some(shares_i), Some(price_i)) =
        (col(DATE), col(TICKER), col(SHARES), col(PRICE))
    else {
        return (vec![], vec![format!(
            "couldn't find the columns — need date, ticker/symbol, shares/quantity and price (found: {})",
            headers.join(", ")
        )]);
    };
    let (kind_i, fee_i) = (col(KIND), col(FEE));

    let mut txs = Vec::new();
    let mut errors = Vec::new();
    for (n, line) in lines {
        let cells = split(line, sep);
        let cell = |i: usize| cells.get(i).map(|s| s.trim()).unwrap_or("");
        let row = || -> Result<Transaction, String> {
            let ticker = TickerSymbol::new(cell(ticker_i)).map_err(|_| format!("bad ticker \"{}\"", cell(ticker_i)))?;
            let date = parse_date(cell(date_i)).ok_or_else(|| format!("bad date \"{}\"", cell(date_i)))?;
            let raw_shares = parse_number(cell(shares_i)).ok_or_else(|| format!("bad quantity \"{}\"", cell(shares_i)))?;
            let price = parse_number(cell(price_i)).ok_or_else(|| format!("bad price \"{}\"", cell(price_i)))?.abs();
            let fee = fee_i.and_then(|i| parse_number(cell(i))).unwrap_or_default().abs();
            let kind = match kind_i.map(cell).filter(|k| !k.is_empty()) {
                Some(k) => parse_kind(k).ok_or_else(|| format!("unknown type \"{k}\""))?,
                // No type column: negative quantity means a sale.
                None if raw_shares < Decimal::ZERO => TransactionType::Sell,
                None => TransactionType::Buy,
            };
            Ok(Transaction {
                id: Uuid::new_v4(),
                portfolio_id,
                ticker,
                transaction_type: kind,
                shares: raw_shares.abs(),
                price,
                date,
                fee,
            })
        };
        match row() {
            Ok(tx) => txs.push(tx),
            Err(e) => errors.push(format!("line {}: {e}", n + 1)),
        }
    }
    (txs, errors)
}

/// Splits one CSV line, honouring double-quoted fields.
fn split(line: &str, sep: char) -> Vec<String> {
    let mut cells = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                cells.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => cells.push(String::new()),
            c => cells.last_mut().unwrap().push(c),
        }
    }
    cells
}

/// `1,234.50`, `$12`, `(3.5)` (negative) → Decimal.
fn parse_number(s: &str) -> Option<Decimal> {
    let negative = s.starts_with('(') && s.ends_with(')');
    let cleaned: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    let n = Decimal::from_str(&cleaned).ok()?;
    Some(if negative { -n.abs() } else { n })
}

/// `2026-05-20`, `2026/05/20`, `2026-05-20T14:30:00Z`, or US `5/20/2026`.
fn parse_date(s: &str) -> Option<String> {
    let s = s.trim();
    let iso = s.get(..10).unwrap_or(s).replace('/', "-");
    let parts: Vec<&str> = iso.split('-').collect();
    let (y, m, d) = match parts.as_slice() {
        [y, m, d] if y.len() == 4 => (y.parse::<u32>().ok()?, m.parse::<u32>().ok()?, d.parse::<u32>().ok()?),
        _ => {
            let us: Vec<&str> = s.split('/').collect();
            match us.as_slice() {
                [m, d, y] if y.len() == 4 => (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?),
                _ => return None,
            }
        }
    };
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| format!("{y:04}-{m:02}-{d:02}"))
}

fn parse_kind(s: &str) -> Option<TransactionType> {
    let lower = s.to_lowercase();
    let word = match lower.as_str() {
        "b" | "bought" | "purchase" => "buy",
        "s" | "sold" => "sell",
        "div" | "dividends" | "cash dividend" => "dividend",
        other => other,
    };
    TransactionType::from_str(word).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_common_broker_formats() {
        let csv = "Trade Date;Symbol;Action;Quantity;Price;Commission\n\
                   2026-05-20;nvda;Buy;1.5;\"$1,215.50\";1.00\n\
                   5/21/2026;AMD;SOLD;2;100;0\n\
                   2026-05-22;;Buy;1;1;0\n\
                   bad;AAPL;Buy;1;1;0\n";
        let (txs, errors) = parse_transactions_csv(csv, Uuid::nil());
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[0].ticker.as_str(), "NVDA");
        assert_eq!(txs[0].price, dec!(1215.50));
        assert_eq!(txs[0].fee, dec!(1.00));
        assert_eq!(txs[1].transaction_type, TransactionType::Sell);
        assert_eq!(txs[1].date, "2026-05-21");
        assert_eq!(errors.len(), 2);
        assert!(errors[0].starts_with("line 4: bad ticker"));
        assert!(errors[1].starts_with("line 5: bad date"));
    }

    #[test]
    fn negative_quantity_without_type_is_a_sale() {
        let (txs, errors) = parse_transactions_csv("date,ticker,shares,price\n2026-01-02,MSFT,-3,400\n", Uuid::nil());
        assert!(errors.is_empty());
        assert_eq!(txs[0].transaction_type, TransactionType::Sell);
        assert_eq!(txs[0].shares, dec!(3));
    }

    #[test]
    fn reports_missing_columns() {
        let (txs, errors) = parse_transactions_csv("foo,bar\n1,2\n", Uuid::nil());
        assert!(txs.is_empty());
        assert!(errors[0].contains("couldn't find the columns"));
    }
}
