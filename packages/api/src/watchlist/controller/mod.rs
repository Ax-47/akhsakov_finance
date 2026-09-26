//! Server functions for the watchlist and alerts. Each delegates to
//! [`WatchlistService`](super::WatchlistService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::watch::{Alert, AlertKind, Note, WatchItem, Watchlist};
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[cfg(feature = "server")]
use super::WatchlistService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Every watched stock once, across all lists.
#[get("/api/watchlist", service: Extension<WatchlistService>)]
pub async fn get_watchlist() -> Result<Vec<WatchItem>, ServerFnError> {
    Ok(service.watchlist()?)
}

/// Adds to the first list.
#[post("/api/watchlist/add", service: Extension<WatchlistService>)]
pub async fn watch_ticker(ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.watch(&ticker)?)
}

/// Removes from every list.
#[post("/api/watchlist/remove", service: Extension<WatchlistService>)]
pub async fn unwatch_ticker(ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.unwatch(&ticker)?)
}

/// Every watchlist with its stocks.
#[get("/api/watchlists", service: Extension<WatchlistService>)]
pub async fn get_watchlists() -> Result<Vec<Watchlist>, ServerFnError> {
    Ok(service.watchlists()?)
}

/// Returns the new list's id.
#[post("/api/watchlists/create", service: Extension<WatchlistService>)]
pub async fn create_watchlist(name: String) -> Result<Uuid, ServerFnError> {
    Ok(service.create_list(&name)?)
}

#[post("/api/watchlists/rename", service: Extension<WatchlistService>)]
pub async fn rename_watchlist(id: Uuid, name: String) -> Result<(), ServerFnError> {
    Ok(service.rename_list(id, &name)?)
}

#[post("/api/watchlists/delete", service: Extension<WatchlistService>)]
pub async fn delete_watchlist(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_list(id)?)
}

#[post("/api/watchlists/add", service: Extension<WatchlistService>)]
pub async fn watch_in(list: Uuid, ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.watch_in(list, &ticker)?)
}

#[post("/api/watchlists/remove", service: Extension<WatchlistService>)]
pub async fn unwatch_from(list: Uuid, ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.unwatch_from(list, &ticker)?)
}

/// Your notes and tags on every stock.
#[get("/api/notes", service: Extension<WatchlistService>)]
pub async fn get_notes() -> Result<Vec<Note>, ServerFnError> {
    Ok(service.notes()?)
}

/// Saves a note; empty text and tags delete it.
#[post("/api/notes/save", service: Extension<WatchlistService>)]
pub async fn save_note(
    ticker: TickerSymbol,
    text: String,
    tags: Vec<String>,
) -> Result<(), ServerFnError> {
    Ok(service.save_note(&ticker, &text, &tags)?)
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
