//! Index membership from Wikipedia's constituent tables, cached in SQLite
//! for a week. If Wikipedia can't be reached the cache is used however old
//! it is, and for the S&P 500 a built-in list covers a fresh install.

use super::sp500;
use crate::{
    database::Database,
    market::repositories::{Constituent, IndexRepository},
};
use async_trait::async_trait;
use dtos::market::MarketIndex;
use rusqlite::params;
use std::time::Duration;

/// Cached members are refreshed after this long.
const MAX_AGE_DAYS: i64 = 7;
/// A parsed table with fewer rows is treated as a page-layout change.
const MIN_MEMBERS: usize = 20;

/// Where each index's table is and which columns to read.
struct Source {
    page: &'static str,
    ticker: &'static [&'static str],
    name: &'static [&'static str],
    sector: &'static [&'static str],
    /// Appended to tickers, e.g. `.BK` for Bangkok.
    suffix: &'static str,
}

fn source(index: MarketIndex) -> Source {
    match index {
        MarketIndex::Sp500 => Source {
            page: "List_of_S%26P_500_companies",
            ticker: &["symbol"],
            name: &["security", "company"],
            sector: &["gics sector"],
            suffix: "",
        },
        MarketIndex::Nasdaq100 => Source {
            page: "List_of_NASDAQ-100_companies",
            ticker: &["ticker", "symbol"],
            name: &["company"],
            sector: &["icb industry", "gics sector", "sector"],
            suffix: "",
        },
        MarketIndex::Dow30 => Source {
            page: "List_of_Dow_Jones_Industrial_Average_companies",
            ticker: &["symbol", "ticker"],
            name: &["company"],
            sector: &["sector", "industry"],
            suffix: "",
        },
        MarketIndex::Set50 => Source {
            page: "SET50_Index",
            ticker: &["symbol"],
            name: &["securities name", "company"],
            sector: &["sector"],
            suffix: ".BK",
        },
    }
}

pub struct WikipediaIndexRepository {
    db: Database,
    http: reqwest::Client,
}

