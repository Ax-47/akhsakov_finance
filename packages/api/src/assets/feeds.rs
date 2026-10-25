//! Fetched prices for hand-priced Thai assets: the Gold Traders
//! Association's gold bar price and the SEC's daily fund NAVs (its v2 open
//! data API). [`PriceSource`] is the port (tests use a fake);
//! [`WebPriceSource`] reads the real sites.

use dtos::assets::ManualPrice;
use rust_decimal::Decimal;
use serde_json::Value;
use std::{str::FromStr, time::Duration};

/// The association's latest announced prices, as JSON (what its site shows).
const GOLD_URL: &str = "https://www.goldtraders.or.th/api/GoldPrices/Latest";
/// Fund profiles; `project_info` searches ids, names and short names.
const SEC_FUND_PROFILES: &str = "https://api.sec.or.th/v2/fund/general-info/profiles";
const SEC_DAILY_NAV: &str = "https://api.sec.or.th/v2/fund/daily-info/nav";
const SEC_KEY_HEADER: &str = "Ocp-Apim-Subscription-Key";
/// The most items the SEC returns in one page.
const SEC_PAGE_SIZE: &str = "100";

/// A fund as the SEC knows it: its project id and, for a fund sold in
/// several classes, the class.
#[derive(Debug, Clone, PartialEq)]
pub struct SecFund {
    pub proj_id: String,
    pub class: Option<String>,
}

#[async_trait::async_trait]
pub trait PriceSource: Send + Sync {
    /// Baht per baht-weight the association pays for a 96.5% gold bar, on
    /// the day of its latest announcement.
    async fn thai_gold(&self) -> Result<ManualPrice, String>;
    /// The fund whose class name or short name is `name` (e.g.
    /// K-CHANGE-SSF or SCBSET).
    async fn find_fund(&self, key: &str, name: &str) -> Result<SecFund, String>;
    /// The fund's latest NAV per unit from `from` to `to` (`YYYY-MM-DD`),
    /// with its date; `None` when it wasn't priced in that time.
    async fn latest_nav(&self, key: &str, fund: &SecFund, from: &str, to: &str) -> Result<Option<ManualPrice>, String>;
}

pub struct WebPriceSource {
    http: reqwest::Client,
}

impl WebPriceSource {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("akhsakov-finance/1.0 (personal portfolio app)")
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("build the HTTP client");
        Self { http }
    }

    /// Sends a request to the SEC, turning a refused key into a message
    /// that says so.
    async fn sec(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
        let response = request.send().await.map_err(|e| format!("The SEC's API didn't answer: {e}"))?;
        match response.status().as_u16() {
            401 | 403 => Err("The SEC didn't accept the API key; check it in Settings".into()),
            421 | 429 => Err("The SEC's API is limiting requests; try again later".into()),
            s if !(200..300).contains(&s) => Err(format!("The SEC's API answered {s}")),
            _ => Ok(response),
        }
    }
}

#[async_trait::async_trait]
impl PriceSource for WebPriceSource {
    async fn thai_gold(&self) -> Result<ManualPrice, String> {
        let body: Value = self
            .http
            .get(GOLD_URL)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::REFERER, "https://www.goldtraders.or.th/")
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("The Gold Traders Association's site didn't answer: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        gold_bar_buy_back(&body).ok_or_else(|| "The Gold Traders Association's answer had no gold bar price".into())
    }

    async fn find_fund(&self, key: &str, name: &str) -> Result<SecFund, String> {
        let query = [("project_info", name.trim()), ("page_size", SEC_PAGE_SIZE)];
        let request = self.http.get(SEC_FUND_PROFILES).header(SEC_KEY_HEADER, key).query(&query);
        let body: Value = self.sec(request).await?.json().await.map_err(|e| e.to_string())?;
        fund_in(&body, name)
    }

