//! Server functions for backups; delegate to
//! [`BackupService`](super::BackupService), injected via `Extension`.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use super::BackupService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// The whole database, base64-encoded.
#[post("/api/backup/download", service: Extension<BackupService>)]
pub async fn download_backup() -> Result<String, ServerFnError> {
    Ok(service.export()?)
}

/// Replaces all data with a downloaded backup (base64).
#[post("/api/backup/restore", service: Extension<BackupService>)]
pub async fn restore_backup(data: String) -> Result<(), ServerFnError> {
    Ok(service.restore(&data)?)
}
