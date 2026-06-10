use crate::{
    events::quote_update_event::QuoteUpdateEvent, infrastructures::yahoo_gateway::YahooGateWay,
    repositories::quote_gateway_errors::QuoteGateWayError,
};
use async_trait::async_trait;
use tokio::sync::watch::Receiver;
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

// ─────────────────────────────────────────────
//  Gateway Trait
// ─────────────────────────────────────────────

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
}
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
}
