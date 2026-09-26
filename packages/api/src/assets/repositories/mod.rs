//! Storage port for asset descriptions and hand-entered prices.

use crate::shared::RepositoryError;
use dtos::assets::{AssetInfo, ManualPrice};
use types::ticker_symbol::TickerSymbol;

pub trait AssetRepository: Send + Sync {
    fn assets(&self) -> Result<Vec<AssetInfo>, RepositoryError>;
    /// Inserts, or replaces the asset with the same ticker.
    fn save_asset(&self, asset: &AssetInfo) -> Result<(), RepositoryError>;
    /// Removes the asset and its prices.
    fn delete_asset(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError>;
    /// Newest first.
    fn prices(&self, ticker: &TickerSymbol) -> Result<Vec<ManualPrice>, RepositoryError>;
    /// Inserts, or replaces the price on the same date.
    fn save_price(&self, ticker: &TickerSymbol, price: &ManualPrice) -> Result<(), RepositoryError>;
    fn delete_price(&self, ticker: &TickerSymbol, date: &str) -> Result<(), RepositoryError>;
}
