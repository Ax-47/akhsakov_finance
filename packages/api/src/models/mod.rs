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
mod stop;
#[cfg(feature = "server")]
mod schedule;
#[cfg(feature = "server")]
mod announce;
#[cfg(feature = "server")]
mod usage;
#[cfg(feature = "server")]
mod responses;
#[cfg(all(test, feature = "server"))]
mod mock_model;
#[cfg(all(test, feature = "server"))]
mod run_tests;

#[cfg(feature = "server")]
pub fn model_services_setup(
    db: crate::database::Database,
    tools: crate::mcp::McpService,
    notifications: crate::notifications::NotificationService,
) -> ModelService {
    let service = ModelService::new(db, tools).with_notifications(notifications);
    service.clone().spawn_scheduler();
    service
}
