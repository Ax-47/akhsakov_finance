//! Server functions for turning the AI connector on and off in Settings,
//! and for giving Claude portfolios of its own. Under `/api/`, so they
//! need a session once sign-in is on.

use dioxus::prelude::*;
use dtos::ai_portfolio::AiPortfolioInfo;
use rust_decimal::Decimal;
use uuid::Uuid;

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

/// The portfolios you've given Claude, oldest first.
#[post("/api/connector/ai-portfolios", service: Extension<McpService>)]
pub async fn get_ai_portfolios() -> Result<Vec<AiPortfolioInfo>, ServerFnError> {
    Ok(service.trading().infos()?)
}

/// Makes a portfolio for Claude to manage, called `name` (blank for
/// "Claude"), with `starting_cash` USD of its own.
#[post("/api/connector/ai-portfolios/start", service: Extension<McpService>)]
pub async fn start_ai_portfolio(name: String, starting_cash: Decimal) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.trading().start(&name, starting_cash).await?)
}

/// Gives one of Claude's portfolios more cash (USD) to invest.
#[post("/api/connector/ai-portfolios/fund", service: Extension<McpService>)]
pub async fn fund_ai_portfolio(id: Uuid, amount: Decimal) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.trading().add_funds(id, amount).await?)
}

/// Claude stops trading in `id`; it stays as an ordinary portfolio.
#[post("/api/connector/ai-portfolios/stop", service: Extension<McpService>)]
pub async fn stop_ai_portfolio(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.trading().stop(id)?)
}
