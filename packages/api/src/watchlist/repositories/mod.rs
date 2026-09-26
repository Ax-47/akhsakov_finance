//! Storage port for watchlists, notes and alerts.

use crate::shared::RepositoryError;
use dtos::watch::{Alert, Note, Watchlist};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub trait WatchlistRepository: Send + Sync {
    /// Every list with its items, in list order.
    fn watchlists(&self) -> Result<Vec<Watchlist>, RepositoryError>;
    /// Appended after the existing lists.
    fn create_list(&self, id: Uuid, name: &str) -> Result<(), RepositoryError>;
    /// `NotFound` if no list has `id`.
    fn rename_list(&self, id: Uuid, name: &str) -> Result<(), RepositoryError>;
    /// Also removes its items.
    fn delete_list(&self, id: Uuid) -> Result<(), RepositoryError>;
    /// Adding a ticker a list already has is a no-op.
    fn watch(&self, list: Uuid, ticker: &TickerSymbol) -> Result<(), RepositoryError>;
    /// From one list, or every list for `None`.
    fn unwatch(&self, list: Option<Uuid>, ticker: &TickerSymbol) -> Result<(), RepositoryError>;

    fn notes(&self) -> Result<Vec<Note>, RepositoryError>;
    /// Inserts or replaces the note for `note.ticker`.
    fn save_note(&self, note: &Note) -> Result<(), RepositoryError>;
    fn delete_note(&self, ticker: &str) -> Result<(), RepositoryError>;

    fn alerts(&self) -> Result<Vec<Alert>, RepositoryError>;
    fn save_alert(&self, alert: &Alert) -> Result<(), RepositoryError>;
    fn delete_alert(&self, id: Uuid) -> Result<(), RepositoryError>;
    /// Records that the alert fired now; `NotFound` if it doesn't exist.
    fn mark_triggered(&self, id: Uuid) -> Result<(), RepositoryError>;
}
