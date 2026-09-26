//! Server functions for the watchlist and alerts. Each delegates to
//! [`WatchlistService`](super::WatchlistService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::watch::{Alert, AlertKind, WatchItem};
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[cfg(feature = "server")]
use super::WatchlistService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

#[get("/api/watchlist", service: Extension<WatchlistService>)]
pub async fn get_watchlist() -> Result<Vec<WatchItem>, ServerFnError> {
    Ok(service.watchlist()?)
}

#[post("/api/watchlist/add", service: Extension<WatchlistService>)]
pub async fn watch_ticker(ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.watch(&ticker)?)
}

#[post("/api/watchlist/remove", service: Extension<WatchlistService>)]
pub async fn unwatch_ticker(ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.unwatch(&ticker)?)
}

#[get("/api/alerts", service: Extension<WatchlistService>)]
pub async fn get_alerts() -> Result<Vec<Alert>, ServerFnError> {
    Ok(service.alerts()?)
}

#[post("/api/alerts/create", service: Extension<WatchlistService>)]
pub async fn create_alert(
    ticker: TickerSymbol,
    kind: AlertKind,
    value: Decimal,
) -> Result<Alert, ServerFnError> {
    Ok(service.create_alert(ticker, kind, value)?)
}

#[post("/api/alerts/delete", service: Extension<WatchlistService>)]
pub async fn delete_alert(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_alert(id)?)
}

#[post("/api/alerts/triggered", service: Extension<WatchlistService>)]
pub async fn mark_alert_triggered(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.mark_triggered(id)?)
}
