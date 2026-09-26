use crate::quote::events::quote_update_event::QuoteUpdateEvent;
use std::collections::HashMap;

#[cfg(feature = "server")]
use crate::quote::services::quote::QuoteService;
use dioxus::fullstack::*;
use dioxus::prelude::*;

#[cfg(feature = "server")]
use dioxus::server::axum::Extension;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use tokio::select;
use types::candle::Candle;
use types::interval::Interval;
use types::quote::Quote;
use types::range::Range;
use types::ticker_symbol::TickerSymbol;

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientEvent {
    Watch(Vec<TickerSymbol>),
    Unwatch(TickerSymbol),
}

#[get("/api/quotes/ws", quote_service: Extension<QuoteService>)]
pub async fn quote_subscribe(
    options: WebSocketOptions,
) -> Result<Websocket<ClientEvent, QuoteUpdateEvent, JsonEncoding>> {
    Ok(options.on_upgrade(
        move |socket: TypedWebsocket<ClientEvent, QuoteUpdateEvent, JsonEncoding>| {
            handle_socket(socket, quote_service)
        },
    ))
}

#[post("/api/quotes/charts", quote_service: Extension<QuoteService>)]
pub async fn get_charts(
    tickers: Vec<TickerSymbol>,
    range: Range,
    interval: Interval,
    is_prepost_market: bool,
) -> Result<HashMap<TickerSymbol, Vec<Candle>>, ServerFnError> {
    let futures = tickers.into_iter().map(|ticker| {
        let qs = quote_service.clone();
        async move {
            let result = qs
                .get_chart(ticker.clone(), range, interval, is_prepost_market)
                .await;
            (ticker, result)
        }
    });
    Ok(futures::future::join_all(futures)
        .await
        .into_iter()
        .filter_map(|(ticker, result)| result.ok().map(|candles| (ticker, candles)))
        .collect())
}
#[get("/api/quotes/chart", quote_service: Extension<QuoteService>)]
pub async fn get_chart(
    ticker: TickerSymbol,
    range: Range,
    interval: Interval,
    is_prepost_market: bool,
) -> Result<Vec<Candle>, ServerFnError> {
    quote_service
        .get_chart(ticker, range, interval, is_prepost_market)
        .await
        .map_err(|e| ServerFnError::ServerError {
            message: e.to_string(),
            code: 400,
            details: None,
        })
}

/// Latest quotes (USD for stocks) for many tickers in one call; tickers
/// that can't be priced are left out.
#[post("/api/quotes/many", quote_service: Extension<QuoteService>)]
pub async fn get_quotes(tickers: Vec<TickerSymbol>) -> Result<HashMap<TickerSymbol, Quote>, ServerFnError> {
    let fetched = futures::future::join_all(tickers.into_iter().map(|ticker| {
        let qs = quote_service.clone();
        async move { (ticker.clone(), qs.get_quote(ticker).await) }
    }))
    .await;
    Ok(fetched
        .into_iter()
        .filter_map(|(ticker, result)| result.ok().map(|q| (ticker, q)))
        .collect())
}

#[get("/api/quotes", quote_service: Extension<QuoteService>)]
pub async fn get_quote(ticker: TickerSymbol) -> Result<Quote, ServerFnError> {
    quote_service
        .get_quote(ticker)
        .await
        .map_err(|e| ServerFnError::ServerError {
            message: e.to_string(),
            code: 400,
            details: None,
        })
}
/// Latest quote in the instrument's own currency (`currency` says which),
/// e.g. to prefill a trade as the broker shows it.
#[get("/api/quotes/native", quote_service: Extension<QuoteService>)]
pub async fn get_native_quote(ticker: TickerSymbol) -> Result<Quote, ServerFnError> {
    quote_service
        .native_quote(ticker)
        .await
        .map_err(|e| ServerFnError::ServerError {
            message: e.to_string(),
            code: 502,
            details: None,
        })
}

/// USD per unit of `currency` at the close on `date` (YYYY-MM-DD).
#[get("/api/quotes/fx", quote_service: Extension<QuoteService>)]
pub async fn get_fx_rate(
    currency: String,
    date: String,
) -> Result<rust_decimal::Decimal, ServerFnError> {
    quote_service
        .usd_rate_on(&currency.to_uppercase(), &date)
        .await
        .map_err(|e| ServerFnError::ServerError {
            message: e.to_string(),
            code: 502,
            details: None,
        })
}

/// Instruments matching a symbol or company name, e.g. "nvidia".
#[get("/api/quotes/search", quote_service: Extension<QuoteService>)]
pub async fn search_tickers(query: String) -> Result<Vec<dtos::watch::SearchHit>, ServerFnError> {
    quote_service
        .search(&query)
        .await
        .map_err(|e| ServerFnError::ServerError {
            message: e.to_string(),
            code: 502,
            details: None,
        })
}

#[cfg(feature = "server")]
async fn handle_socket(
    mut socket: TypedWebsocket<ClientEvent, QuoteUpdateEvent, JsonEncoding>,
    quote_service: Extension<QuoteService>,
) {
    use tokio::sync::broadcast::error::RecvError;

    let mut rx = quote_service.subscribe().await;
    let mut watched: Vec<TickerSymbol> = Vec::new();

    loop {
        select! {
            msg = socket.recv() => match msg {
                Ok(ClientEvent::Watch(tickers)) => {
                    for ticker in &tickers {
                        if quote_service.watch(ticker.clone()).await.is_err() {
                            tracing::warn!("failed to watch {ticker}");
                        }
                    }
                    watched = tickers;
                    rx = quote_service.subscribe().await;
                }
                Ok(ClientEvent::Unwatch(ticker)) => {
                    let _ = quote_service.unwatch(&ticker).await;
                    watched.retain(|t| t != &ticker);
                    rx = quote_service.subscribe().await;
                }
                Err(_) => break,
            },
            res = rx.recv() => match res {
                Ok(QuoteUpdateEvent::QuoteUpdate(update)) => {
                    // Stocks in other currencies are sent in USD.
                    let Some(update) = quote_service.to_usd(update).await else {
                        continue;
                    };
                    if socket.send(QuoteUpdateEvent::QuoteUpdate(update)).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("quote broadcast lagged {n} messages; resubscribing");
                    rx = quote_service.subscribe().await;
                }
                Err(RecvError::Closed) => break,
            }
        }
    }

    // Unwatch everything this connection held so the service stops
    // fetching prices for a dead socket.
    for ticker in &watched {
        let _ = quote_service.unwatch(ticker).await;
    }
}
