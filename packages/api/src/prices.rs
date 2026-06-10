use dioxus::{
    fullstack::{JsonEncoding, Streaming},
    logger::tracing,
    prelude::*,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json;

use futures::future::join_all;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
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
    /// 5-year monthly beta vs S&P 500 from Yahoo `defaultKeyStatistics`.
    /// `None` for instruments that don't report beta (some ETFs, crypto).
    pub beta: Option<Decimal>,
}

// ─── Yahoo session ────────────────────────────────────────────────────────────

/// Holds a `reqwest::Client` whose internal cookie jar is pre-seeded with the
/// Yahoo Finance session cookies, plus the matching crumb token.
struct YahooSession {
    /// The client carries the cookie jar — never share a client across sessions.
    client: reqwest::Client,
    crumb: String,
}

static SESSION: Lazy<Mutex<Option<YahooSession>>> = Lazy::new(|| Mutex::new(None));

const YF_HOME: &str = "https://finance.yahoo.com/";
const CRUMB_URL: &str = "https://query1.finance.yahoo.com/v1/test/getcrumb";

/// Build a fresh cookie-jar client, seed it by visiting Yahoo Finance, then
/// exchange the cookies for a crumb token.  Returns `None` on any failure.
async fn init_session() -> Option<YahooSession> {
    let jar = Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar)
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/124.0.0.0 Safari/537.36",
        )
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    // Step 1 — seed the cookie jar.
    client.get(YF_HOME).send().await.ok()?;

    // Step 2 — exchange cookies for a crumb (plain-text response).
    let crumb = client
        .get(CRUMB_URL)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?
        .trim()
        .to_string();

    if crumb.is_empty() || crumb.to_lowercase().contains("unauthorized") {
        tracing::warn!("Yahoo crumb fetch returned empty/error");
        return None;
    }

    tracing::info!(
        "Yahoo session ready (crumb={}…)",
        &crumb[..crumb.len().min(6)]
    );
    Some(YahooSession { client, crumb })
}

/// Return `(client_clone, crumb)` for the current session, optionally forcing
/// a full re-initialisation (used after detecting an expired crumb).
async fn get_session(reset: bool) -> Option<(reqwest::Client, String)> {
    let mut guard = SESSION.lock().await;
    if reset || guard.is_none() {
        *guard = init_session().await;
    }
    guard.as_ref().map(|s| (s.client.clone(), s.crumb.clone()))
}

// ─── Beta cache ───────────────────────────────────────────────────────────────

struct CachedBeta {
    value: Option<Decimal>,
    fetched_at: Instant,
}

/// Beta rarely changes — refresh every 6 hours.
const BETA_TTL: Duration = Duration::from_secs(6 * 3600);

static BETA_CACHE: Lazy<Mutex<HashMap<String, CachedBeta>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ─── Server function ──────────────────────────────────────────────────────────

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
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }))
}

// ─── Fetch orchestration ──────────────────────────────────────────────────────

/// Fetch all tickers, retrying once with a fresh session if the crumb has
/// expired mid-stream.
async fn fetch_prices(tickers: &[String]) -> Vec<PriceUpdate> {
    for attempt in 0u8..2 {
        let Some((client, crumb)) = get_session(attempt > 0).await else {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        };

        // Shared flag: any concurrent fetch_chart sets this on crumb error.
        let crumb_expired = Arc::new(AtomicBool::new(false));

        let updates: Vec<Option<PriceUpdate>> = join_all(
            tickers
                .iter()
                .map(|t| fetch_one(&client, &crumb, t, Arc::clone(&crumb_expired))),
        )
        .await;

        if crumb_expired.load(Ordering::Relaxed) && attempt == 0 {
            tracing::warn!("Yahoo crumb expired — reinitialising session");
            // Invalidate the global session so next get_session(true) rebuilds.
            *SESSION.lock().await = None;
            continue;
        }

        return updates.into_iter().flatten().collect();
    }

    tracing::error!("Could not obtain a valid Yahoo session after 2 attempts");
    vec![]
}

/// Fetch price data and attach a (possibly cached) beta, concurrently.
async fn fetch_one(
    client: &reqwest::Client,
    crumb: &str,
    ticker: &str,
    crumb_expired: Arc<AtomicBool>,
) -> Option<PriceUpdate> {
    let (chart_result, beta) = tokio::join!(
        fetch_chart(client, crumb, ticker, Arc::clone(&crumb_expired)),
        resolve_beta(client, crumb, ticker),
    );
    let (price, change_pct) = chart_result?;
    Some(PriceUpdate {
        ticker: ticker.to_string(),
        price,
        change_pct,
        beta,
    })
}

// ─── Chart (price + change_pct) ───────────────────────────────────────────────

async fn fetch_chart(
    client: &reqwest::Client,
    crumb: &str,
    ticker: &str,
    crumb_expired: Arc<AtomicBool>,
) -> Option<(Decimal, Decimal)> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/\
         {ticker}?interval=1d&range=2d&crumb={crumb}"
    );
    let json: serde_json::Value = client.get(&url).send().await.ok()?.json().await.ok()?;

    let closes = &json["chart"]["result"][0]["indicators"]["quote"][0]["close"];
    let current = closes[1].as_f64()?;
    let prev = closes[0].as_f64().unwrap_or(current);

    let price = Decimal::try_from(current).ok()?;
    let change_pct = if prev != 0.0 {
        Decimal::try_from((current - prev) / prev * 100.0).unwrap_or(Decimal::ZERO)
    } else {
        Decimal::ZERO
    };
    Some((price, change_pct))
}

// ─── Beta (cached) ────────────────────────────────────────────────────────────

async fn resolve_beta(client: &reqwest::Client, crumb: &str, ticker: &str) -> Option<Decimal> {
    // Fast path: still within TTL.
    {
        let cache = BETA_CACHE.lock().await;
        if let Some(e) = cache.get(ticker) {
            if e.fetched_at.elapsed() < BETA_TTL {
                return e.value;
            }
        }
    }

    // Slow path: remote fetch.
    let fetched = fetch_beta_remote(client, crumb, ticker).await;
    BETA_CACHE.lock().await.insert(
        ticker.to_string(),
        CachedBeta {
            value: fetched,
            fetched_at: Instant::now(),
        },
    );
    fetched
}

async fn fetch_beta_remote(client: &reqwest::Client, crumb: &str, ticker: &str) -> Option<Decimal> {
    let url = format!(
        "https://query1.finance.yahoo.com/v10/finance/quoteSummary/\
         {ticker}?modules=defaultKeyStatistics&crumb={crumb}"
    );
    let json: serde_json::Value = client
        .get(&url)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if is_crumb_error(&json) {
        // Beta cache miss is non-critical; the price-stream retry handles the
        // crumb reset.  Just return None silently.
        return None;
    }

    let raw = json["quoteSummary"]["result"][0]["defaultKeyStatistics"]["beta"]["raw"].as_f64()?;
    Decimal::try_from(raw).ok()
}

// ─── Helper ───────────────────────────────────────────────────────────────────

fn is_crumb_error(json: &serde_json::Value) -> bool {
    // Chart v8:      {"chart":{"error":{"code":"Unauthorized"}}}
    // quoteSummary:  {"quoteSummary":{"error":{"code":"Unauthorized"}}}
    // quota/other:   {"finance":{"error":{"code":"Unauthorized"}}}
    let code = json["chart"]["error"]["code"]
        .as_str()
        .or_else(|| json["quoteSummary"]["error"]["code"].as_str())
        .or_else(|| json["finance"]["error"]["code"].as_str());
    matches!(code, Some("Unauthorized"))
}
