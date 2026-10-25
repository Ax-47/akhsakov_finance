//! The live price of one stock, for a page about it: its own websocket
//! watching just that ticker, so a stock you don't hold moves too.

use api::{events::quote_update_event::QuoteUpdateEvent, quote::quote::ClientEvent, quote_subscribe};
use dioxus::{
    fullstack::{use_websocket, WebSocketOptions},
    prelude::*,
};
use rust_decimal::Decimal;
use std::{cell::RefCell, rc::Rc};
use types::ticker_symbol::TickerSymbol;

/// One streamed price.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LiveTick {
    /// In USD, like every other price the app shows.
    pub usd: Decimal,
    /// In the stock's own currency, like its chart.
    pub native: Decimal,
    /// When it traded, in Unix seconds.
    pub ts: i64,
}

/// The latest streamed price of `ticker`, `None` until one arrives (or
/// never, for an asset priced by hand). Applied at most once a second
/// (every three in Lite), like [`super::use_price_stream`].
pub fn use_live_tick(ticker: TickerSymbol) -> ReadSignal<Option<LiveTick>> {
    let mut latest = use_signal(|| None::<LiveTick>);
    let mut socket = use_websocket(|| quote_subscribe(WebSocketOptions::new()));
    let watch = ticker.clone();
    use_future(move || {
        let watch = watch.clone();
        async move {
            let _ = socket.send(ClientEvent::Watch(vec![watch])).await;
        }
    });
    let pending = use_hook(|| Rc::new(RefCell::new(None::<LiveTick>)));
    use_future(move || {
        let (pending, ticker) = (pending.clone(), ticker.clone());
        async move {
            while let Ok(QuoteUpdateEvent::QuoteUpdate(update)) = socket.recv().await {
                if update.ticker_symbol != ticker || update.current_price <= Decimal::ZERO {
                    continue;
                }
                let tick = LiveTick {
                    usd: update.current_price,
                    native: update.native_price.unwrap_or(update.current_price),
                    ts: update.timestamp,
                };
                // Only the first tick of a quiet spell schedules a flush.
                if pending.borrow_mut().replace(tick).is_none() {
                    let pending = pending.clone();
                    spawn(async move {
                        crate::notify::poll_delay(crate::perf::price_flush_ms()).await;
                        if let Some(tick) = pending.borrow_mut().take() {
                            latest.set(Some(tick));
                        }
                    });
                }
            }
        }
    });
    latest.into()
}
