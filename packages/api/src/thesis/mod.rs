//! Investment theses: one per holding of each portfolio, with a journal
//! that you and an AI assistant (through the MCP connector) add to.
//!
//! `repositories` is the storage port, `infrastructures` its SQLite adapter,
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
pub use services::ThesisService;

#[cfg(feature = "server")]
pub fn thesis_services_setup(db: crate::database::Database) -> ThesisService {
    ThesisService::new(std::sync::Arc::new(
        infrastructures::SqliteThesisRepository::new(db),
    ))
}
