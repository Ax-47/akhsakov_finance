use api::{
    events::quote_update_event::QuoteUpdateEvent, get_chart, quote::quote::ClientEvent,
    quote_subscribe,
};
use dioxus::{fullstack::WebSocketOptions, prelude::*};
use std::{collections::HashMap, time::Duration};
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

    use_future(move || async move {
        let current_tickers = tickers.read().clone();
        if current_tickers.is_empty() {
            return;
        }

        let chart_futures = current_tickers.iter().map(|ticker| {
            let ticker = ticker.clone();
            async move {
                let result = get_chart(ticker.clone(), range, interval, is_prepost_market).await;
                (ticker, result)
            }
        });
        let charts = futures::future::join_all(chart_futures).await;
        chart_map.with_mut(|map| {
            for (ticker, result) in charts {
                if let Ok(candles) = result {
                    map.insert(ticker, candles);
                }
            }
        });

        let seed_quotes: Vec<(TickerSymbol, Quote)> = chart_map
            .read()
            .iter()
            .filter_map(|(ticker, candles)| {
                let last = candles.last()?;
                Some((
                    ticker.clone(),
                    Quote {
                        ticker_symbol: ticker.clone(),
                        current_price: last.close,
                        previous_close_price: last.close,
                        timestamp: last.ts.timestamp(),
                    },
                ))
            })
            .collect();
        price_map.with_mut(|map| {
            for (ticker, quote) in seed_quotes {
                map.insert(ticker, quote);
            }
        });

        loop {
            let Ok(stream) = quote_subscribe(WebSocketOptions::new()).await else {
                gloo_timers::future::sleep(Duration::from_secs(2)).await;
                continue;
            };
            let _ = stream.send(ClientEvent::Watch(current_tickers.clone()));
            while let Ok(updates) = stream.recv().await {
                price_map.with_mut(|map| {
                    let QuoteUpdateEvent::Quote(update) = updates;
                    map.insert(update.ticker_symbol.clone(), update);
                });
            }
            gloo_timers::future::sleep(Duration::from_secs(2)).await;
        }
    });

    (price_map.into(), chart_map.into())
}
