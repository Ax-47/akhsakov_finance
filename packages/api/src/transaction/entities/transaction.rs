use serde::{Deserialize, Serialize};
use types::{
    cash_flow::CashFlow, money::Money, quantity::Quantity, ticker_symbol::TickerSymbol,
    trade_date::TradeDate, transaction_type::TransactionType,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub ticker: TickerSymbol,
    pub kind: TransactionType,
    pub date: TradeDate,
    pub quantity: Quantity, // shares / units
    pub price: Money,       // per share in portfolio currency
    pub note: String,
}

impl Transaction {
    pub fn new(
        ticker: TickerSymbol,
        kind: TransactionType,
        date: TradeDate,
        quantity: Quantity,
        price: Money,
        note: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            ticker,
            kind,
            date,
            quantity,
            price,
            note: note.into(),
        }
    }
    pub fn cash_flow(&self) -> CashFlow {
        match self.kind {
            TransactionType::Buy => CashFlow::Out(self.quantity * self.price),
            TransactionType::Sell => CashFlow::In(self.quantity * self.price),
            TransactionType::Split => CashFlow::In(Money::zero(self.price.currency())),
            TransactionType::Transfer => CashFlow::In(Money::zero(self.price.currency())),
            _ => CashFlow::In(Money::zero(self.price.currency())), //TODO:
        }
    }

    pub fn cost_basis(&self) -> Money {
        self.quantity * self.price
    }
}

