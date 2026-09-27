use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(feature = "server")]
use chrono::NaiveDate;
#[cfg(feature = "server")]
use rust_decimal::Decimal;
#[cfg(feature = "server")]
use tokio::sync::{broadcast::Receiver, Mutex, RwLock};
#[cfg(feature = "server")]
use types::quote::{Quote, QuoteUpdate};
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

#[cfg(feature = "server")]
use crate::{
    events::quote_update_event::QuoteUpdateEvent,
    repositories::{
        manual_quotes::ManualQuotes, quote_cache::QuoteCache, quote_gateway::QuoteGateway,
        quote_gateway_errors::QuoteGateWayError,
    },
};

/// A provider call taking longer than this counts as failed, so the saved
/// copy is served instead of keeping the app waiting (the provider's own
/// retries can take minutes when it's unreachable).
const QUOTE_TIMEOUT: Duration = Duration::from_secs(8);
const CHART_TIMEOUT: Duration = Duration::from_secs(20);

/// Live FX rates are reused this long.
const LIVE_FX_TTL: Duration = Duration::from_secs(60);
/// Daily FX history is refetched this often.
const DAILY_FX_TTL: Duration = Duration::from_secs(6 * 3600);

// ─────────────────────────────────────────────
//  Service
// ─────────────────────────────────────────────
//
/// Prices for the rest of the app. Every stock is converted to USD (the
/// app's base currency): live quotes at the live rate, candles at each
/// day's rate. FX pairs (`…=X`) and indices (`^…`) keep their own units.
/// With a cache, the last good prices are served (marked `stale`) when
/// the provider can't be reached.
#[cfg(feature = "server")]
#[derive(Clone)]
pub struct QuoteService {
    /// Reads (quotes, charts, search) run concurrently; only changing the
    /// streamed tickers takes the lock exclusively.
    gateway: Arc<RwLock<dyn QuoteGateway>>,
    fx: Arc<Mutex<FxCache>>,
    cache: Option<Arc<dyn QuoteCache>>,
    /// Assets priced by hand instead of by the provider.
    manual: Option<Arc<dyn ManualQuotes>>,
}

#[cfg(feature = "server")]
#[derive(Default)]
struct FxCache {
    /// Trading currency per instrument; it never changes.
    currency: HashMap<TickerSymbol, String>,
    /// USD per unit of a currency, live.
    live: HashMap<String, (Instant, Decimal)>,
    /// USD per unit at each day's close, oldest first.
    daily: HashMap<String, (Instant, Arc<Vec<(NaiveDate, Decimal)>>)>,
}

/// Daily candles from hand-entered prices: each weekday from the first
/// entry to `today` carries the latest price entered on or before it,
/// limited to `range`.
#[cfg(feature = "server")]
pub fn manual_candles(prices: &[dtos::assets::ManualPrice], range: Range, today: NaiveDate) -> Vec<Candle> {
    use chrono::{Datelike, TimeZone, Weekday};
    let mut series: Vec<(NaiveDate, Decimal)> = prices
        .iter()
        .filter_map(|p| Some((NaiveDate::parse_from_str(&p.date, "%Y-%m-%d").ok()?, p.price)))
        .collect();
    series.sort_by_key(|(d, _)| *d);
    let Some(&(first, _)) = series.first() else {
        return vec![];
    };
    let days_back: i64 = match range {
        Range::D5 => 5,
        Range::M1 => 31,
        Range::M3 => 92,
        Range::M6 => 183,
        Range::Y1 => 366,
        Range::Y2 => 731,
        Range::Y5 => 1827,
        Range::Y10 => 3653,
        Range::Ytd => today.ordinal0() as i64,
        Range::Max => i64::MAX / 4,
        _ => 1,
    };
    let start = first.max(today - chrono::Duration::days(days_back.min(365 * 200)));
    let mut out = Vec::new();
    let mut day = start;
    while day <= today {
        if !matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            if let Some(price) = rate_on(&series, day) {
                let ts = chrono::Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).expect("midnight"));
                out.push(Candle { ts, open: price, high: price, low: price, close: price, volume: None, adj_factor: None });
            }
        }
        day += chrono::Duration::days(1);
    }
    out
}

/// Whether prices of `ticker` are converted to USD.
pub fn is_convertible(ticker: &str) -> bool {
    !ticker.starts_with('^') && !ticker.ends_with("=X")
}

