//! Sign-in: whether it's required, and who is signed in.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AuthStatus {
    /// Signing in is needed to use the app (someone set a password, or
    /// the server requires it).
    pub required: bool,
    /// No account exists yet.
    pub needs_setup: bool,
    /// The signed-in user, if any.
    pub user: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub created_at: String,
}

pub const MIN_PASSWORD_LEN: usize = 8;

/// Lowercase letters, digits, `.`, `_` and `-`; 3 to 32 characters.
pub fn validate_username(name: &str) -> Result<(), String> {
    let ok = (3..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err("Usernames are 3–32 lowercase letters, digits, . _ or -".into())
    }
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!("Use at least {MIN_PASSWORD_LEN} characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_names_and_passwords() {
        assert!(validate_username("mikhail").is_ok());
        assert!(validate_username("Mikhail").is_err());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("a b c").is_err());
        assert!(validate_password("short").is_err());
        assert!(validate_password("long enough").is_ok());
    }
}
