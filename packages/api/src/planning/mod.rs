//! Planning data: target weights for rebalancing, and savings goals.
//! The calculations themselves live in `dtos::planning` and run client-side.
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
pub use services::PlanningService;

#[cfg(feature = "server")]
pub fn planning_services_setup(db: crate::database::Database) -> PlanningService {
    PlanningService::new(std::sync::Arc::new(
        infrastructures::SqlitePlanningRepository::new(db),
    ))
}
