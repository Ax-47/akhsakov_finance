//! Server functions for portfolios and transactions. Each delegates to
//! [`PortfolioService`](super::PortfolioService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::{csv_import::ImportResult, portfolio::GetDashBoardResponse, Transaction};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::PortfolioService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Every portfolio with its holdings, plus all transactions.
#[get("/api/dashboard", service: Extension<PortfolioService>)]
pub async fn get_dashboard() -> Result<GetDashBoardResponse, ServerFnError> {
    Ok(service.dashboard()?)
}

/// Returns the new portfolio's id.
#[post("/api/portfolios/create", service: Extension<PortfolioService>)]
pub async fn create_portfolio(name: String) -> Result<Uuid, ServerFnError> {
    Ok(service.create_portfolio(&name)?)
}

#[post("/api/portfolios/rename", service: Extension<PortfolioService>)]
pub async fn rename_portfolio(id: Uuid, name: String) -> Result<(), ServerFnError> {
    Ok(service.rename_portfolio(id, &name)?)
}

/// Deletes the portfolio and all of its transactions.
#[post("/api/portfolios/delete", service: Extension<PortfolioService>)]
pub async fn delete_portfolio(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_portfolio(id)?)
}

/// Adds a transaction, or updates the one with the same id.
#[post("/api/transactions/save", service: Extension<PortfolioService>)]
pub async fn save_transaction(transaction: Transaction) -> Result<(), ServerFnError> {
    Ok(service.save_transaction(transaction)?)
}

#[post("/api/transactions/delete", service: Extension<PortfolioService>)]
pub async fn delete_transaction(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_transaction(id)?)
}

/// Imports a broker CSV export into `portfolio_id`.
#[post("/api/transactions/import", service: Extension<PortfolioService>)]
pub async fn import_transactions(
    portfolio_id: Uuid,
    csv: String,
) -> Result<ImportResult, ServerFnError> {
    Ok(service.import_csv(portfolio_id, &csv)?)
}