impl WikipediaIndexRepository {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            http: reqwest::Client::builder()
                .user_agent("AkhsakovFinance/1.0 (personal portfolio app)")
                .timeout(Duration::from_secs(20))
                .build()
                .expect("build the HTTP client"),
        }
    }

    async fn fetch(&self, index: MarketIndex) -> Result<Vec<Constituent>, String> {
        let src = source(index);
        let html = self
            .http
            .get(format!("https://en.wikipedia.org/wiki/{}", src.page))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        let members = parse_members(&html, &src);
        if members.len() < MIN_MEMBERS {
            return Err(format!("couldn't read the {} table", index.label()));
        }
        Ok(members)
    }

    /// Cached members and whether they're fresh.
    fn cached(&self, index: MarketIndex) -> Option<(Vec<Constituent>, bool)> {
        let key = index.key();
        self.db
            .with(|c| {
                let fresh: bool = c
                    .query_row(
                        "SELECT julianday('now') - julianday(fetched_at) < ?2
                         FROM index_fetched WHERE index_key = ?1",
                        params![key, MAX_AGE_DAYS],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                let members = c
                    .prepare(
                        "SELECT ticker, name, sector FROM index_members
                         WHERE index_key = ?1 ORDER BY position",
                    )?
                    .query_map([key], |r| {
                        Ok(Constituent {
                            ticker: r.get(0)?,
                            name: r.get(1)?,
                            sector: r.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok((members, fresh))
            })
            .ok()
            .filter(|(members, _)| !members.is_empty())
    }

    fn store(&self, index: MarketIndex, members: &[Constituent]) {
        let key = index.key();
        let result = self.db.transaction(|tx| {
            tx.execute("DELETE FROM index_members WHERE index_key = ?1", [key])?;
            for (i, m) in members.iter().enumerate() {
                tx.execute(
                    "INSERT INTO index_members (index_key, position, ticker, name, sector)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![key, i as i64, m.ticker, m.name, m.sector],
                )?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO index_fetched (index_key, fetched_at)
                 VALUES (?1, datetime('now'))",
                [key],
            )?;
            Ok(())
        });
        if let Err(e) = result {
            tracing::warn!("couldn't cache {} members: {e}", index.label());
        }
    }
}

#[async_trait]
impl IndexRepository for WikipediaIndexRepository {
    async fn constituents(&self, index: MarketIndex) -> Result<Vec<Constituent>, String> {
        let cached = self.cached(index);
        if let Some((members, true)) = &cached {
            return Ok(members.clone());
        }
        match self.fetch(index).await {
            Ok(members) => {
                self.store(index, &members);
                Ok(members)
            }
            Err(e) => {
                tracing::warn!("{} members from Wikipedia: {e}", index.label());
                match (cached, index) {
                    (Some((members, _)), _) => Ok(members),
                    (None, MarketIndex::Sp500) => Ok(sp500::SP500
                        .iter()
                        .map(|(ticker, name, sector)| Constituent {
                            ticker: ticker.to_string(),
                            name: name.to_string(),
                            sector: sector.to_string(),
                        })
                        .collect()),
                    (None, _) => Err(e),
                }
            }
        }
    }
}

// ─── HTML ─────────────────────────────────────────────────────────────────────

/// Members from the first table whose header has ticker, name and sector
/// columns.
fn parse_members(html: &str, src: &Source) -> Vec<Constituent> {
    for table in tables(html) {
        let Some(header) = table.first() else { continue };
        let header: Vec<String> = header.iter().map(|h| normalize_header(h)).collect();
        let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
        let (Some(t), Some(n), Some(s)) = (col(src.ticker), col(src.name), col(src.sector)) else {
            continue;
        };
        return table[1..]
            .iter()
            .filter_map(|row| {
                let ticker = yahoo_ticker(row.get(t)?, src.suffix)?;
                Some(Constituent {
                    ticker,
                    name: row.get(n)?.clone(),
                    sector: row.get(s).cloned().unwrap_or_default(),
                })
            })
            .collect();
    }
    vec![]
}

/// `BRK.B` → `BRK-B`; appends the exchange suffix. Rejects non-symbols.
fn yahoo_ticker(raw: &str, suffix: &str) -> Option<String> {
    let t = raw.trim().to_uppercase().replace('.', "-");
    let ok = !t.is_empty()
        && t.len() <= 8
        && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    ok.then(|| format!("{t}{suffix}"))
}

/// Lowercase, without footnotes like `[1]`.
fn normalize_header(h: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in h.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_lowercase()
}

/// Every `<table>` as rows of cell texts (header row included).
fn tables(html: &str) -> Vec<Vec<Vec<String>>> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<table").map(|i| i + pos) {
        let Some(end) = lower[start..].find("</table>").map(|i| i + start) else { break };
        let (body, body_lower) = (&html[start..end], &lower[start..end]);
        let rows: Vec<Vec<String>> = split_tag(body, body_lower, "tr")
            .into_iter()
            .map(|(row, row_lower)| {
                let mut cells = split_tag(row, row_lower, "td");
                cells.extend(split_tag(row, row_lower, "th"));
                // Keep document order across <th> and <td>.
                cells.sort_by_key(|(c, _)| c.as_ptr() as usize);
                cells.into_iter().map(|(c, _)| clean(c)).collect()
            })
            .filter(|cells: &Vec<String>| !cells.is_empty())
            .collect();
        out.push(rows);
        pos = end + 8;
    }
    out
}

/// Inner HTML of each `<tag …>…</tag>` in `s` (not nested).
fn split_tag<'a>(s: &'a str, lower: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find(&open).map(|i| i + pos) {
        // Skip e.g. <th> matching <thead>.
        let after = lower.as_bytes().get(i + open.len()).copied();
        if !matches!(after, Some(b'>' | b' ' | b'\n' | b'\t')) {
            pos = i + open.len();
            continue;
        }
        let Some(content) = lower[i..].find('>').map(|j| i + j + 1) else { break };
        let end = lower[content..].find(&close).map(|j| content + j).unwrap_or(lower.len());
        out.push((&s[content..end], &lower[content..end]));
        pos = end;
    }
    out
}

/// Text of a cell: tags removed, entities decoded, whitespace collapsed.
fn clean(cell: &str) -> String {
    let mut text = String::with_capacity(cell.len());
    let mut in_tag = false;
    for c in cell.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"
        <table class="infobox"><tr><th>Ticker</th><td>ignored</td></tr></table>
        <table class="wikitable sortable" id="constituents"><tbody>
        <tr><th>Symbol</th><th>Security</th><th>GICS Sector<sup>[1]</sup></th></tr>
        <tr><td><a href="x">BRK.B</a></td><td>Berkshire Hathaway</td><td>Financials</td></tr>
        <tr><td>AT&amp;T</td><td>bad</td><td>x</td></tr>
        <tr>
          <td>AOS</td><td>A. O. Smith</td><td>Industrials</td></tr>
        </tbody></table>"#;

    #[test]
    fn reads_the_constituents_table() {
        let src = source(MarketIndex::Sp500);
        let members = parse_members(HTML, &src);
        assert_eq!(members.len(), 2, "rows without a valid ticker are skipped");
        assert_eq!(members[0].ticker, "BRK-B");
        assert_eq!(members[0].sector, "Financials");
        assert_eq!(members[1].name, "A. O. Smith");
    }

    #[test]
    fn thai_tickers_get_the_exchange_suffix() {
        let html = "<table><tr><th>Symbol</th><th>Securities Name</th><th>Sector</th></tr>\
                    <tr><td>AOT</td><td>Airports of Thailand</td><td>Transportation &amp; Logistics</td></tr></table>";
        let members = parse_members(html, &source(MarketIndex::Set50));
        assert_eq!(members[0].ticker, "AOT.BK");
        assert_eq!(members[0].sector, "Transportation & Logistics");
    }

    #[test]
    fn caches_members_in_sqlite() {
        let repo = WikipediaIndexRepository::new(Database::in_memory().unwrap());
        assert!(repo.cached(MarketIndex::Dow30).is_none());
        let members = vec![Constituent {
            ticker: "AAPL".into(),
            name: "Apple".into(),
            sector: "Tech".into(),
        }];
        repo.store(MarketIndex::Dow30, &members);
        assert_eq!(repo.cached(MarketIndex::Dow30), Some((members, true)));
    }
}
