//! Server functions for chart drawings. Each delegates to
//! [`DrawingService`](super::DrawingService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::drawings::{ChartDrawing, ChartPoint, DrawingKind};
use types::ticker_symbol::TickerSymbol;

#[cfg(feature = "server")]
use super::DrawingService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// The drawings on a stock's chart, oldest first.
#[post("/api/drawings", service: Extension<DrawingService>)]
pub async fn get_drawings(ticker: TickerSymbol) -> Result<Vec<ChartDrawing>, ServerFnError> {
    Ok(service.drawings(&ticker)?)
}

/// Saves a drawing and returns it with its id.
#[post("/api/drawings/add", service: Extension<DrawingService>)]
pub async fn add_drawing(ticker: TickerSymbol, kind: DrawingKind, points: Vec<ChartPoint>) -> Result<ChartDrawing, ServerFnError> {
    Ok(service.add(&ticker, kind, points)?)
}

#[post("/api/drawings/delete", service: Extension<DrawingService>)]
pub async fn delete_drawing(ticker: TickerSymbol, id: i64) -> Result<(), ServerFnError> {
    Ok(service.delete(&ticker, id)?)
}

/// Removes every drawing on a stock's chart.
#[post("/api/drawings/clear", service: Extension<DrawingService>)]
pub async fn clear_drawings(ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.clear(&ticker)?)
}
