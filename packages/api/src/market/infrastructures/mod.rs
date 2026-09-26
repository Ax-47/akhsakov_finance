//! Adapters: a built-in index constituent list, and Yahoo Finance quotes.

mod sp500;

use crate::market::repositories::{
    Constituent, IndexRepository, MarketDataGateway, MarketQuote,
};
use async_trait::async_trait;
use dtos::market::MarketIndex;
use rust_decimal::prelude::ToPrimitive;
use types::ticker_symbol::TickerSymbol;
use yfinance_rs::{Ticker, YfClient};

/// Constituents compiled into the app; see [`sp500`].
pub struct StaticIndexRepository;

impl IndexRepository for StaticIndexRepository {
    fn constituents(&self, index: MarketIndex) -> Vec<Constituent> {
        match index {
            MarketIndex::Sp500 => sp500::SP500.to_vec(),
        }
    }
}

/// Holds one client so Yahoo's cookie / crumb is reused across requests.
pub struct YahooMarketGateway {
    client: YfClient,
}

impl YahooMarketGateway {
    pub fn new() -> Self {
        Self {
            client: crate::shared::yahoo_client(),
        }
    }
}

#[async_trait]
impl MarketDataGateway for YahooMarketGateway {
    async fn quotes(&self, tickers: &[TickerSymbol]) -> Result<Vec<MarketQuote>, String> {
        if tickers.is_empty() {
            return Ok(vec![]);
        }
        // One batched request (yfinance-rs splits long lists into chunks).
        // A dropped connection fails without a retry upstream, so try twice.
        let symbols = || tickers.iter().map(|t| t.as_str());
        let quotes = match yfinance_rs::quotes(&self.client, symbols()).await {
            Ok(quotes) => quotes,
            Err(first) => {
                tracing::warn!("Yahoo quotes failed, retrying once: {first}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                yfinance_rs::quotes(&self.client, symbols())
                    .await
                    .map_err(|e| e.to_string())?
            }
        };
        Ok(quotes
            .into_iter()
            .filter_map(|q| {
                Some(MarketQuote {
                    ticker: TickerSymbol::new(&q.instrument.symbol.to_string()).ok()?,
                    name: q.name,
                    price: q.price?.into_inner().to_f64()?,
                    previous_close: q.previous_close.and_then(|p| p.into_inner().to_f64()),
                })
            })
            .collect())
    }

    async fn market_cap(&self, ticker: &TickerSymbol) -> Result<Option<f64>, String> {
        let stats = Ticker::new(&self.client, ticker.as_str())
            .key_statistics()
            .await
            .map_err(|e| e.to_string())?;
        Ok(stats.market_cap.and_then(|m| m.amount().to_f64()))
    }
}
