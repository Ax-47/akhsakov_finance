//! Server functions for sign-in, under `/api/auth/` (reachable without a
//! session). The session token travels in `Authorization: Bearer …`.

use dioxus::{fullstack::HeaderMap, prelude::*};
use dtos::auth::{AuthStatus, User};

#[cfg(feature = "server")]
use super::AuthService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// The bearer token in `headers`, if any.
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
}

#[cfg(feature = "server")]
fn signed_in(service: &AuthService, headers: &HeaderMap) -> Result<String, ServerFnError> {
    service
        .user_for(bearer(headers))?
        .ok_or_else(|| ServerFnError::ServerError {
            message: "Sign in first".into(),
            code: 401,
            details: None,
        })
}

#[post("/api/auth/status", service: Extension<AuthService>, headers: HeaderMap)]
pub async fn auth_status() -> Result<AuthStatus, ServerFnError> {
    Ok(service.status(bearer(&headers))?)
}

/// Creates the first account; returns its session token.
#[post("/api/auth/setup", service: Extension<AuthService>)]
pub async fn auth_setup(username: String, password: String) -> Result<String, ServerFnError> {
    Ok(service.setup(&username, &password)?)
}

/// Returns a session token.
#[post("/api/auth/login", service: Extension<AuthService>)]
pub async fn auth_login(username: String, password: String) -> Result<String, ServerFnError> {
    Ok(service.login(&username, &password).await?)
}

#[post("/api/auth/logout", service: Extension<AuthService>, headers: HeaderMap)]
pub async fn auth_logout() -> Result<(), ServerFnError> {
    if let Some(token) = bearer(&headers) {
        service.logout(token)?;
    }
    Ok(())
}

#[post("/api/auth/users", service: Extension<AuthService>, headers: HeaderMap)]
pub async fn list_users() -> Result<Vec<User>, ServerFnError> {
    signed_in(&service, &headers)?;
    Ok(service.users()?)
}

#[post("/api/auth/users/add", service: Extension<AuthService>, headers: HeaderMap)]
pub async fn add_user(username: String, password: String) -> Result<(), ServerFnError> {
    signed_in(&service, &headers)?;
    Ok(service.add_user(&username, &password)?)
}

#[post("/api/auth/users/remove", service: Extension<AuthService>, headers: HeaderMap)]
pub async fn remove_user(username: String) -> Result<(), ServerFnError> {
    let me = signed_in(&service, &headers)?;
    Ok(service.remove_user(&me, &username)?)
}

#[post("/api/auth/password", service: Extension<AuthService>, headers: HeaderMap)]
pub async fn change_password(current: String, new: String) -> Result<(), ServerFnError> {
    let me = signed_in(&service, &headers)?;
    let token = bearer(&headers).unwrap_or_default().to_string();
    Ok(service.change_password(&me, &token, &current, &new)?)
}
