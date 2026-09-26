//! Server functions for asset descriptions and hand-entered prices. Each
//! delegates to [`AssetService`](super::AssetService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::assets::{AssetInfo, ManualPrice};
use types::ticker_symbol::TickerSymbol;

#[cfg(feature = "server")]
use super::AssetService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Every described asset, by ticker.
#[get("/api/assets", service: Extension<AssetService>)]
pub async fn get_assets() -> Result<Vec<AssetInfo>, ServerFnError> {
    Ok(service.assets()?)
}

#[post("/api/assets/save", service: Extension<AssetService>)]
pub async fn save_asset(asset: AssetInfo) -> Result<(), ServerFnError> {
    Ok(service.save_asset(asset)?)
}

/// Forgets the description and any entered prices (not the transactions).
#[post("/api/assets/delete", service: Extension<AssetService>)]
pub async fn delete_asset(ticker: TickerSymbol) -> Result<(), ServerFnError> {
    Ok(service.delete_asset(ticker)?)
}

/// Prices entered for a hand-priced asset, newest first.
#[post("/api/assets/prices", service: Extension<AssetService>)]
pub async fn get_manual_prices(ticker: TickerSymbol) -> Result<Vec<ManualPrice>, ServerFnError> {
    Ok(service.prices(ticker)?)
}

#[post("/api/assets/prices/save", service: Extension<AssetService>)]
pub async fn save_manual_price(ticker: TickerSymbol, price: ManualPrice) -> Result<(), ServerFnError> {
    Ok(service.save_price(ticker, price)?)
}

#[post("/api/assets/prices/delete", service: Extension<AssetService>)]
pub async fn delete_manual_price(ticker: TickerSymbol, date: String) -> Result<(), ServerFnError> {
    Ok(service.delete_price(ticker, date)?)
}
