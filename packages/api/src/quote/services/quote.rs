use std::sync::Arc;
use tokio::sync::{watch::Receiver, Mutex};
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

use crate::{
    events::quote_update_event::QuoteUpdateEvent,
    repositories::{quote_gateway::QuoteGateway, quote_gateway_errors::QuoteGateWayError},
};

// ─────────────────────────────────────────────
//  Service
// ─────────────────────────────────────────────
#[derive(Clone)]
pub struct QuoteService {
    gateway: Arc<Mutex<dyn QuoteGateway>>,
}

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
