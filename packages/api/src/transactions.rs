use dioxus::prelude::*;
use dtos::transaction::{AppData, Transaction, TransactionType};
use uuid::Uuid;

/// Return mock transaction history.
/// Replace this body with a real database query when ready.
#[post("/api/transactions")]
pub async fn get_transactions() -> Result<AppData, ServerFnError> {
    let txs = vec![
        Transaction {
            id: Uuid::new_v4(),
            ticker: "AAPL".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 10.0,
            price: 150.00,
            fees: 1.00,
            date: "2024-01-15".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "MSFT".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 5.0,
            price: 380.00,
            fees: 1.00,
            date: "2024-02-01".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "BTC".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 0.25,
            price: 58_000.00,
            fees: 5.00,
            date: "2024-03-10".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "SPY".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 8.0,
            price: 500.00,
            fees: 1.00,
            date: "2024-04-05".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "AAPL".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 5.0,
            price: 170.00,
            fees: 1.00,
            date: "2024-06-20".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "NVDA".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 3.0,
            price: 800.00,
            fees: 2.00,
            date: "2024-08-15".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "AAPL".to_string(),
            transaction_type: TransactionType::Sell,
            shares: 3.0,
            price: 190.00,
            fees: 1.00,
            date: "2024-09-01".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "AAPL".to_string(),
            transaction_type: TransactionType::Dividend,
            shares: 0.0,
            price: 3.82,
            fees: 0.00,
            date: "2024-11-15".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "MSFT".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 3.0,
            price: 395.00,
            fees: 1.00,
            date: "2024-12-10".to_string(),
        },
        Transaction {
            id: Uuid::new_v4(),
            ticker: "BND".to_string(),
            transaction_type: TransactionType::Buy,
            shares: 30.0,
            price: 71.50,
            fees: 1.00,
            date: "2025-01-08".to_string(),
        },
    ];

    Ok(AppData { transactions: txs })
}
