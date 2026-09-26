use api::{
    events::quote_update_event::QuoteUpdateEvent,
    quote::quote::{get_quotes, ClientEvent},
    quote_subscribe,
};
use dioxus::{
    fullstack::{use_websocket, WebSocketOptions},
    prelude::*,
};
use rust_decimal::Decimal;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use types::{quote::Quote, ticker_symbol::TickerSymbol};

/// Set when prices come from the saved copy because Yahoo can't be
/// reached; cleared by the next live price.
pub static OFFLINE: GlobalSignal<bool> = Signal::global(|| false);

/// How often streamed prices reach the UI. Yahoo can send several ticks a
/// second per stock; writing each one re-rendered every page that shows a
/// price, which made the app lag.
const FLUSH_MS: u32 = 1000;

/// Live quotes for `tickers`: one batched request for the starting prices,
/// then updates over a websocket, applied at most once per [`FLUSH_MS`].
/// Changing `tickers` re-subscribes.
pub fn use_price_stream(tickers: Memo<Vec<TickerSymbol>>) -> ReadSignal<HashMap<TickerSymbol, Quote>> {
    let mut price_map = use_signal(HashMap::<TickerSymbol, Quote>::new);
    let mut socket = use_websocket(|| quote_subscribe(WebSocketOptions::new()));

    use_effect(move || {
        let current = tickers.read().clone();
        if current.is_empty() {
            return;
        }
        spawn(async move {
            // Only fetch tickers not already priced; the stream keeps the rest fresh.
            let missing: Vec<TickerSymbol> = current
                .iter()
                .filter(|t| !price_map.peek().contains_key(*t))
                .cloned()
                .collect();
            if !missing.is_empty() {
                if let Ok(quotes) = get_quotes(missing).await {
                    if !quotes.is_empty() {
                        *OFFLINE.write() = quotes.values().any(|q| q.stale);
                    }
                    price_map.with_mut(|map| map.extend(quotes));
                }
            }
            let _ = socket.send(ClientEvent::Watch(current)).await;
        });
    });

    // Latest streamed price per ticker, not yet shown. Not reactive.
    let pending = use_hook(|| Rc::new(RefCell::new(HashMap::<TickerSymbol, Decimal>::new())));

    let incoming = pending.clone();
    use_future(move || {
        let incoming = incoming.clone();
        async move {
            while let Ok(QuoteUpdateEvent::QuoteUpdate(update)) = socket.recv().await {
                incoming
                    .borrow_mut()
                    .insert(update.ticker_symbol, update.current_price);
            }
        }
    });

    use_future(move || {
        let pending = pending.clone();
        async move {
            loop {
                crate::notify::poll_delay(FLUSH_MS).await;
                let updates = std::mem::take(&mut *pending.borrow_mut());
                if updates.is_empty() {
                    continue;
                }
                // Only write (and so re-render) when a shown price moved.
                let changed = {
                    let map = price_map.peek();
                    updates.iter().any(|(t, p)| {
                        map.get(t).is_some_and(|q| q.current_price != *p || q.stale)
                    })
                };
                if changed {
                    price_map.with_mut(|map| {
                        for (t, p) in updates {
                            if let Some(old) = map.get_mut(&t) {
                                old.current_price = p;
                                old.stale = false;
                            }
                        }
                    });
                }
                if *OFFLINE.peek() {
                    *OFFLINE.write() = false;
                }
            }
        }
    });

    price_map.into()
}
