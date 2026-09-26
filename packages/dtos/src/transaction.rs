use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub ticker: TickerSymbol,
    pub transaction_type: TransactionType,
    pub shares: Decimal,
    pub price: Decimal,
    pub date: String, // ISO 8601: YYYY-MM-DD
    /// Commission / fees paid on this transaction.
    #[serde(default)]
    pub fee: Decimal,
    /// Currency `price` and `fee` are in: the stock's trading currency.
    #[serde(default = "usd")]
    pub currency: String,
    /// USD per unit of `currency` on the trade date (1 for USD). Amounts
    /// are compared and summed in USD.
    #[serde(default = "one")]
    pub fx_to_usd: Decimal,
}

fn usd() -> String {
    "USD".into()
}

fn one() -> Decimal {
    Decimal::ONE
}

/// Ticker used for cash deposits and withdrawals.
pub const CASH_TICKER: &str = "$CASH";

impl Transaction {
    pub fn is_cash(&self) -> bool {
        self.ticker.as_str() == CASH_TICKER
    }

    pub fn is_usd(&self) -> bool {
        self.currency == "USD"
    }

    /// `price` in USD at the trade-date rate.
    pub fn usd_price(&self) -> Decimal {
        self.price * self.fx_to_usd
    }

    /// `fee` in USD at the trade-date rate.
    pub fn usd_fee(&self) -> Decimal {
        self.fee * self.fx_to_usd
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    pub transactions: Vec<Transaction>,
}
