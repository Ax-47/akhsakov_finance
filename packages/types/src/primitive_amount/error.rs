use rust_decimal::Decimal;
#[derive(Debug, thiserror::Error)]

pub enum PrimitiveError {
    #[error("Value cannot be negative: {0}")]
    Negative(Decimal),
    #[error("Value must be positive: {0}")]
    NonPositive(Decimal),
    #[error("Value cannot be NaN")]
    NotANumber,
    #[error("Value cannot be infinite")]
    Infinite,
}
