//! Adapters: SQLite accounts / sessions, Argon2 password hashing, and the
//! HTTP middleware that enforces sign-in.

pub mod middleware;

use crate::{
    auth::repositories::{AuthRepository, PasswordHasher},
    database::Database,
    shared::RepositoryError,
};
use argon2::{
    password_hash::{PasswordHash, SaltString},
    Argon2, PasswordHasher as _, PasswordVerifier as _,
};
use dtos::auth::User;
use rusqlite::{params, OptionalExtension};

pub struct SqliteAuthRepository {
    db: Database,
}

impl SqliteAuthRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl AuthRepository for SqliteAuthRepository {
    fn user_count(&self) -> Result<usize, RepositoryError> {
        let n: i64 = self
            .db
            .with(|c| c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)))?;
        Ok(n as usize)
    }

    fn users(&self) -> Result<Vec<User>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.prepare("SELECT username, created_at FROM users ORDER BY created_at, username")?
                .query_map([], |r| {
                    Ok(User {
                        username: r.get(0)?,
                        created_at: r.get(1)?,
                    })
                })?
                .collect()
        })?)
    }

    fn password_hash(&self, username: &str) -> Result<Option<String>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                [username],
                |r| r.get(0),
            )
            .optional()
        })?)
    }

    fn add_user(&self, username: &str, password_hash: &str) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
                params![username, password_hash],
            )
        })?;
        Ok(())
    }

    fn set_password(&self, username: &str, password_hash: &str) -> Result<(), RepositoryError> {
        let changed = self.db.with(|c| {
            c.execute(
                "UPDATE users SET password_hash = ?2 WHERE username = ?1",
                params![username, password_hash],
            )
        })?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(format!("user {username}")));
        }
        Ok(())
    }

    fn remove_user(&self, username: &str) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM users WHERE username = ?1", [username]))?;
        Ok(())
    }

    fn create_session(&self, token: &str, username: &str, days: u32) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO sessions (token, username, expires_at)
                 VALUES (?1, ?2, datetime('now', ?3))",
                params![token, username, format!("+{days} days")],
            )
        })?;
        Ok(())
    }

    fn session_user(&self, token: &str) -> Result<Option<String>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.query_row(
                "SELECT username FROM sessions WHERE token = ?1 AND expires_at > datetime('now')",
                [token],
                |r| r.get(0),
            )
            .optional()
        })?)
    }

    fn delete_session(&self, token: &str) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM sessions WHERE token = ?1", [token]))?;
        Ok(())
    }

    fn delete_other_sessions(&self, username: &str, keep: &str) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "DELETE FROM sessions WHERE username = ?1 AND token <> ?2",
                params![username, keep],
            )
        })?;
        Ok(())
    }
}

/// Argon2id with a random salt per password.
pub struct Argon2Hasher;

impl PasswordHasher for Argon2Hasher {
    fn hash(&self, password: &str) -> Result<String, String> {
        // 16 random bytes (uuid v4 draws them from the OS).
        let salt = SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes()).map_err(|e| e.to_string())?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| e.to_string())
    }

    fn verify(&self, password: &str, hash: &str) -> bool {
        PasswordHash::new(hash)
            .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_verify_and_differ_per_salt() {
        let h = Argon2Hasher;
        let (a, b) = (h.hash("correct horse").unwrap(), h.hash("correct horse").unwrap());
        assert_ne!(a, b);
        assert!(h.verify("correct horse", &a));
        assert!(!h.verify("wrong", &a));
        assert!(!h.verify("x", "not a hash"));
    }
}
