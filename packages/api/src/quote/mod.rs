pub(crate) mod controller;
pub mod events;

#[cfg(feature = "server")]
pub(crate) mod infrastructures;

pub(crate) mod repositories;

pub(crate) mod services;
#[cfg(feature = "server")]
use crate::services::quote::QuoteService;
use std::sync::Arc;

#[cfg(feature = "server")]
use tokio::sync::Mutex;

pub use controller::*;

#[cfg(feature = "server")]
pub fn quote_services_setup() -> QuoteService {
    let quote_gateway = Arc::new(Mutex::new(
        infrastructures::yahoo_gateway::YahooGateWay::new(),
    ));
    QuoteService::new(quote_gateway)
}
