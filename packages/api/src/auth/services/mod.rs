//! Accounts and sign-in. Until someone creates an account the app is open
//! (a personal, local install); after that every API call needs a session,
//! so the server can be reached from other devices safely. Setting
//! `AKHSAKOV_REQUIRE_LOGIN=1` requires an account from the start.

use crate::{
    auth::repositories::{AuthRepository, PasswordHasher},
    shared::ServiceError,
};
use dtos::auth::{validate_password, validate_username, AuthStatus, User};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

const SESSION_DAYS: u32 = 30;
/// Slows password guessing.
const FAILED_LOGIN_DELAY: Duration = Duration::from_millis(600);

#[derive(Clone)]
pub struct AuthService {
    repo: Arc<dyn AuthRepository>,
    hasher: Arc<dyn PasswordHasher>,
    always_required: bool,
}

/// 244 random bits, hex.
fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn invalid(msg: String) -> ServiceError {
    ServiceError::Validation(msg)
}

impl AuthService {
    pub fn new(repo: Arc<dyn AuthRepository>, hasher: Arc<dyn PasswordHasher>, always_required: bool) -> Self {
        Self {
            repo,
            hasher,
            always_required,
        }
    }

    /// Whether API calls need a session.
    pub fn required(&self) -> Result<bool, ServiceError> {
        Ok(self.always_required || self.repo.user_count()? > 0)
    }

    pub fn user_for(&self, token: Option<&str>) -> Result<Option<String>, ServiceError> {
        match token.filter(|t| !t.is_empty()) {
            Some(t) => Ok(self.repo.session_user(t)?),
            None => Ok(None),
        }
    }

    pub fn status(&self, token: Option<&str>) -> Result<AuthStatus, ServiceError> {
        Ok(AuthStatus {
            required: self.required()?,
            needs_setup: self.repo.user_count()? == 0,
            user: self.user_for(token)?,
        })
    }

    /// Creates the first account and signs it in; returns the session token.
    pub fn setup(&self, username: &str, password: &str) -> Result<String, ServiceError> {
        if self.repo.user_count()? > 0 {
            return Err(invalid("An account already exists; sign in instead".into()));
        }
        self.create_user(username, password)?;
        self.start_session(username.trim())
    }

    /// Returns a session token.
    pub async fn login(&self, username: &str, password: &str) -> Result<String, ServiceError> {
        let username = username.trim().to_lowercase();
        // Unknown names still pay for a hash check, so timing doesn't reveal
        // which usernames exist.
        let ok = match self.repo.password_hash(&username)? {
            Some(hash) => self.hasher.verify(password, &hash),
            None => {
                static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                let dummy = DUMMY.get_or_init(|| self.hasher.hash(&new_token()).unwrap_or_default());
                let _ = self.hasher.verify(password, dummy);
                false
            }
        };
        if !ok {
            tokio::time::sleep(FAILED_LOGIN_DELAY).await;
            return Err(invalid("Wrong username or password".into()));
        }
        self.start_session(&username)
    }

    pub fn logout(&self, token: &str) -> Result<(), ServiceError> {
        Ok(self.repo.delete_session(token)?)
    }

    pub fn users(&self) -> Result<Vec<User>, ServiceError> {
        Ok(self.repo.users()?)
    }

    /// Adds another account (sharing the same data).
    pub fn add_user(&self, username: &str, password: &str) -> Result<(), ServiceError> {
        self.create_user(username, password)
    }

    /// You can't remove yourself or the last account.
    pub fn remove_user(&self, acting: &str, username: &str) -> Result<(), ServiceError> {
        if acting == username {
            return Err(invalid("You can't remove your own account".into()));
        }
        if !self.repo.users()?.iter().any(|u| u.username == username) {
            return Err(ServiceError::NotFound(format!("user {username}")));
        }
        Ok(self.repo.remove_user(username)?)
    }

