//! Backend: server functions organised as bounded contexts, each split
//! into repositories (ports), infrastructures (adapters), services (use
//! cases) and a controller (server functions).

#[cfg(feature = "server")]
pub mod database;
pub mod shared;

pub mod auth;
pub use auth::controller::*;

pub mod research;
pub use research::controller::*;

pub mod backup;
pub use backup::controller::*;

pub mod economy;
pub use economy::controller::*;

pub mod market;
pub use market::controller::*;

pub mod notifications;
pub use notifications::controller::*;

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
    let quotes = quote_services_setup(db.clone());
    let fx: std::sync::Arc<dyn shared::FxRates> = std::sync::Arc::new(QuoteFxRates(quotes.clone()));
    let portfolio = portfolio::portfolio_services_setup(db.clone(), fx.clone());
    let watchlist = watchlist::watchlist_services_setup(db.clone());
    let notifications = notifications::notification_services_setup(
        db.clone(),
        watchlist.clone(),
        portfolio.clone(),
        quotes.clone(),
    );
    let auth = auth::auth_services_setup(db.clone());
    // Layers wrap what's added before them: the sign-in check runs first,
    // then the services are attached.
    let router = router
        .layer(Extension(auth.clone()))
        .layer(Extension(quotes))
        .layer(Extension(research::research_services_setup(fx.clone())))
        .layer(Extension(economy::economy_services_setup()))
        .layer(Extension(backup::backup_services_setup(db.clone())))
        .layer(Extension(market::market_services_setup(db.clone(), fx)))
        .layer(Extension(portfolio))
        .layer(Extension(watchlist))
        .layer(Extension(notifications))
        .layer(Extension(planning::planning_services_setup(db.clone())))
        .layer(Extension(settings::settings_services_setup(db)));
    auth::protect(router, auth)
}
