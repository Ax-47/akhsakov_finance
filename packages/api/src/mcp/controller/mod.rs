//! Server functions for managing named MCP connections in Settings, and for
//! creating AI-managed paper portfolios. Under `/api/`, so they
//! need a session once sign-in is on.

use dioxus::prelude::*;
use dtos::{ai_portfolio::AiPortfolioInfo, mcp::{McpAccessPreset, McpAuditEvent, McpConnection, McpConnectionSecret, McpPortfolioScope}};
use rust_decimal::Decimal;
use uuid::Uuid;

#[cfg(feature = "server")]
use super::McpService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

#[post("/api/connector/connections", service: Extension<McpService>)]
pub async fn get_mcp_connections() -> Result<Vec<McpConnection>, ServerFnError> {
    Ok(service.connections()?)
}

#[post("/api/connector/portfolio-scopes", service: Extension<McpService>)]
pub async fn get_mcp_portfolio_scopes() -> Result<Vec<McpPortfolioScope>, ServerFnError> { Ok(service.portfolio_scopes()?) }

#[post("/api/connector/connections/create", service: Extension<McpService>)]
pub async fn create_mcp_connection(name:String,preset:McpAccessPreset,portfolio_ids:Vec<Uuid>) -> Result<McpConnectionSecret,ServerFnError>{Ok(service.create_connection(&name,preset,portfolio_ids)?)}

#[post("/api/connector/connections/update", service: Extension<McpService>)]
pub async fn update_mcp_connection(id:Uuid,name:String,preset:McpAccessPreset,enabled:bool,portfolio_ids:Vec<Uuid>)->Result<(),ServerFnError>{Ok(service.update_connection(id,&name,preset,enabled,portfolio_ids)?)}

#[post("/api/connector/connections/rotate", service: Extension<McpService>)]
pub async fn rotate_mcp_connection(id:Uuid)->Result<McpConnectionSecret,ServerFnError>{Ok(service.rotate_connection(id)?)}

#[post("/api/connector/connections/delete", service: Extension<McpService>)]
pub async fn delete_mcp_connection(id:Uuid)->Result<(),ServerFnError>{Ok(service.delete_connection(id)?)}

#[post("/api/connector/connections/audit", service: Extension<McpService>)]
pub async fn get_mcp_audit_events(id:Uuid)->Result<Vec<McpAuditEvent>,ServerFnError>{Ok(service.audit_events(id)?)}

#[post("/api/connector/enabled", service: Extension<McpService>)]
pub async fn has_mcp_connections() -> Result<bool, ServerFnError> {
    Ok(service.connections()?.iter().any(|c|c.enabled))
}

/// AI-managed paper portfolios, oldest first.
#[post("/api/connector/ai-portfolios", service: Extension<McpService>)]
pub async fn get_ai_portfolios() -> Result<Vec<AiPortfolioInfo>, ServerFnError> {
    Ok(service.trading().infos()?)
}

/// Makes an AI-managed portfolio, called `name` (blank for "AI"), with
/// `starting_cash` USD of its own.
#[post("/api/connector/ai-portfolios/start", service: Extension<McpService>)]
pub async fn start_ai_portfolio(name: String, starting_cash: Decimal) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.trading().start(&name, starting_cash).await?)
}

/// Gives one of the AI portfolios more cash (USD) to invest.
#[post("/api/connector/ai-portfolios/fund", service: Extension<McpService>)]
pub async fn fund_ai_portfolio(id: Uuid, amount: Decimal) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.trading().add_funds(id, amount).await?)
}

/// Stops AI trading in `id`; it stays as an ordinary portfolio.
#[post("/api/connector/ai-portfolios/stop", service: Extension<McpService>)]
pub async fn stop_ai_portfolio(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.trading().stop(id)?)
}
