//! Adapters: FRED (St. Louis Fed) CSV downloads, and Yahoo quotes.

use crate::{
    economy::repositories::{GaugeSource, Level, MacroSource},
    shared::yahoo_raw::{num, text, YahooRaw},
};
use async_trait::async_trait;
use std::time::Duration;

/// FRED's public CSV download; no API key needed.
pub struct FredSource {
    http: reqwest::Client,
}

impl FredSource {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                // FRED's CDN stalls clients it doesn't recognise as download
                // tools; the CSV endpoint is meant for curl-style scripts.
                .user_agent("curl/8.0 (akhsakov-finance)")
                .timeout(Duration::from_secs(20))
                .build()
                .expect("build the HTTP client"),
        }
    }
}

#[async_trait]
impl MacroSource for FredSource {
    async fn series(&self, id: &str, start: &str) -> Result<Vec<(String, f64)>, String> {
        let csv = self
            .http
            .get("https://fred.stlouisfed.org/graph/fredgraph.csv")
            .query(&[("id", id), ("cosd", start)])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        Ok(parse_fred_csv(&csv))
    }
}

/// `observation_date,ID` rows; FRED writes `.` or nothing for gaps.
fn parse_fred_csv(csv: &str) -> Vec<(String, f64)> {
    csv.lines()
        .skip(1)
        .filter_map(|line| {
            let (date, value) = line.split_once(',')?;
            Some((date.trim().to_string(), value.trim().parse().ok()?))
        })
        .collect()
}

pub struct YahooGauges {
    raw: YahooRaw,
}

impl YahooGauges {
    pub fn new() -> Self {
        Self { raw: YahooRaw::new() }
    }
}

#[async_trait]
impl GaugeSource for YahooGauges {
    async fn levels(&self, symbols: &[String]) -> Result<Vec<Level>, String> {
        Ok(self
            .raw
            .quotes(symbols)
            .await?
            .iter()
            .filter_map(|q| {
                Some(Level {
                    symbol: text(q, "symbol")?,
                    value: num(q, "regularMarketPrice")?,
                    change_pct: num(q, "regularMarketChangePercent"),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fred_csv_skipping_gaps() {
        let csv = "observation_date,DGS10\n2026-09-24,4.12\n2026-09-25,.\n2026-09-26,\n2026-09-29,4.2\n";
        assert_eq!(
            parse_fred_csv(csv),
            vec![("2026-09-24".into(), 4.12), ("2026-09-29".into(), 4.2)]
        );
    }
}
