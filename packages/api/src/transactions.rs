use dioxus::prelude::*;
use dtos::transaction::{AppData, Transaction};
use uuid::Uuid;

/// Return mock transaction history.
/// Replace this body with a real database query when ready.
#[post("/api/transactions")]
pub async fn get_transactions() -> Result<AppData, ServerFnError> {
    let txs = vec![];

    Ok(AppData { transactions: txs })
}
