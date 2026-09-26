//! Axum middleware: once sign-in is required, every `/api/` call except
//! sign-in itself and the public quote stream needs a valid session.

use crate::auth::{controller::bearer, AuthService};
use dioxus::server::axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Paths reachable without a session.
fn is_public(path: &str) -> bool {
    !path.starts_with("/api/") || path.starts_with("/api/auth/") || path == "/api/quotes/ws"
}

pub async fn require_sign_in(State(auth): State<AuthService>, request: Request, next: Next) -> Response {
    if is_public(request.uri().path()) {
        return next.run(request).await;
    }
    match auth.required() {
        Ok(false) => next.run(request).await,
        Ok(true) => match auth.user_for(bearer(request.headers())) {
            Ok(Some(_)) => next.run(request).await,
            _ => (StatusCode::UNAUTHORIZED, "Sign in first").into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_paths() {
        assert!(is_public("/"));
        assert!(is_public("/assets/tailwind.css"));
        assert!(is_public("/api/auth/login"));
        assert!(is_public("/api/quotes/ws"));
        assert!(!is_public("/api/dashboard"));
        assert!(!is_public("/api/quotes/chart"));
    }
}
