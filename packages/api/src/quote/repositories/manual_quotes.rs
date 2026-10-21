//! Port for prices you enter yourself, for assets Yahoo doesn't price
//! (Thai mutual funds, deposits, bonds, physical gold …).

use dtos::assets::ManualPrice;
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;

/// What's known about an asset priced by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct ManualSeries {
    pub currency: String,
    /// Every price entered, in any order.
    pub prices: Vec<ManualPrice>,
    /// Simple interest a year, in percent, earned since the latest entry.
    pub yearly_rate: Option<Decimal>,
}

pub trait ManualQuotes: Send + Sync {
    /// The asset's prices when it's priced by hand; `None` for
    /// market-priced tickers.
    fn manual(&self, ticker: &TickerSymbol) -> Option<ManualSeries>;
}
