use crate::events::quote_update_event::QuoteUpdateEvent;

#[cfg(feature = "server")]
use crate::services::quote::QuoteService;
use dioxus::fullstack::*;
use dioxus::prelude::*;

#[cfg(feature = "server")]
use dioxus::server::axum::Extension;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use tokio::select;
use types::candle::Candle;
use types::interval::Interval;
use types::range::Range;
use types::ticker_symbol::TickerSymbol;

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientEvent {
    Watch(Vec<TickerSymbol>),
    Unwatch(TickerSymbol),
}

#[get("/api/quotes", quote_service: Extension<QuoteService>)]
pub async fn quote_subscribe(
    options: WebSocketOptions,
) -> Result<Websocket<ClientEvent, QuoteUpdateEvent, JsonEncoding>> {
    Ok(options.on_upgrade(
        move |socket: TypedWebsocket<ClientEvent, QuoteUpdateEvent, JsonEncoding>| {
            handle_socket(socket, quote_service)
        },
    ))
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

#[cfg(feature = "server")]
async fn handle_socket(
    mut socket: TypedWebsocket<ClientEvent, QuoteUpdateEvent, JsonEncoding>,
    quote_service: Extension<QuoteService>,
) {
    let mut rx = quote_service.subscribe().await;
    loop {
        select! {
            msg = socket.recv() => match msg {
                Ok(ClientEvent::Watch(tickers)) => {
                    for ticker in tickers {
                        let _ = quote_service.watch(ticker).await;
                    }
                    rx = quote_service.subscribe().await;
                }
                Ok(ClientEvent::Unwatch(ticker)) => {
                    let _ = quote_service.unwatch(&ticker).await;
                    rx = quote_service.subscribe().await;
                }
                Err(_)=>break,
            },
            res = rx.recv() =>match res {
                Ok(q) =>{
                    println!("{q:?}");
                    if socket.send(q).await.is_err() {
                        break;
                    }
                },
                Err(_)=>{},
            }
        }
    }
}
