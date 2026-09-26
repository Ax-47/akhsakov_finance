//! Port for the last good prices, used when the provider can't be reached.

use types::{candle::Candle, quote::Quote, ticker_symbol::TickerSymbol};

/// Best effort: failures to save are ignored by callers.
pub trait QuoteCache: Send + Sync {
    fn save_quote(&self, quote: &Quote);
    fn load_quote(&self, ticker: &TickerSymbol) -> Option<Quote>;
    fn save_chart(&self, key: &str, candles: &[Candle]);
    fn load_chart(&self, key: &str) -> Option<Vec<Candle>>;
}