    async fn latest_nav(&self, key: &str, fund: &SecFund, from: &str, to: &str) -> Result<Option<ManualPrice>, String> {
        let mut query = vec![
            ("proj_id", fund.proj_id.as_str()),
            ("start_nav_date", from),
            ("end_nav_date", to),
            ("page_size", SEC_PAGE_SIZE),
        ];
        if let Some(class) = &fund.class {
            query.push(("fund_class_name", class));
        }
        let request = self.http.get(SEC_DAILY_NAV).header(SEC_KEY_HEADER, key).query(&query);
        let text = self.sec(request).await?.text().await.map_err(|e| e.to_string())?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        let body: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        Ok(latest_nav_in(&body, fund.class.as_deref()))
    }
}

/// The gold bar buy-back price (`bL_BuyPrice`) in the association's latest
/// prices, on the day of `asTime` (Bangkok time, `YYYY-MM-DDTHH:MM:SS`).
fn gold_bar_buy_back(body: &Value) -> Option<ManualPrice> {
    let date = text(&body["asTime"])?.get(..10)?.to_string();
    let price = Decimal::from_str(&text(&body["bL_BuyPrice"])?.replace(',', "")).ok().filter(|p| *p > Decimal::ZERO)?;
    Some(ManualPrice { date, price })
}

/// The items of an SEC answer: `{"items": [...], "next_cursor": ...}`.
fn items(body: &Value) -> impl Iterator<Item = &Value> {
    body["items"].as_array().or_else(|| body.as_array()).into_iter().flatten()
}

/// A text field that may also come as a number; `None` when empty.
fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The fund named `name` in a fund profile search: the class with that
/// name, or else the fund with that short name (and its class, when it has
/// just one). Funds still open come before closed ones.
fn fund_in(body: &Value, name: &str) -> Result<SecFund, String> {
    let name = name.trim();
    let named = |v: &Value| text(v).is_some_and(|t| t.eq_ignore_ascii_case(name));
    let mut funds: Vec<&Value> = items(body).filter(|f| text(&f["proj_id"]).is_some()).collect();
    funds.sort_by_key(|f| f["fund_status"].as_str() != Some("Registered"));
    if let Some(f) = funds.iter().find(|f| named(&f["fund_class_name"])) {
        return Ok(SecFund { proj_id: text(&f["proj_id"]).unwrap_or_default(), class: text(&f["fund_class_name"]) });
    }
    let fund = funds.iter().find(|f| named(&f["proj_abbr_name"])).ok_or_else(|| format!("The SEC has no fund called {name}"))?;
    let proj_id = text(&fund["proj_id"]).unwrap_or_default();
    let mut classes: Vec<String> =
        funds.iter().filter(|f| text(&f["proj_id"]).as_ref() == Some(&proj_id)).filter_map(|f| text(&f["fund_class_name"])).collect();
    classes.sort();
    classes.dedup();
    match classes.len() {
        0 | 1 => Ok(SecFund { proj_id, class: classes.pop() }),
        _ => Err(format!("{name} is sold in several classes ({}); use the class you hold as the ticker", classes.join(", "))),
    }
}

