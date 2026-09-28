//! Portfolios an AI assistant manages through the connector, each with the
//! paper money you give it.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Starting capital offered when you set one up, in USD.
pub const DEFAULT_STARTING_CASH: Decimal = dec!(10000);

/// Name given to a new AI portfolio (a number is added if it's taken).
pub const AI_PORTFOLIO_NAME: &str = "AI";

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AiPortfolioInfo {
    pub portfolio_id: Uuid,
    pub name: String,
    /// Money you've given it: deposits less withdrawals, in USD.
    pub funded: Decimal,
    /// Uninvested cash, in USD.
    pub cash: Decimal,
    /// Buys and sells it has made.
    pub trades: usize,
}
