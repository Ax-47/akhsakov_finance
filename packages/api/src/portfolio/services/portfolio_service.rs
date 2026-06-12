use chrono::FixedOffset;
use dioxus::prelude::*;
use dtos::portfolio::GetPortfolioResponse;
use dtos::{
    asset::get_asset_response::GetAssetResponse,
    portfolio::get_portfolio_response::GetDashBoardResponse, Transaction,
};

#[cfg(feature = "server")]
use futures::future::join_all;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use types::candle::Candle;
use types::interval::Interval;
use types::range::Range;
use types::{
    asset_class::AssetClass, currency::Currency, money::Money, quantity::Quantity,
    ticker_symbol::TickerSymbol, transaction_type::TransactionType,
};
use uuid::Uuid;

use crate::get_chart;
#[post("/api/portfolios")]
pub async fn get_portfolios() -> Result<Vec<GetPortfolioResponse>, ServerFnError> {
    let pid = Uuid::new_v4();

    let assets = vec![
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("AAPL").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(0.51)).unwrap(),
            cost: Money::new(dec!(298.38), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("AMD").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(0.51)).unwrap(),
            cost: Money::new(dec!(447.83), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("TSM").unwrap(),
            asset_class: AssetClass::Crypto,
            quantity: Quantity::new(dec!(0.38)).unwrap(),
            cost: Money::new(dec!(397.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("VOO").unwrap(),
            asset_class: AssetClass::Etf,
            quantity: Quantity::new(dec!(0.22)).unwrap(),
            cost: Money::new(dec!(695.05), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("NVDA").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(1.40)).unwrap(),
            cost: Money::new(dec!(217.82), Currency::Usd).unwrap(),
        },
    ];

    let assets2 = vec![GetAssetResponse {
        portfolio_id: pid,
        ticker_symbol: TickerSymbol::new("AMD").unwrap(),
        asset_class: AssetClass::Stock,
        quantity: Quantity::new(dec!(0.51)).unwrap(),
        cost: Money::new(dec!(232.59), Currency::Usd).unwrap(),
    }];

    Ok(vec![
        GetPortfolioResponse {
            id: pid,
            name: "Portfolio1".to_string(),
            assets,
        },
        GetPortfolioResponse {
            id: pid,
            name: "Portfolio2".to_string(),
            assets: assets2,
        },
    ])
}

#[get("/api/dashboard")]
pub async fn get_dashboard() -> Result<GetDashBoardResponse, ServerFnError> {
    let pid = Uuid::new_v4();

    let pid1 = Uuid::new_v4();
    let assets = vec![
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("AAPL").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(0.511797)).unwrap(),
            cost: Money::new(dec!(298.38), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("AMD").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(0.519376)).unwrap(),
            cost: Money::new(dec!(447.83), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("TSM").unwrap(),
            asset_class: AssetClass::Crypto,
            quantity: Quantity::new(dec!(0.383375)).unwrap(),
            cost: Money::new(dec!(397.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("VOO").unwrap(),
            asset_class: AssetClass::Etf,
            quantity: Quantity::new(dec!(0.220027)).unwrap(),
            cost: Money::new(dec!(695.05), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("NVDA").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(1.405399)).unwrap(),
            cost: Money::new(dec!(217.82), Currency::Usd).unwrap(),
        },
    ];

    let assets2 = vec![GetAssetResponse {
        portfolio_id: pid1,
        ticker_symbol: TickerSymbol::new("AMD").unwrap(),
        asset_class: AssetClass::Stock,
        quantity: Quantity::new(dec!(0.0594)).unwrap(),
        cost: Money::new(dec!(514.98), Currency::Usd).unwrap(),
    }];

    let transactions = vec![
        Transaction {
            id: Uuid::new_v4(),
            portfolio_id: pid,
            ticker: TickerSymbol::new("NVDA").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.354565),
            price: dec!(215.87),
            date: "2026-06-01".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("NVDA").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.354565),
            price: dec!(215.87),
            date: "2026-06-01".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("NVDA").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.696269),
            price: dec!(219.80),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("AMD").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.148258),
            price: dec!(514.98),
            date: "2026-06-04".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("AMD").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.371118),
            price: dec!(421.00),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("VOO").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.220027),
            price: dec!(695.05),
            date: "2026-06-01".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("AAPL").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.511797),
            price: dec!(298.38),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: TickerSymbol::new("TSM").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.383375),
            price: dec!(397.00),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),
            portfolio_id: pid1,
            ticker: TickerSymbol::new("AMD").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.0594),
            price: dec!(514.98),
            date: "2026-05-20".into(),
        },
    ];

    Ok(GetDashBoardResponse {
        portfolios: vec![
            GetPortfolioResponse {
                id: pid,
                name: "Portfolio1".into(),
                assets,
            },
            GetPortfolioResponse {
                id: pid1,
                name: "Portfolio2".into(),
                assets: assets2,
            },
        ],
        transactions,
    })
}
// api.rs
#[server]
pub async fn get_portfolio_history(
    transactions: Vec<Transaction>,
    range: Range,
    interval: Interval,
) -> Result<Vec<(String, Decimal)>, ServerFnError> {
    use chrono::Utc;
    use std::collections::{BTreeSet, HashMap};

    let tickers: Vec<TickerSymbol> = transactions
        .iter()
        .map(|t| t.ticker.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Use get_chart server fn instead of raw reqwest
    let fetched: Vec<(TickerSymbol, Vec<Candle>)> = join_all(tickers.iter().map(|ticker| {
        let t = ticker.clone();
        async move {
            let candles = get_chart(t.clone(), range, interval, true)
                .await
                .unwrap_or_default();
            (t, candles)
        }
    }))
    .await;

    let price_histories: HashMap<TickerSymbol, Vec<(i64, Decimal)>> = fetched
        .into_iter()
        .filter(|(_, candles)| !candles.is_empty())
        .map(|(ticker, candles)| {
            let pairs = candles
                .into_iter()
                .map(|c| (c.ts.timestamp(), c.close))
                .collect();
            (ticker, pairs)
        })
        .collect();

    if price_histories.is_empty() {
        return Ok(vec![]);
    }

    let baseline: Decimal = tickers
        .iter()
        .map(|ticker| {
            transactions
                .iter()
                .filter(|tx| tx.ticker == *ticker)
                .map(|tx| match tx.transaction_type {
                    TransactionType::Buy => tx.shares * tx.price,
                    TransactionType::Sell => -(tx.shares * tx.price),
                    _ => Decimal::ZERO,
                })
                .sum::<Decimal>()
        })
        .sum();

    if baseline <= Decimal::ZERO {
        return Ok(vec![]);
    }

    let all_timestamps: Vec<i64> = price_histories
        .values()
        .flat_map(|pairs| pairs.iter().map(|(ts, _)| *ts))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let result = all_timestamps
        .iter()
        .filter_map(|unix_ts| {
            let dt = chrono::DateTime::from_timestamp(*unix_ts, 0)?.with_timezone(&Utc);
            let label = format_label(range, &dt);

            let portfolio_value: Decimal = tickers
                .iter()
                .map(|ticker| {
                    let shares = shares_at(ticker, &transactions, *unix_ts);
                    if shares <= Decimal::ZERO {
                        return Decimal::ZERO;
                    }
                    let price = price_histories
                        .get(ticker)
                        .and_then(|h| h.iter().min_by_key(|(ts, _)| ts.abs_diff(*unix_ts)))
                        .map(|(_, p)| *p)
                        .unwrap_or(Decimal::ZERO);
                    shares * price
                })
                .sum();

            if portfolio_value <= Decimal::ZERO {
                return None;
            }

            let pct = (portfolio_value - baseline) / baseline * dec!(100);
            Some((label, pct))
        })
        .collect();

    Ok(result)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn shares_at(ticker: &TickerSymbol, transactions: &[Transaction], unix_ts: i64) -> Decimal {
    transactions
        .iter()
        .filter(|tx| tx.ticker == *ticker && date_str_to_unix(&tx.date) <= unix_ts)
        .map(|tx| match tx.transaction_type {
            TransactionType::Buy => tx.shares,
            TransactionType::Sell => -tx.shares,
            _ => Decimal::ZERO,
        })
        .sum()
}

fn format_label(range: Range, dt: &chrono::DateTime<chrono::Utc>) -> String {
    let tz = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
    let local = dt.with_timezone(&tz);
    match range {
        Range::D1 | Range::D5 => local.format("%-d/%m %H:%M").to_string(),
        _ => local.format("%-d %b '%y").to_string(),
    }
}

#[cfg(feature = "server")]
async fn fetch_ticker(
    client: &reqwest::Client,
    ticker: &str,
    interval: &str,
    range: &str,
) -> (TickerSymbol, Vec<(i64, Decimal)>, Option<Decimal>) {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?interval={interval}&range={range}"
    );

    let Some(json) = client
        .get(&url)
        .send()
        .await
        .ok()
        .and_then(|r| futures::executor::block_on(r.json::<serde_json::Value>()).ok())
    else {
        return (TickerSymbol::new(ticker).unwrap(), vec![], None);
    };

    let Some(result) = json["chart"]["result"].get(0) else {
        return (TickerSymbol::new(ticker).unwrap(), vec![], None);
    };

    let prev_close = result["meta"]["chartPreviousClose"]
        .as_f64()
        .and_then(Decimal::from_f64_retain);

    let timestamps = result["timestamp"].as_array().cloned().unwrap_or_default();
    let closes = result["indicators"]["quote"][0]["close"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let pairs = timestamps
        .iter()
        .zip(closes.iter())
        .filter_map(|(ts, cl)| Some((ts.as_i64()?, Decimal::from_f64_retain(cl.as_f64()?)?)))
        .collect();

    (TickerSymbol::new(ticker).unwrap(), pairs, prev_close)
}

fn date_str_to_unix(date: &str) -> i64 {
    use chrono::NaiveDate;
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
        .unwrap_or(0)
}
