use dioxus::{
    fullstack::{JsonEncoding, Streaming},
    prelude::*,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

/// Fetch live prices for a list of tickers.
/// Returns ticker → (current_price, daily_change_pct).
///
/// Currently returns mock data. Replace the server-side body with a real
/// Yahoo Finance / Alpha Vantage call when ready.
#[post("/api/prices")]
pub async fn get_live_prices(
    tickers: Vec<String>,
) -> Result<HashMap<String, (Decimal, Decimal)>, ServerFnError> {
    let mock: &[(&str, Decimal, Decimal)] = &[
        ("AAPL", dec!(188.64), dec!(1.23)),
        ("MSFT", dec!(414.78), dec!(-0.31)),
        ("BTC", dec!(67420.0), dec!(2.05)),
        ("SPY", dec!(528.12), dec!(0.44)),
        ("BND", dec!(72.18), dec!(0.07)),
        ("USD", dec!(1.00), dec!(0.00)),
        ("GOOGL", dec!(178.02), dec!(0.89)),
        ("AMZN", dec!(196.34), dec!(-0.52)),
        ("NVDA", dec!(875.40), dec!(3.21)),
        ("AMD", dec!(514.98), dec!(1.87)),
        ("TSM", dec!(397.00), dec!(0.95)),
        ("VOO", dec!(695.05), dec!(0.62)),
        ("TSLA", dec!(172.50), dec!(-1.45)),
    ];

    let prices = tickers
        .into_iter()
        .filter_map(|ticker| {
            mock.iter()
                .find(|(t, _, _)| *t == ticker.as_str())
                .map(|&(_, price, change)| (ticker, (price, change)))
        })
        .collect();

    Ok(prices)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PriceUpdate {
    pub ticker: String,
    pub price: Decimal,
    pub change_pct: Decimal,
}
#[get("/api/prices/stream")]
pub async fn price_stream(
    tickers: Vec<String>,
) -> Result<Streaming<Vec<PriceUpdate>, JsonEncoding>, ServerFnError> {
    Ok(Streaming::spawn(|tx| async move {
        loop {
            let updates = fetch_prices(&tickers).await;
            if tx.unbounded_send(updates).is_err() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }))
}

async fn fetch_prices(tickers: &[String]) -> Vec<PriceUpdate> {
    let client = reqwest::Client::new();
    let futures = tickers.iter().map(|ticker| fetch_one(&client, ticker));
    futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect()
}

async fn fetch_one(client: &reqwest::Client, ticker: &str) -> Option<PriceUpdate> {
    let url =
        format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?interval=1d&range=2d");

    let json = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;

    let closes = &json["chart"]["result"][0]["indicators"]["quote"][0]["close"];
    let current = closes[1].as_f64()?;
    let prev = closes[0].as_f64().unwrap_or(current);

    let price = Decimal::try_from(current).ok()?;
    let change_pct = if prev != 0.0 {
        Decimal::try_from((current - prev) / prev * 100.0).unwrap_or(Decimal::ZERO)
    } else {
        Decimal::ZERO
    };

    Some(PriceUpdate {
        ticker: ticker.to_string(),
        price,
        change_pct,
    })
}
