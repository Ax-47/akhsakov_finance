//! Ports for accounts and sessions.

use crate::shared::RepositoryError;
use dtos::auth::User;

pub trait AuthRepository: Send + Sync {
    fn user_count(&self) -> Result<usize, RepositoryError>;
    fn users(&self) -> Result<Vec<User>, RepositoryError>;
    fn password_hash(&self, username: &str) -> Result<Option<String>, RepositoryError>;
    fn add_user(&self, username: &str, password_hash: &str) -> Result<(), RepositoryError>;
    /// `NotFound` if there's no such user.
    fn set_password(&self, username: &str, password_hash: &str) -> Result<(), RepositoryError>;
    /// Also ends the user's sessions.
    fn remove_user(&self, username: &str) -> Result<(), RepositoryError>;

    /// Starts a session lasting `days`.
    fn create_session(&self, token: &str, username: &str, days: u32) -> Result<(), RepositoryError>;
    /// The user of an unexpired session.
    fn session_user(&self, token: &str) -> Result<Option<String>, RepositoryError>;
    fn delete_session(&self, token: &str) -> Result<(), RepositoryError>;
    /// Every session of `username` except `keep`.
    fn delete_other_sessions(&self, username: &str, keep: &str) -> Result<(), RepositoryError>;
}

/// One-way password hashing.
pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: &str) -> Result<String, String>;
    fn verify(&self, password: &str, hash: &str) -> bool;
}
