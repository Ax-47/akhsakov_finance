use crate::{
    money::MoneyError, quantity::QuantityError, ticker::TickerError, trade_date::TradeDateError,
    transaction_type::TransactionTypeError,
};

#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    #[error(transparent)]
    Money(#[from] MoneyError),
    #[error(transparent)]
    Quantity(#[from] QuantityError),
    #[error(transparent)]
    Ticker(#[from] TickerError),
    #[error(transparent)]
    TradeDate(#[from] TradeDateError),
    #[error("Unknown transaction type: '{0}'")]
    TransactionType(#[from] TransactionTypeError),
    #[error("Insufficient shares: tried to sell {sell} but only have {held}")]
    InsufficientShares { sell: f64, held: f64 },
}
