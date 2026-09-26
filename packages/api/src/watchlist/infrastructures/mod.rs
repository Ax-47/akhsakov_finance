//! SQLite adapter for [`WatchlistRepository`]. Tags are stored one per
//! line.

use crate::{
    database::Database, shared::RepositoryError, watchlist::repositories::WatchlistRepository,
};
use dtos::watch::{Alert, AlertKind, Note, WatchItem, Watchlist};
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
    fn watchlists(&self) -> Result<Vec<Watchlist>, RepositoryError> {
        let (lists, items): (Vec<(String, String)>, Vec<(String, String, String)>) =
            self.db.with(|c| {
                let lists = c
                    .prepare("SELECT id, name FROM watchlists ORDER BY position, name")?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                let items = c
                    .prepare(
                        "SELECT list_id, ticker, added_at FROM watch_items ORDER BY added_at, ticker",
                    )?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok((lists, items))
            })?;
        lists
            .into_iter()
            .map(|(id, name)| {
                Ok(Watchlist {
                    items: items
                        .iter()
                        .filter(|(list, ..)| *list == id)
                        .map(|(_, t, added_at)| {
                            Ok(WatchItem {
                                ticker: TickerSymbol::new(t).map_err(|_| corrupt("ticker", t))?,
                                added_at: added_at.clone(),
                            })
                        })
                        .collect::<Result<_, RepositoryError>>()?,
                    id: Uuid::parse_str(&id).map_err(|_| corrupt("watchlist id", &id))?,
                    name,
                })
            })
            .collect()
    }

    fn create_list(&self, id: Uuid, name: &str) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO watchlists (id, name, position)
                 VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM watchlists))",
                params![id.to_string(), name],
            )
        })?;
        Ok(())
    }

    fn rename_list(&self, id: Uuid, name: &str) -> Result<(), RepositoryError> {
        let changed = self.db.with(|c| {
            c.execute(
                "UPDATE watchlists SET name = ?2 WHERE id = ?1",
                params![id.to_string(), name],
            )
        })?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(format!("watchlist {id}")));
        }
        Ok(())
    }

    fn delete_list(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM watchlists WHERE id = ?1", [id.to_string()]))?;
        Ok(())
    }

    fn watch(&self, list: Uuid, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO watch_items (list_id, ticker) VALUES (?1, ?2)",
                params![list.to_string(), ticker.as_str()],
            )
        })?;
        Ok(())
    }

    fn unwatch(&self, list: Option<Uuid>, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db.with(|c| match list {
            Some(list) => c.execute(
                "DELETE FROM watch_items WHERE list_id = ?1 AND ticker = ?2",
                params![list.to_string(), ticker.as_str()],
            ),
            None => c.execute("DELETE FROM watch_items WHERE ticker = ?1", [ticker.as_str()]),
        })?;
        Ok(())
    }

    fn notes(&self) -> Result<Vec<Note>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.prepare("SELECT ticker, text, tags, updated_at FROM notes ORDER BY ticker")?
                .query_map([], |r| {
                    let tags: String = r.get(2)?;
                    Ok(Note {
                        ticker: r.get(0)?,
                        text: r.get(1)?,
                        tags: tags
                            .split('\n')
                            .filter(|t| !t.is_empty())
                            .map(str::to_string)
                            .collect(),
                        updated_at: r.get(3)?,
                    })
                })?
                .collect()
        })?)
    }

    fn save_note(&self, note: &Note) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO notes (ticker, text, tags, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                params![note.ticker, note.text, note.tags.join("\n")],
            )
        })?;
        Ok(())
    }

    fn delete_note(&self, ticker: &str) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM notes WHERE ticker = ?1", [ticker]))?;
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
