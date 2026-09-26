//! User settings.
//!
//! `repositories` is the storage port, `infrastructures` its SQLite adapter,
//! `services` the use cases and `controller` the server functions.

pub(crate) mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::SettingsService;

#[cfg(feature = "server")]
pub fn settings_services_setup(db: crate::database::Database) -> SettingsService {
    SettingsService::new(std::sync::Arc::new(
        infrastructures::SqliteSettingsRepository::new(db),
    ))
}
