//! Market overview use cases: index and watchlist heatmaps, the screener,
//! an earnings / dividend calendar and sector peers. Everything is priced
//! in USD. Results are cached briefly so many open pages share one
//! upstream request.

use crate::{
    market::repositories::{
        Constituent, IndexRepository, MarketDataGateway, MarketQuote, TickerCalendar,
    },
    shared::{FxRates, ServiceError},
};
use chrono::{Duration as Days, NaiveDate};
use dtos::insights::DividendInfo;
use dtos::market::{
    region_currency, CalendarEvent, EventKind, HeatmapItem, MarketIndex, PeerGroup,
    ScreenFilter, ScreenResult, StockRow,
};
use rust_decimal::prelude::ToPrimitive;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use types::ticker_symbol::TickerSymbol;

/// Prices are refreshed at most this often.
const QUOTE_TTL: Duration = Duration::from_secs(30);
const SCREEN_TTL: Duration = Duration::from_secs(60);
/// Dividend dates change rarely.
const CALENDAR_TTL: Duration = Duration::from_secs(12 * 3600);
pub const MAX_TICKERS: usize = 100;
const MAX_PEERS: usize = 10;
/// Calendar window around today.
const CALENDAR_PAST_DAYS: i64 = 14;
const CALENDAR_AHEAD_DAYS: i64 = 120;

type Cache<K, V> = Arc<Mutex<HashMap<K, (Instant, V)>>>;

#[derive(Clone)]
pub struct MarketService {
    index: Arc<dyn IndexRepository>,
    gateway: Arc<dyn MarketDataGateway>,
    fx: Arc<dyn FxRates>,
    index_cache: Cache<MarketIndex, Vec<HeatmapItem>>,
    screen_cache: Cache<String, ScreenResult>,
    calendar_cache: Cache<String, TickerCalendar>,
}

/// Reads a cache entry younger than `ttl`.
async fn cached<K: std::hash::Hash + Eq, V: Clone>(
    cache: &Cache<K, V>,
    key: &K,
    ttl: Duration,
) -> Option<V> {
    let cache = cache.lock().await;
    cache
        .get(key)
        .filter(|(at, _)| at.elapsed() < ttl)
        .map(|(_, v)| v.clone())
}

fn upstream(e: String) -> ServiceError {
    ServiceError::Upstream(e)
}

impl MarketService {
    pub fn new(
        index: Arc<dyn IndexRepository>,
        gateway: Arc<dyn MarketDataGateway>,
        fx: Arc<dyn FxRates>,
    ) -> Self {
        Self {
            index,
            gateway,
            fx,
            index_cache: Default::default(),
            screen_cache: Default::default(),
            calendar_cache: Default::default(),
        }
    }

    /// USD per unit of each currency in `quotes`.
    async fn usd_rates(&self, quotes: &[MarketQuote]) -> Result<HashMap<String, f64>, ServiceError> {
        let mut rates = HashMap::from([("USD".to_string(), 1.0)]);
        for q in quotes {
            if !rates.contains_key(&q.currency) {
                let rate = self.usd_rate(&q.currency).await?;
                rates.insert(q.currency.clone(), rate);
            }
        }
        Ok(rates)
    }

    async fn usd_rate(&self, currency: &str) -> Result<f64, ServiceError> {
        if currency == "USD" {
            return Ok(1.0);
        }
        self.fx
            .usd_per_unit_now(currency)
            .await
            .map_err(upstream)?
            .to_f64()
            .ok_or_else(|| upstream(format!("bad {currency} rate")))
    }

    async fn quotes(
        &self,
        tickers: &[String],
    ) -> Result<(Vec<MarketQuote>, HashMap<String, f64>), ServiceError> {
        let quotes = self.gateway.quotes(tickers).await.map_err(upstream)?;
        let rates = self.usd_rates(&quotes).await?;
        Ok((quotes, rates))
    }

