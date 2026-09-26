//! Portfolios and their transactions.
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
pub use services::PortfolioService;

#[cfg(feature = "server")]
pub fn portfolio_services_setup(
    db: crate::database::Database,
    fx: std::sync::Arc<dyn crate::shared::FxRates>,
) -> PortfolioService {
    PortfolioService::new(
        std::sync::Arc::new(infrastructures::SqlitePortfolioRepository::new(db)),
        fx,
    )
}
