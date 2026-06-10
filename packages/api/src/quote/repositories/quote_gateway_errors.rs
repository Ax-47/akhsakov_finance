use thiserror::Error;

use crate::infrastructures::yahoo_gateway_error::YahooGateWayError;

// ─────────────────────────────────────────────
//  Generic Gateway Error (domain-level)
// ─────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum QuoteGateWayError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Invalid ticker symbol: {0}")]
    InvalidTicker(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<YahooGateWayError> for QuoteGateWayError {
    fn from(e: YahooGateWayError) -> Self {
        match e {
            YahooGateWayError::WebSocket(msg) => QuoteGateWayError::StreamError(msg.to_string()),
            YahooGateWayError::Json(msg) => QuoteGateWayError::RepositoryError(msg.to_string()),
            YahooGateWayError::Ticker(msg) => QuoteGateWayError::InvalidTicker(msg.to_string()),
            _ => QuoteGateWayError::Unknown("idk".to_string()),
        }
    }
}
