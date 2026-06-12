use api::{
    events::quote_update_event::QuoteUpdateEvent,
    quote::quote::{get_charts, get_quote, ClientEvent},
    quote_subscribe,
};
use dioxus::{
    fullstack::{use_websocket, WebSocketOptions},
    prelude::*,
};
use std::collections::HashMap;
use types::{
    candle::Candle, interval::Interval, quote::Quote, range::Range, ticker_symbol::TickerSymbol,
};

pub fn use_price_stream(
    tickers: Memo<Vec<TickerSymbol>>,
    range: Range,
    interval: Interval,
    is_prepost_market: bool,
) -> (
    ReadSignal<HashMap<TickerSymbol, Quote>>,
    ReadSignal<HashMap<TickerSymbol, Vec<Candle>>>,
) {
    let mut price_map = use_signal(|| HashMap::<TickerSymbol, Quote>::new());
    let mut chart_map = use_signal(|| HashMap::<TickerSymbol, Vec<Candle>>::new());
    let mut socket = use_websocket(|| quote_subscribe(WebSocketOptions::new()));

    use_effect(move || {
        let current_tickers = tickers.read().clone();
        if current_tickers.is_empty() {
            return;
        }
        spawn(async move {
            let Ok(charts) =
                get_charts(current_tickers.clone(), range, interval, is_prepost_market).await
            else {
                return;
            };

            let quote_futures = current_tickers.iter().map(|ticker| {
                let t = ticker.clone();
                async move {
                    let result = get_quote(t.clone()).await;
                    (t, result)
                }
            });
            let quotes: HashMap<TickerSymbol, Quote> = futures::future::join_all(quote_futures)
                .await
                .into_iter()
                .filter_map(|(ticker, result)| result.ok().map(|q| (ticker, q)))
                .collect();
            price_map.with_mut(|map| {
                for (ticker, quote) in quotes {
                    map.insert(ticker, quote);
                }
            });
            chart_map.with_mut(|map| {
                for (ticker, candles) in charts {
                    map.insert(ticker, candles);
                }
            });
            let _ = socket.send(ClientEvent::Watch(current_tickers)).await;
        });
    });

    use_future(move || async move {
        while let Ok(updates) = socket.recv().await {
            price_map.with_mut(|map| {
                let QuoteUpdateEvent::QuoteUpdate(update) = updates;
                if let Some(old) = map.get_mut(&update.ticker_symbol) {
                    old.current_price = update.current_price;
                }
            });
        }
    });

    (price_map.into(), chart_map.into())
}
