//! Transaction import from broker CSV exports.
//!
//! Columns are matched by header name (case-insensitive), so most broker
//! exports work as-is: date, ticker/symbol, type/action/side,
//! shares/quantity, price, fee/commission, currency and fx rate. `,`, `;`
//! and tab separators and quoted fields are supported. Rows in another
//! currency without a rate get `fx_to_usd = 0`, which the server fills in
//! from the trade date's exchange rate.
//!
//! [`Broker`] presets read the exports of Settrade Streaming, Dime!,
//! Webull and Interactive Brokers without editing: their column names,
//! Thai dates (day first, Buddhist-era years), `.BK` symbols, unfilled
//! orders and IBKR's multi-section statements are handled.

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

// Header names, compared after [`normalize_header`]. Earlier names win
// when a file has several (e.g. Webull's "avg price" before "price").
const DATE: &[&str] = &[
    "date", "trade date", "transaction date", "time", "datetime", "date/time", "tradedate",
    "filled time", "execution time", "settlement date", "วันที่", "วันที่ซื้อขาย", "วันที่ทำรายการ",
];
const TICKER: &[&str] = &[
    "ticker", "symbol", "instrument", "stock", "security", "stock name", "หลักทรัพย์",
    "ชื่อย่อหลักทรัพย์", "ชื่อหุ้น", "หุ้น",
];
const KIND: &[&str] = &[
    "type", "action", "side", "transaction type", "activity", "buy/sell", "order side",
    "ประเภท", "ประเภทรายการ", "ซื้อ/ขาย", "ฝั่ง",
];
const SHARES: &[&str] = &[
    "shares", "quantity", "qty", "units", "unit", "filled", "filled qty", "volume",
    "executed volume", "matched volume", "amount", "จำนวน", "จำนวนหุ้น", "หน่วย",
];
const PRICE: &[&str] = &[
    "avg price", "average price", "price", "price per share", "execution price", "fill price",
    "t. price", "tradeprice", "matched price", "executed price", "ราคา", "ราคาต่อหุ้น", "ราคาที่ได้",
];
const FEE: &[&str] = &[
    "fee", "fees", "commission", "commissions", "comm/fee", "ibcommission", "comm+vat",
    "commission+vat", "total fee", "ค่าธรรมเนียม", "ค่าคอมมิชชั่น", "ค่าธรรมเนียม+vat",
];
const CURRENCY: &[&str] = &["currency", "ccy", "trade currency", "currencyprimary", "สกุลเงิน"];
const FX: &[&str] = &["fx_to_usd", "fx rate", "exchange rate", "fx", "fxratetobase"];
/// Order state (Webull); rows that aren't filled are skipped.
const STATUS: &[&str] = &["status", "order status", "สถานะ"];

/// Where an export came from. `Auto` reads any file with recognisable
/// column names (and spots IBKR statements by their layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Broker {
    #[default]
    Auto,
    /// Settrade Streaming (most Thai brokers): SET stocks in THB.
    Streaming,
    /// Dime! (KKP): US stocks in USD, dates day first.
    Dime,
    /// Webull: order history; only filled orders count.
    Webull,
    /// Interactive Brokers activity statement or Flex query.
    Ibkr,
}

impl Broker {
    pub const ALL: [Broker; 5] = [Self::Auto, Self::Streaming, Self::Dime, Self::Webull, Self::Ibkr];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Any broker (detect columns)",
            Self::Streaming => "Streaming (Settrade)",
            Self::Dime => "Dime!",
            Self::Webull => "Webull",
            Self::Ibkr => "Interactive Brokers",
        }
    }

    /// Short explanation of what to export, shown under the picker.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Auto => "Needs columns for date, ticker/symbol, quantity and price; type/action and fee are optional.",
            Self::Streaming => "Streaming → Portfolio → Trade history → Export. Symbols get .BK and THB; Thai dates (also Buddhist-era years) work.",
            Self::Dime => "Dime! → Account → Statement → Export CSV. Dates are read day first (DD/MM/YYYY).",
            Self::Webull => "Webull → Orders → Order history → Export. Only filled orders are imported.",
            Self::Ibkr => "IBKR → Reports → Activity statement (CSV) or a Flex query with trades. Only the Trades section is read.",
        }
    }

    fn day_first(self) -> bool {
        matches!(self, Self::Streaming | Self::Dime)
    }

    /// (currency, symbol suffix) assumed when the file doesn't say.
    fn market(self) -> (&'static str, &'static str) {
        match self {
            Self::Streaming => ("THB", ".BK"),
            _ => ("USD", ""),
        }
    }
}

