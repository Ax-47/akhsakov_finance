//! Market overview: index and watchlist heatmaps, stock screener, earnings
//! and dividend calendar, sector peers.
//!
//! `repositories` holds the ports (index membership, market data),
//! `infrastructures` their adapters (Wikipedia with a SQLite cache, Yahoo),
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
pub use services::MarketService;

#[cfg(feature = "server")]
pub fn market_services_setup(
    db: crate::database::Database,
    fx: std::sync::Arc<dyn crate::shared::FxRates>,
) -> MarketService {
    use std::sync::Arc;
    MarketService::new(
        Arc::new(infrastructures::WikipediaIndexRepository::new(db)),
        Arc::new(infrastructures::YahooMarketGateway::new()),
        fx,
    )
}
