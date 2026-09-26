//! The AI connector: an MCP (Model Context Protocol) server at `/mcp`, so
//! Claude (claude.ai, the Claude apps, Claude Code) can read your
//! portfolios and read and write your theses. It's off until you turn it
//! on in Settings, which creates the key every request must carry.
//!
//! `repositories` is the key store port, `infrastructures` its SQLite
//! adapter and the HTTP endpoint, `services` the protocol and its tools,
//! and `controller` the server functions Settings uses.

pub(crate) mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::McpService;

#[cfg(feature = "server")]
pub fn mcp_services_setup(
    db: crate::database::Database,
    theses: crate::thesis::ThesisService,
    portfolios: crate::portfolio::PortfolioService,
    watchlist: crate::watchlist::WatchlistService,
) -> McpService {
    McpService::new(
        std::sync::Arc::new(infrastructures::SqliteKeyRepository::new(db)),
        services::tools::Tools::new(theses, portfolios, watchlist),
    )
}

/// Adds the endpoint: `/mcp` with the key in `Authorization: Bearer …`, and
/// `/mcp/{key}` for clients that only take a URL. Outside `/api/`, so the
/// sign-in check leaves it to the key.
#[cfg(feature = "server")]
pub fn routes(
    router: dioxus::server::axum::Router,
    service: McpService,
) -> dioxus::server::axum::Router {
    router.merge(infrastructures::http::router(service))
}
