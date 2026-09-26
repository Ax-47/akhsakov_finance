//! Ports for the monthly report.

use async_trait::async_trait;
use dtos::Transaction;
use rust_decimal::Decimal;
use types::{range::Range, ticker_symbol::TickerSymbol};

/// Transactions, price history and how to show money.
#[async_trait]
pub trait ReportData: Send + Sync {
    fn transactions(&self) -> Result<Vec<Transaction>, String>;
    /// `(YYYY-MM-DD, USD close)`, oldest first.
    async fn closes(&self, ticker: &TickerSymbol, range: Range) -> Result<Vec<(String, Decimal)>, String>;
    /// Display currency symbol and units of it per USD.
    async fn display(&self) -> (String, Decimal);
}

/// Whether to send the report on the 1st, and which month went last.
pub trait ReportConfig: Send + Sync {
    fn enabled(&self) -> Result<bool, String>;
    fn set_enabled(&self, on: bool) -> Result<(), String>;
    fn last_sent(&self) -> Result<Option<String>, String>;
    fn set_last_sent(&self, month: &str) -> Result<(), String>;
}

/// Delivers the report (in the app and to push channels).
#[async_trait]
pub trait ReportSender: Send + Sync {
    async fn send(&self, title: &str, body: &str) -> Result<(), String>;
}
