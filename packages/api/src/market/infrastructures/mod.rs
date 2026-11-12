//! Adapters: index membership from Wikipedia (with a built-in fallback),
//! and Yahoo Finance for quotes, screens and calendars.

mod sp500;
mod wikipedia;

pub use wikipedia::WikipediaIndexRepository;

use crate::{
    market::repositories::{BoardRow, MarketDataGateway, MarketQuote, TickerCalendar},
    shared::yahoo_raw::{date, num, text, YahooRaw},
};
use async_trait::async_trait;
use dtos::market::{ScreenFilter, ScreenSort, SCREEN_PAGE_SIZE};
use serde_json::{json, Value};
use yfinance_rs::{Ticker, YfClient};

pub struct YahooMarketGateway {
    raw: YahooRaw,
    /// Holds one client so Yahoo's cookie / crumb is reused across requests.
    client: YfClient,
}

impl YahooMarketGateway {
    pub fn new() -> Self {
        Self {
            raw: YahooRaw::new(),
            client: crate::shared::yahoo_client(),
        }
    }
}

impl YahooMarketGateway {
    /// Yahoo's quote objects for `tickers`.
    async fn raw_quotes(&self, tickers: &[String]) -> Result<Vec<Value>, String> {
        if tickers.is_empty() {
            return Ok(vec![]);
        }
        // A dropped connection fails without an upstream retry: try twice.
        match self.raw.quotes(tickers).await {
            Ok(v) => Ok(v),
            Err(first) => {
                tracing::warn!("Yahoo quotes failed, retrying once: {first}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                self.raw.quotes(tickers).await
            }
        }
    }
}

#[async_trait]
impl MarketDataGateway for YahooMarketGateway {
    async fn quotes(&self, tickers: &[String]) -> Result<Vec<MarketQuote>, String> {
        Ok(self.raw_quotes(tickers).await?.iter().filter_map(to_quote).collect())
    }

    async fn board(&self, tickers: &[String]) -> Result<Vec<BoardRow>, String> {
        Ok(self.raw_quotes(tickers).await?.iter().filter_map(to_board).collect())
    }

    async fn screen(&self, filter: &ScreenFilter) -> Result<(u32, Vec<MarketQuote>), String> {
        let page = self
            .raw
            .screen(
                screen_query(filter),
                sort_field(filter.sort),
                filter.descending,
                SCREEN_PAGE_SIZE,
                filter.page * SCREEN_PAGE_SIZE,
            )
            .await?;
        let quotes = page
            .quotes
            .iter()
            .filter_map(to_quote)
            .filter(|q| !is_depositary_receipt(&q.ticker))
            .collect();
        Ok((page.total, quotes))
    }

