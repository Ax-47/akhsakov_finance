use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use types::transaction_type::TransactionType;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub ticker: String,
    pub transaction_type: TransactionType,
    pub shares: Decimal,
    pub price: Decimal,
    pub date: String, // ISO 8601: YYYY-MM-DD
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    pub transactions: Vec<Transaction>,
}