/// The day an FX candle's close belongs to. FX days start at midnight
/// London time, 23:00 UTC in summer, so the UTC date would be a day early
/// and every price would convert at the next day's rate.
#[cfg(feature = "server")]
pub fn fx_day(ts: chrono::DateTime<chrono::Utc>) -> NaiveDate {
    (ts + chrono::Duration::hours(6)).date_naive()
}

/// Rate on `date`: the last close on or before it, else the earliest one.
#[cfg(feature = "server")]
pub fn rate_on(series: &[(NaiveDate, Decimal)], date: NaiveDate) -> Option<Decimal> {
    let i = series.partition_point(|(d, _)| *d <= date);
    series.get(i.saturating_sub(1)).map(|(_, r)| *r)
}

#[cfg(feature = "server")]
fn fx_pair(code: &str) -> Result<TickerSymbol, QuoteGateWayError> {
    TickerSymbol::new(&format!("{code}USD=X"))
        .map_err(|_| QuoteGateWayError::InvalidTicker(format!("{code}USD=X")))
}

#[cfg(feature = "server")]
fn fx_error(code: &str) -> QuoteGateWayError {
    QuoteGateWayError::GateWayError(format!("no USD exchange rate for {code}"))
}

#[cfg(feature = "server")]
impl QuoteService {
    pub fn new(gateway: Arc<RwLock<dyn QuoteGateway>>) -> Self {
        Self {
            gateway,
            fx: Default::default(),
            cache: None,
            manual: None,
        }
    }

    pub fn with_cache(mut self, cache: Arc<dyn QuoteCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn with_manual(mut self, manual: Arc<dyn ManualQuotes>) -> Self {
        self.manual = Some(manual);
        self
    }

    /// Currency and prices of a hand-priced asset.
    fn manual(&self, ticker: &TickerSymbol) -> Option<(String, Vec<dtos::assets::ManualPrice>)> {
        self.manual.as_ref()?.manual(ticker)
    }

    /// Candles from the provider, saved for offline use; the saved copy if
    /// the provider fails.
    async fn fetch_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, QuoteGateWayError> {
        if let Some((_, prices)) = self.manual(&ticker) {
            return Ok(manual_candles(&prices, range, chrono::Utc::now().date_naive()));
        }
        let key = format!("{ticker}|{}|{interval:?}|{is_prepost_market}", range.code());
        let fetched = tokio::time::timeout(
            CHART_TIMEOUT,
            async { self.gateway.read().await.get_chart(ticker, range, interval, is_prepost_market).await },
        )
        .await
        .unwrap_or_else(|_| Err(QuoteGateWayError::GateWayError("the price provider didn't answer in time".into())));
        match (fetched, &self.cache) {
            (Ok(candles), Some(cache)) => {
                if !candles.is_empty() {
                    cache.save_chart(&key, &candles);
                }
                Ok(candles)
            }
            (Err(e), Some(cache)) => cache.load_chart(&key).ok_or(e),
            (result, None) => result,
        }
    }

    pub async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
        self.gateway.read().await.subscribe().await
    }

    pub async fn watch(&self, ticker: TickerSymbol) -> Result<(), QuoteGateWayError> {
        // Hand-priced assets have nothing to stream.
        if self.manual(&ticker).is_some() {
            return Ok(());
        }
        self.gateway.write().await.add_ticker(ticker).await
    }

    pub async fn unwatch(&self, ticker: &TickerSymbol) -> Result<(), QuoteGateWayError> {
        if self.manual(ticker).is_some() {
            return Ok(());
        }
        self.gateway.write().await.remove_ticker(ticker).await
    }

