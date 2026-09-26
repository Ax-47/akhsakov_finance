//! Server functions for settings, delegating to
//! [`SettingsService`](super::SettingsService).

use dioxus::prelude::*;
use dtos::settings::Settings;

#[cfg(feature = "server")]
use super::SettingsService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

#[get("/api/settings", service: Extension<SettingsService>)]
pub async fn get_settings() -> Result<Settings, ServerFnError> {
    Ok(service.get()?)
}

#[post("/api/settings/save", service: Extension<SettingsService>)]
pub async fn save_settings(settings: Settings) -> Result<(), ServerFnError> {
    Ok(service.save(settings)?)
}
