//! Shared kernel: error types and the FX port used across bounded
//! contexts, plus the Yahoo clients their market-data adapters share.

#[cfg(feature = "server")]
pub mod yahoo_raw;

#[cfg(feature = "server")]
use dioxus::prelude::ServerFnError;

/// Failure reported by a repository (storage port).
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("storage failure: {0}")]
    Storage(String),
    #[error("stored data is invalid: {0}")]
    Corrupt(String),
}

/// Failure of a use case, as reported to the caller.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("{0}")]
    Validation(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Storage(String),
    /// A market-data provider failed or is rate-limiting.
    #[error("{0}")]
    Upstream(String),
}

impl From<RepositoryError> for ServiceError {
    fn from(e: RepositoryError) -> Self {
        match e {
            RepositoryError::NotFound(what) => Self::NotFound(what),
            other => Self::Storage(other.to_string()),
        }
    }
}

#[cfg(feature = "server")]
impl From<ServiceError> for ServerFnError {
    fn from(e: ServiceError) -> Self {
        let code = match e {
            ServiceError::Validation(_) => 400,
            ServiceError::NotFound(_) => 404,
            ServiceError::Storage(_) => 500,
            ServiceError::Upstream(_) => 502,
        };
        ServerFnError::ServerError {
            message: user_message(&e),
            code,
            details: None,
        }
    }
}

/// What the app shows for an error. A market-data failure that reads like
/// a raw HTTP error (a URL, "error sending request") is logged here and
/// replaced with a plain sentence: the details mean nothing to the reader
/// and are too long to fit on a phone.
#[cfg(feature = "server")]
fn user_message(e: &ServiceError) -> String {
    let text = e.to_string();
    // Model connection errors keep their detail: it's how you find a
    // mistyped address or key.
    let raw = !text.starts_with("Model ")
        && ["http://", "https://", "error sending request", "gateway error", "connection"]
            .iter()
            .any(|m| text.to_lowercase().contains(m));
    match e {
        ServiceError::Upstream(_) if raw => {
            tracing::warn!("upstream error: {text}");
            UPSTREAM_MESSAGE.to_string()
        }
        _ => text,
    }
}

/// Shown instead of a raw market-data error.
pub const UPSTREAM_MESSAGE: &str = "The market data source isn't answering right now. Try again shortly.";

#[cfg(all(test, feature = "server"))]
mod user_message_tests {
    use super::*;

    #[test]
    fn hides_raw_upstream_errors_only() {
        let raw = ServiceError::Upstream("error sending request for url (https://fc.yahoo.com/consent)".into());
        assert_eq!(user_message(&raw), UPSTREAM_MESSAGE);
        let model = ServiceError::Upstream("Model request failed: error sending request for url (https://api.example.com/v1)".into());
        assert!(user_message(&model).contains("api.example.com"), "model errors keep their detail");
        let model = ServiceError::Upstream("The model returned no assistant message.".into());
        assert_eq!(user_message(&model), "The model returned no assistant message.");
        let invalid = ServiceError::Validation("See https://example.com".into());
        assert_eq!(user_message(&invalid), "See https://example.com", "only upstream errors are rewritten");
    }
}

#[cfg(feature = "server")]
impl From<crate::database::DatabaseError> for RepositoryError {
    fn from(e: crate::database::DatabaseError) -> Self {
        Self::Storage(e.to_string())
    }
}

/// Yahoo Finance client shared by every market-data adapter's setup.
///
/// Yahoo quietly closes idle keep-alive connections. Reusing one fails with
/// "error sending request", which yfinance-rs doesn't retry (it only retries
/// connect errors and timeouts). Dropping pooled connections after a short
/// idle period means each request after a pause opens a fresh one.
#[cfg(feature = "server")]
pub fn yahoo_client() -> yfinance_rs::YfClient {
    use std::time::Duration;
    let http = reqwest::Client::builder()
        // yfinance-rs's own defaults, which a custom client doesn't get.
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(15))
        .tcp_keepalive(Duration::from_secs(15))
        .build()
        .expect("build the HTTP client");
    yfinance_rs::YfClient::builder()
        .custom_client(http)
        .build()
        .expect("build the Yahoo client")
}

/// Exchange rates to USD, the app's base currency. Implemented over the
/// quote service; used by contexts that price things in other currencies.
#[cfg(feature = "server")]
#[async_trait::async_trait]
pub trait FxRates: Send + Sync {
    /// USD per unit of `currency` on `date` (YYYY-MM-DD).
    async fn usd_per_unit(&self, currency: &str, date: &str) -> Result<rust_decimal::Decimal, String>;
    /// USD per unit of `currency` now.
    async fn usd_per_unit_now(&self, currency: &str) -> Result<rust_decimal::Decimal, String>;
    /// The currency `ticker` trades in.
    async fn currency_of(&self, ticker: &types::ticker_symbol::TickerSymbol) -> Result<String, String>;
}