    /// Every constituent with a quote, largest first.
    pub async fn index_heatmap(&self, index: MarketIndex) -> Result<Vec<HeatmapItem>, ServiceError> {
        if let Some(items) = cached(&self.index_cache, &index, QUOTE_TTL).await {
            return Ok(items);
        }
        let members = self.index.constituents(index).await.map_err(upstream)?;
        let tickers: Vec<String> = members.iter().map(|m| m.ticker.clone()).collect();
        let (quotes, rates) = self.quotes(&tickers).await?;
        let mut items: Vec<HeatmapItem> = members
            .iter()
            .filter_map(|m| {
                let q = quotes.iter().find(|q| q.ticker == m.ticker)?;
                let rate = rates.get(&q.currency).copied()?;
                Some(heat_item(q, Some(m), rate))
            })
            .collect();
        if items.is_empty() {
            return Err(upstream("No prices available right now".into()));
        }
        items.sort_by(|a, b| b.market_cap.unwrap_or(0.0).total_cmp(&a.market_cap.unwrap_or(0.0)));
        self.index_cache
            .lock()
            .await
            .insert(index, (Instant::now(), items.clone()));
        Ok(items)
    }

    /// Any stocks (e.g. a watchlist), in the order given, priced in USD.
    pub async fn tickers_heatmap(
        &self,
        tickers: Vec<TickerSymbol>,
    ) -> Result<Vec<HeatmapItem>, ServiceError> {
        let tickers = limit(tickers)?;
        let (quotes, rates) = self.quotes(&tickers).await?;
        Ok(tickers
            .iter()
            .filter_map(|t| {
                let q = quotes.iter().find(|q| &q.ticker == t)?;
                let rate = rates.get(&q.currency).copied()?;
                Some(heat_item(q, None, rate))
            })
            .collect())
    }

    /// One page of stocks matching `filter`. Money bounds are USD and are
    /// converted to the market's currency for Yahoo.
    pub async fn screen(&self, filter: ScreenFilter) -> Result<ScreenResult, ServiceError> {
        filter.validate().map_err(ServiceError::Validation)?;
        let key = serde_json::to_string(&filter).unwrap_or_default();
        if let Some(result) = cached(&self.screen_cache, &key, SCREEN_TTL).await {
            return Ok(result);
        }
        let local_rate = self.usd_rate(region_currency(&filter.region)).await?;
        let to_local = |usd: Option<f64>| usd.map(|v| v / local_rate);
        let local = ScreenFilter {
            min_market_cap: to_local(filter.min_market_cap),
            max_market_cap: to_local(filter.max_market_cap),
            ..filter.clone()
        };
        let (total, quotes) = self.gateway.screen(&local).await.map_err(upstream)?;
        let rates = self.usd_rates(&quotes).await?;
        let result = ScreenResult {
            total,
            rows: quotes
                .iter()
                .filter_map(|q| Some(row(q, None, rates.get(&q.currency).copied()?)))
                .collect(),
        };
        self.screen_cache
            .lock()
            .await
            .insert(key, (Instant::now(), result.clone()));
        Ok(result)
    }

