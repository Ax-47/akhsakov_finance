//! Monthly reports: a summary of last month (value, return, dividends,
//! trades, movers) sent to your push channels on the 1st, or built on
//! demand for any month. The maths lives in `dtos::report`.
//!
//! `repositories` holds the ports, `infrastructures` their adapters,
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
pub use services::ReportService;

/// The report service, with its monthly schedule started.
#[cfg(feature = "server")]
pub fn report_services_setup(
    db: crate::database::Database,
    portfolio: crate::portfolio::PortfolioService,
    quotes: crate::quote::services::quote::QuoteService,
    notifications: crate::notifications::NotificationService,
    settings: crate::settings::SettingsService,
) -> ReportService {
    use infrastructures::*;
    use std::sync::Arc;
    let service = ReportService::new(
        Arc::new(AppReportData { portfolio, quotes, settings }),
        Arc::new(SqliteReportConfig::new(db)),
        Arc::new(NotifySender(notifications)),
    );
    service.clone().spawn_schedule();
    service
}
