use dioxus::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
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
