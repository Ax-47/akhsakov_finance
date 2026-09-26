//! Port for whole-database snapshots.

/// Errors are human-readable reasons.
pub trait BackupStore: Send + Sync {
    fn export(&self) -> Result<Vec<u8>, String>;
    /// Replaces all data; must leave it unchanged on failure.
    fn restore(&self, bytes: &[u8]) -> Result<(), String>;
}
