//! Backups: the whole database as one downloadable file, and restoring it.
//!
//! `repositories` is the snapshot port, `infrastructures` its SQLite
//! adapter, `services` the use cases and `controller` the server functions.

pub(crate) mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::BackupService;

#[cfg(feature = "server")]
pub fn backup_services_setup(db: crate::database::Database) -> BackupService {
    BackupService::new(std::sync::Arc::new(infrastructures::SqliteBackupStore(db)))
}
