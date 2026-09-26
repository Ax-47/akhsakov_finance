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
    #[error("file error: {0}")]
    File(String),
    #[error("not a backup of this app: {0}")]
    NotABackup(String),
}

/// A file restored over live data must be an intact database of this app,
/// no newer than this version.
fn check_backup(conn: &Connection) -> Result<(), DatabaseError> {
    let bad = |why: &str| DatabaseError::NotABackup(why.to_string());
    let ok: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|_| bad("the file isn't a SQLite database"))?;
    if ok != "ok" {
        return Err(bad("the file is damaged"));
    }
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 || version as usize > MIGRATIONS.len() {
        return Err(bad("it's from an unknown or newer version"));
    }
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('portfolios', 'transactions')",
        [],
        |r| r.get(0),
    )?;
    if tables != 2 {
        return Err(bad("it has no portfolio tables"));
    }
    Ok(())
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

    /// A consistent copy of the whole database as SQLite file bytes.
    pub fn export(&self) -> Result<Vec<u8>, DatabaseError> {
        let path = std::env::temp_dir().join(format!("akhsakov-export-{}.db", uuid::Uuid::new_v4()));
        self.with(|c| c.execute("VACUUM INTO ?1", [path.to_string_lossy()]))?;
        let bytes = std::fs::read(&path).map_err(|e| DatabaseError::File(e.to_string()));
        let _ = std::fs::remove_file(&path);
        bytes
    }

    /// Replaces every table with the contents of `bytes` (an exported
    /// database), then upgrades it to the current schema. The file is
    /// checked first; on any problem nothing changes.
    pub fn restore(&self, bytes: &[u8]) -> Result<(), DatabaseError> {
        let path = std::env::temp_dir().join(format!("akhsakov-restore-{}.db", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).map_err(|e| DatabaseError::File(e.to_string()))?;
        let result = (|| {
            let source = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            check_backup(&source)?;
            let mut live = self.0.lock().map_err(|_| DatabaseError::Poisoned)?;
            rusqlite::backup::Backup::new(&source, &mut live)?
                .run_to_completion(256, std::time::Duration::ZERO, None)?;
            live.pragma_update(None, "foreign_keys", "ON")?;
            migrate(&live)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&path);
        result
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
    // 3: each trade's currency and its USD rate on the trade date
    "ALTER TABLE transactions ADD COLUMN currency TEXT NOT NULL DEFAULT 'USD';
    ALTER TABLE transactions ADD COLUMN fx_to_usd TEXT NOT NULL DEFAULT '1';",
    // 4: cached index membership
    "CREATE TABLE index_members (
        index_key  TEXT NOT NULL,
        position   INTEGER NOT NULL,
        ticker     TEXT NOT NULL,
        name       TEXT NOT NULL,
        sector     TEXT NOT NULL,
        PRIMARY KEY (index_key, position)
    );
    CREATE TABLE index_fetched (
        index_key   TEXT PRIMARY KEY,
        fetched_at  TEXT NOT NULL
    );",
    // 5: named watchlists (existing items move to the first) and stock notes
    "CREATE TABLE watchlists (
        id        TEXT PRIMARY KEY,
        name      TEXT NOT NULL,
        position  INTEGER NOT NULL
    );
    INSERT INTO watchlists (id, name, position)
        VALUES ('00000000-0000-0000-0000-000000000001', 'Watchlist', 0);
    CREATE TABLE watch_items (
        list_id   TEXT NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
        ticker    TEXT NOT NULL,
        added_at  TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (list_id, ticker)
    );
    INSERT INTO watch_items (list_id, ticker, added_at)
        SELECT '00000000-0000-0000-0000-000000000001', ticker, added_at FROM watchlist;
    DROP TABLE watchlist;
    CREATE TABLE notes (
        ticker      TEXT PRIMARY KEY,
        text        TEXT NOT NULL,
        tags        TEXT NOT NULL DEFAULT '',
        updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // 6: notifications, push channels, and saved prices for offline use
    "CREATE TABLE notifications (
        id          TEXT PRIMARY KEY,
        title       TEXT NOT NULL,
        body        TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (datetime('now')),
        read        INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE notify_config (
        key    TEXT PRIMARY KEY,
        value  TEXT NOT NULL
    );
    CREATE TABLE quote_cache (
        ticker      TEXT PRIMARY KEY,
        json        TEXT NOT NULL,
        updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE chart_cache (
        key         TEXT PRIMARY KEY,
        json        TEXT NOT NULL,
        updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // 7: accounts and sign-in sessions
    "CREATE TABLE users (
        username       TEXT PRIMARY KEY,
        password_hash  TEXT NOT NULL,
        created_at     TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE sessions (
        token       TEXT PRIMARY KEY,
        username    TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
        created_at  TEXT NOT NULL DEFAULT (datetime('now')),
        expires_at  TEXT NOT NULL
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

#[cfg(test)]
mod backup_tests {
    use super::*;

    fn names(db: &Database) -> Vec<String> {
        db.with(|c| c.prepare("SELECT name FROM portfolios")?.query_map([], |r| r.get(0))?.collect())
            .unwrap()
    }

    #[test]
    fn export_then_restore_round_trips() {
        let a = Database::in_memory().unwrap();
        a.with(|c| c.execute("INSERT INTO portfolios (id, name) VALUES ('p', 'Kept')", [])).unwrap();
        let bytes = a.export().unwrap();
        assert!(bytes.starts_with(b"SQLite format 3"));

        let b = Database::in_memory().unwrap();
        b.with(|c| c.execute("INSERT INTO portfolios (id, name) VALUES ('q', 'Replaced')", [])).unwrap();
        b.restore(&bytes).unwrap();
        assert_eq!(names(&b), vec!["Kept".to_string()]);

        assert!(matches!(b.restore(b"not a database"), Err(DatabaseError::NotABackup(_))));
        assert_eq!(names(&b), vec!["Kept".to_string()], "a bad file changes nothing");
    }
}
