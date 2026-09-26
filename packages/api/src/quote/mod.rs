pub(crate) mod controller;
pub mod events;

#[cfg(feature = "server")]
pub(crate) mod infrastructures;

#[cfg(feature = "server")]
pub(crate) mod repositories;

#[cfg(feature = "server")]
pub(crate) mod services;
#[cfg(feature = "server")]
use crate::quote::services::quote::QuoteService;
#[cfg(feature = "server")]
use std::sync::Arc;


pub use controller::*;

/// Quotes from Yahoo, with the last good prices kept in `db` for offline
/// use.
#[cfg(feature = "server")]
pub fn quote_services_setup(db: crate::database::Database) -> QuoteService {
    let quote_gateway = Arc::new(tokio::sync::RwLock::new(
        infrastructures::yahoo_gateway::YahooGateWay::new(),
    ));
    QuoteService::new(quote_gateway)
        .with_cache(Arc::new(infrastructures::sqlite_cache::SqliteQuoteCache::new(db)))
}

/// [`crate::shared::FxRates`] backed by the quote service's FX data.
#[cfg(feature = "server")]
pub struct QuoteFxRates(pub QuoteService);

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl crate::shared::FxRates for QuoteFxRates {
    async fn usd_per_unit(
        &self,
        currency: &str,
        date: &str,
    ) -> Result<rust_decimal::Decimal, String> {
        self.0
            .usd_rate_on(currency, date)
            .await
            .map_err(|e| e.to_string())
    }

    async fn usd_per_unit_now(&self, currency: &str) -> Result<rust_decimal::Decimal, String> {
        self.0.usd_rate(currency).await.map_err(|e| e.to_string())
    }

    async fn currency_of(&self, ticker: &types::ticker_symbol::TickerSymbol) -> Result<String, String> {
        self.0.currency_of(ticker).await.map_err(|e| e.to_string())
    }
}
