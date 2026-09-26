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
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use types::{quote::Quote, ticker_symbol::TickerSymbol};

/// Set when prices come from the saved copy because Yahoo can't be
/// reached; cleared by the next live price.
pub static OFFLINE: GlobalSignal<bool> = Signal::global(|| false);

/// Where the last prices are kept on this device.
const QUOTES_KEY: &str = "quotes";
/// Streamed prices are saved every this many flushes.
const SAVE_EVERY: u32 = 60;

/// Bumped by [`reprice`].
static REPRICE: GlobalSignal<u32> = Signal::global(|| 0);

/// Fetches every streamed price again, e.g. after entering a price by
/// hand (hand-priced assets aren't streamed).
pub fn reprice() {
    *REPRICE.write() += 1;
}

/// Live quotes for `tickers`: one batched request for the starting prices,
/// then updates over a websocket, applied at most once a second (every
/// three in Lite; see [`crate::perf::price_flush_ms`]). Yahoo can send
/// several ticks a second per stock; writing each one re-rendered every
/// page that shows a price, which made the app lag. Changing `tickers`
/// re-subscribes.
pub fn use_price_stream(tickers: Memo<Vec<TickerSymbol>>) -> ReadSignal<HashMap<TickerSymbol, Quote>> {
    let mut price_map = use_signal(HashMap::<TickerSymbol, Quote>::new);
    let mut socket = use_websocket(|| quote_subscribe(WebSocketOptions::new()));
    let repriced = use_hook(|| Rc::new(Cell::new(0_u32)));

    use_effect(move || {
        let current = tickers.read().clone();
        let round = REPRICE();
        if current.is_empty() {
            return;
        }
        let refetch_all = repriced.replace(round) != round;
        spawn(async move {
            // Only fetch tickers not already priced; the stream keeps the rest fresh.
            let missing: Vec<TickerSymbol> = current
                .iter()
                .filter(|t| refetch_all || !price_map.peek().contains_key(*t))
                .cloned()
                .collect();
            if !missing.is_empty() {
                match get_quotes(missing.clone()).await {
                    Ok(quotes) => {
                        if !quotes.is_empty() {
                            *OFFLINE.write() = quotes.values().any(|q| q.stale);
                        }
                        price_map.with_mut(|map| map.extend(quotes));
                        crate::offline::save(QUOTES_KEY, &*price_map.peek());
                    }
                    // Server unreachable: the prices saved on this device.
                    Err(_) => {
                        if let Some(saved) = crate::offline::load::<HashMap<TickerSymbol, Quote>>(QUOTES_KEY).await {
                            price_map.with_mut(|map| {
                                for t in &missing {
                                    if let Some(q) = saved.get(t) {
                                        map.entry(t.clone()).or_insert(Quote { stale: true, ..q.clone() });
                                    }
                                }
                            });
                            *OFFLINE.write() = true;
                        }
                    }
                }
            }
            let _ = socket.send(ClientEvent::Watch(current)).await;
        });
    });

    // Latest streamed price per ticker, not yet shown. Not reactive.
    let pending = use_hook(|| Rc::new(RefCell::new(HashMap::<TickerSymbol, Decimal>::new())));

    use_future(move || {
        let pending = pending.clone();
        async move {
            // A flush is scheduled only while updates wait, so a quiet
            // market costs nothing (no timer ticking every second).
            let scheduled = Rc::new(Cell::new(false));
            while let Ok(QuoteUpdateEvent::QuoteUpdate(update)) = socket.recv().await {
                pending
                    .borrow_mut()
                    .insert(update.ticker_symbol, update.current_price);
                if !scheduled.replace(true) {
                    let (pending, scheduled) = (pending.clone(), scheduled.clone());
                    spawn(async move {
                        crate::notify::poll_delay(crate::perf::price_flush_ms()).await;
                        scheduled.set(false);
                        let updates = std::mem::take(&mut *pending.borrow_mut());
                        apply(price_map, updates);
                    });
                }
            }
        }
    });

    price_map.into()
}

/// Writes streamed prices, only when a shown price moved (every write
/// re-renders the pages that show prices).
fn apply(mut price_map: Signal<HashMap<TickerSymbol, Quote>>, updates: HashMap<TickerSymbol, Decimal>) {
    let changed = {
        let map = price_map.peek();
        updates
            .iter()
            .any(|(t, p)| map.get(t).is_some_and(|q| q.current_price != *p || q.stale))
    };
    thread_local! {
        static FLUSHES: Cell<u32> = const { Cell::new(0) };
    }
    if changed && FLUSHES.with(|n| n.replace(n.get() + 1) % SAVE_EVERY == 0) {
        crate::offline::save(QUOTES_KEY, &*price_map.peek());
    }
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
