//! Server functions for theses. Each delegates to
//! [`ThesisService`](super::ThesisService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::thesis::{Thesis, ThesisDraft, ThesisEntry};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[cfg(feature = "server")]
use super::ThesisService;
#[cfg(feature = "server")]
use dtos::thesis::Author;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Every thesis in every portfolio, with its journal.
#[get("/api/theses", service: Extension<ThesisService>)]
pub async fn get_theses() -> Result<Vec<Thesis>, ServerFnError> {
    Ok(service.theses(None)?)
}

/// Saves your thesis on a holding; clearing every field (with an empty
/// journal) deletes it.
#[post("/api/theses/save", service: Extension<ThesisService>)]
pub async fn save_thesis(
    portfolio_id: Uuid,
    ticker: TickerSymbol,
    draft: ThesisDraft,
) -> Result<(), ServerFnError> {
    Ok(service.save(portfolio_id, &ticker, draft, Author::You, None)?)
}

/// Deletes the thesis and its journal.
#[post("/api/theses/delete", service: Extension<ThesisService>)]
pub async fn delete_thesis(portfolio_id: Uuid, ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.delete(portfolio_id, &ticker)?)
}

#[post("/api/theses/journal/add", service: Extension<ThesisService>)]
pub async fn add_thesis_entry(
    portfolio_id: Uuid,
    ticker: TickerSymbol,
    text: String,
) -> Result<ThesisEntry, ServerFnError> {
    Ok(service.add_entry(portfolio_id, &ticker, &text, Author::You)?)
}

#[post("/api/theses/journal/delete", service: Extension<ThesisService>)]
pub async fn delete_thesis_entry(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_entry(id)?)
}
