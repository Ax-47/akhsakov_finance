//! Storage port for settings, as key / value pairs.

use crate::shared::RepositoryError;
use std::collections::HashMap;

pub trait SettingsRepository: Send + Sync {
    fn all(&self) -> Result<HashMap<String, String>, RepositoryError>;
    /// Upserts every pair in one transaction.
    fn set_all(&self, pairs: &[(&str, String)]) -> Result<(), RepositoryError>;
}
