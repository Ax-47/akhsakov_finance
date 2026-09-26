//! Storage port for the watchlist and alerts.

use crate::shared::RepositoryError;
use dtos::watch::{Alert, WatchItem};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub trait WatchlistRepository: Send + Sync {
    fn watchlist(&self) -> Result<Vec<WatchItem>, RepositoryError>;
    /// Adding a ticker that is already watched is a no-op.
    fn watch(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError>;
    fn unwatch(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError>;

    fn alerts(&self) -> Result<Vec<Alert>, RepositoryError>;
    fn save_alert(&self, alert: &Alert) -> Result<(), RepositoryError>;
    fn delete_alert(&self, id: Uuid) -> Result<(), RepositoryError>;
    /// Records that the alert fired now; `NotFound` if it doesn't exist.
    fn mark_triggered(&self, id: Uuid) -> Result<(), RepositoryError>;
}
