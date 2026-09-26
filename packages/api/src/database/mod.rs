//! SQLite storage shared by every repository: one connection, versioned
//! migrations, and a sample portfolio on first run.

use rusqlite::Connection;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

/// Where the database lives unless `AKHSAKOV_DB` says otherwise.
const DEFAULT_PATH: &str = "akhsakov_finance.db";

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database lock poisoned")]
    Poisoned,
}

/// Cheap to clone; all clones share one connection.
#[derive(Clone)]
pub struct Database(Arc<Mutex<Connection>>);

impl Database {
    /// Opens `$AKHSAKOV_DB` (or `akhsakov_finance.db`), migrating and
    /// seeding as needed.
    pub fn open_default() -> Result<Self, DatabaseError> {
        let path = std::env::var("AKHSAKOV_DB").unwrap_or_else(|_| DEFAULT_PATH.to_string());
        Self::open(Path::new(&path))
    }

    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        Self::init(Connection::open(path)?, true)
    }

    /// Empty in-memory database, for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        Self::init(Connection::open_in_memory()?, false)
    }

    fn init(conn: Connection, seed: bool) -> Result<Self, DatabaseError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&conn)?;
        if seed {
            seed_sample_data(&conn)?;
        }
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    /// Runs `f` with the connection.
    pub fn with<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, DatabaseError> {
        let conn = self.0.lock().map_err(|_| DatabaseError::Poisoned)?;
        Ok(f(&conn)?)
    }

    /// Runs `f` inside a transaction, committing only if it succeeds.
    pub fn transaction<T>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction) -> rusqlite::Result<T>,
    ) -> Result<T, DatabaseError> {
        let mut conn = self.0.lock().map_err(|_| DatabaseError::Poisoned)?;
        let tx = conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

/// Each entry upgrades the schema by one version (`PRAGMA user_version`).
const MIGRATIONS: &[&str] = &[
    // 1: portfolios and transactions
    "CREATE TABLE portfolios (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE transactions (
        id            TEXT PRIMARY KEY,
        portfolio_id  TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        ticker        TEXT NOT NULL,
        kind          TEXT NOT NULL,
        shares        TEXT NOT NULL,
        price         TEXT NOT NULL,
        fee           TEXT NOT NULL DEFAULT '0',
        date          TEXT NOT NULL
    );
    CREATE INDEX transactions_portfolio ON transactions(portfolio_id);",
    // 2: watchlist, alerts, settings, target weights, goals
    "CREATE TABLE watchlist (
        ticker    TEXT PRIMARY KEY,
        added_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE alerts (
        id            TEXT PRIMARY KEY,
        ticker        TEXT NOT NULL,
        kind          TEXT NOT NULL,
        value         TEXT NOT NULL,
        created_at    TEXT NOT NULL DEFAULT (datetime('now')),
        triggered_at  TEXT
    );
    CREATE TABLE settings (
        key    TEXT PRIMARY KEY,
        value  TEXT NOT NULL
    );
    CREATE TABLE targets (
        portfolio_id  TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        ticker        TEXT NOT NULL,
        weight        TEXT NOT NULL,
        PRIMARY KEY (portfolio_id, ticker)
    );
    CREATE TABLE goals (
        id            TEXT PRIMARY KEY,
        name          TEXT NOT NULL,
        target        TEXT NOT NULL,
        date          TEXT NOT NULL,
        monthly       TEXT NOT NULL DEFAULT '0',
        portfolio_id  TEXT REFERENCES portfolios(id) ON DELETE CASCADE
    );",
];

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version.max(0) as usize) {
        conn.execute_batch(sql)?;
        conn.pragma_update(None, "user_version", i as i64 + 1)?;
    }
    Ok(())
}

/// The original demo portfolios, so a fresh install has something to show.
fn seed_sample_data(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM portfolios", [], |r| r.get(0))?;
    if count > 0 {
        return Ok(());
    }
    let (p1, p2) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    conn.execute(
        "INSERT INTO portfolios (id, name) VALUES (?1, 'Portfolio1'), (?2, 'Portfolio2')",
        [&p1, &p2],
    )?;
    let trades: [(&str, &str, &str, &str, &str); 9] = [
        (&p1, "NVDA", "0.354565", "215.87", "2026-06-01"),
        (&p1, "NVDA", "0.354565", "215.87", "2026-06-01"),
        (&p1, "NVDA", "0.696269", "219.80", "2026-05-20"),
        (&p1, "AMD", "0.148258", "514.98", "2026-06-04"),
        (&p1, "AMD", "0.371118", "421.00", "2026-05-20"),
        (&p1, "VOO", "0.220027", "695.05", "2026-06-01"),
        (&p1, "AAPL", "0.511797", "298.38", "2026-05-20"),
        (&p1, "TSM", "0.383375", "397.00", "2026-05-20"),
        (&p2, "AMD", "0.0594", "514.98", "2026-05-20"),
    ];
    for (portfolio, ticker, shares, price, date) in trades {
        conn.execute(
            "INSERT INTO transactions (id, portfolio_id, ticker, kind, shares, price, fee, date)
             VALUES (?1, ?2, ?3, 'Buy', ?4, ?5, '0', ?6)",
            [
                &uuid::Uuid::new_v4().to_string(),
                portfolio,
                ticker,
                shares,
                price,
                date,
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_seeds_once() {
        let db = Database::init(Connection::open_in_memory().unwrap(), true).unwrap();
        let count = |table: &str| {
            db.with(|c| {
                c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
            })
            .unwrap()
        };
        assert_eq!(count("portfolios"), 2);
        assert_eq!(count("transactions"), 9);
        // Re-running migrations and seeding is a no-op.
        db.with(|c| {
            migrate(c)?;
            seed_sample_data(c)
        })
        .unwrap();
        assert_eq!(count("portfolios"), 2);
        let version: i64 = db
            .with(|c| c.pragma_query_value(None, "user_version", |r| r.get(0)))
            .unwrap();
        assert_eq!(version as usize, MIGRATIONS.len());
    }
}
