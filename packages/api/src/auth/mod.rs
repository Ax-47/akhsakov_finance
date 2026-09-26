//! Accounts and sign-in.
//!
//! `repositories` holds the ports (accounts, sessions, password hashing),
//! `infrastructures` their adapters (SQLite, Argon2) and the HTTP
//! middleware, `services` the use cases and `controller` the server
//! functions.

pub mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::AuthService;

#[cfg(feature = "server")]
pub fn auth_services_setup(db: crate::database::Database) -> AuthService {
    use std::sync::Arc;
    let always = std::env::var("AKHSAKOV_REQUIRE_LOGIN").is_ok_and(|v| v == "1" || v == "true");
    AuthService::new(
        Arc::new(infrastructures::SqliteAuthRepository::new(db)),
        Arc::new(infrastructures::Argon2Hasher),
        always,
    )
}

/// Adds the sign-in check in front of `router`'s API.
#[cfg(feature = "server")]
pub fn protect(router: dioxus::server::axum::Router, auth: AuthService) -> dioxus::server::axum::Router {
    router.layer(dioxus::server::axum::middleware::from_fn_with_state(
        auth,
        infrastructures::middleware::require_sign_in,
    ))
}
