//! Backup use cases: download everything as one file, restore it later or
//! on another machine. Files travel base64-encoded.

use crate::{backup::repositories::BackupStore, shared::ServiceError};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::Arc;

/// Larger uploads are refused.
const MAX_BYTES: usize = 200 * 1024 * 1024;

#[derive(Clone)]
pub struct BackupService {
    store: Arc<dyn BackupStore>,
}

impl BackupService {
    pub fn new(store: Arc<dyn BackupStore>) -> Self {
        Self { store }
    }

    pub fn export(&self) -> Result<String, ServiceError> {
        let bytes = self.store.export().map_err(ServiceError::Storage)?;
        Ok(STANDARD.encode(bytes))
    }

    pub fn restore(&self, data: &str) -> Result<(), ServiceError> {
        if data.len() > MAX_BYTES / 3 * 4 {
            return Err(ServiceError::Validation("That file is too large".into()));
        }
        let bytes = STANDARD
            .decode(data.trim())
            .map_err(|_| ServiceError::Validation("The upload was corrupted".into()))?;
        self.store.restore(&bytes).map_err(ServiceError::Validation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backup::infrastructures::SqliteBackupStore, database::Database};

    #[test]
    fn round_trips_through_base64() {
        let db = Database::in_memory().unwrap();
        let s = BackupService::new(Arc::new(SqliteBackupStore(db)));
        let data = s.export().unwrap();
        s.restore(&data).unwrap();
        assert!(matches!(s.restore("!!!"), Err(ServiceError::Validation(_))));
        assert!(matches!(
            s.restore(&STANDARD.encode(b"hello")),
            Err(ServiceError::Validation(m)) if m.contains("isn't a backup")
        ));
    }
}