/// Lowercase, trimmed, without a byte-order mark, quotes, or a trailing
/// unit such as `(USD)`.
fn normalize_header(h: &str) -> String {
    let h = h.trim().trim_start_matches('\u{feff}').trim_matches('"').trim().to_lowercase();
    let h = match h.find(" (") {
        Some(i) if h.ends_with(')') => h[..i].to_string(),
        _ => h,
    };
    h.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// IBKR activity statements hold many sections (`Trades,Header,…`,
/// `Trades,Data,Order,…`); keeps the trade orders as a plain table.
fn ibkr_trades(text: &str) -> Option<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let cells = split(line, ',');
        match cells.iter().map(|c| c.trim()).collect::<Vec<_>>().as_slice() {
            ["Trades", "Header", rest @ ..] if out.is_empty() => out.push(rest.join(",")),
            ["Trades", "Data", "Order", ..] if !out.is_empty() => {
                out.push(cells[2..].iter().map(|c| quote(c)).collect::<Vec<_>>().join(","))
            }
            _ => {}
        }
    }
    (!out.is_empty()).then(|| out.join("\n"))
}

fn quote(cell: &str) -> String {
    if cell.contains([',', '"']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// Parses CSV text into transactions for `portfolio_id`; bad lines are
/// reported and skipped rather than failing the whole import.
pub fn parse_transactions_csv(text: &str, portfolio_id: Uuid) -> (Vec<Transaction>, Vec<String>) {
    parse_broker_csv(text, portfolio_id, Broker::Auto)
}

/// [`parse_transactions_csv`] for an export of `broker`.
pub fn parse_broker_csv(text: &str, portfolio_id: Uuid, broker: Broker) -> (Vec<Transaction>, Vec<String>) {
    let looks_ibkr = text.lines().any(|l| l.starts_with("Trades,Header,"));
    let (text, broker) = match (broker, looks_ibkr) {
        (Broker::Auto | Broker::Ibkr, true) => match ibkr_trades(text) {
            Some(t) => (t, Broker::Ibkr),
            None => return (vec![], vec!["no trades found in the statement".into()]),
        },
        _ => (text.to_string(), broker),
    };
    let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty());
    let Some((_, header)) = lines.next() else {
        return (vec![], vec!["the file is empty".into()]);
    };
    let sep = [',', ';', '\t']
        .into_iter()
        .max_by_key(|c| header.matches(*c).count())
        .unwrap_or(',');
    let headers: Vec<String> = split(header, sep).iter().map(|h| normalize_header(h)).collect();
    let col = |names: &[&str]| {
        names.iter().find_map(|n| headers.iter().position(|h| h == n))
    };
    let (default_currency, suffix) = broker.market();

    let (Some(date_i), Some(ticker_i), Some(shares_i), Some(price_i)) =
        (col(DATE), col(TICKER), col(SHARES), col(PRICE))
    else {
        return (vec![], vec![format!(
            "couldn't find the columns — need date, ticker/symbol, shares/quantity and price (found: {})",
            headers.join(", ")
        )]);
    };
    let (kind_i, fee_i, currency_i, fx_i) = (col(KIND), col(FEE), col(CURRENCY), col(FX));
    let status_i = col(STATUS);

    let mut txs = Vec::new();
    let mut errors = Vec::new();
    for (n, line) in lines {
        let cells = split(line, sep);
        let cell = |i: usize| cells.get(i).map(|s| s.trim()).unwrap_or("");
        // Cancelled or pending orders (Webull) aren't trades.
        if let Some(i) = status_i {
            let status = cell(i).to_lowercase();
            if !status.is_empty() && !["filled", "executed", "matched", "completed", "done", "สำเร็จ", "จับคู่แล้ว"].contains(&status.as_str()) {
                continue;
            }
        }
        let row = || -> Result<Transaction, String> {
            let raw_ticker = cell(ticker_i).to_uppercase();
            let raw_ticker = if !suffix.is_empty() && !raw_ticker.is_empty() && !raw_ticker.contains('.') {
                format!("{raw_ticker}{suffix}")
            } else {
                raw_ticker
            };
            let ticker = TickerSymbol::new(&raw_ticker).map_err(|_| format!("bad ticker \"{}\"", cell(ticker_i)))?;
            let date = parse_date(cell(date_i), broker.day_first())
                .ok_or_else(|| format!("bad date \"{}\"", cell(date_i)))?;
            let raw_shares = parse_number(cell(shares_i)).ok_or_else(|| format!("bad quantity \"{}\"", cell(shares_i)))?;
            let price = parse_number(cell(price_i)).ok_or_else(|| format!("bad price \"{}\"", cell(price_i)))?.abs();
            let fee = fee_i.and_then(|i| parse_number(cell(i))).unwrap_or_default().abs();
            let kind = match kind_i.map(cell).filter(|k| !k.is_empty()) {
                Some(k) => parse_kind(k).ok_or_else(|| format!("unknown type \"{k}\""))?,
                // No type column: negative quantity means a sale.
                None if raw_shares < Decimal::ZERO => TransactionType::Sell,
                None => TransactionType::Buy,
            };
            let currency = currency_i
                .map(|i| cell(i).to_uppercase())
                .filter(|c| c.len() == 3 && c.chars().all(|c| c.is_ascii_alphabetic()))
                .unwrap_or_else(|| default_currency.into());
            let fx_to_usd = if currency == "USD" {
                Decimal::ONE
            } else {
                fx_i.and_then(|i| parse_number(cell(i))).filter(|r| *r > Decimal::ZERO).unwrap_or_default()
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
                currency,
                fx_to_usd,
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

/// `2026-05-20`, `2026/05/20`, `2026-05-20T14:30:00Z`, `20260520`, US
/// `5/20/2026` or, with `day_first`, `20/05/2026`. Buddhist-era years
/// (`2569`) become Common Era.
fn parse_date(s: &str, day_first: bool) -> Option<String> {
    let s = s.trim();
    let first = s.split([' ', 'T', ',']).next().unwrap_or(s);
    let digits = |p: &str| p.parse::<u32>().ok();
    let (y, m, d) = if first.len() == 8 && first.chars().all(|c| c.is_ascii_digit()) {
        (digits(&first[..4])?, digits(&first[4..6])?, digits(&first[6..])?)
    } else {
        let parts: Vec<&str> = first.split(['-', '/', '.']).collect();
        match parts.as_slice() {
            [y, m, d] if y.len() == 4 => (digits(y)?, digits(m)?, digits(d)?),
            [a, b, y] if y.len() == 4 && day_first => (digits(y)?, digits(b)?, digits(a)?),
            [a, b, y] if y.len() == 4 => (digits(y)?, digits(a)?, digits(b)?),
            _ => return None,
        }
    };
    let y = if y > 2400 { y - 543 } else { y };
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| format!("{y:04}-{m:02}-{d:02}"))
}

fn parse_kind(s: &str) -> Option<TransactionType> {
    let lower = s.to_lowercase();
    let word = match lower.as_str() {
        "b" | "bot" | "bought" | "purchase" | "market buy" | "limit buy" | "ซื้อ" => "buy",
        "s" | "sld" | "sold" | "market sell" | "limit sell" | "ขาย" => "sell",
        "div" | "dividends" | "cash dividend" | "ปันผล" | "เงินปันผล" => "dividend",
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

    #[test]
    fn dates() {
        assert_eq!(parse_date("20/05/2026", true).as_deref(), Some("2026-05-20"));
        assert_eq!(parse_date("20/05/2569", true).as_deref(), Some("2026-05-20"), "Buddhist era");
        assert_eq!(parse_date("05/20/2026 09:31:02 EDT", false).as_deref(), Some("2026-05-20"));
        assert_eq!(parse_date("20260520", false).as_deref(), Some("2026-05-20"));
        assert_eq!(parse_date("2026-05-20, 10:30:00", false).as_deref(), Some("2026-05-20"));
        assert_eq!(parse_date("20/05/2026", false), None, "no month 20");
    }

    #[test]
    fn streaming_export() {
        let csv = "\u{feff}วันที่,หลักทรัพย์,ซื้อ/ขาย,จำนวน,ราคา,ค่าธรรมเนียม+VAT\n\
                   15/01/2569,PTT,B,1000,33.50,56.12\n\
                   20/02/2569,ptt,ขาย,500,35,30\n";
        let (txs, errors) = parse_broker_csv(csv, Uuid::nil(), Broker::Streaming);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(txs[0].ticker.as_str(), "PTT.BK");
        assert_eq!((txs[0].date.as_str(), txs[0].currency.as_str()), ("2026-01-15", "THB"));
        assert_eq!(txs[0].fx_to_usd, Decimal::ZERO, "the server fills in the rate");
        assert_eq!(txs[1].transaction_type, TransactionType::Sell);
    }

    #[test]
    fn dime_export() {
        let csv = "Date,Symbol,Side,Unit,Price (USD),Commission (USD)\n03/02/2026,AAPL,Buy,0.5,230.10,0.15\n";
        let (txs, errors) = parse_broker_csv(csv, Uuid::nil(), Broker::Dime);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!((txs[0].date.as_str(), txs[0].price, txs[0].fee), ("2026-02-03", dec!(230.10), dec!(0.15)));
    }

    #[test]
    fn webull_skips_unfilled_orders() {
        let csv = "Name,Symbol,Side,Status,Filled,Total Qty,Price,Avg Price,Time-in-Force,Placed Time,Filled Time\n\
                   NVIDIA,NVDA,Buy,Filled,3,3,@120,120.55,DAY,05/20/2026 09:30:00 EDT,05/20/2026 09:31:02 EDT\n\
                   NVIDIA,NVDA,Sell,Cancelled,0,3,@150,,GTC,05/21/2026 09:30:00 EDT,\n";
        let (txs, errors) = parse_broker_csv(csv, Uuid::nil(), Broker::Webull);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(txs.len(), 1);
        assert_eq!((txs[0].shares, txs[0].price, txs[0].date.as_str()), (dec!(3), dec!(120.55), "2026-05-20"));
    }

    #[test]
    fn ibkr_statement_is_detected() {
        let csv = "Statement,Header,Field Name,Field Value\n\
                   Statement,Data,Title,Activity Statement\n\
                   Trades,Header,DataDiscriminator,Asset Category,Currency,Symbol,Date/Time,Quantity,T. Price,C. Price,Proceeds,Comm/Fee,Basis,Realized P/L,MTM P/L,Code\n\
                   Trades,Data,Order,Stocks,USD,AAPL,\"2026-05-20, 10:30:00\",10,150,151,-1500,-1,1501,0,10,O\n\
                   Trades,Data,Order,Stocks,USD,AAPL,\"2026-06-02, 11:00:00\",-4,160,160,640,-1,-600.4,38.6,0,C\n\
                   Trades,SubTotal,,Stocks,USD,AAPL,,6,,,,-2,,,,\n";
        let (txs, errors) = parse_transactions_csv(csv, Uuid::nil());
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(txs.len(), 2);
        assert_eq!((txs[0].transaction_type.clone(), txs[0].fee), (TransactionType::Buy, dec!(1)));
        assert_eq!((txs[1].transaction_type.clone(), txs[1].shares), (TransactionType::Sell, dec!(4)));

        let flex = "TradeDate,Symbol,Buy/Sell,Quantity,TradePrice,IBCommission,CurrencyPrimary\n20260520,VOD,BUY,100,0.72,-3,GBP\n";
        let (txs, errors) = parse_broker_csv(flex, Uuid::nil(), Broker::Ibkr);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!((txs[0].currency.as_str(), txs[0].fee, txs[0].date.as_str()), ("GBP", dec!(3), "2026-05-20"));
    }
}