    async fn calendar(&self, ticker: &str) -> Result<TickerCalendar, String> {
        let c = Ticker::new(&self.client, ticker)
            .calendar()
            .await
            .map_err(|e| e.to_string())?;
        Ok(TickerCalendar {
            earnings: c
                .earnings_dates
                .iter()
                .map(|d| d.date_naive().to_string())
                .collect(),
            ex_dividend: c.ex_dividend_date.map(|d| d.to_string()),
            payment: c.dividend_payment_date.map(|d| d.to_string()),
        })
    }
}

/// The currency a Yahoo quote object is in, and what its prices are
/// multiplied by to get major units (London quotes come in pence).
fn currency_unit(v: &Value) -> (String, f64) {
    let raw_currency = text(v, "currency").unwrap_or_else(|| "USD".into());
    match raw_currency.as_str() {
        "GBp" | "GBX" => ("GBP".to_string(), 0.01),
        "ZAc" => ("ZAR".to_string(), 0.01),
        "ILA" => ("ILS".to_string(), 0.01),
        _ => (raw_currency.to_uppercase(), 1.0),
    }
}

/// A Yahoo quote object as a board row, in major currency units.
fn to_board(v: &Value) -> Option<BoardRow> {
    use dtos::board::{BoardQuote, Session};
    let (currency, minor) = currency_unit(v);
    let price_field = |key: &str| num(v, key).map(|x| x * minor).filter(|x| *x > 0.0);
    let session = Session::from_state(&text(v, "marketState").unwrap_or_default());
    let ext_price = match session {
        Session::Regular => None,
        Session::Pre => price_field("preMarketPrice"),
        Session::Post | Session::Closed => price_field("postMarketPrice"),
    };
    let ticker = text(v, "symbol")?;
    Some(BoardRow {
        quote: BoardQuote {
            name: text(v, "shortName").or_else(|| text(v, "longName")).unwrap_or_else(|| ticker.clone()),
            ticker,
            currency,
            exchange: text(v, "fullExchangeName").or_else(|| text(v, "exchange")),
            sector: None,
            price: price_field("regularMarketPrice")?,
            previous_close: price_field("regularMarketPreviousClose"),
            open: price_field("regularMarketOpen"),
            day_high: price_field("regularMarketDayHigh"),
            day_low: price_field("regularMarketDayLow"),
            volume: num(v, "regularMarketVolume").filter(|x| *x > 0.0),
            avg_volume: num(v, "averageDailyVolume3Month").filter(|x| *x > 0.0),
            bid: price_field("bid"),
            ask: price_field("ask"),
            high_52w: price_field("fiftyTwoWeekHigh"),
            low_52w: price_field("fiftyTwoWeekLow"),
            market_cap_usd: None,
            pe: num(v, "trailingPE"),
            session,
            ext_price,
            time: num(v, "regularMarketTime").map(|t| t as i64),
        },
        market_cap: num(v, "marketCap"),
    })
}

/// A Yahoo quote object as a [`MarketQuote`], in major currency units.
fn to_quote(v: &Value) -> Option<MarketQuote> {
    let (currency, minor) = currency_unit(v);
    let price_field = |key: &str| num(v, key).map(|x| x * minor);
    let mut earnings: Vec<String> = ["earningsTimestampStart", "earningsTimestamp", "earningsTimestampEnd"]
        .iter()
        .filter_map(|k| date(v, k))
        .collect();
    earnings.sort();
    earnings.dedup();
    Some(MarketQuote {
        ticker: text(v, "symbol")?,
        name: text(v, "shortName").or_else(|| text(v, "longName")),
        price: price_field("regularMarketPrice")?,
        change_pct: num(v, "regularMarketChangePercent"),
        market_cap: num(v, "marketCap"),
        pe: num(v, "trailingPE"),
        forward_pe: num(v, "forwardPE"),
        dividend_yield_pct: num(v, "dividendYield")
            .or_else(|| num(v, "trailingAnnualDividendYield").map(|y| y * 100.0)),
        dividend_rate: price_field("dividendRate").or_else(|| price_field("trailingAnnualDividendRate")),
        volume: num(v, "regularMarketVolume"),
        high_52w: price_field("fiftyTwoWeekHigh"),
        low_52w: price_field("fiftyTwoWeekLow"),
        earnings_dates: earnings,
        earnings_estimated: v
            .get("isEarningsDateEstimate")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        dividend_date: date(v, "dividendDate"),
        currency,
    })
}

/// Thai depositary receipts (e.g. `NVDA80.BK`) carry the foreign
/// company's market cap, so they'd top every Thai screen.
fn is_depositary_receipt(ticker: &str) -> bool {
    let Some(code) = ticker.strip_suffix(".BK") else {
        return false;
    };
    // Letters followed by a two-digit issuer number.
    let (name, issuer) = code.split_at(code.len().saturating_sub(2));
    issuer.len() == 2
        && issuer.chars().all(|c| c.is_ascii_digit())
        && name.ends_with(|c: char| c.is_ascii_alphabetic())
}

fn sort_field(sort: ScreenSort) -> &'static str {
    match sort {
        ScreenSort::MarketCap => "intradaymarketcap",
        ScreenSort::Change => "percentchange",
        ScreenSort::Volume => "dayvolume",
        ScreenSort::Pe => "peratio.lasttwelvemonths",
        ScreenSort::DividendYield => "forward_dividend_yield",
    }
}