    /// Earnings and dividend dates for `tickers` from two weeks ago to four
    /// months ahead, soonest first.
    pub async fn calendar(
        &self,
        tickers: Vec<TickerSymbol>,
    ) -> Result<Vec<CalendarEvent>, ServiceError> {
        let tickers = limit(tickers)?;
        let today = chrono::Utc::now().date_naive();
        let (quotes, rates) = self.quotes(&tickers).await?;
        let mut events = Vec::new();
        for q in &quotes {
            let rate = rates.get(&q.currency).copied().unwrap_or(1.0);
            // Ex-dividend dates only come from the per-stock calendar.
            let cal = if q.dividend_rate.is_some_and(|d| d > 0.0) {
                self.ticker_calendar(&q.ticker).await
            } else {
                TickerCalendar::default()
            };
            events.extend(events_for(q, &cal, rate));
        }
        let (from, to) = (
            today - Days::days(CALENDAR_PAST_DAYS),
            today + Days::days(CALENDAR_AHEAD_DAYS),
        );
        events.retain(|e| {
            NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").is_ok_and(|d| d >= from && d <= to)
        });
        events.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then(a.kind.cmp(&b.kind))
                .then(a.ticker.cmp(&b.ticker))
        });
        events.dedup_by(|a, b| a.date == b.date && a.kind == b.kind && a.ticker == b.ticker);
        Ok(events)
    }

    /// Forecast dividends for `tickers` (payers only), in USD, with their
    /// next ex-dividend and payment dates.
    pub async fn dividends(&self, tickers: Vec<TickerSymbol>) -> Result<Vec<DividendInfo>, ServiceError> {
        let tickers = limit(tickers)?;
        let (quotes, rates) = self.quotes(&tickers).await?;
        let today = chrono::Utc::now().date_naive().to_string();
        let mut out = Vec::new();
        for q in quotes.iter().filter(|q| q.dividend_rate.is_some_and(|d| d > 0.0)) {
            let rate = rates.get(&q.currency).copied().unwrap_or(1.0);
            let cal = self.ticker_calendar(&q.ticker).await;
            let upcoming = |d: &Option<String>| d.clone().filter(|d| *d >= today);
            out.push(DividendInfo {
                ticker: q.ticker.clone(),
                annual_per_share: q.dividend_rate.unwrap_or(0.0) * rate,
                current_yield: q.dividend_yield_pct.map(|y| y / 100.0),
                next_ex_date: upcoming(&cal.ex_dividend),
                next_payment: upcoming(&cal.payment).or_else(|| upcoming(&q.dividend_date)),
            });
        }
        Ok(out)
    }

    /// Cached; failures count as "no dates" so one stock can't sink the list.
    async fn ticker_calendar(&self, ticker: &str) -> TickerCalendar {
        let key = ticker.to_string();
        if let Some(c) = cached(&self.calendar_cache, &key, CALENDAR_TTL).await {
            return c;
        }
        match self.gateway.calendar(ticker).await {
            Ok(c) => {
                self.calendar_cache
                    .lock()
                    .await
                    .insert(key, (Instant::now(), c.clone()));
                c
            }
            Err(e) => {
                tracing::warn!("calendar for {ticker}: {e}");
                TickerCalendar::default()
            }
        }
    }

    /// The largest stocks in `ticker`'s sector, from the first index that
    /// lists it; `None` if no index does.
    pub async fn peers(&self, ticker: TickerSymbol) -> Result<Option<PeerGroup>, ServiceError> {
        for index in MarketIndex::ALL {
            let Ok(members) = self.index.constituents(index).await else {
                continue;
            };
            let Some(me) = members.iter().find(|m| m.ticker == ticker.as_str()) else {
                continue;
            };
            let same: Vec<&Constituent> = members.iter().filter(|m| m.sector == me.sector).collect();
            let tickers: Vec<String> = same.iter().map(|m| m.ticker.clone()).collect();
            let (quotes, rates) = self.quotes(&tickers).await?;
            let mut rows: Vec<StockRow> = same
                .iter()
                .filter_map(|m| {
                    let q = quotes.iter().find(|q| q.ticker == m.ticker)?;
                    Some(row(q, Some(&m.name), rates.get(&q.currency).copied()?))
                })
                .collect();
            rows.sort_by(|a, b| b.market_cap.unwrap_or(0.0).total_cmp(&a.market_cap.unwrap_or(0.0)));
            // Keep the stock itself even if it's not among the largest.
            let mine = rows.iter().position(|r| r.ticker == ticker.as_str());
            if let Some(i) = mine.filter(|i| *i >= MAX_PEERS) {
                let own = rows.remove(i);
                rows.truncate(MAX_PEERS - 1);
                rows.push(own);
            }
            rows.truncate(MAX_PEERS);
            return Ok(Some(PeerGroup {
                index,
                sector: me.sector.clone(),
                rows,
            }));
        }
        Ok(None)
    }
}

fn limit(tickers: Vec<TickerSymbol>) -> Result<Vec<String>, ServiceError> {
    if tickers.len() > MAX_TICKERS {
        return Err(ServiceError::Validation(format!(
            "At most {MAX_TICKERS} stocks at a time"
        )));
    }
    Ok(tickers.into_iter().map(Into::into).collect())
}

/// `q` in USD at `rate`.
fn row(q: &MarketQuote, name: Option<&str>, rate: f64) -> StockRow {
    StockRow {
        ticker: q.ticker.clone(),
        name: name
            .map(str::to_string)
            .or_else(|| q.name.clone())
            .unwrap_or_else(|| q.ticker.clone()),
        price: q.price * rate,
        change_pct: q.change_pct,
        market_cap: q.market_cap.map(|c| c * rate),
        pe: q.pe,
        forward_pe: q.forward_pe,
        dividend_yield: q.dividend_yield_pct.map(|y| y / 100.0),
        volume: q.volume,
        high_52w: q.high_52w.map(|v| v * rate),
        low_52w: q.low_52w.map(|v| v * rate),
    }
}

