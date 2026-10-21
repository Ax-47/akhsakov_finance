//! Assets: what each ticker is (stock, fund, gold, deposit …), prices you
//! enter for assets without a market price, and Thai tax-saving wrappers.
//!
//! `repositories` is the storage port, `infrastructures` its SQLite adapter,
//! `services` the use cases and `controller` the server functions. The
//! quote context reads the hand-entered prices through its own port.

pub(crate) mod controller;
#[cfg(feature = "server")]
pub(crate) mod infrastructures;
#[cfg(feature = "server")]
pub(crate) mod repositories;
#[cfg(feature = "server")]
pub(crate) mod services;
#[cfg(feature = "server")]
pub(crate) mod feeds;

pub use controller::*;

#[cfg(feature = "server")]
pub use services::AssetService;

#[cfg(feature = "server")]
pub fn asset_services_setup(db: crate::database::Database) -> AssetService {
    let service = AssetService::new(std::sync::Arc::new(infrastructures::SqliteAssetRepository::new(db)))
        .with_feeds(std::sync::Arc::new(feeds::WebPriceSource::new()));
    service.clone().spawn_feed_updates();
    service
}
