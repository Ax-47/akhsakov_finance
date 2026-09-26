//! Server functions for the market overview. Each delegates to
//! [`MarketService`](super::MarketService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::market::{HeatmapItem, MarketIndex};
use types::ticker_symbol::TickerSymbol;

#[cfg(feature = "server")]
use super::MarketService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// An index's largest constituents with live price, change and market cap.
#[post("/api/market/index", service: Extension<MarketService>)]
pub async fn get_index_heatmap(index: MarketIndex) -> Result<Vec<HeatmapItem>, ServerFnError> {
    Ok(service.index_heatmap(index).await?)
}

/// Live price, change and market cap for any stocks, e.g. a watchlist.
#[post("/api/market/tickers", service: Extension<MarketService>)]
pub async fn get_tickers_heatmap(
    tickers: Vec<TickerSymbol>,
) -> Result<Vec<HeatmapItem>, ServerFnError> {
    Ok(service.tickers_heatmap(tickers).await?)
}