/// Yahoo's operator tree for `filter`.
fn screen_query(f: &ScreenFilter) -> Value {
    let mut operands = vec![json!({"operator": "eq", "operands": ["region", f.region]})];
    if let Some(sector) = &f.sector {
        operands.push(json!({"operator": "eq", "operands": ["sector", sector]}));
    }
    let mut range = |field: &str, lo: Option<f64>, hi: Option<f64>| match (lo, hi) {
        (Some(lo), Some(hi)) => operands.push(json!({"operator": "btwn", "operands": [field, lo, hi]})),
        (Some(lo), None) => operands.push(json!({"operator": "gte", "operands": [field, lo]})),
        (None, Some(hi)) => operands.push(json!({"operator": "lte", "operands": [field, hi]})),
        (None, None) => {}
    };
    range("intradaymarketcap", f.min_market_cap, f.max_market_cap);
    range("peratio.lasttwelvemonths", f.min_pe, f.max_pe);
    range("forward_dividend_yield", f.min_dividend_yield, None);
    range("percentchange", f.min_change, f.max_change);
    json!({"operator": "and", "operands": operands})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_screen_query() {
        let f = ScreenFilter {
            sector: Some("Technology".into()),
            min_pe: Some(0.0),
            max_pe: Some(30.0),
            max_change: Some(-2.0),
            ..ScreenFilter::default()
        };
        let q = screen_query(&f);
        let clauses: Vec<(String, String)> = q["operands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["operator"].as_str().unwrap().to_string(), c["operands"].to_string()))
            .collect();
        let has = |op: &str, operands: &str| clauses.contains(&(op.into(), operands.into()));
        assert_eq!(q["operator"], "and");
        assert!(has("eq", r#"["region","us"]"#));
        assert!(has("eq", r#"["sector","Technology"]"#));
        assert!(has("btwn", r#"["peratio.lasttwelvemonths",0.0,30.0]"#));
        assert!(has("gte", r#"["intradaymarketcap",10000000000.0]"#));
        assert!(has("lte", r#"["percentchange",-2.0]"#));
        assert_eq!(clauses.len(), 5);
    }

    #[test]
    fn spots_thai_depositary_receipts() {
        assert!(is_depositary_receipt("NVDA80.BK"));
        assert!(is_depositary_receipt("BRKB80.BK"));
        assert!(!is_depositary_receipt("PTT.BK"));
        assert!(!is_depositary_receipt("SCB.BK"));
        assert!(!is_depositary_receipt("AAPL"));
    }

    #[test]
    fn reads_a_board_row() {
        let row = to_board(&json!({
            "symbol": "AMD", "shortName": "Advanced Micro Devices, Inc.", "currency": "USD",
            "fullExchangeName": "NasdaqGS", "marketState": "POST",
            "regularMarketPrice": 236.5, "regularMarketPreviousClose": 230.0, "regularMarketOpen": 231.0,
            "regularMarketDayHigh": 238.0, "regularMarketDayLow": 229.5, "regularMarketVolume": 41_000_000,
            "averageDailyVolume3Month": 52_000_000, "bid": 236.4, "ask": 236.6,
            "fiftyTwoWeekHigh": 267.0, "fiftyTwoWeekLow": 76.5, "marketCap": 3.8e11, "trailingPE": 110.2,
            "preMarketPrice": 1.0, "postMarketPrice": 237.1, "regularMarketTime": 1794430800
        }))
        .unwrap();
        let q = &row.quote;
        assert_eq!((q.ticker.as_str(), q.name.as_str(), q.exchange.as_deref()), ("AMD", "Advanced Micro Devices, Inc.", Some("NasdaqGS")));
        assert_eq!((q.price, q.previous_close, q.open), (236.5, Some(230.0), Some(231.0)));
        assert_eq!((q.day_high, q.day_low, q.volume, q.avg_volume), (Some(238.0), Some(229.5), Some(41e6), Some(52e6)));
        assert_eq!((q.bid, q.ask, q.high_52w, q.low_52w), (Some(236.4), Some(236.6), Some(267.0), Some(76.5)));
        assert_eq!(q.session, dtos::board::Session::Post);
        assert_eq!(q.ext_price, Some(237.1), "after hours: the after-hours trade, not the morning's");
        assert_eq!((q.time, row.market_cap, q.market_cap_usd), (Some(1794430800), Some(3.8e11), None));
    }

    #[test]
    fn a_board_row_needs_a_price_and_skips_empty_fields() {
        assert!(to_board(&json!({"symbol": "X"})).is_none());
        let row = to_board(&json!({
            "symbol": "VOD.L", "currency": "GBp", "marketState": "REGULAR", "regularMarketPrice": 125.8,
            "bid": 0.0, "ask": 126.0, "regularMarketVolume": 0, "postMarketPrice": 127.0
        }))
        .unwrap();
        let q = &row.quote;
        assert_eq!((q.currency.as_str(), q.name.as_str()), ("GBP", "VOD.L"));
        assert!((q.price - 1.258).abs() < 1e-12 && (q.ask.unwrap() - 1.26).abs() < 1e-12);
        assert_eq!((q.bid, q.volume, q.ext_price), (None, None, None), "zeroes mean no data; no extended price in regular hours");
    }

    #[test]
    fn london_pence_become_pounds() {
        let q = to_quote(&json!({
            "symbol": "VOD.L", "currency": "GBp", "regularMarketPrice": 125.8,
            "fiftyTwoWeekHigh": 130.0, "marketCap": 3.0e10
        }))
        .unwrap();
        assert_eq!(q.currency, "GBP");
        assert!((q.price - 1.258).abs() < 1e-12);
        assert!((q.high_52w.unwrap() - 1.3).abs() < 1e-12);
        assert_eq!(q.market_cap, Some(3.0e10), "caps are already in pounds");
    }
}
