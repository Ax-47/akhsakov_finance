//! Watched tickers and price alerts.
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
pub use services::WatchlistService;

#[cfg(feature = "server")]
pub fn watchlist_services_setup(db: crate::database::Database) -> WatchlistService {
    WatchlistService::new(std::sync::Arc::new(
        infrastructures::SqliteWatchlistRepository::new(db),
    ))
}
