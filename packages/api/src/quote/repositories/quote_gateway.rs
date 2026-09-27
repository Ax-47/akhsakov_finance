#[cfg(feature = "server")]
use crate::{
    events::quote_update_event::QuoteUpdateEvent, infrastructures::yahoo_gateway::YahooGateWay,
    repositories::quote_gateway_errors::QuoteGateWayError,
};

#[cfg(feature = "server")]
use async_trait::async_trait;

#[cfg(feature = "server")]
use tokio::sync::broadcast::Receiver;
#[cfg(feature = "server")]
use types::quote::Quote;
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

// ─────────────────────────────────────────────
//  Gateway Trait
// ─────────────────────────────────────────────

#[cfg(feature = "server")]
#[async_trait]
pub trait QuoteGateway: Send + Sync {
    async fn subscribe(&self) -> Receiver<QuoteUpdateEvent>;
    /// Streams prices for `tickers`, counting one more watcher for each.
    async fn add_tickers(&mut self, tickers: Vec<TickerSymbol>) -> Result<(), QuoteGateWayError>;
    /// Drops one watcher for each of `tickers`; unwatched ones stop streaming.
    async fn remove_tickers(&mut self, tickers: Vec<TickerSymbol>) -> Result<(), QuoteGateWayError>;
    async fn get_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, QuoteGateWayError>;

    async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError>;

    /// Instruments whose symbol or name matches `query`.
    async fn search(&self, query: &str) -> Result<Vec<dtos::watch::SearchHit>, QuoteGateWayError>;
}

#[cfg(feature = "server")]
#[async_trait]
impl QuoteGateway for YahooGateWay {
    async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
        self.subscribe().await
    }

    async fn add_tickers(&mut self, tickers: Vec<TickerSymbol>) -> Result<(), QuoteGateWayError> {
        self.add_tickers(tickers).await.map_err(Into::into)
    }

    async fn remove_tickers(&mut self, tickers: Vec<TickerSymbol>) -> Result<(), QuoteGateWayError> {
        self.remove_tickers(tickers).await.map_err(Into::into)
    }

    async fn get_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, QuoteGateWayError> {
        self.get_chart(ticker, range, interval, is_prepost_market)
            .await
            .map_err(Into::into)
    }

    async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError> {
        self.get_quote(ticker).await.map_err(Into::into)
    }

    async fn search(&self, query: &str) -> Result<Vec<dtos::watch::SearchHit>, QuoteGateWayError> {
        self.search(query).await.map_err(Into::into)
    }
}
