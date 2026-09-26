//! Storage port for the connector's key.

use crate::shared::RepositoryError;

pub trait KeyRepository: Send + Sync {
    /// `None` while the connector is off.
    fn key(&self) -> Result<Option<String>, RepositoryError>;
    /// Replaces the key; `None` turns the connector off.
    fn set_key(&self, key: Option<&str>) -> Result<(), RepositoryError>;
}
