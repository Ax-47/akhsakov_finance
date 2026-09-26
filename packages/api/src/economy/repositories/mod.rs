//! Ports for economic data.

use async_trait::async_trait;

/// Economic time series by id. Errors are provider messages.
#[async_trait]
pub trait MacroSource: Send + Sync {
    /// `(YYYY-MM-DD, value)` from `start` on, oldest first; missing
    /// observations are left out.
    async fn series(&self, id: &str, start: &str) -> Result<Vec<(String, f64)>, String>;
}

/// Latest level and today's % change of a market symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    pub symbol: String,
    pub value: f64,
    pub change_pct: Option<f64>,
}

#[async_trait]
pub trait GaugeSource: Send + Sync {
    async fn levels(&self, symbols: &[String]) -> Result<Vec<Level>, String>;
}
