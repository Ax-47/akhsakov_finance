//! Direct access to two Yahoo endpoints yfinance-rs doesn't fully expose:
//! batched quotes with every field (market cap, P/E, dividend and earnings
//! dates) and the screener with full-size pages. Shared by the market-data
//! adapters; results are raw JSON objects, read with [`num`] / [`text`].

use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::Mutex;

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";
const COOKIE_URL: &str = "https://fc.yahoo.com/consent";
const CRUMB_URL: &str = "https://query1.finance.yahoo.com/v1/test/getcrumb";
const QUOTE_URL: &str = "https://query1.finance.yahoo.com/v7/finance/quote";
const SCREENER_URL: &str = "https://query1.finance.yahoo.com/v1/finance/screener";
/// Symbols per quote request.
const QUOTE_CHUNK: usize = 100;
/// Largest screener page Yahoo serves.
pub const MAX_SCREEN_SIZE: u32 = 250;

pub struct YahooRaw {
    http: reqwest::Client,
    /// (cookie, crumb), fetched on first use and after auth failures.
    auth: Mutex<Option<(String, String)>>,
}

/// One screener page.
pub struct ScreenPage {
    /// Matches in total, across pages.
    pub total: u32,
    pub quotes: Vec<Value>,
}

impl Default for YahooRaw {
    fn default() -> Self {
        Self::new()
    }
}

impl YahooRaw {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                // Yahoo drops idle connections; see `super::yahoo_client`.
                .pool_idle_timeout(Duration::from_secs(15))
                .build()
                .expect("build the HTTP client"),
            auth: Mutex::new(None),
        }
    }

    /// Quotes for `symbols`, all fields, in any order; unknown symbols are
    /// left out.
    pub async fn quotes(&self, symbols: &[String]) -> Result<Vec<Value>, String> {
        // Chunks go out together; the first one also fetches credentials.
        let chunk = |symbols: &[String]| {
            let joined = symbols.join(",");
            async move {
                let body = self
                    .authed(|http, crumb| {
                        http.get(QUOTE_URL)
                            .query(&[("symbols", joined.as_str()), ("crumb", crumb)])
                    })
                    .await?;
                Ok::<_, String>(
                    body.pointer("/quoteResponse/result")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default(),
                )
            }
        };
        let mut chunks = symbols.chunks(QUOTE_CHUNK);
        let Some(first) = chunks.next() else {
            return Ok(vec![]);
        };
        let mut out = chunk(first).await?;
        for items in futures::future::join_all(chunks.map(chunk)).await {
            out.extend(items?);
        }
        Ok(out)
    }

    /// One page of an equity screen. `query` is Yahoo's operator tree, e.g.
    /// `{"operator":"eq","operands":["region","us"]}`.
    pub async fn screen(
        &self,
        query: Value,
        sort_field: &str,
        descending: bool,
        size: u32,
        offset: u32,
    ) -> Result<ScreenPage, String> {
        let payload = json!({
            "size": size.min(MAX_SCREEN_SIZE),
            "offset": offset,
            "sortField": sort_field,
            "sortType": if descending { "DESC" } else { "ASC" },
            "quoteType": "EQUITY",
            "query": query,
            "userId": "",
            "userIdType": "guid",
        })
        .to_string();
        let body = self
            .authed(|http, crumb| {
                http.post(SCREENER_URL)
                    .query(&[
                        ("crumb", crumb),
                        ("formatted", "false"),
                        ("lang", "en-US"),
                        ("region", "US"),
                    ])
                    .header("content-type", "application/json")
                    .body(payload.clone())
            })
            .await?;
        let result = body
            .pointer("/finance/result/0")
            .ok_or_else(|| screener_error(&body))?;
        Ok(ScreenPage {
            total: result.get("total").and_then(Value::as_u64).unwrap_or(0) as u32,
            quotes: result
                .get("quotes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        })
    }

    /// Sends `request` with the cookie and crumb, refreshing them once if
    /// Yahoo rejects them. Returns the parsed JSON body.
    async fn authed(
        &self,
        request: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
    ) -> Result<Value, String> {
        for attempt in 0..2 {
            let (cookie, crumb) = self.credentials(attempt > 0).await?;
            let response = request(&self.http, &crumb)
                .header("cookie", cookie)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = response.status();
            if (status == 401 || status == 403) && attempt == 0 {
                continue;
            }
            let text = response.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("Yahoo returned {status}"));
            }
            return serde_json::from_str(&text).map_err(|e| format!("bad Yahoo response: {e}"));
        }
        Err("Yahoo rejected the request".into())
    }

    async fn credentials(&self, refresh: bool) -> Result<(String, String), String> {
        let mut auth = self.auth.lock().await;
        if refresh {
            *auth = None;
        }
        if let Some(pair) = auth.as_ref() {
            return Ok(pair.clone());
        }
        let response = self
            .http
            .get(COOKIE_URL)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let cookie = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|v| v.split(';').next())
            .map(str::trim)
            .filter(|pair| pair.contains('='))
            .collect::<Vec<_>>()
            .join("; ");
        if cookie.is_empty() {
            return Err("Yahoo sent no cookie".into());
        }
        let crumb = self
            .http
            .get(CRUMB_URL)
            .header("cookie", &cookie)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        let crumb = crumb.trim().to_string();
        if crumb.is_empty() || crumb.len() > 128 || crumb.contains(['<', '{', ' ']) {
            return Err("Yahoo sent no crumb (rate limited?)".into());
        }
        *auth = Some((cookie, crumb.clone()));
        Ok((auth.as_ref().unwrap().0.clone(), crumb))
    }
}

fn screener_error(body: &Value) -> String {
    body.pointer("/finance/error/description")
        .and_then(Value::as_str)
        .unwrap_or("screener returned no result")
        .to_string()
}

/// A numeric field; Yahoo sends plain numbers when `formatted=false` and
/// `{"raw": …}` objects otherwise.
pub fn num(v: &Value, key: &str) -> Option<f64> {
    let field = v.get(key)?;
    field
        .as_f64()
        .or_else(|| field.get("raw").and_then(Value::as_f64))
}

pub fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A unix-seconds field as `YYYY-MM-DD` (UTC).
pub fn date(v: &Value, key: &str) -> Option<String> {
    let secs = num(v, key)? as i64;
    chrono::DateTime::from_timestamp(secs, 0).map(|d| d.date_naive().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_and_formatted_fields() {
        let v = json!({"a": 1.5, "b": {"raw": 2.0, "fmt": "2.00"}, "t": "x", "d": 1767225600});
        assert_eq!(num(&v, "a"), Some(1.5));
        assert_eq!(num(&v, "b"), Some(2.0));
        assert_eq!(num(&v, "missing"), None);
        assert_eq!(text(&v, "t").as_deref(), Some("x"));
        assert_eq!(date(&v, "d").as_deref(), Some("2026-01-01"));
    }
}
