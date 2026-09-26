//! Market overview: index and watchlist heatmaps.
//!
//! `repositories` holds the ports (index membership, market data),
//! `infrastructures` their adapters (a built-in constituent list, Yahoo),
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
pub fn market_services_setup() -> MarketService {
    use std::sync::Arc;
    MarketService::new(
        Arc::new(infrastructures::StaticIndexRepository),
        Arc::new(infrastructures::YahooMarketGateway::new()),
    )
}
