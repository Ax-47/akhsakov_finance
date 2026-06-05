pub trait CurrencyInfo {
    fn symbol() -> &'static str;
    fn code() -> &'static str;
}

#[derive(Debug, Clone, Copy)]
pub struct Thb;
#[derive(Debug, Clone, Copy)]
pub struct Usd;

impl CurrencyInfo for Thb {
    fn symbol() -> &'static str {
        "฿"
    }
    fn code() -> &'static str {
        "THB"
    }
}

impl CurrencyInfo for Usd {
    fn symbol() -> &'static str {
        "$"
    }
    fn code() -> &'static str {
        "USD"
    }
}