    /// Up to 8 matches for a free-text query; empty queries return nothing.
    pub async fn search(
        &self,
        query: &str,
    ) -> Result<Vec<dtos::watch::SearchHit>, QuoteGateWayError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }
        let mut hits = self.gateway.read().await.search(query).await?;
        hits.truncate(8);
        Ok(hits)
    }

    /// Latest quote, in USD for stocks.
    pub async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError> {
        let mut q = self.native_quote(ticker.clone()).await?;
        if is_convertible(&ticker) && q.currency != "USD" {
            let rate = self.usd_rate(&q.currency).await?;
            q.current_price *= rate;
            q.previous_close_price *= rate;
            q.currency = "USD".into();
        }
        Ok(q)
    }

    /// Latest quote in the instrument's own currency, e.g. for recording a
    /// trade as the broker shows it.
    pub async fn native_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError> {
        if let Some((currency, prices)) = self.manual(&ticker) {
            let (current, previous) = dtos::assets::latest_two(&prices).ok_or_else(|| {
                QuoteGateWayError::GateWayError(format!("no price entered for {ticker} yet"))
            })?;
            self.fx.lock().await.currency.insert(ticker.clone(), currency.clone());
            return Ok(Quote {
                ticker_symbol: ticker,
                current_price: current,
                previous_close_price: previous,
                timestamp: chrono::Utc::now().timestamp(),
                currency,
                stale: false,
            });
        }
        let fetched = tokio::time::timeout(QUOTE_TIMEOUT, async { self.gateway.read().await.get_quote(ticker.clone()).await })
            .await
            .unwrap_or_else(|_| Err(QuoteGateWayError::GateWayError("the price provider didn't answer in time".into())));
        let q = match (fetched, &self.cache) {
            (Ok(q), Some(cache)) => {
                cache.save_quote(&q);
                q
            }
            (Err(e), Some(cache)) => match cache.load_quote(&ticker) {
                Some(q) => Quote { stale: true, ..q },
                None => return Err(e),
            },
            (result, None) => result?,
        };
        self.fx
            .lock()
            .await
            .currency
            .insert(ticker, q.currency.clone());
        Ok(q)
    }

    /// Candles, in USD for stocks (each at its day's exchange rate).
    pub async fn get_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
        is_prepost_market: bool,
    ) -> Result<Vec<Candle>, QuoteGateWayError> {
        if !is_convertible(&ticker) {
            return self
                .fetch_chart(ticker, range, interval, is_prepost_market)
                .await;
        }
        // The currency is cached after the first lookup; fetch both at once.
        let (candles, code) = tokio::join!(
            self.fetch_chart(ticker.clone(), range, interval, is_prepost_market),
            self.currency_of(&ticker)
        );
        let (mut candles, code) = (candles?, code?);
        if code == "USD" {
            return Ok(candles);
        }
        let series = self.daily_rates(&code).await?;
        for c in &mut candles {
            let rate = rate_on(&series, c.ts.date_naive()).ok_or_else(|| fx_error(&code))?;
            c.open *= rate;
            c.high *= rate;
            c.low *= rate;
            c.close *= rate;
        }
        Ok(candles)
    }

    /// Candles in the currency the instrument trades in, and that currency.
    pub async fn native_chart(
        &self,
        ticker: TickerSymbol,
        range: Range,
        interval: Interval,
    ) -> Result<(Vec<Candle>, String), QuoteGateWayError> {
        let (candles, code) = tokio::join!(
            self.fetch_chart(ticker.clone(), range, interval, false),
            self.currency_of(&ticker)
        );
        Ok((candles?, code?))
    }

    /// A streamed price in USD; `None` if the rate isn't available.
    pub async fn to_usd(&self, mut update: QuoteUpdate) -> Option<QuoteUpdate> {
        if !is_convertible(&update.ticker_symbol) {
            return Some(update);
        }
        let code = self.currency_of(&update.ticker_symbol).await.ok()?;
        if code != "USD" {
            update.current_price *= self.usd_rate(&code).await.ok()?;
        }
        Some(update)
    }

    /// USD per unit of `code` on `date` (YYYY-MM-DD).
    pub async fn usd_rate_on(&self, code: &str, date: &str) -> Result<Decimal, QuoteGateWayError> {
        if code == "USD" {
            return Ok(Decimal::ONE);
        }
        let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| QuoteGateWayError::Unknown(format!("bad date {date}")))?;
        let series = self.daily_rates(code).await?;
        rate_on(&series, date).ok_or_else(|| fx_error(code))
    }

    /// The currency `ticker` trades in.
    pub async fn currency_of(&self, ticker: &TickerSymbol) -> Result<String, QuoteGateWayError> {
        if let Some(code) = self.fx.lock().await.currency.get(ticker) {
            return Ok(code.clone());
        }
        Ok(self.native_quote(ticker.clone()).await?.currency)
    }

    /// USD per unit of `code`, live.
    pub async fn usd_rate(&self, code: &str) -> Result<Decimal, QuoteGateWayError> {
        if code == "USD" {
            return Ok(Decimal::ONE);
        }
        if let Some((at, rate)) = self.fx.lock().await.live.get(code) {
            if at.elapsed() < LIVE_FX_TTL {
                return Ok(*rate);
            }
        }
        let rate = self.native_quote(fx_pair(code)?).await?.current_price;
        if rate <= Decimal::ZERO {
            return Err(fx_error(code));
        }
        self.fx
            .lock()
            .await
            .live
            .insert(code.to_string(), (Instant::now(), rate));
        Ok(rate)
    }

    /// Every daily USD rate for `code` Yahoo has, oldest first (so old
    /// candles, e.g. for crisis scenarios, convert at their own rate).
    async fn daily_rates(
        &self,
        code: &str,
    ) -> Result<Arc<Vec<(NaiveDate, Decimal)>>, QuoteGateWayError> {
        if let Some((at, series)) = self.fx.lock().await.daily.get(code) {
            if at.elapsed() < DAILY_FX_TTL {
                return Ok(series.clone());
            }
        }
        let candles = self
            .fetch_chart(fx_pair(code)?, Range::Max, Interval::D1, false)
            .await?;
        let mut series: Vec<(NaiveDate, Decimal)> = candles
            .into_iter()
            .filter(|c| c.close > Decimal::ZERO)
            .map(|c| (fx_day(c.ts), c.close))
            .collect();
        series.sort_by_key(|(d, _)| *d);
        if series.is_empty() {
            return Err(fx_error(code));
        }
        let series = Arc::new(series);
        self.fx
            .lock()
            .await
            .daily
            .insert(code.to_string(), (Instant::now(), series.clone()));
        Ok(series)
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::{
        database::Database, quote::infrastructures::sqlite_cache::SqliteQuoteCache,
    };
    use async_trait::async_trait;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Serves AAPL at 200 until `online` is switched off.
    struct Flaky {
        online: Arc<AtomicBool>,
    }

    #[async_trait]
    impl QuoteGateway for Flaky {
        async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
            tokio::sync::broadcast::channel(1).1
        }
        async fn add_ticker(&mut self, _: TickerSymbol) -> Result<(), QuoteGateWayError> {
            Ok(())
        }
        async fn remove_ticker(&mut self, _: &TickerSymbol) -> Result<(), QuoteGateWayError> {
            Ok(())
        }
        async fn get_chart(
            &self,
            _: TickerSymbol,
            _: Range,
            _: Interval,
            _: bool,
        ) -> Result<Vec<Candle>, QuoteGateWayError> {
            if !self.online.load(Ordering::SeqCst) {
                return Err(QuoteGateWayError::GateWayError("offline".into()));
            }
            Ok(vec![Candle {
                ts: chrono::Utc::now(),
                open: dec!(1),
                high: dec!(2),
                low: dec!(1),
                close: dec!(2),
                volume: None,
                adj_factor: None,
            }])
        }
        async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError> {
            if !self.online.load(Ordering::SeqCst) {
                return Err(QuoteGateWayError::GateWayError("offline".into()));
            }
            Ok(Quote {
                ticker_symbol: ticker,
                current_price: dec!(200),
                previous_close_price: dec!(190),
                timestamp: 1,
                currency: "USD".into(),
                stale: false,
            })
        }
        async fn search(&self, _: &str) -> Result<Vec<dtos::watch::SearchHit>, QuoteGateWayError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn serves_saved_prices_when_offline() {
        let online = Arc::new(AtomicBool::new(true));
        let service = QuoteService::new(Arc::new(RwLock::new(Flaky { online: online.clone() })))
            .with_cache(Arc::new(SqliteQuoteCache::new(Database::in_memory().unwrap())));
        let aapl = TickerSymbol::new("AAPL").unwrap();
        assert!(!service.get_quote(aapl.clone()).await.unwrap().stale);
        service.get_chart(aapl.clone(), Range::M1, Interval::D1, false).await.unwrap();

        online.store(false, Ordering::SeqCst);
        let q = service.get_quote(aapl.clone()).await.unwrap();
        assert!(q.stale);
        assert_eq!(q.current_price, dec!(200));
        assert_eq!(service.get_chart(aapl.clone(), Range::M1, Interval::D1, false).await.unwrap().len(), 1);
        assert!(service.get_chart(aapl, Range::Y1, Interval::D1, false).await.is_err(), "never fetched");
        assert!(service.get_quote(TickerSymbol::new("MSFT").unwrap()).await.is_err());
    }

    #[test]
    fn picks_the_last_rate_on_or_before_a_date() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let series = vec![
            (d("2026-01-02"), dec!(0.030)),
            (d("2026-01-05"), dec!(0.031)),
        ];
        assert_eq!(rate_on(&series, d("2026-01-04")), Some(dec!(0.030)), "weekend");
        assert_eq!(rate_on(&series, d("2026-01-05")), Some(dec!(0.031)));
        assert_eq!(rate_on(&series, d("2025-12-01")), Some(dec!(0.030)), "before history");
        assert_eq!(rate_on(&[], d("2026-01-01")), None);
        assert!(is_convertible("PTT.BK") && !is_convertible("^GSPC") && !is_convertible("USDTHB=X"));
    }

    /// Answers AAPL at 200 until `online` is off, then never answers.
    struct Hangs {
        online: Arc<AtomicBool>,
    }

    #[async_trait]
    impl QuoteGateway for Hangs {
        async fn subscribe(&self) -> Receiver<QuoteUpdateEvent> {
            tokio::sync::broadcast::channel(1).1
        }
        async fn add_ticker(&mut self, _: TickerSymbol) -> Result<(), QuoteGateWayError> {
            Ok(())
        }
        async fn remove_ticker(&mut self, _: &TickerSymbol) -> Result<(), QuoteGateWayError> {
            Ok(())
        }
        async fn get_chart(&self, _: TickerSymbol, _: Range, _: Interval, _: bool) -> Result<Vec<Candle>, QuoteGateWayError> {
            std::future::pending().await
        }
        async fn get_quote(&self, ticker: TickerSymbol) -> Result<Quote, QuoteGateWayError> {
            if !self.online.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            Flaky { online: self.online.clone() }.get_quote(ticker).await
        }
        async fn search(&self, _: &str) -> Result<Vec<dtos::watch::SearchHit>, QuoteGateWayError> {
            Ok(vec![])
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_provider_that_never_answers_falls_back_to_saved_prices() {
        let online = Arc::new(AtomicBool::new(true));
        let service = QuoteService::new(Arc::new(RwLock::new(Hangs { online: online.clone() })))
            .with_cache(Arc::new(SqliteQuoteCache::new(Database::in_memory().unwrap())));
        let aapl = TickerSymbol::new("AAPL").unwrap();
        service.get_quote(aapl.clone()).await.unwrap();
        online.store(false, Ordering::SeqCst);
        let q = service.get_quote(aapl.clone()).await.unwrap();
        assert!(q.stale && q.current_price == dec!(200));
        assert!(service.get_chart(aapl, Range::M1, Interval::D1, false).await.is_err(), "times out, nothing saved");
    }

    struct OneFund;
    impl ManualQuotes for OneFund {
        fn manual(&self, t: &TickerSymbol) -> Option<(String, Vec<dtos::assets::ManualPrice>)> {
            let p = |d: &str, v| dtos::assets::ManualPrice { date: d.into(), price: v };
            (t.as_str() == "KFSSF").then(|| ("THB".into(), vec![p("2026-01-05", dec!(10)), p("2026-01-07", dec!(11))]))
        }
    }

    #[tokio::test]
    async fn hand_priced_assets_skip_the_provider() {
        let online = Arc::new(AtomicBool::new(false));
        let service = QuoteService::new(Arc::new(RwLock::new(Flaky { online }))).with_manual(Arc::new(OneFund));
        let fund = TickerSymbol::new("KFSSF").unwrap();
        let q = service.native_quote(fund.clone()).await.unwrap();
        assert_eq!((q.current_price, q.previous_close_price, q.currency.as_str()), (dec!(11), dec!(10), "THB"));
        assert!(service.watch(fund).await.is_ok());

        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let prices = OneFund.manual(&TickerSymbol::new("KFSSF").unwrap()).unwrap().1;
        // Mon 5th to Mon 12th: six weekdays, the weekend skipped.
        let candles = manual_candles(&prices, Range::Max, d("2026-01-12"));
        let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
        assert_eq!(closes, vec![dec!(10), dec!(10), dec!(11), dec!(11), dec!(11), dec!(11)]);
        assert_eq!(manual_candles(&prices, Range::D5, d("2026-01-12")).len(), 4);
    }

    #[test]
    fn fx_candles_belong_to_the_london_day() {
        use chrono::TimeZone;
        let at = |h: u32| chrono::Utc.with_ymd_and_hms(2026, 7, 5, h, 0, 0).unwrap();
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        // Monday's candle opens Sunday 23:00 UTC in summer, 00:00 in winter.
        assert_eq!(fx_day(at(23)), d("2026-07-06"));
        assert_eq!(fx_day(at(0)), d("2026-07-05"));
    }
}
