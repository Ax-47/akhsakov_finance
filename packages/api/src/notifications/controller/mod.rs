//! Server functions for notifications; delegate to
//! [`NotificationService`](super::NotificationService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::notifications::{Channels, Notification};

#[cfg(feature = "server")]
use super::NotificationService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// The latest notifications, newest first.
#[get("/api/notifications", service: Extension<NotificationService>)]
pub async fn get_notifications() -> Result<Vec<Notification>, ServerFnError> {
    Ok(service.recent()?)
}

#[post("/api/notifications/read", service: Extension<NotificationService>)]
pub async fn mark_notifications_read() -> Result<(), ServerFnError> {
    Ok(service.mark_all_read()?)
}

#[get("/api/notifications/channels", service: Extension<NotificationService>)]
pub async fn get_channels() -> Result<Channels, ServerFnError> {
    Ok(service.channels()?)
}

#[post("/api/notifications/channels", service: Extension<NotificationService>)]
pub async fn save_channels(channels: Channels) -> Result<(), ServerFnError> {
    Ok(service.save_channels(channels)?)
}

/// Sends a test message through `channels`; `(channel, error)` per channel,
/// `None` meaning it worked.
#[post("/api/notifications/test", service: Extension<NotificationService>)]
pub async fn test_channels(channels: Channels) -> Result<Vec<(String, Option<String>)>, ServerFnError> {
    Ok(service
        .test(channels)
        .await?
        .into_iter()
        .map(|(name, result)| (name, result.err()))
        .collect())
}
