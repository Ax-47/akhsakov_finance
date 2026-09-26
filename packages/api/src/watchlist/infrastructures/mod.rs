//! SQLite adapter for [`WatchlistRepository`].

use crate::{
    database::Database, shared::RepositoryError, watchlist::repositories::WatchlistRepository,
};
use dtos::watch::{Alert, AlertKind, WatchItem};
use rusqlite::params;
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub struct SqliteWatchlistRepository {
    db: Database,
}

impl SqliteWatchlistRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

fn corrupt(what: &str, value: &str) -> RepositoryError {
    RepositoryError::Corrupt(format!("{what} \"{value}\""))
}

impl WatchlistRepository for SqliteWatchlistRepository {
    fn watchlist(&self) -> Result<Vec<WatchItem>, RepositoryError> {
        let rows: Vec<(String, String)> = self.db.with(|c| {
            c.prepare("SELECT ticker, added_at FROM watchlist ORDER BY added_at, ticker")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(t, added_at)| {
                Ok(WatchItem {
                    ticker: TickerSymbol::new(&t).map_err(|_| corrupt("ticker", &t))?,
                    added_at,
                })
            })
            .collect()
    }

    fn watch(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO watchlist (ticker) VALUES (?1)",
                [ticker.as_str()],
            )
        })?;
        Ok(())
    }

    fn unwatch(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM watchlist WHERE ticker = ?1", [ticker.as_str()]))?;
        Ok(())
    }

    fn alerts(&self) -> Result<Vec<Alert>, RepositoryError> {
        type Row = (String, String, String, String, String, Option<String>);
        let rows: Vec<Row> = self.db.with(|c| {
            c.prepare("SELECT id, ticker, kind, value, created_at, triggered_at FROM alerts ORDER BY created_at DESC")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(id, ticker, kind, value, created_at, triggered_at)| {
                Ok(Alert {
                    id: Uuid::parse_str(&id).map_err(|_| corrupt("alert id", &id))?,
                    ticker: TickerSymbol::new(&ticker).map_err(|_| corrupt("ticker", &ticker))?,
                    kind: AlertKind::from_str(&kind).map_err(|_| corrupt("alert kind", &kind))?,
                    value: Decimal::from_str(&value).map_err(|_| corrupt("alert value", &value))?,
                    created_at,
                    triggered_at,
                })
            })
            .collect()
    }

    fn save_alert(&self, alert: &Alert) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO alerts (id, ticker, kind, value, triggered_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![alert.id.to_string(), alert.ticker.as_str(), alert.kind.to_string(), alert.value.to_string(), alert.triggered_at],
            )
        })?;
        Ok(())
    }

    fn delete_alert(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM alerts WHERE id = ?1", [id.to_string()]))?;
        Ok(())
    }

    fn mark_triggered(&self, id: Uuid) -> Result<(), RepositoryError> {
        let changed = self.db.with(|c| {
            c.execute(
                "UPDATE alerts SET triggered_at = datetime('now') WHERE id = ?1 AND triggered_at IS NULL",
                [id.to_string()],
            )
        })?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(format!("active alert {id}")));
        }
        Ok(())
    }
}
