//! Provider-neutral OpenAI-compatible model connections and paper-trader runs.

mod controller;

pub use controller::*;

#[cfg(feature = "server")]
mod service;
#[cfg(feature = "server")]
pub use service::ModelService;
#[cfg(feature = "server")]
mod race;

#[cfg(feature = "server")]
pub fn model_services_setup(
    db: crate::database::Database,
    tools: crate::mcp::McpService,
) -> ModelService {
    ModelService::new(db, tools)
}
