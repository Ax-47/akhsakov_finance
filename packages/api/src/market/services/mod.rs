//! Market overview use cases. Heatmaps are cached briefly so many open
//! pages share one upstream request.

use crate::{
    market::repositories::{IndexRepository, MarketDataGateway, MarketQuote},
    shared::ServiceError,
};
use dtos::market::{HeatmapItem, MarketIndex};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use types::ticker_symbol::TickerSymbol;

/// Prices are refreshed at most this often.
const QUOTE_TTL: Duration = Duration::from_secs(30);
/// Market caps of watchlist stocks move slowly; refetch hourly.
const CAP_TTL: Duration = Duration::from_secs(3600);
/// Share counts behind index market caps change with buybacks and splits.
const SHARES_TTL: Duration = Duration::from_secs(24 * 3600);
pub const MAX_TICKERS: usize = 100;

#[derive(Clone)]
pub struct MarketService {
    index: Arc<dyn IndexRepository>,
    gateway: Arc<dyn MarketDataGateway>,
    index_cache: Arc<Mutex<HashMap<MarketIndex, (Instant, Vec<HeatmapItem>)>>>,
    cap_cache: Arc<Mutex<HashMap<TickerSymbol, (Instant, Option<f64>)>>>,
    /// Shares outstanding (billions) derived from live market caps.
    shares: Arc<Mutex<Shares>>,
}

#[derive(Default)]
struct Shares {
    by_ticker: HashMap<String, f64>,
    calibrated_at: Option<Instant>,
    calibrating: bool,
}

fn change_pct(q: &MarketQuote) -> Option<f64> {
    q.previous_close
        .filter(|p| *p > 0.0)
        .map(|p| (q.price / p - 1.0) * 100.0)
}

impl MarketService {
    pub fn new(index: Arc<dyn IndexRepository>, gateway: Arc<dyn MarketDataGateway>) -> Self {
        Self {
            index,
            gateway,
            index_cache: Default::default(),
            cap_cache: Default::default(),
            shares: Default::default(),
        }
    }

    /// Every constituent with a quote, largest first. Market cap is shares
    /// outstanding times the live price: shares come from Yahoo's market
    /// caps (recalibrated daily in the background), falling back to the
    /// index repository's figures until then.
    pub async fn index_heatmap(&self, index: MarketIndex) -> Result<Vec<HeatmapItem>, ServiceError> {
        if let Some((at, items)) = self.index_cache.lock().await.get(&index) {
            if at.elapsed() < QUOTE_TTL {
                return Ok(items.clone());
            }
        }
        let constituents = self.index.constituents(index);
        let tickers: Vec<TickerSymbol> = constituents
            .iter()
            .filter_map(|c| TickerSymbol::new(c.ticker).ok())
            .collect();
        let quotes = self
            .gateway
            .quotes(&tickers)
            .await
            .map_err(ServiceError::Upstream)?;
        self.calibrate_shares_in_background(&quotes).await;
        let shares = self.shares.lock().await;
        let mut items: Vec<HeatmapItem> = constituents
            .iter()
            .filter_map(|c| {
                let q = quotes.iter().find(|q| q.ticker.as_str() == c.ticker)?;
                let shares_bn = shares.by_ticker.get(c.ticker).copied().unwrap_or(c.shares_bn);
                Some(HeatmapItem {
                    ticker: c.ticker.to_string(),
                    name: c.name.to_string(),
                    sector: Some(c.sector.to_string()),
                    price: q.price,
                    change_pct: change_pct(q),
                    market_cap: Some(shares_bn * 1e9 * q.price),
                })
            })
            .collect();
        drop(shares);
        if items.is_empty() {
            return Err(ServiceError::Upstream("No prices available right now".into()));
        }
        items.sort_by(|a, b| b.market_cap.unwrap_or(0.0).total_cmp(&a.market_cap.unwrap_or(0.0)));
        self.index_cache
            .lock()
            .await
            .insert(index, (Instant::now(), items.clone()));
        Ok(items)
    }

    /// Starts deriving share counts (market cap ÷ price) for `quotes` unless
    /// fresh ones exist or a run is under way. Doesn't wait for it.
    async fn calibrate_shares_in_background(&self, quotes: &[MarketQuote]) {
        {
            let mut shares = self.shares.lock().await;
            let fresh = shares.calibrated_at.is_some_and(|at| at.elapsed() < SHARES_TTL);
            if fresh || shares.calibrating {
                return;
            }
            shares.calibrating = true;
        }
        let (gateway, shares) = (self.gateway.clone(), self.shares.clone());
        let prices: Vec<(TickerSymbol, f64)> =
            quotes.iter().map(|q| (q.ticker.clone(), q.price)).collect();
        let requested = prices.len();
        tokio::spawn(async move {
            let derived = futures::future::join_all(prices.into_iter().map(|(t, price)| {
                let gateway = gateway.clone();
                async move {
                    let cap = gateway.market_cap(&t).await.ok().flatten()?;
                    (price > 0.0).then(|| (t.to_string(), cap / price / 1e9))
                }
            }))
            .await;
            let mut shares = shares.lock().await;
            let found: Vec<(String, f64)> = derived.into_iter().flatten().collect();
            // Mostly failed (rate limiting): try again on the next request.
            if found.len() * 2 >= requested {
                shares.calibrated_at = Some(Instant::now());
            }
            shares.by_ticker.extend(found);
            shares.calibrating = false;
        });
    }

