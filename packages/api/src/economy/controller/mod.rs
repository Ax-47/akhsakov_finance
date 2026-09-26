//! Server function for the economy page; delegates to
//! [`EconomyService`](super::EconomyService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::economy::EconomySnapshot;

#[cfg(feature = "server")]
use super::EconomyService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Market gauges, macro indicators and the Treasury yield curve.
#[get("/api/economy", service: Extension<EconomyService>)]
pub async fn get_economy() -> Result<EconomySnapshot, ServerFnError> {
    Ok(service.snapshot().await?)
}
