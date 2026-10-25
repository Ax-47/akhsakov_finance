//! Chart drawings: the price levels, trend lines and Fibonacci
//! retracements drawn on each stock's chart.
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
pub use services::DrawingService;

#[cfg(feature = "server")]
pub fn drawing_services_setup(db: crate::database::Database) -> DrawingService {
    DrawingService::new(std::sync::Arc::new(infrastructures::SqliteDrawingRepository::new(db)))
}
