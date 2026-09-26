//! Port for prices you enter yourself, for assets Yahoo doesn't price
//! (Thai mutual funds, deposits, bonds, physical gold …).

use dtos::assets::ManualPrice;
use types::ticker_symbol::TickerSymbol;

pub trait ManualQuotes: Send + Sync {
    /// The asset's currency and every price entered for it, when it's
    /// priced by hand; `None` for market-priced tickers.
    fn manual(&self, ticker: &TickerSymbol) -> Option<(String, Vec<ManualPrice>)>;
}
