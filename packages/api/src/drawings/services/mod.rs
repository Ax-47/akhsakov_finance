//! Use cases for chart drawings.

use crate::{drawings::repositories::DrawingRepository, shared::ServiceError};
use dtos::drawings::{validate, ChartDrawing, ChartPoint, DrawingKind, MAX_PER_TICKER};
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;

#[derive(Clone)]
pub struct DrawingService {
    repo: Arc<dyn DrawingRepository>,
}

impl DrawingService {
    pub fn new(repo: Arc<dyn DrawingRepository>) -> Self {
        Self { repo }
    }

    pub fn drawings(&self, ticker: &TickerSymbol) -> Result<Vec<ChartDrawing>, ServiceError> {
        Ok(self.repo.drawings(ticker)?)
    }

    /// Saves a drawing and returns it with its id.
    pub fn add(&self, ticker: &TickerSymbol, kind: DrawingKind, points: Vec<ChartPoint>) -> Result<ChartDrawing, ServiceError> {
        validate(kind, &points).map_err(ServiceError::Validation)?;
        if self.repo.drawings(ticker)?.len() >= MAX_PER_TICKER {
            return Err(ServiceError::Validation(format!(
                "A chart keeps up to {MAX_PER_TICKER} drawings; remove some first"
            )));
        }
        let id = self.repo.add(ticker, kind, &points)?;
        Ok(ChartDrawing { id, kind, points })
    }

    pub fn delete(&self, ticker: &TickerSymbol, id: i64) -> Result<(), ServiceError> {
        Ok(self.repo.delete(ticker, id)?)
    }

    pub fn clear(&self, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.clear(ticker)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, drawings::infrastructures::SqliteDrawingRepository};

    #[test]
    fn keeps_drawings_per_stock() {
        let s = DrawingService::new(Arc::new(SqliteDrawingRepository::new(Database::in_memory().unwrap())));
        let (aapl, msft) = (TickerSymbol::new("AAPL").unwrap(), TickerSymbol::new("MSFT").unwrap());
        let p = |ts, price| ChartPoint { ts, price };
        let level = s.add(&aapl, DrawingKind::Level, vec![p(1_700_000_000, 190.5)]).unwrap();
        let fib = s.add(&aapl, DrawingKind::Fibonacci, vec![p(1_700_000_000, 150.0), p(1_705_000_000, 200.0)]).unwrap();
        s.add(&msft, DrawingKind::Trend, vec![p(1, 300.0), p(2, 310.0)]).unwrap();
        assert_eq!(s.drawings(&aapl).unwrap(), vec![level.clone(), fib.clone()]);
        assert!(matches!(s.add(&aapl, DrawingKind::Trend, vec![p(1, 1.0)]), Err(ServiceError::Validation(_))));

        s.delete(&aapl, level.id).unwrap();
        // Another stock's id doesn't delete across stocks.
        s.delete(&msft, fib.id).unwrap();
        assert_eq!(s.drawings(&aapl).unwrap(), vec![fib]);
        s.clear(&aapl).unwrap();
        assert!(s.drawings(&aapl).unwrap().is_empty());
        assert_eq!(s.drawings(&msft).unwrap().len(), 1);
    }
}