    /// Also signs out your other devices.
    pub fn change_password(
        &self,
        username: &str,
        token: &str,
        current: &str,
        new: &str,
    ) -> Result<(), ServiceError> {
        let hash = self
            .repo
            .password_hash(username)?
            .ok_or_else(|| ServiceError::NotFound(format!("user {username}")))?;
        if !self.hasher.verify(current, &hash) {
            return Err(invalid("Your current password isn't right".into()));
        }
        validate_password(new).map_err(invalid)?;
        let hash = self.hasher.hash(new).map_err(ServiceError::Storage)?;
        self.repo.set_password(username, &hash)?;
        Ok(self.repo.delete_other_sessions(username, token)?)
    }

    fn create_user(&self, username: &str, password: &str) -> Result<(), ServiceError> {
        let username = username.trim();
        validate_username(username).map_err(invalid)?;
        validate_password(password).map_err(invalid)?;
        if self.repo.password_hash(username)?.is_some() {
            return Err(invalid(format!("“{username}” is taken")));
        }
        let hash = self.hasher.hash(password).map_err(ServiceError::Storage)?;
        Ok(self.repo.add_user(username, &hash)?)
    }

    fn start_session(&self, username: &str) -> Result<String, ServiceError> {
        let token = new_token();
        self.repo.create_session(&token, username, SESSION_DAYS)?;
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::infrastructures::{Argon2Hasher, SqliteAuthRepository},
        database::Database,
    };

    fn service(always: bool) -> AuthService {
        AuthService::new(
            Arc::new(SqliteAuthRepository::new(Database::in_memory().unwrap())),
            Arc::new(Argon2Hasher),
            always,
        )
    }

    #[tokio::test]
    async fn open_until_an_account_exists() {
        let s = service(false);
        let st = s.status(None).unwrap();
        assert!(!st.required && st.needs_setup && st.user.is_none());
        assert!(service(true).required().unwrap(), "env can require it");

        assert!(matches!(s.setup("Bad Name", "long enough"), Err(ServiceError::Validation(_))));
        assert!(matches!(s.setup("owner", "short"), Err(ServiceError::Validation(_))));
        let token = s.setup("owner", "long enough").unwrap();
        let st = s.status(Some(&token)).unwrap();
        assert!(st.required && !st.needs_setup);
        assert_eq!(st.user.as_deref(), Some("owner"));
        assert!(s.setup("second", "long enough").is_err(), "setup only once");
    }

    #[tokio::test]
    async fn login_logout_and_passwords() {
        let s = service(false);
        let first = s.setup("owner", "long enough").unwrap();
        assert!(matches!(s.login("owner", "nope nope").await, Err(ServiceError::Validation(_))));
        assert!(s.login("ghost", "long enough").await.is_err());
        let second = s.login("OWNER", "long enough").await.unwrap();
        assert_eq!(s.user_for(Some(&second)).unwrap().as_deref(), Some("owner"));

        s.change_password("owner", &second, "long enough", "even longer").unwrap();
        assert_eq!(s.user_for(Some(&first)).unwrap(), None, "other devices signed out");
        assert!(s.user_for(Some(&second)).unwrap().is_some());
        assert!(s.login("owner", "even longer").await.is_ok());

        s.logout(&second).unwrap();
        assert_eq!(s.user_for(Some(&second)).unwrap(), None);
        assert_eq!(s.user_for(Some("made-up")).unwrap(), None);
    }

    #[tokio::test]
    async fn managing_users() {
        let s = service(false);
        s.setup("owner", "long enough").unwrap();
        s.add_user("partner", "another one").unwrap();
        assert!(s.add_user("partner", "another one").is_err());
        assert_eq!(s.users().unwrap().len(), 2);
        assert!(s.remove_user("owner", "owner").is_err());
        s.remove_user("owner", "partner").unwrap();
        assert_eq!(s.users().unwrap().len(), 1);
    }
}
