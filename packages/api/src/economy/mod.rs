//! Economy: market gauges (VIX, yields, oil, …), macro indicators and the
//! Treasury yield curve.
//!
//! `repositories` holds the ports (macro series, market gauges),
//! `infrastructures` their adapters (FRED, Yahoo), `services` the use case
//! and `controller` the server function.

pub(crate) mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::EconomyService;

#[cfg(feature = "server")]
pub fn economy_services_setup() -> EconomyService {
    use std::sync::Arc;
    EconomyService::new(
        Arc::new(infrastructures::FredSource::new()),
        Arc::new(infrastructures::YahooGauges::new()),
    )
}
