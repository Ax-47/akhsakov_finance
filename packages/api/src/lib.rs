//! Backend: server functions organised as bounded contexts, each split
//! into repositories (ports), infrastructures (adapters), services (use
//! cases) and a controller (server functions).

#[cfg(feature = "server")]
pub mod database;
pub mod shared;

pub mod research;
pub use research::controller::*;

pub mod market;
pub use market::controller::*;

pub mod planning;
pub use planning::controller::*;

pub mod portfolio;
pub use portfolio::controller::*;

pub mod quote;
pub use quote::*;

pub mod settings;
pub use settings::controller::*;

pub mod watchlist;
pub use watchlist::controller::*;

/// Adds every context's service to the router, sharing one database.
#[cfg(feature = "server")]
pub fn with_services(router: dioxus::server::axum::Router) -> dioxus::server::axum::Router {
    use dioxus::server::axum::Extension;
    let db = database::Database::open_default().expect("open the database");
    router
        .layer(Extension(quote_services_setup()))
        .layer(Extension(research::research_services_setup()))
        .layer(Extension(market::market_services_setup()))
        .layer(Extension(portfolio::portfolio_services_setup(db.clone())))
        .layer(Extension(watchlist::watchlist_services_setup(db.clone())))
        .layer(Extension(planning::planning_services_setup(db.clone())))
        .layer(Extension(settings::settings_services_setup(db)))
}
