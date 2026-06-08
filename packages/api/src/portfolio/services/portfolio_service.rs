use dioxus::prelude::*;
use dtos::portfolio::GetPortfolioResponse;
use dtos::{
    asset::get_asset_response::GetAssetResponse,
    portfolio::get_portfolio_response::GetDashBoardResponse, Transaction,
};
use futures::future::join_all;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use types::{
    asset_class::AssetClass, currency::Currency, money::Money, quantity::Quantity,
    ticker_symbol::TickerSymbol, transaction_type::TransactionType,
};
use uuid::Uuid;
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
            ticker: "NVDA".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.354565),
            price: dec!(215.87),
            date: "2026-06-01".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "NVDA".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.354565),
            price: dec!(215.87),
            date: "2026-06-01".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "NVDA".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.696269),
            price: dec!(219.80),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "AMD".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.148258),
            price: dec!(514.98),
            date: "2026-06-04".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "AMD".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.371118),
            price: dec!(421.00),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "VOO".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.220027),
            price: dec!(695.05),
            date: "2026-06-01".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "AAPL".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.511797),
            price: dec!(298.38),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),

            portfolio_id: pid,
            ticker: "TSM".into(),
            transaction_type: TransactionType::Buy,
            shares: dec!(0.383375),
            price: dec!(397.00),
            date: "2026-05-20".into(),
        },
        Transaction {
            id: Uuid::new_v4(),
            portfolio_id: pid1,
            ticker: "AMD".into(),
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
    period: String,
) -> Result<Vec<(String, Decimal)>, ServerFnError> {
    use chrono::Utc;
    use std::collections::{BTreeSet, HashMap};

    let (interval, range) = match period.as_str() {
        "1D" => ("5m", "1d"),
        "5D" => ("30m", "5d"),
        "1M" => ("1d", "1mo"),
        "6M" => ("1wk", "6mo"),
        "YTD" => ("1wk", "ytd"),
        "1Y" => ("1mo", "1y"),
        "All" => ("1mo", "max"),
        _ => ("1wk", "6mo"),
    };

    let tickers: Vec<String> = transactions
        .iter()
        .map(|t| t.ticker.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // ── 1. Fetch all tickers in parallel ────────────────────────────────────
    let fetch_futures = tickers.iter().map(|ticker| {
        let client = client.clone();
        let ticker = ticker.clone();
        let interval = interval.to_string();
        let range = range.to_string();
        async move {
            let url = format!(
                "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
                ticker, interval, range
            );
            let Ok(resp) = client.get(&url).send().await else {
                return (ticker, vec![]);
            };
            let Ok(json) = resp.json::<serde_json::Value>().await else {
                return (ticker, vec![]);
            };
            let Some(result) = json["chart"]["result"].get(0) else {
                return (ticker, vec![]);
            };

            let timestamps = result["timestamp"].as_array().cloned().unwrap_or_default();
            let closes = result["indicators"]["quote"][0]["close"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            let pairs: Vec<(i64, Decimal)> = timestamps
                .iter()
                .zip(closes.iter())
                .filter_map(|(ts, cl)| {
                    let t = ts.as_i64()?;
                    // FIX: was Decimal::from_f64_retain — keep this
                    let p = Decimal::from_f64_retain(cl.as_f64()?)?;
                    Some((t, p))
                })
                .collect();

            (ticker, pairs)
        }
    });

    // All HTTP calls run concurrently
    let fetched: Vec<(String, Vec<(i64, Decimal)>)> = join_all(fetch_futures).await;

    let price_histories: HashMap<String, Vec<(i64, Decimal)>> = fetched
        .into_iter()
        .filter(|(_, pairs)| !pairs.is_empty())
        .collect();

    if price_histories.is_empty() {
        return Ok(vec![]);
    }

    // ── 2. Union of all timestamps (not just one ticker's) ──────────────────
    //   Use BTreeSet so they come out sorted.
    let all_timestamps: Vec<i64> = price_histories
        .values()
        .flat_map(|pairs| pairs.iter().map(|(ts, _)| *ts))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // ── 3. Build result ──────────────────────────────────────────────────────
    let result = all_timestamps
        .iter()
        .filter_map(|unix_ts| {
            let dt = chrono::DateTime::from_timestamp(*unix_ts, 0)?.with_timezone(&Utc);

            let label = match period.as_str() {
                "1D" | "5D" => dt.format("%-d/%m %H:%M").to_string(),
                _ => dt.format("%-d %b '%y").to_string(),
            };

            let portfolio_value: Decimal = tickers
                .iter()
                .map(|ticker| {
                    let shares: Decimal = transactions
                        .iter()
                        .filter(|tx| tx.ticker == *ticker && date_str_to_unix(&tx.date) <= *unix_ts)
                        .map(|tx| match tx.transaction_type {
                            TransactionType::Buy => tx.shares,
                            TransactionType::Sell => -tx.shares,
                            _ => Decimal::ZERO,
                        })
                        .sum();

                    if shares <= Decimal::ZERO {
                        return Decimal::ZERO;
                    }

                    // FIX: abs_diff instead of (ts - unix_ts).abs()
                    //      The old subtraction could underflow on i64.
                    let price = price_histories
                        .get(ticker)
                        .and_then(|h| {
                            h.iter()
                                .min_by_key(|(ts, _)| ts.abs_diff(*unix_ts))
                                .map(|(_, p)| *p)
                        })
                        .unwrap_or(Decimal::ZERO);

                    shares * price
                })
                .sum();

            if portfolio_value <= Decimal::ZERO {
                return None;
            }

            Some((label, portfolio_value))
        })
        .collect();

    Ok(result)
}

fn date_str_to_unix(date: &str) -> i64 {
    use chrono::NaiveDate;
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0)
}
