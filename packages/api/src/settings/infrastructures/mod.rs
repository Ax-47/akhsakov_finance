//! SQLite adapter for [`SettingsRepository`].

use crate::{
    database::Database, settings::repositories::SettingsRepository, shared::RepositoryError,
};
use std::collections::HashMap;

pub struct SqliteSettingsRepository {
    db: Database,
}

impl SqliteSettingsRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl SettingsRepository for SqliteSettingsRepository {
    fn all(&self) -> Result<HashMap<String, String>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.prepare("SELECT key, value FROM settings")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?)
    }

    fn set_all(&self, pairs: &[(&str, String)]) -> Result<(), RepositoryError> {
        self.db.transaction(|t| {
            let mut upsert =
                t.prepare("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)")?;
            for (key, value) in pairs {
                upsert.execute([key, &value.as_str()])?;
            }
            Ok(())
        })?;
        Ok(())
    }
}
