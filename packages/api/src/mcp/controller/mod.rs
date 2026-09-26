//! Server functions for turning the AI connector on and off in Settings,
//! and for giving Claude a portfolio of its own. Under `/api/`, so they
//! need a session once sign-in is on.

use dioxus::prelude::*;
use dtos::ai_portfolio::AiPortfolioInfo;
use rust_decimal::Decimal;

#[cfg(feature = "server")]
use super::McpService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// The connector's key, or `None` while it's off.
#[post("/api/connector/key", service: Extension<McpService>)]
pub async fn get_connector_key() -> Result<Option<String>, ServerFnError> {
    Ok(service.key()?)
}

/// Turns the connector on with a new key; the old one stops working.
#[post("/api/connector/new-key", service: Extension<McpService>)]
pub async fn new_connector_key() -> Result<String, ServerFnError> {
    Ok(service.new_key()?)
}

#[post("/api/connector/off", service: Extension<McpService>)]
pub async fn turn_off_connector() -> Result<(), ServerFnError> {
    Ok(service.turn_off()?)
}

/// Claude's own portfolio, or `None` if you haven't given it one.
#[post("/api/connector/ai-portfolio", service: Extension<McpService>)]
pub async fn get_ai_portfolio() -> Result<Option<AiPortfolioInfo>, ServerFnError> {
    Ok(service.trading().info()?)
}

/// Makes a portfolio for Claude to manage, with `starting_cash` USD in it.
#[post("/api/connector/ai-portfolio/start", service: Extension<McpService>)]
pub async fn start_ai_portfolio(starting_cash: Decimal) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.trading().start(starting_cash).await?)
}

/// Gives Claude more cash (USD) to invest.
#[post("/api/connector/ai-portfolio/fund", service: Extension<McpService>)]
pub async fn fund_ai_portfolio(amount: Decimal) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.trading().add_funds(amount).await?)
}

/// Claude stops trading; the portfolio stays as an ordinary one.
#[post("/api/connector/ai-portfolio/stop", service: Extension<McpService>)]
pub async fn stop_ai_portfolio() -> Result<(), ServerFnError> {
    Ok(service.trading().stop()?)
}