/// The latest NAV per unit in a daily NAV answer, for `class` when given.
fn latest_nav_in(body: &Value, class: Option<&str>) -> Option<ManualPrice> {
    items(body)
        .filter(|n| class.is_none_or(|c| text(&n["fund_class_name"]).is_none_or(|t| t.eq_ignore_ascii_case(c))))
        .filter_map(|n| {
            let date = text(&n["nav_date"])?.get(..10)?.to_string();
            let price = Decimal::from_str(&text(&n["last_val"])?).ok().filter(|p| *p > Decimal::ZERO)?;
            Some(ManualPrice { date, price })
        })
        .max_by(|a, b| a.date.cmp(&b.date))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use serde_json::json;

    #[test]
    fn reads_the_gold_bar_price() {
        let latest = json!({"goldPriceID": 9001, "asTime": "2026-10-20T09:31:00", "seq": 1,
            "bL_BuyPrice": 41950.0, "bL_SellPrice": 42050.0, "oM965_BuyPrice": 41110.24});
        let on = |d: &str, p| Some(ManualPrice { date: d.into(), price: p });
        assert_eq!(gold_bar_buy_back(&latest), on("2026-10-20", dec!(41950)));
        assert_eq!(gold_bar_buy_back(&json!({"asTime": "2026-10-20T09:31:00", "bL_BuyPrice": "41,950.00"})), on("2026-10-20", dec!(41950)));
        assert_eq!(gold_bar_buy_back(&json!({"asTime": "2026-10-20T09:31:00", "bL_BuyPrice": null})), None);
        assert_eq!(gold_bar_buy_back(&json!({"bL_BuyPrice": 41950})), None);
    }

    #[test]
    fn finds_the_fund_or_its_class() {
        let body = json!({"message": "OK", "page_size": 100, "next_cursor": "", "items": [
            {"proj_id": "M0001_2550", "proj_abbr_name": "K-CHANGE", "fund_status": "Registered", "fund_class_name": "K-CHANGE-A(A)"},
            {"proj_id": "M0001_2550", "proj_abbr_name": "K-CHANGE", "fund_status": "Registered", "fund_class_name": "K-CHANGE-SSF"},
            {"proj_id": "M0099_2540", "proj_abbr_name": "SCBSET", "fund_status": "Canceled", "fund_class_name": null},
            {"proj_id": "M0200_2555", "proj_abbr_name": "SCBSET", "fund_status": "Registered", "fund_class_name": ""},
            {"proj_id": "M0300_2560", "proj_abbr_name": "KFGBRAND", "fund_status": "Registered", "fund_class_name": "KFGBRAND-A"}
        ]});
        let fund = |id: &str, class: Option<&str>| Ok(SecFund { proj_id: id.into(), class: class.map(Into::into) });
        assert_eq!(fund_in(&body, "k-change-ssf"), fund("M0001_2550", Some("K-CHANGE-SSF")));
        // An open fund wins over a closed one with the same short name.
        assert_eq!(fund_in(&body, "SCBSET"), fund("M0200_2555", None));
        // A fund with one class gets it; with several, the class must be named.
        assert_eq!(fund_in(&body, "KFGBRAND"), fund("M0300_2560", Some("KFGBRAND-A")));
        assert!(fund_in(&body, "K-CHANGE").unwrap_err().contains("K-CHANGE-A(A), K-CHANGE-SSF"));
        assert!(fund_in(&body, "NOPE").is_err());
        assert!(fund_in(&json!({}), "X").is_err());
    }

    #[test]
    fn reads_the_latest_nav() {
        let body = json!({"items": [
            {"proj_id": "M1", "nav_date": "2026-10-16", "fund_class_name": "A", "last_val": 12.1},
            {"proj_id": "M1", "nav_date": "2026-10-17", "fund_class_name": "A", "last_val": 12.3456},
            {"proj_id": "M1", "nav_date": "2026-10-19", "fund_class_name": "B", "last_val": 9.5},
            {"proj_id": "M1", "nav_date": "2026-10-20", "fund_class_name": "A", "last_val": null}
        ]});
        let price = |d: &str, p| Some(ManualPrice { date: d.into(), price: p });
        assert_eq!(latest_nav_in(&body, Some("A")), price("2026-10-17", dec!(12.3456)));
        assert_eq!(latest_nav_in(&body, None), price("2026-10-19", dec!(9.5)));
        let dated = json!({"items": [{"nav_date": "2026-10-20T00:00:00", "last_val": "10.5"}]});
        assert_eq!(latest_nav_in(&dated, None), price("2026-10-20", dec!(10.5)));
        assert_eq!(latest_nav_in(&json!({"items": []}), None), None);
    }
}
