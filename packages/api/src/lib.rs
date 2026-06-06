//! Shared fullstack server functions.
use dioxus::prelude::*;

pub mod asset;
pub mod portfolio;
pub use portfolio::services::portfolio_service::*;
pub mod transaction;

pub mod prices;
pub use prices::get_live_prices;

pub mod transactions;
pub use transactions::get_transactions;

/// Echo the user input on the server.
#[post("/api/echo")]
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}
