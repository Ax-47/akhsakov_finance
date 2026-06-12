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
    async fn add_ticker(&mut self, ticker: TickerSymbol) -> Result<(), QuoteGateWayError>;
    async fn remove_ticker(&mut self, ticker: &TickerSymbol) -> Result<(), QuoteGateWayError>;
    async fn get_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, QuoteGateWayError>;

    async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError>;
}

#[cfg(feature = "server")]
#[async_trait]
impl QuoteGateway for YahooGateWay {
    async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
        self.subscribe().await
    }

    async fn add_ticker(&mut self, ticker: TickerSymbol) -> Result<(), QuoteGateWayError> {
        self.add_ticker(ticker).await.map_err(Into::into)
    }

    async fn remove_ticker(&mut self, ticker: &TickerSymbol) -> Result<(), QuoteGateWayError> {
        self.remove_ticker(ticker).await.map_err(Into::into)
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
}