    /// Any stocks (e.g. a watchlist), in the order given. Stocks without a
    /// known market cap keep `market_cap: None`.
    pub async fn tickers_heatmap(
        &self,
        tickers: Vec<TickerSymbol>,
    ) -> Result<Vec<HeatmapItem>, ServiceError> {
        if tickers.len() > MAX_TICKERS {
            return Err(ServiceError::Validation(format!(
                "At most {MAX_TICKERS} stocks per heatmap"
            )));
        }
        let quotes = self
            .gateway
            .quotes(&tickers)
            .await
            .map_err(ServiceError::Upstream)?;
        let caps = self.market_caps(&tickers).await;
        Ok(tickers
            .iter()
            .filter_map(|t| {
                let q = quotes.iter().find(|q| &q.ticker == t)?;
                Some(HeatmapItem {
                    ticker: t.to_string(),
                    name: q.name.clone().unwrap_or_else(|| t.to_string()),
                    sector: None,
                    price: q.price,
                    change_pct: change_pct(q),
                    market_cap: caps.get(t).copied().flatten(),
                })
            })
            .collect())
    }

    /// Cached market caps; failed lookups count as unknown.
    async fn market_caps(&self, tickers: &[TickerSymbol]) -> HashMap<TickerSymbol, Option<f64>> {
        let mut caps: HashMap<TickerSymbol, Option<f64>> = HashMap::new();
        let mut missing = Vec::new();
        {
            let cache = self.cap_cache.lock().await;
            for t in tickers {
                match cache.get(t) {
                    Some((at, cap)) if at.elapsed() < CAP_TTL => {
                        caps.insert(t.clone(), *cap);
                    }
                    _ => missing.push(t.clone()),
                }
            }
        }
        let fetched = futures::future::join_all(missing.into_iter().map(|t| async move {
            let cap = self.gateway.market_cap(&t).await;
            (t, cap)
        }))
        .await;
        let mut cache = self.cap_cache.lock().await;
        for (t, cap) in fetched {
            // Don't cache failures (often rate limiting); retry next time.
            if let Ok(cap) = cap {
                cache.insert(t.clone(), (Instant::now(), cap));
                caps.insert(t, cap);
            } else {
                caps.insert(t, None);
            }
        }
        caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::repositories::Constituent;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TwoStocks;

    impl IndexRepository for TwoStocks {
        fn constituents(&self, _: MarketIndex) -> Vec<Constituent> {
            vec![
                Constituent { ticker: "SMALL", name: "Small", sector: "Tech", shares_bn: 1.0 },
                Constituent { ticker: "BIG", name: "Big", sector: "Energy", shares_bn: 10.0 },
                Constituent { ticker: "GONE", name: "Delisted", sector: "Tech", shares_bn: 1.0 },
            ]
        }
    }

    /// Prices every known ticker at 110, up from 100; counts requests.
    #[derive(Default)]
    struct FakeGateway {
        requests: AtomicUsize,
    }

    #[async_trait]
    impl MarketDataGateway for FakeGateway {
        async fn quotes(&self, tickers: &[TickerSymbol]) -> Result<Vec<MarketQuote>, String> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Ok(tickers
                .iter()
                .filter(|t| t.as_str() != "GONE")
                .map(|t| MarketQuote {
                    ticker: t.clone(),
                    name: Some(format!("{t} Inc")),
                    price: 110.0,
                    previous_close: Some(100.0),
                })
                .collect())
        }
        async fn market_cap(&self, t: &TickerSymbol) -> Result<Option<f64>, String> {
            if t.as_str() == "FAIL" {
                Err("rate limited".into())
            } else {
                Ok(Some(5e9))
            }
        }
    }

    #[tokio::test]
    async fn index_heatmap_sizes_by_live_cap_and_caches() {
        let gateway = Arc::new(FakeGateway::default());
        let service = MarketService::new(Arc::new(TwoStocks), gateway.clone());
        let items = service.index_heatmap(MarketIndex::Sp500).await.unwrap();
        assert_eq!(items.len(), 2, "stocks without a quote are left out");
        assert_eq!(items[0].ticker, "BIG");
        assert!((items[0].market_cap.unwrap() - 10e9 * 110.0).abs() < 1.0);
        assert!((items[0].change_pct.unwrap() - 10.0).abs() < 1e-9);
        service.index_heatmap(MarketIndex::Sp500).await.unwrap();
        assert_eq!(gateway.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn index_caps_recalibrate_from_live_market_caps() {
        let service = MarketService::new(Arc::new(TwoStocks), Arc::new(FakeGateway::default()));
        service.index_heatmap(MarketIndex::Sp500).await.unwrap();
        // Let the background calibration finish, then bypass the quote cache.
        for _ in 0..50 {
            if service.shares.lock().await.calibrated_at.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        service.index_cache.lock().await.clear();
        let items = service.index_heatmap(MarketIndex::Sp500).await.unwrap();
        // The fake reports a 5B cap for every stock.
        assert!(items.iter().all(|i| (i.market_cap.unwrap() - 5e9).abs() < 1.0));
    }

    #[tokio::test]
    async fn tickers_heatmap_keeps_order_and_tolerates_missing_caps() {
        let service = MarketService::new(Arc::new(TwoStocks), Arc::new(FakeGateway::default()));
        let tickers = ["FAIL", "AAPL"].map(|t| TickerSymbol::new(t).unwrap()).to_vec();
        let items = service.tickers_heatmap(tickers).await.unwrap();
        assert_eq!(items[0].ticker, "FAIL");
        assert_eq!(items[0].market_cap, None);
        assert_eq!(items[1].market_cap, Some(5e9));
        assert_eq!(items[1].name, "AAPL Inc");

        let too_many = (0..=MAX_TICKERS).map(|i| TickerSymbol::new(&format!("T{i}")).unwrap()).collect();
        assert!(matches!(
            service.tickers_heatmap(too_many).await,
            Err(ServiceError::Validation(_))
        ));
    }
}
