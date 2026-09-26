//! SQLite adapter for [`BackupStore`]: the database file itself.

use crate::{backup::repositories::BackupStore, database::{Database, DatabaseError}};

pub struct SqliteBackupStore(pub Database);

impl BackupStore for SqliteBackupStore {
    fn export(&self) -> Result<Vec<u8>, String> {
        self.0.export().map_err(|e| e.to_string())
    }

    fn restore(&self, bytes: &[u8]) -> Result<(), String> {
        self.0.restore(bytes).map_err(|e| match e {
            DatabaseError::NotABackup(why) => format!("That file isn't a backup of this app: {why}"),
            e => e.to_string(),
        })
    }
}
