use api::{
    events::quote_update_event::QuoteUpdateEvent,
    quote::quote::{get_quotes, ClientEvent},
    quote_subscribe,
};
use dioxus::{
    fullstack::{use_websocket, WebSocketOptions},
    prelude::*,
};
use std::collections::HashMap;
use types::{quote::Quote, ticker_symbol::TickerSymbol};

/// Set when prices come from the saved copy because Yahoo can't be
/// reached; cleared by the next live price.
pub static OFFLINE: GlobalSignal<bool> = Signal::global(|| false);

/// Live quotes for `tickers`: one batched request for the starting prices,
/// then updates over a websocket. Changing `tickers` re-subscribes.
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

    use_future(move || async move {
        while let Ok(QuoteUpdateEvent::QuoteUpdate(update)) = socket.recv().await {
            price_map.with_mut(|map| {
                if let Some(old) = map.get_mut(&update.ticker_symbol) {
                    old.current_price = update.current_price;
                    old.stale = false;
                }
            });
            if *OFFLINE.peek() {
                *OFFLINE.write() = false;
            }
        }
    });

    price_map.into()
}
