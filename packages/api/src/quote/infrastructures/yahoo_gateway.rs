use crate::{
    events::quote_update_event::QuoteUpdateEvent,
    infrastructures::{
        yahoo_gateway_error::YahooGateWayError,
        yahoo_gateway_mapper::{to_candle, to_yinterval, to_yrange},
    },
};
use rust_decimal::Decimal;
use std::{collections::HashSet, sync::Arc, time::Duration};
#[cfg(feature = "server")]
use tokio::sync::{
    broadcast::{channel, Receiver, Sender},
    Mutex,
};
use types::{
    candle::Candle, interval::Interval, quote::Quote, range::Range, ticker_symbol::TickerSymbol,
};
#[cfg(feature = "server")]
use yfinance_rs::{
    Interval as YInterval, Range as YRange, StreamBuilder, StreamHandle, StreamMethod, Ticker,
    YfClient,
};
#[cfg(feature = "server")]
pub struct YahooGateWay {
    client: YfClient,
    handle: Option<StreamHandle>,
    sender: Sender<QuoteUpdateEvent>,
    tickers: Arc<Mutex<HashSet<TickerSymbol>>>,
}
#[cfg(feature = "server")]
impl YahooGateWay {
    pub fn new() -> Self {
        let (tx, _) = channel(256);
        Self {
            client: YfClient::default(),
            handle: None,
            sender: tx,
            tickers: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    async fn subscribe_gateway(&mut self) -> Result<(), YahooGateWayError> {
        if let Some(old) = self.handle.take() {
            old.stop().await;
        }
        let tickers: Vec<TickerSymbol> = self.tickers.lock().await.iter().cloned().collect();
        let (handle, mut receiver) = StreamBuilder::new(&self.client)
            .symbols(tickers.iter().map(|s| s.as_str()))
            .method(StreamMethod::WebsocketWithFallback)
            .interval(Duration::from_secs(1))
            .diff_only(true)
            .cache_mode(yfinance_rs::CacheMode::Use)
            .start()
            .await?;

        self.handle = Some(handle);
        let tx = self.sender.clone();
        tokio::spawn(async move {
            while let Some(update) = receiver.recv().await {
                let Ok(symbol) = TickerSymbol::new(update.instrument.symbol.as_str()) else {
                    continue;
                };
                let quote = Quote {
                    ticker_symbol: symbol,
                    current_price: update
                        .price
                        .map(|p| p.into_inner())
                        .unwrap_or(Decimal::ZERO),
                    previous_close_price: update
                        .previous_close
                        .map(|p| p.into_inner())
                        .unwrap_or(Decimal::ZERO),
                    timestamp: update.ts.timestamp(),
                };
                let _ = tx.send(QuoteUpdateEvent::Quote(quote));
            }
        });
        Ok(())
    }
    pub async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
        self.sender.subscribe()
    }
    pub async fn add_ticker(&mut self, ticker: TickerSymbol) -> Result<(), YahooGateWayError> {
        self.tickers.lock().await.insert(ticker);
        self.subscribe_gateway().await
    }

    pub async fn remove_ticker(&mut self, ticker: &TickerSymbol) -> Result<(), YahooGateWayError> {
        self.tickers.lock().await.remove(ticker);
        self.subscribe_gateway().await
    }

    pub async fn get_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, YahooGateWayError> {
        let ticker = Ticker::new(&self.client, ticker);
        Ok(ticker
            .history(
                Some(to_yrange(range)),
                Some(to_yinterval(interval)),
                is_prepost_market,
            )
            .await?
            .into_iter()
            .map(to_candle)
            .collect())
    }
}
