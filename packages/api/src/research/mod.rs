//! Stock research: fundamentals, news, analyst rating changes, ownership,
//! corporate actions and options.
//!
//! `repositories` is the market-data port, `infrastructures` its Yahoo
//! adapter, `services` the use cases and `controller` the server functions.

pub(crate) mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::ResearchService;

#[cfg(feature = "server")]
pub fn research_services_setup() -> ResearchService {
    ResearchService::new(std::sync::Arc::new(
        infrastructures::YahooResearchGateway::new(),
    ))
}