/// `q` as a heatmap tile in USD; `member` supplies name and sector.
fn heat_item(q: &MarketQuote, member: Option<&Constituent>, rate: f64) -> HeatmapItem {
    let r = row(q, member.map(|m| m.name.as_str()), rate);
    HeatmapItem {
        ticker: r.ticker,
        name: r.name,
        sector: member.map(|m| m.sector.clone()),
        price: r.price,
        change_pct: r.change_pct,
        market_cap: r.market_cap,
        pe: r.pe,
        forward_pe: r.forward_pe,
        dividend_yield: r.dividend_yield,
        high_52w: r.high_52w,
        low_52w: r.low_52w,
    }
}

/// Earnings and dividend events for one stock from its quote and calendar.
fn events_for(q: &MarketQuote, cal: &TickerCalendar, rate: f64) -> Vec<CalendarEvent> {
    let name = q.name.clone().unwrap_or_else(|| q.ticker.clone());
    let annual = q.dividend_rate.map(|d| d * rate);
    let event = |date: &str, kind: EventKind, estimated: bool| CalendarEvent {
        date: date.to_string(),
        ticker: q.ticker.clone(),
        name: name.clone(),
        kind,
        annual_dividend: (kind != EventKind::Earnings).then_some(annual).flatten(),
        estimated,
    };
    let mut out: Vec<CalendarEvent> = q
        .earnings_dates
        .iter()
        .chain(&cal.earnings)
        .map(|d| event(d, EventKind::Earnings, q.earnings_estimated))
        .collect();
    out.extend(cal.ex_dividend.iter().map(|d| event(d, EventKind::ExDividend, false)));
    out.extend(
        cal.payment
            .iter()
            .chain(&q.dividend_date)
            .map(|d| event(d, EventKind::DividendPayment, false)),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeIndex;

    #[async_trait]
    impl IndexRepository for FakeIndex {
        async fn constituents(&self, index: MarketIndex) -> Result<Vec<Constituent>, String> {
            let m = |t: &str, s: &str| Constituent {
                ticker: t.into(),
                name: format!("{t} Corp"),
                sector: s.into(),
            };
            match index {
                MarketIndex::Sp500 => Ok(vec![
                    m("SMALL", "Tech"),
                    m("BIG", "Energy"),
                    m("MID", "Tech"),
                    m("GONE", "Tech"),
                ]),
                MarketIndex::Set50 => Ok(vec![m("PTT.BK", "Energy")]),
                _ => Err("offline".into()),
            }
        }
    }

    /// Prices at 110 (+10%); caps by name; `.BK` stocks in THB; counts
    /// quote requests.
    #[derive(Default)]
    struct FakeGateway {
        requests: AtomicUsize,
        last_screen: std::sync::Mutex<Option<ScreenFilter>>,
    }

    fn quote(t: &str) -> MarketQuote {
        MarketQuote {
            ticker: t.into(),
            name: Some(format!("{t} Inc")),
            currency: if t.ends_with(".BK") { "THB" } else { "USD" }.into(),
            price: 110.0,
            change_pct: Some(10.0),
            market_cap: Some(match t {
                "BIG" => 1000.0,
                "MID" => 500.0,
                _ => 100.0,
            }),
            dividend_yield_pct: Some(2.0),
            dividend_rate: (t != "SMALL").then_some(4.0),
            earnings_dates: vec!["2099-01-01".into()],
            ..Default::default()
        }
    }

    #[async_trait]
    impl MarketDataGateway for FakeGateway {
        async fn quotes(&self, tickers: &[String]) -> Result<Vec<MarketQuote>, String> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Ok(tickers.iter().filter(|t| *t != "GONE").map(|t| quote(t)).collect())
        }
        async fn screen(&self, f: &ScreenFilter) -> Result<(u32, Vec<MarketQuote>), String> {
            *self.last_screen.lock().unwrap() = Some(f.clone());
            Ok((42, vec![quote("PTT.BK")]))
        }
        async fn calendar(&self, _: &str) -> Result<TickerCalendar, String> {
            let today = chrono::Utc::now().date_naive();
            Ok(TickerCalendar {
                earnings: vec![],
                ex_dividend: Some((today + Days::days(5)).to_string()),
                payment: Some((today + Days::days(20)).to_string()),
            })
        }
    }

    /// THB is 0.03 USD.
    struct FakeFx;

    #[async_trait]
    impl FxRates for FakeFx {
        async fn usd_per_unit(&self, c: &str, _: &str) -> Result<Decimal, String> {
            self.usd_per_unit_now(c).await
        }
        async fn usd_per_unit_now(&self, c: &str) -> Result<Decimal, String> {
            Ok(if c == "THB" { Decimal::new(3, 2) } else { Decimal::ONE })
        }
        async fn currency_of(&self, _: &TickerSymbol) -> Result<String, String> {
            Ok("USD".into())
        }
    }

    fn service(gateway: Arc<FakeGateway>) -> MarketService {
        MarketService::new(Arc::new(FakeIndex), gateway, Arc::new(FakeFx))
    }

    #[tokio::test]
    async fn index_heatmap_is_sorted_priced_in_usd_and_cached() {
        let gateway = Arc::new(FakeGateway::default());
        let s = service(gateway.clone());
        let items = s.index_heatmap(MarketIndex::Sp500).await.unwrap();
        assert_eq!(items.len(), 3, "stocks without a quote are left out");
        assert_eq!(items[0].ticker, "BIG");
        assert_eq!(items[0].sector.as_deref(), Some("Energy"));
        s.index_heatmap(MarketIndex::Sp500).await.unwrap();
        assert_eq!(gateway.requests.load(Ordering::SeqCst), 1);

        let thai = s.index_heatmap(MarketIndex::Set50).await.unwrap();
        assert!((thai[0].price - 3.3).abs() < 1e-9);
        assert!((thai[0].market_cap.unwrap() - 3.0).abs() < 1e-9);
        assert!(matches!(
            s.index_heatmap(MarketIndex::Dow30).await,
            Err(ServiceError::Upstream(_))
        ));
    }

    #[tokio::test]
    async fn screen_converts_money_bounds_to_the_market_currency() {
        let gateway = Arc::new(FakeGateway::default());
        let s = service(gateway.clone());
        let filter = ScreenFilter {
            region: "th".into(),
            min_market_cap: Some(3.0e9),
            ..ScreenFilter::default()
        };
        let result = s.screen(filter).await.unwrap();
        assert_eq!(result.total, 42);
        assert!((result.rows[0].price - 3.3).abs() < 1e-9);
        assert_eq!(result.rows[0].dividend_yield, Some(0.02));
        let sent = gateway.last_screen.lock().unwrap().clone().unwrap();
        assert!((sent.min_market_cap.unwrap() - 100e9).abs() < 1.0, "$3B is ฿100B");
        assert!(matches!(
            s.screen(ScreenFilter {
                region: "xx".into(),
                ..Default::default()
            })
            .await,
            Err(ServiceError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn calendar_merges_quote_and_dividend_dates() {
        let s = service(Arc::new(FakeGateway::default()));
        let t = |s: &str| TickerSymbol::new(s).unwrap();
        let events = s.calendar(vec![t("SMALL"), t("BIG")]).await.unwrap();
        let kinds: Vec<(&str, EventKind)> =
            events.iter().map(|e| (e.ticker.as_str(), e.kind)).collect();
        // 2099 earnings are out of range; SMALL pays no dividend.
        assert_eq!(
            kinds,
            vec![
                ("BIG", EventKind::ExDividend),
                ("BIG", EventKind::DividendPayment)
            ]
        );
        assert_eq!(events[0].annual_dividend, Some(4.0));
    }

    #[tokio::test]
    async fn dividends_for_payers_only() {
        let s = service(Arc::new(FakeGateway::default()));
        let t = |s: &str| TickerSymbol::new(s).unwrap();
        let d = s.dividends(vec![t("SMALL"), t("PTT.BK")]).await.unwrap();
        assert_eq!(d.len(), 1, "SMALL pays nothing");
        assert!((d[0].annual_per_share - 0.12).abs() < 1e-12, "฿4 = $0.12");
        assert_eq!(d[0].current_yield, Some(0.02));
        assert!(d[0].next_ex_date.is_some());
    }

    #[tokio::test]
    async fn peers_share_the_sector_and_include_the_stock() {
        let s = service(Arc::new(FakeGateway::default()));
        let group = s
            .peers(TickerSymbol::new("SMALL").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!((group.index, group.sector.as_str()), (MarketIndex::Sp500, "Tech"));
        let tickers: Vec<&str> = group.rows.iter().map(|r| r.ticker.as_str()).collect();
        assert_eq!(tickers, vec!["MID", "SMALL"]);
        assert_eq!(group.rows[0].name, "MID Corp");
        assert!(s
            .peers(TickerSymbol::new("NOPE").unwrap())
            .await
            .unwrap()
            .is_none());
    }
}
