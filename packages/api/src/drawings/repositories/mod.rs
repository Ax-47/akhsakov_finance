//! Storage port for chart drawings.

use crate::shared::RepositoryError;
use dtos::drawings::{ChartDrawing, ChartPoint, DrawingKind};
use types::ticker_symbol::TickerSymbol;

pub trait DrawingRepository: Send + Sync {
    /// Oldest first.
    fn drawings(&self, ticker: &TickerSymbol) -> Result<Vec<ChartDrawing>, RepositoryError>;
    /// The new drawing's id.
    fn add(&self, ticker: &TickerSymbol, kind: DrawingKind, points: &[ChartPoint]) -> Result<i64, RepositoryError>;
    fn delete(&self, ticker: &TickerSymbol, id: i64) -> Result<(), RepositoryError>;
    fn clear(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError>;
}
