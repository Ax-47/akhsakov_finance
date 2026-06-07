use std::collections::HashMap;

use dioxus::prelude::*;

/// Fetch live prices for a list of tickers.
/// Returns ticker → (current_price, daily_change_pct).
///
/// Currently returns mock data. Replace the server-side body with a real
/// Yahoo Finance / Alpha Vantage call when ready.
#[post("/api/prices")]
pub async fn get_live_prices(
    tickers: Vec<String>,
) -> Result<HashMap<String, (f64, f64)>, ServerFnError> {
    let mock: &[(&str, f64, f64)] = &[
        ("AAPL", 188.64, 1.23),
        ("MSFT", 414.78, -0.31),
        ("BTC", 67420.0, 2.05),
        ("SPY", 528.12, 0.44),
        ("BND", 72.18, 0.07),
        ("USD", 1.00, 0.00),
        ("GOOGL", 178.02, 0.89),
        ("AMZN", 196.34, -0.52),
        ("NVDA", 875.40, 3.21),
        ("TSLA", 172.50, -1.45),
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
