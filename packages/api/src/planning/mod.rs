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
pub use services::{DcaService, PlanningService};

#[cfg(feature = "server")]
pub fn planning_services_setup(db: crate::database::Database) -> PlanningService {
    PlanningService::new(std::sync::Arc::new(
        infrastructures::SqlitePlanningRepository::new(db),
    ))
}

/// Monthly plans, with their hourly reminder check started.
#[cfg(feature = "server")]
pub fn dca_services_setup(
    db: crate::database::Database,
    portfolio: crate::portfolio::PortfolioService,
    notifications: crate::notifications::NotificationService,
) -> DcaService {
    use infrastructures::*;
    use std::sync::Arc;
    let service = DcaService::new(
        Arc::new(SqlitePlanningRepository::new(db)),
        Arc::new(PortfolioTrades(portfolio)),
        Arc::new(NotifyReminder(notifications)),
    );
    service.clone().spawn_reminders();
    service
}
