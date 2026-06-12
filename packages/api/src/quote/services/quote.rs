use std::sync::Arc;

#[cfg(feature = "server")]
use tokio::sync::{broadcast::Receiver, Mutex};
#[cfg(feature = "server")]
use types::quote::Quote;
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

#[cfg(feature = "server")]
use crate::{
    events::quote_update_event::QuoteUpdateEvent,
    repositories::{quote_gateway::QuoteGateway, quote_gateway_errors::QuoteGateWayError},
};

// ─────────────────────────────────────────────
//  Service
// ─────────────────────────────────────────────
//
#[cfg(feature = "server")]
#[derive(Clone)]
pub struct QuoteService {
    gateway: Arc<Mutex<dyn QuoteGateway>>,
}

#[cfg(feature = "server")]
impl QuoteService {
    pub fn new(gateway: Arc<Mutex<dyn QuoteGateway>>) -> Self {
        Self { gateway }
    }

    pub async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
        self.gateway.lock().await.subscribe().await
    }

    pub async fn watch(&self, ticker: TickerSymbol) -> Result<(), QuoteGateWayError> {
        self.gateway.lock().await.add_ticker(ticker).await
    }

    pub async fn unwatch(&self, ticker: &TickerSymbol) -> Result<(), QuoteGateWayError> {
        self.gateway.lock().await.remove_ticker(ticker).await
    }

    pub async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError> {
        self.gateway.lock().await.get_quote(ticker).await
    }
    pub async fn get_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, QuoteGateWayError> {
        self.gateway
            .lock()
            .await
            .get_chart(ticker, range, interval, is_prepost_market)
            .await
    }
}
