//! SQLite adapter for [`DrawingRepository`].

use crate::{database::Database, drawings::repositories::DrawingRepository, shared::RepositoryError};
use dtos::drawings::{ChartDrawing, ChartPoint, DrawingKind};
use rusqlite::params;
use types::ticker_symbol::TickerSymbol;

pub struct SqliteDrawingRepository {
    db: Database,
}

impl SqliteDrawingRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl DrawingRepository for SqliteDrawingRepository {
    fn drawings(&self, ticker: &TickerSymbol) -> Result<Vec<ChartDrawing>, RepositoryError> {
        let rows: Vec<(i64, String, String)> = self.db.with(|c| {
            c.prepare("SELECT id, kind, points FROM chart_drawings WHERE ticker = ?1 ORDER BY id")?
                .query_map([ticker.as_str()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(id, kind, points)| {
                let corrupt = |what: &str| RepositoryError::Corrupt(format!("drawing {id}: {what}"));
                Ok(ChartDrawing {
                    id,
                    kind: DrawingKind::from_key(&kind).ok_or_else(|| corrupt("kind"))?,
                    points: serde_json::from_str(&points).map_err(|_| corrupt("points"))?,
                })
            })
            .collect()
    }

    fn add(&self, ticker: &TickerSymbol, kind: DrawingKind, points: &[ChartPoint]) -> Result<i64, RepositoryError> {
        let json = serde_json::to_string(points).map_err(|e| RepositoryError::Corrupt(e.to_string()))?;
        Ok(self.db.with(|c| {
            c.execute(
                "INSERT INTO chart_drawings (ticker, kind, points) VALUES (?1, ?2, ?3)",
                params![ticker.as_str(), kind.key(), json],
            )?;
            Ok(c.last_insert_rowid())
        })?)
    }

    fn delete(&self, ticker: &TickerSymbol, id: i64) -> Result<(), RepositoryError> {
        self.db.with(|c| c.execute("DELETE FROM chart_drawings WHERE ticker = ?1 AND id = ?2", params![ticker.as_str(), id]))?;
        Ok(())
    }

    fn clear(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db.with(|c| c.execute("DELETE FROM chart_drawings WHERE ticker = ?1", [ticker.as_str()]))?;
        Ok(())
    }
}