// // ─── CSV Row — Generic format ─────────────────────────────────────────────────
// //
// // Expected columns: ticker, type, date, quantity, price, [fee], [note], [broker]
//
// #[derive(Debug, serde::Deserialize)]
// pub struct CsvRow {
//     pub ticker: String,
//     #[serde(rename = "type")]
//     pub kind: String,
//     pub date: String,
//     pub quantity: f64,
//     pub price: f64,
//     #[serde(default)]
//     pub fee: f64,
//     #[serde(default)]
//     pub note: String,
//     #[serde(default)]
//     pub broker: String,
// }
//
// impl CsvRow {
//     pub fn into_transaction(self) -> Result<Transaction, DomainError> {
//         let ticker = Ticker::new(&self.ticker)?;
//         let kind: TransactionKind = self.kind.parse()?;
//         let date = parse_date_flexible(&self.date)
//             .map_err(|_| DomainError::InvalidTicker(format!("bad date: {}", self.date)))?;
//
//         let mut tx = Transaction::new(ticker, kind, date, self.quantity, self.price, self.note)?;
//         if !self.broker.is_empty() {
//             tx = tx.with_broker(self.broker);
//         }
//         Ok(tx)
//     }
// }
//
// // ─── CSV Row — Brokerage export format ───────────────────────────────────────
// //
// // Matches the export format:
// //   Symbol, Current Price, Date, Time, Change, Open, High, Low, Volume,
// //   Trade Date, Purchase Price, Quantity, Commission, High Limit, Low Limit,
// //   Comment, Transaction Type
// //
// // Live-market columns (Current Price, Change, Open, High, Low, Volume) are
// // intentionally ignored — they reflect the price at export time, not trade time.
//
// #[derive(Debug, serde::Deserialize)]
// pub struct BrokerageCsvRow {
//     #[serde(rename = "Symbol")]
//     pub symbol: String,
//
//     // live-data columns — present in export but not stored
//     #[serde(rename = "Current Price")]
//     pub _current_price: Option<f64>,
//     #[serde(rename = "Date")]
//     pub _date: Option<String>,
//     #[serde(rename = "Time")]
//     pub _time: Option<String>,
//     #[serde(rename = "Change")]
//     pub _change: Option<f64>,
//     #[serde(rename = "Open")]
//     pub _open: Option<f64>,
//     #[serde(rename = "High")]
//     pub _high: Option<f64>,
//     #[serde(rename = "Low")]
//     pub _low: Option<f64>,
//     #[serde(rename = "Volume")]
//     pub _volume: Option<u64>,
//
//     // trade columns
//     #[serde(rename = "Trade Date")]
//     pub trade_date: String, // YYYYMMDD  e.g. "20260601"
//
//     #[serde(rename = "Purchase Price")]
//     pub purchase_price: f64,
//
//     #[serde(rename = "Quantity")]
//     pub quantity: f64,
//
//     #[serde(rename = "Commission", default)]
//     pub commission: f64,
//
//     // optional columns
//     #[serde(rename = "High Limit", default)]
//     pub _high_limit: Option<f64>,
//     #[serde(rename = "Low Limit", default)]
//     pub _low_limit: Option<f64>,
//     #[serde(rename = "Comment", default)]
//     pub comment: String,
//
//     #[serde(rename = "Transaction Type")]
//     pub transaction_type: String, // "BUY" | "SELL" | "DIVIDEND" …
// }
//
// impl BrokerageCsvRow {
//     pub fn into_transaction(self) -> Result<Transaction, DomainError> {
//         let ticker = Ticker::new(&self.symbol)?;
//         let kind: TransactionKind = self.transaction_type.parse()?;
//
//         // Trade Date is YYYYMMDD with no separators
//         let date = NaiveDate::parse_from_str(&self.trade_date, "%Y%m%d").map_err(|_| {
//             DomainError::InvalidTicker(format!("bad Trade Date: {}", self.trade_date))
//         })?;
//
//         let tx = Transaction::new(
//             ticker,
//             kind,
//             date,
//             self.quantity,
//             self.purchase_price,
//             self.commission,
//             self.comment,
//         )?;
//
//         Ok(tx)
//     }
// }
//
// // ─── CsvParser — detects format automatically ─────────────────────────────────
//
// pub struct CsvParser;
//
// impl CsvParser {
//     /// Parse CSV content, auto-detecting brokerage vs generic format from headers.
//     pub fn parse(content: &str) -> Result<Vec<Transaction>, String> {
//         let header = content.lines().next().unwrap_or("").to_lowercase();
//
//         if header.contains("trade date") && header.contains("purchase price") {
//             Self::parse_brokerage(content)
//         } else {
//             Self::parse_generic(content)
//         }
//     }
//
//     fn parse_brokerage(content: &str) -> Result<Vec<Transaction>, String> {
//         let mut rdr = csv::ReaderBuilder::new()
//             .trim(csv::Trim::All)
//             .from_reader(content.as_bytes());
//
//         let mut out = vec![];
//         for (i, result) in rdr.deserialize::<BrokerageCsvRow>().enumerate() {
//             let row = result.map_err(|e| format!("Row {}: {e}", i + 2))?;
//             let tx = row
//                 .into_transaction()
//                 .map_err(|e| format!("Row {}: {e}", i + 2))?;
//             out.push(tx);
//         }
//         Ok(out)
//     }
//
//     fn parse_generic(content: &str) -> Result<Vec<Transaction>, String> {
//         let mut rdr = csv::ReaderBuilder::new()
//             .trim(csv::Trim::All)
//             .from_reader(content.as_bytes());
//
//         let mut out = vec![];
//         for (i, result) in rdr.deserialize::<CsvRow>().enumerate() {
//             let row = result.map_err(|e| format!("Row {}: {e}", i + 2))?;
//             let tx = row
//                 .into_transaction()
//                 .map_err(|e| format!("Row {}: {e}", i + 2))?;
//             out.push(tx);
//         }
//         Ok(out)
//     }
// }
//
// // ─── Helpers ──────────────────────────────────────────────────────────────────
//
// fn parse_date_flexible(s: &str) -> Result<NaiveDate, ()> {
//     NaiveDate::parse_from_str(s, "%Y-%m-%d")
//         .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
//         .or_else(|_| NaiveDate::parse_from_str(s, "%m/%d/%Y"))
//         .or_else(|_| NaiveDate::parse_from_str(s, "%d/%m/%Y"))
//         .or_else(|_| NaiveDate::parse_from_str(s, "%Y%m%d"))
//         .map_err(|_| ())
// }
