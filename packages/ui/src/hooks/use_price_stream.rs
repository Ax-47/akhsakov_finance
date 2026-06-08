use api::{price_stream, PriceUpdate};
use dioxus::prelude::*;
use std::collections::HashMap;

pub fn use_price_stream(tickers: Vec<String>) -> ReadSignal<HashMap<String, PriceUpdate>> {
    let price_map = use_signal(|| HashMap::<String, PriceUpdate>::new());
    use_effect(move || {
        let tickers = tickers.clone();

        if tickers.is_empty() {
            return;
        }

        let mut pm = price_map.clone();
        spawn(async move {
            let Ok(mut stream) = price_stream(tickers).await else {
                return;
            };
            while let Some(updates) = stream.next().await {
                pm.with_mut(|map| {
                    if let Ok(updates) = updates {
                        for u in updates {
                            map.insert(u.ticker.clone(), u);
                        }
                    }
                });
            }
        });
    });

    price_map.into()
}
