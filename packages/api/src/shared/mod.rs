//! Shared kernel: error types used across bounded contexts.

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
            message: e.to_string(),
            code,
            details: None,
        }
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
