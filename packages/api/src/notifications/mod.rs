//! Notifications: fired alerts, shown in the app and pushed to your phone
//! or chat (ntfy, Telegram, webhook), and the server-side alert monitor.
//!
//! `repositories` holds the ports (storage, push, and the monitor's views
//! of alerts, holdings and prices), `infrastructures` their adapters,
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
pub use services::{AlertMonitor, NotificationService};

/// The notification service, with the alert monitor started in the
/// background.
#[cfg(feature = "server")]
pub fn notification_services_setup(
    db: crate::database::Database,
    watchlist: crate::watchlist::WatchlistService,
    portfolio: crate::portfolio::PortfolioService,
    quotes: crate::quote::services::quote::QuoteService,
) -> NotificationService {
    use infrastructures::*;
    use std::sync::Arc;
    let service = NotificationService::new(
        Arc::new(SqliteNotificationRepository::new(db)),
        Arc::new(HttpPusher::new()),
    );
    AlertMonitor::new(
        Arc::new(WatchlistAlerts(watchlist)),
        Arc::new(PortfolioHoldings(portfolio)),
        Arc::new(QuotePrices(quotes)),
        service.clone(),
    )
    .spawn();
    service
}
