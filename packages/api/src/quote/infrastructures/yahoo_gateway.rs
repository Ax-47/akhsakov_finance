#[cfg(feature = "server")]
use crate::quote::infrastructures::yahoo_gateway_mapper::to_quote;
use crate::{
    events::quote_update_event::QuoteUpdateEvent,
    infrastructures::{
        yahoo_gateway_error::YahooGateWayError,
        yahoo_gateway_mapper::{to_candle, to_yinterval, to_yrange},
    },
};
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::Arc, time::Duration};
#[cfg(feature = "server")]
use tokio::sync::{
    broadcast::{channel, Receiver, Sender},
    Mutex,
};
#[cfg(feature = "server")]
use types::quote::QuoteUpdate;
use types::{
    candle::Candle, interval::Interval, quote::Quote, range::Range, ticker_symbol::TickerSymbol,
};
#[cfg(feature = "server")]
use yfinance_rs::{StreamBuilder, StreamHandle, StreamMethod, Ticker, YfClient};
#[cfg(feature = "server")]
pub struct YahooGateWay {
    client: YfClient,
    handle: Option<StreamHandle>,
    sender: Sender<QuoteUpdateEvent>,
    /// Streamed tickers, with how many watchers each has.
    tickers: Arc<Mutex<HashMap<TickerSymbol, usize>>>,
}
#[cfg(feature = "server")]
impl YahooGateWay {
    pub fn new() -> Self {
        let (tx, _) = channel(256);
        Self {
            client: crate::shared::yahoo_client(),
            handle: None,
            sender: tx,
            tickers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn subscribe_gateway(&mut self) -> Result<(), YahooGateWayError> {
        if let Some(old) = self.handle.take() {
            old.stop().await;
        }
        let tickers: Vec<TickerSymbol> = self.tickers.lock().await.keys().cloned().collect();
        if tickers.is_empty() {
            return Ok(());
        }
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
                let quote = QuoteUpdate {
                    ticker_symbol: symbol,
                    current_price: update
                        .price
                        .map(|p| p.into_inner())
                        .unwrap_or(Decimal::ZERO),
                    timestamp: update.ts.timestamp(),
                };
                let _ = tx.send(QuoteUpdateEvent::QuoteUpdate(quote));
            }
        });
        Ok(())
    }
    pub async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
        self.sender.subscribe()
    }
    /// Adds a watcher to each ticker. The stream restarts (once) only when
    /// a ticker nobody watched yet joins it.
    pub async fn add_tickers(&mut self, tickers: Vec<TickerSymbol>) -> Result<(), YahooGateWayError> {
        let mut added = false;
        {
            let mut watched = self.tickers.lock().await;
            for ticker in tickers {
                let n = watched.entry(ticker).or_insert(0);
                added |= *n == 0;
                *n += 1;
            }
        }
        if added {
            self.subscribe_gateway().await?;
        }
        Ok(())
    }

    /// Drops a watcher from each ticker; the stream restarts (once) only
    /// when a ticker loses its last watcher.
    pub async fn remove_tickers(&mut self, tickers: Vec<TickerSymbol>) -> Result<(), YahooGateWayError> {
        let mut removed = false;
        {
            let mut watched = self.tickers.lock().await;
            for ticker in &tickers {
                if let Some(n) = watched.get_mut(ticker) {
                    *n -= 1;
                    if *n == 0 {
                        watched.remove(ticker);
                        removed = true;
                    }
                }
            }
        }
        if removed {
            self.subscribe_gateway().await?;
        }
        Ok(())
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

    pub async fn search(
        &self,
        query: &str,
    ) -> Result<Vec<dtos::watch::SearchHit>, YahooGateWayError> {
        let response = yfinance_rs::search(&self.client, query).await?;
        Ok(response
            .results
            .into_iter()
            .map(|r| dtos::watch::SearchHit {
                symbol: r.instrument.symbol.to_string(),
                name: r.name,
                exchange: r.instrument.exchange.map(|e| e.to_string()),
                kind: format!("{:?}", r.instrument.kind),
            })
            .collect())
    }

    pub async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, YahooGateWayError> {
        let ticker = Ticker::new(&self.client, ticker);
        Ok(to_quote(ticker.quote().await?)?)
    }
}
