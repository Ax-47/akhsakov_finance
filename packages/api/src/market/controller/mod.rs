//! Server functions for the market overview. Each delegates to
//! [`MarketService`](super::MarketService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::insights::DividendInfo;
use dtos::market::{CalendarEvent, HeatmapItem, MarketIndex, PeerGroup, ScreenFilter, ScreenResult};
use types::ticker_symbol::TickerSymbol;

#[cfg(feature = "server")]
use super::MarketService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// An index's constituents with live price, change and market cap (USD).
#[post("/api/market/index", service: Extension<MarketService>)]
pub async fn get_index_heatmap(index: MarketIndex) -> Result<Vec<HeatmapItem>, ServerFnError> {
    Ok(service.index_heatmap(index).await?)
}

/// Live price, change and market cap (USD) for any stocks, e.g. a watchlist.
#[post("/api/market/tickers", service: Extension<MarketService>)]
pub async fn get_tickers_heatmap(
    tickers: Vec<TickerSymbol>,
) -> Result<Vec<HeatmapItem>, ServerFnError> {
    Ok(service.tickers_heatmap(tickers).await?)
}

/// One page of stocks matching `filter`.
#[post("/api/market/screen", service: Extension<MarketService>)]
pub async fn screen_stocks(filter: ScreenFilter) -> Result<ScreenResult, ServerFnError> {
    Ok(service.screen(filter).await?)
}

/// Earnings and dividend dates for `tickers`, soonest first.
#[post("/api/market/calendar", service: Extension<MarketService>)]
pub async fn get_calendar(tickers: Vec<TickerSymbol>) -> Result<Vec<CalendarEvent>, ServerFnError> {
    Ok(service.calendar(tickers).await?)
}

/// The largest stocks in `ticker`'s sector; `None` if it's in no index.
#[post("/api/market/peers", service: Extension<MarketService>)]
pub async fn get_peers(ticker: TickerSymbol) -> Result<Option<PeerGroup>, ServerFnError> {
    Ok(service.peers(ticker).await?)
}

/// Forecast dividends (USD) and next dates for the payers among `tickers`.
#[post("/api/market/dividends", service: Extension<MarketService>)]
pub async fn get_dividends(tickers: Vec<TickerSymbol>) -> Result<Vec<DividendInfo>, ServerFnError> {
    Ok(service.dividends(tickers).await?)
}
