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
    // The app creates no triggers or views; a file carrying them could run
    // its own SQL on every later write (e.g. plant a session).
    let extras: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('trigger', 'view')",
        [],
        |r| r.get(0),
    )?;
    if extras > 0 {
        return Err(bad("it contains triggers or views"));
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
        Self::open(&Self::default_path())
    }

    /// `$AKHSAKOV_DB`, or `akhsakov_finance.db` in the working directory.
    pub fn default_path() -> std::path::PathBuf {
        std::env::var("AKHSAKOV_DB").unwrap_or_else(|_| DEFAULT_PATH.to_string()).into()
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
        // Only a brand-new file gets the samples. Checking for "no
        // portfolios" instead brought them back every time the app started
        // after you'd deleted them all.
        let fresh = conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? == 0;
        migrate(&conn)?;
        if seed && fresh {
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

    /// A consistent copy of the whole database as SQLite file bytes,
    /// without sign-in sessions, connector keys or model API keys (a
    /// backup must not carry live tokens).
    pub fn export(&self) -> Result<Vec<u8>, DatabaseError> {
        let path = std::env::temp_dir().join(format!("akhsakov-export-{}.db", uuid::Uuid::new_v4()));
        let result = (|| {
            self.with(|c| c.execute("VACUUM INTO ?1", [path.to_string_lossy()]))?;
            {
                let copy = Connection::open(&path)?;
                copy.execute("DELETE FROM sessions", [])?;
                copy.execute("DELETE FROM mcp_access", [])?;
                copy.execute("DELETE FROM mcp_connections", [])?;
                copy.execute("UPDATE ai_trader_configs SET mcp_connection_id = NULL", [])?;
                copy.execute("DELETE FROM ai_model_secrets", [])?;
                copy.execute_batch("VACUUM")?;
            }
            std::fs::read(&path).map_err(|e| DatabaseError::File(e.to_string()))
        })();
        let _ = std::fs::remove_file(&path);
        result
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
            let accounts = Accounts::read(&live)?;
            rusqlite::backup::Backup::new(&source, &mut live)?
                .run_to_completion(256, std::time::Duration::ZERO, None)?;
            live.pragma_update(None, "foreign_keys", "ON")?;
            migrate(&live)?;
            accounts.write(&live)?;
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

/// Accounts, sessions and MCP connections of the running server. A
/// restore keeps them: a backup can't remove sign-in (by holding no
/// accounts), add accounts, bring back old sessions or swap the key.
struct Accounts {
    users: Vec<(String, String, String)>,
    sessions: Vec<(String, String, String, String)>,
    mcp_connections: Vec<(String, String, i64, String, String, String, String, Option<String>)>,
    mcp_scopes: Vec<(String, String)>,
    mcp_audit: Vec<(i64, String, String, String, Option<String>, String, i64, Option<String>)>,
}

impl Accounts {
    fn read(conn: &Connection) -> rusqlite::Result<Self> {
        let users = conn
            .prepare("SELECT username, password_hash, created_at FROM users")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let sessions = conn
            .prepare("SELECT token, username, created_at, expires_at FROM sessions")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mcp_connections = conn
            .prepare("SELECT id,name,enabled,preset,token_hash,created_at,updated_at,last_used_at FROM mcp_connections")?
            .query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mcp_scopes = conn.prepare("SELECT connection_id,portfolio_id FROM mcp_connection_portfolios")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mcp_audit = conn.prepare("SELECT id,connection_id,at,method,tool,portfolio_ids,success,error_category FROM mcp_audit")?
            .query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Self {
            users,
            sessions,
            mcp_connections,
            mcp_scopes,
            mcp_audit,
        })
    }

    /// Replaces the restored accounts with these. With no accounts here
    /// (a fresh install), the backup's accounts stay, minus any sessions.
    fn write(&self, conn: &Connection) -> rusqlite::Result<()> {
        conn.execute("DELETE FROM sessions", [])?;
        conn.execute("DELETE FROM mcp_access", [])?;
        conn.execute("DELETE FROM mcp_connections", [])?;
        for row in &self.mcp_connections {
            conn.execute("INSERT INTO mcp_connections(id,name,enabled,preset,token_hash,created_at,updated_at,last_used_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", rusqlite::params![row.0,row.1,row.2,row.3,row.4,row.5,row.6,row.7])?;
        }
        for row in &self.mcp_scopes {
            // A restore can replace the portfolio set. Keep the local
            // credential, but never recreate a scope to a missing portfolio.
            conn.execute("INSERT INTO mcp_connection_portfolios(connection_id,portfolio_id) SELECT ?1,?2 WHERE EXISTS(SELECT 1 FROM portfolios WHERE id=?2)", rusqlite::params![row.0,row.1])?;
        }
        conn.execute("UPDATE mcp_connections SET enabled=0 WHERE NOT EXISTS(SELECT 1 FROM mcp_connection_portfolios s WHERE s.connection_id=mcp_connections.id)", [])?;
        for row in &self.mcp_audit {
            conn.execute("INSERT INTO mcp_audit(id,connection_id,at,method,tool,portfolio_ids,success,error_category) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", rusqlite::params![row.0,row.1,row.2,row.3,row.4,row.5,row.6,row.7])?;
        }
        if self.users.is_empty() {
            return Ok(());
        }
        conn.execute("DELETE FROM users", [])?;
        for (name, hash, created) in &self.users {
            conn.execute(
                "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
                [name, hash, created],
            )?;
        }
        for (token, name, created, expires) in &self.sessions {
            conn.execute(
                "INSERT INTO sessions (token, username, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                [token, name, created, expires],
            )?;
        }
        Ok(())
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
    // 8: highest value seen by portfolio drawdown alerts
    "ALTER TABLE alerts ADD COLUMN peak TEXT;",
    // 9: a thesis per holding of each portfolio, its journal, and the key
    // that lets an AI assistant in through the MCP connector
    "CREATE TABLE theses (
        portfolio_id  TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        ticker        TEXT NOT NULL,
        thesis        TEXT NOT NULL DEFAULT '',
        exit_if       TEXT NOT NULL DEFAULT '',
        target_price  TEXT,
        conviction    INTEGER,
        review_on     TEXT,
        status        TEXT NOT NULL DEFAULT 'OnTrack',
        updated_by    TEXT NOT NULL DEFAULT 'You',
        updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (portfolio_id, ticker)
    );
    CREATE TABLE thesis_log (
        id            TEXT PRIMARY KEY,
        portfolio_id  TEXT NOT NULL,
        ticker        TEXT NOT NULL,
        author        TEXT NOT NULL,
        text          TEXT NOT NULL,
        created_at    TEXT NOT NULL DEFAULT (datetime('now')),
        FOREIGN KEY (portfolio_id, ticker) REFERENCES theses(portfolio_id, ticker) ON DELETE CASCADE
    );
    CREATE INDEX thesis_log_holding ON thesis_log(portfolio_id, ticker);
    CREATE TABLE mcp_access (
        id          INTEGER PRIMARY KEY CHECK (id = 1),
        token       TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // 10: what each asset is (fund, gold, deposit …), prices entered by
    // hand for assets Yahoo doesn't price, and Thai tax-saving wrappers
    "CREATE TABLE assets (
        ticker    TEXT PRIMARY KEY,
        class     TEXT NOT NULL DEFAULT 'stock',
        name      TEXT NOT NULL DEFAULT '',
        currency  TEXT NOT NULL DEFAULT 'USD',
        manual    INTEGER NOT NULL DEFAULT 0,
        wrapper   TEXT
    );
    CREATE TABLE manual_prices (
        ticker  TEXT NOT NULL,
        date    TEXT NOT NULL,
        price   TEXT NOT NULL,
        PRIMARY KEY (ticker, date)
    );",
    // 11: monthly investment (DCA) plans
    "CREATE TABLE dca_plans (
        id             TEXT PRIMARY KEY,
        portfolio_id   TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        ticker         TEXT NOT NULL,
        amount         TEXT NOT NULL,
        currency       TEXT NOT NULL DEFAULT 'USD',
        day            INTEGER NOT NULL,
        active         INTEGER NOT NULL DEFAULT 1,
        start          TEXT NOT NULL,
        last_done      TEXT,
        last_reminded  TEXT
    );",
    // 12: AI-managed portfolios, each with its own paper money
    "CREATE TABLE ai_portfolios (
        portfolio_id  TEXT PRIMARY KEY REFERENCES portfolios(id) ON DELETE CASCADE,
        created_at    TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // 13: provider-neutral model connections, trader configuration and run audit
    "CREATE TABLE ai_model_profiles (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        base_url    TEXT NOT NULL,
        model       TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE ai_model_secrets (
        profile_id  TEXT PRIMARY KEY REFERENCES ai_model_profiles(id) ON DELETE CASCADE,
        api_key     TEXT NOT NULL
    );
    CREATE TABLE ai_trader_configs (
        portfolio_id  TEXT PRIMARY KEY REFERENCES ai_portfolios(portfolio_id) ON DELETE CASCADE,
        profile_id    TEXT REFERENCES ai_model_profiles(id) ON DELETE SET NULL,
        strategy      TEXT NOT NULL
    );
    CREATE TABLE ai_runs (
        id                 TEXT PRIMARY KEY,
        portfolio_id       TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        status             TEXT NOT NULL,
        profile_name       TEXT NOT NULL,
        model              TEXT NOT NULL,
        started_at         TEXT NOT NULL DEFAULT (datetime('now')),
        finished_at        TEXT,
        final_response     TEXT,
        error              TEXT,
        prompt_tokens      INTEGER,
        completion_tokens  INTEGER,
        total_tokens       INTEGER
    );
    CREATE UNIQUE INDEX ai_runs_one_active ON ai_runs(portfolio_id) WHERE status = 'running';
    CREATE TABLE ai_run_events (
        run_id      TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
        sequence    INTEGER NOT NULL,
        tool        TEXT NOT NULL,
        arguments   TEXT NOT NULL,
        success     INTEGER NOT NULL,
        detail      TEXT NOT NULL,
        PRIMARY KEY (run_id, sequence)
    );",
    // 14: durable, portfolio-scoped trader memory and context budgets
    "ALTER TABLE ai_trader_configs ADD COLUMN memory_char_limit INTEGER NOT NULL DEFAULT 8000;
    ALTER TABLE ai_trader_configs ADD COLUMN context_token_limit INTEGER NOT NULL DEFAULT 32768;
    CREATE TABLE ai_trader_memory (
        portfolio_id          TEXT PRIMARY KEY REFERENCES ai_portfolios(portfolio_id) ON DELETE CASCADE,
        decision_summary      TEXT NOT NULL DEFAULT '',
        unresolved_questions  TEXT NOT NULL DEFAULT '',
        updated_at            TEXT NOT NULL DEFAULT (datetime('now')),
        source_run_id          TEXT REFERENCES ai_runs(id) ON DELETE SET NULL
    );",
    // 15: synchronized, auditable competitions between AI portfolios
    "CREATE TABLE ai_races (
        id                         TEXT PRIMARY KEY,
        name                       TEXT NOT NULL,
        status                     TEXT NOT NULL DEFAULT 'draft',
        starting_capital           REAL NOT NULL,
        rounds                     INTEGER NOT NULL,
        completed_rounds           INTEGER NOT NULL DEFAULT 0,
        trading_frequency_minutes  INTEGER NOT NULL,
        round_timeout_seconds      INTEGER NOT NULL,
        created_at                 TEXT NOT NULL DEFAULT (datetime('now')),
        started_at                 TEXT,
        finished_at                TEXT
    );
    CREATE TABLE ai_race_contestants (
        race_id       TEXT NOT NULL REFERENCES ai_races(id) ON DELETE CASCADE,
        portfolio_id  TEXT NOT NULL REFERENCES ai_portfolios(portfolio_id) ON DELETE RESTRICT,
        PRIMARY KEY (race_id, portfolio_id)
    );
    CREATE TABLE ai_race_rounds (
        race_id          TEXT NOT NULL REFERENCES ai_races(id) ON DELETE CASCADE,
        round_number     INTEGER NOT NULL,
        status           TEXT NOT NULL,
        snapshot_at      TEXT NOT NULL DEFAULT (datetime('now')),
        market_snapshot  TEXT NOT NULL DEFAULT '{}',
        started_at       TEXT NOT NULL DEFAULT (datetime('now')),
        finished_at      TEXT,
        PRIMARY KEY (race_id, round_number)
    );
    CREATE TABLE ai_race_metrics (
        race_id             TEXT NOT NULL REFERENCES ai_races(id) ON DELETE CASCADE,
        round_number        INTEGER NOT NULL,
        portfolio_id        TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        portfolio_value     REAL NOT NULL,
        cash                REAL NOT NULL,
        fees                REAL NOT NULL,
        turnover            REAL NOT NULL,
        failed_model_runs   INTEGER NOT NULL,
        PRIMARY KEY (race_id, round_number, portfolio_id)
    );
    CREATE TABLE ai_race_audit (
        race_id   TEXT NOT NULL REFERENCES ai_races(id) ON DELETE CASCADE,
        sequence  INTEGER NOT NULL,
        at        TEXT NOT NULL DEFAULT (datetime('now')),
        kind      TEXT NOT NULL,
        detail    TEXT NOT NULL,
        PRIMARY KEY (race_id, sequence)
    );
    ALTER TABLE ai_runs ADD COLUMN race_id TEXT REFERENCES ai_races(id) ON DELETE SET NULL;
    ALTER TABLE ai_runs ADD COLUMN race_round INTEGER;
    CREATE INDEX ai_runs_race ON ai_runs(race_id, race_round);",
    // 16: named, independently scoped MCP connections. The old plaintext
    // global key is deliberately invalidated during this migration.
    "DELETE FROM mcp_access;
    CREATE TABLE mcp_connections (
        id            TEXT PRIMARY KEY,
        name          TEXT NOT NULL,
        enabled       INTEGER NOT NULL DEFAULT 1,
        preset        TEXT NOT NULL CHECK(preset IN ('read_only','thesis_editor','trader')),
        token_hash    TEXT NOT NULL,
        created_at    TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
        last_used_at  TEXT
    );
    CREATE TABLE mcp_connection_portfolios (
        connection_id TEXT NOT NULL REFERENCES mcp_connections(id) ON DELETE CASCADE,
        portfolio_id  TEXT NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
        PRIMARY KEY(connection_id, portfolio_id)
    );
    CREATE TABLE mcp_audit (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        connection_id   TEXT NOT NULL REFERENCES mcp_connections(id) ON DELETE CASCADE,
        at              TEXT NOT NULL DEFAULT (datetime('now')),
        method          TEXT NOT NULL,
        tool            TEXT,
        portfolio_ids   TEXT NOT NULL DEFAULT '[]',
        success         INTEGER NOT NULL,
        error_category  TEXT
    );
    CREATE INDEX mcp_audit_connection ON mcp_audit(connection_id, id DESC);",
    // 17: an AI portfolio can be driven by an MCP connection instead of a
    // model profile.
    "ALTER TABLE ai_trader_configs ADD COLUMN mcp_connection_id TEXT REFERENCES mcp_connections(id) ON DELETE SET NULL;",
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
/// Called only for a database that has just been created.
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

    #[test]
    fn deleted_samples_stay_deleted() {
        let path = std::env::temp_dir().join(format!("akhsakov-seed-{}.db", uuid::Uuid::new_v4()));
        let portfolios = |db: &Database| {
            db.with(|c| c.query_row("SELECT COUNT(*) FROM portfolios", [], |r| r.get::<_, i64>(0)))
                .unwrap()
        };
        let db = Database::open(&path).unwrap();
        assert_eq!(portfolios(&db), 2, "a new file gets the samples");
        db.with(|c| c.execute("DELETE FROM portfolios", [])).unwrap();
        drop(db);

        let reopened = Database::open(&path).unwrap();
        assert_eq!(portfolios(&reopened), 0, "reopening doesn't bring them back");
        drop(reopened);
        let _ = std::fs::remove_file(&path);
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

    fn count(db: &Database, table: &str) -> i64 {
        db.with(|c| c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)))
            .unwrap()
    }

    fn add_account(db: &Database, name: &str, token: &str) {
        db.with(|c| {
            c.execute("INSERT INTO users (username, password_hash) VALUES (?1, 'h')", [name])?;
            c.execute(
                "INSERT INTO sessions (token, username, expires_at) VALUES (?1, ?2, datetime('now', '+1 day'))",
                [token, name],
            )
        })
        .unwrap();
    }

    #[test]
    fn backups_carry_no_sessions_and_restores_keep_accounts() {
        let a = Database::in_memory().unwrap();
        add_account(&a, "old", "old-token");
        let bytes = a.export().unwrap();
        assert!(!bytes.windows(9).any(|w| w == b"old-token"), "no session tokens in a backup");

        // A server with accounts keeps exactly its own after a restore.
        let b = Database::in_memory().unwrap();
        add_account(&b, "owner", "owner-token");
        b.restore(&bytes).unwrap();
        let users: Vec<String> = b
            .with(|c| c.prepare("SELECT username FROM users")?.query_map([], |r| r.get(0))?.collect())
            .unwrap();
        assert_eq!(users, vec!["owner".to_string()]);
        assert_eq!(count(&b, "sessions"), 1);

        // Restoring a backup made before any account can't open the app up.
        let empty = Database::in_memory().unwrap().export().unwrap();
        b.restore(&empty).unwrap();
        assert_eq!(count(&b, "users"), 1);

        // A fresh install takes the backup's accounts, but no sessions.
        let fresh = Database::in_memory().unwrap();
        fresh.restore(&bytes).unwrap();
        assert_eq!((count(&fresh, "users"), count(&fresh, "sessions")), (1, 0));
    }

    #[test]
    fn backups_carry_no_connector_credentials_and_restores_keep_local_connections() {
        let add = |db: &Database, id: &str, hash: &str| db.with(|c| c.execute("INSERT INTO mcp_connections(id,name,preset,token_hash) VALUES(?1,'local','read_only',?2)", rusqlite::params![id,hash])).unwrap();
        let count_connections=|db:&Database| count(db,"mcp_connections");
        let a = Database::in_memory().unwrap();
        add(&a,"11111111-1111-1111-1111-111111111111","secret-hash-in-backup-source");
        let bytes = a.export().unwrap();
        assert!(!bytes.windows(28).any(|w| w == b"secret-hash-in-backup-source"));

        let b = Database::in_memory().unwrap();
        add(&b,"22222222-2222-2222-2222-222222222222","live-local-hash");
        b.restore(&bytes).unwrap();
        assert_eq!(count_connections(&b),1);

        let c = Database::in_memory().unwrap();
        c.restore(&bytes).unwrap();
        assert_eq!(count_connections(&c),0);
    }

    #[test]
    fn rejects_backups_with_triggers() {
        let a = Database::in_memory().unwrap();
        a.with(|c| {
            c.execute_batch(
                "CREATE TRIGGER t AFTER INSERT ON portfolios BEGIN DELETE FROM portfolios; END;",
            )
        })
        .unwrap();
        let bytes = a.export().unwrap();
        let b = Database::in_memory().unwrap();
        assert!(matches!(b.restore(&bytes), Err(DatabaseError::NotABackup(_))));
    }
}
