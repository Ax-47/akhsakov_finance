#[derive(Debug, thiserror::Error)]
pub enum PrimitiveError {
    #[error("Value cannot be negative: {0}")]
    Negative(f64),
    #[error("Value must be positive: {0}")]
    NonPositive(f64),
    #[error("Value cannot be NaN")]
    NotANumber,
    #[error("Value cannot be infinite")]
    Infinite,
}
