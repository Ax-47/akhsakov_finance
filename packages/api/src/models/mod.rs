//! Provider-neutral OpenAI-compatible model connections and paper-trader runs.

mod controller;

pub use controller::*;

#[cfg(feature = "server")]
mod service;
#[cfg(feature = "server")]
pub use service::ModelService;
#[cfg(feature = "server")]
mod race;
#[cfg(all(test, feature = "server"))]
mod mock_model;
#[cfg(all(test, feature = "server"))]
mod run_tests;

#[cfg(feature = "server")]
pub fn model_services_setup(
    db: crate::database::Database,
    tools: crate::mcp::McpService,
) -> ModelService {
    ModelService::new(db, tools)
}
