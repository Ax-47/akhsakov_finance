//! SQLite adapter for [`KeyRepository`], and the HTTP endpoint.

pub mod http;

use crate::{database::Database, mcp::repositories::KeyRepository, shared::RepositoryError};
use rusqlite::OptionalExtension;

pub struct SqliteKeyRepository {
    db: Database,
}

impl SqliteKeyRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl KeyRepository for SqliteKeyRepository {
    fn key(&self) -> Result<Option<String>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.query_row("SELECT token FROM mcp_access WHERE id = 1", [], |r| r.get(0))
                .optional()
        })?)
    }

    fn set_key(&self, key: Option<&str>) -> Result<(), RepositoryError> {
        self.db.with(|c| match key {
            Some(key) => c.execute(
                "INSERT OR REPLACE INTO mcp_access (id, token, created_at) VALUES (1, ?1, datetime('now'))",
                [key],
            ),
            None => c.execute("DELETE FROM mcp_access", []),
        })?;
        Ok(())
    }
}
