use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub ticker: TickerSymbol,
    pub transaction_type: TransactionType,
    pub shares: Decimal,
    pub price: Decimal,
    pub date: String, // ISO 8601: YYYY-MM-DD
    /// Commission / fees paid on this transaction.
    #[serde(default)]
    pub fee: Decimal,
}

/// Ticker used for cash deposits and withdrawals.
pub const CASH_TICKER: &str = "$CASH";

impl Transaction {
    pub fn is_cash(&self) -> bool {
        self.ticker.as_str() == CASH_TICKER
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    pub transactions: Vec<Transaction>,
}
