//! Storage port for theses and their journals.

use crate::shared::RepositoryError;
use dtos::thesis::{Author, Thesis, ThesisDraft, ThesisEntry};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub trait ThesisRepository: Send + Sync {
    /// Every thesis with its journal (newest first), of one portfolio or
    /// all of them.
    fn theses(&self, portfolio: Option<Uuid>) -> Result<Vec<Thesis>, RepositoryError>;
    /// Inserts or updates the thesis fields, keeping its journal.
    /// `NotFound` if the portfolio doesn't exist.
    fn save(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        draft: &ThesisDraft,
        by: Author,
    ) -> Result<(), RepositoryError>;
    /// Also deletes its journal.
    fn delete(&self, portfolio: Uuid, ticker: &TickerSymbol) -> Result<(), RepositoryError>;
    /// Adds to the journal of an existing thesis.
    fn add_entry(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        entry: &ThesisEntry,
    ) -> Result<(), RepositoryError>;
    fn delete_entry(&self, id: Uuid) -> Result<(), RepositoryError>;
}
