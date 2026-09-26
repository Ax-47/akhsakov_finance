//! Server functions for turning the AI connector on and off in Settings.
//! Under `/api/`, so they need a session once sign-in is on.

use dioxus::prelude::*;

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
