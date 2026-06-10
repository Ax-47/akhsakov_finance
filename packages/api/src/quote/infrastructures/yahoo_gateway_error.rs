use thiserror::Error;
use types::ticker_symbol;
use yfinance_rs::YfError;
#[derive(Debug, Error)]
pub enum YahooGateWayError {
    #[error(transparent)]
    WebSocket(#[from] YfError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Ticker(#[from] ticker_symbol::TickerSymbolError),
}
