//! SQLite adapter for [`ThesisRepository`].

use crate::{database::Database, shared::RepositoryError, thesis::repositories::ThesisRepository};
use dtos::thesis::{Author, Thesis, ThesisDraft, ThesisEntry, ThesisStatus};
use rusqlite::params;
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub struct SqliteThesisRepository {
    db: Database,
}

impl SqliteThesisRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

fn corrupt(what: &str, value: &str) -> RepositoryError {
    RepositoryError::Corrupt(format!("{what} \"{value}\""))
}

struct ThesisRow {
    portfolio: String,
    ticker: String,
    thesis: String,
    exit_if: String,
    target_price: Option<String>,
    conviction: Option<i64>,
    review_on: Option<String>,
    status: String,
    updated_by: String,
    updated_at: String,
}

struct EntryRow {
    id: String,
    portfolio: String,
    ticker: String,
    author: String,
    text: String,
    created_at: String,
}

impl ThesisRepository for SqliteThesisRepository {
    fn theses(&self, portfolio: Option<Uuid>) -> Result<Vec<Thesis>, RepositoryError> {
        let scope = portfolio.map(|p| p.to_string());
        let (rows, entries): (Vec<ThesisRow>, Vec<EntryRow>) = self.db.with(|c| {
            let rows = c
                .prepare(
                    "SELECT portfolio_id, ticker, thesis, exit_if, target_price, conviction,
                            review_on, status, updated_by, updated_at
                     FROM theses WHERE ?1 IS NULL OR portfolio_id = ?1 ORDER BY ticker",
                )?
                .query_map([&scope], |r| {
                    Ok(ThesisRow {
                        portfolio: r.get(0)?,
                        ticker: r.get(1)?,
                        thesis: r.get(2)?,
                        exit_if: r.get(3)?,
                        target_price: r.get(4)?,
                        conviction: r.get(5)?,
                        review_on: r.get(6)?,
                        status: r.get(7)?,
                        updated_by: r.get(8)?,
                        updated_at: r.get(9)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?;
            let entries = c
                .prepare(
                    "SELECT id, portfolio_id, ticker, author, text, created_at
                     FROM thesis_log WHERE ?1 IS NULL OR portfolio_id = ?1
                     ORDER BY created_at DESC, rowid DESC",
                )?
                .query_map([&scope], |r| {
                    Ok(EntryRow {
                        id: r.get(0)?,
                        portfolio: r.get(1)?,
                        ticker: r.get(2)?,
                        author: r.get(3)?,
                        text: r.get(4)?,
                        created_at: r.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?;
            Ok((rows, entries))
        })?;
        rows.into_iter()
            .map(|row| {
                let log = entries
                    .iter()
                    .filter(|e| e.portfolio == row.portfolio && e.ticker == row.ticker)
                    .map(|e| {
                        Ok(ThesisEntry {
                            id: Uuid::parse_str(&e.id).map_err(|_| corrupt("journal id", &e.id))?,
                            author: Author::from_str(&e.author).map_err(|_| corrupt("author", &e.author))?,
                            text: e.text.clone(),
                            created_at: e.created_at.clone(),
                        })
                    })
                    .collect::<Result<_, RepositoryError>>()?;
                let target_price = row
                    .target_price
                    .as_deref()
                    .map(|p| Decimal::from_str(p).map_err(|_| corrupt("target price", p)))
                    .transpose()?;
                Ok(Thesis {
                    portfolio_id: Uuid::parse_str(&row.portfolio)
                        .map_err(|_| corrupt("portfolio id", &row.portfolio))?,
                    ticker: TickerSymbol::new(&row.ticker).map_err(|_| corrupt("ticker", &row.ticker))?,
                    draft: ThesisDraft {
                        thesis: row.thesis,
                        exit_if: row.exit_if,
                        target_price,
                        conviction: row.conviction.and_then(|c| u8::try_from(c).ok()),
                        review_on: row.review_on,
                        status: ThesisStatus::from_str(&row.status)
                            .map_err(|_| corrupt("thesis status", &row.status))?,
                    },
                    updated_by: Author::from_str(&row.updated_by)
                        .map_err(|_| corrupt("author", &row.updated_by))?,
                    updated_at: row.updated_at,
                    log,
                })
            })
            .collect()
    }

    fn save(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        draft: &ThesisDraft,
        by: Author,
    ) -> Result<(), RepositoryError> {
        let saved = self.db.with(|c| {
            let exists: bool = c.query_row(
                "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = ?1)",
                [portfolio.to_string()],
                |r| r.get(0),
            )?;
            if !exists {
                return Ok(false);
            }
            // An upsert, not INSERT OR REPLACE: replacing the row would
            // delete its journal through the foreign key.
            c.execute(
                "INSERT INTO theses (portfolio_id, ticker, thesis, exit_if, target_price, conviction,
                                     review_on, status, updated_by, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
                 ON CONFLICT (portfolio_id, ticker) DO UPDATE SET
                     thesis = excluded.thesis, exit_if = excluded.exit_if,
                     target_price = excluded.target_price, conviction = excluded.conviction,
                     review_on = excluded.review_on, status = excluded.status,
                     updated_by = excluded.updated_by, updated_at = excluded.updated_at",
                params![
                    portfolio.to_string(),
                    ticker.as_str(),
                    draft.thesis,
                    draft.exit_if,
                    draft.target_price.map(|p| p.to_string()),
                    draft.conviction,
                    draft.review_on,
                    draft.status.to_string(),
                    by.to_string(),
                ],
            )?;
            Ok(true)
        })?;
        if !saved {
            return Err(RepositoryError::NotFound(format!("portfolio {portfolio}")));
        }
        Ok(())
    }

    fn delete(&self, portfolio: Uuid, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "DELETE FROM theses WHERE portfolio_id = ?1 AND ticker = ?2",
                params![portfolio.to_string(), ticker.as_str()],
            )
        })?;
        Ok(())
    }

    fn add_entry(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        entry: &ThesisEntry,
    ) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO thesis_log (id, portfolio_id, ticker, author, text) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    entry.id.to_string(),
                    portfolio.to_string(),
                    ticker.as_str(),
                    entry.author.to_string(),
                    entry.text,
                ],
            )
        })?;
        Ok(())
    }

    fn delete_entry(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM thesis_log WHERE id = ?1", [id.to_string()]))?;
        Ok(())
    }
}
