use thiserror::Error;

#[cfg(feature = "server")]
use crate::infrastructures::yahoo_gateway_error::YahooGateWayError;

// ─────────────────────────────────────────────
//  Generic Gateway Error (domain-level)
// ─────────────────────────────────────────────

#[cfg(feature = "server")]
#[derive(Debug, Error)]
pub enum QuoteGateWayError {
    #[error("gateway error: {0}")]
    GateWayError(String),

    #[error("Invalid ticker symbol: {0}")]
    InvalidTicker(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[cfg(feature = "server")]
impl From<YahooGateWayError> for QuoteGateWayError {
    fn from(e: YahooGateWayError) -> Self {
        match e {
            YahooGateWayError::GateWay(msg) => QuoteGateWayError::GateWayError(msg.to_string()),
            YahooGateWayError::Json(msg) => QuoteGateWayError::RepositoryError(msg.to_string()),
            YahooGateWayError::Ticker(msg) => QuoteGateWayError::InvalidTicker(msg.to_string()),
            _ => QuoteGateWayError::Unknown("idk".to_string()),
        }
    }
}
