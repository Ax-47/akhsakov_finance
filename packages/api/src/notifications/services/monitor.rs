//! Checks active price alerts against live prices every minute.

use super::NotificationService;
use crate::notifications::repositories::{AlertStore, EventSource, HistorySource, HoldingsSource, PriceSource};
use dtos::{
    market::EventKind,
    planning::days_between,
    position::{compute_positions, portfolio_summary},
    watch::{Alert, AlertKind},
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use types::ticker_symbol::TickerSymbol;

const INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct AlertMonitor {
    alerts: Arc<dyn AlertStore>,
    holdings: Arc<dyn HoldingsSource>,
    prices: Arc<dyn PriceSource>,
    notifications: NotificationService,
    /// For technical and calendar alerts; without them those never fire.
    history: Option<Arc<dyn HistorySource>>,
    events: Option<Arc<dyn EventSource>>,
}

impl AlertMonitor {
    pub fn new(
        alerts: Arc<dyn AlertStore>,
        holdings: Arc<dyn HoldingsSource>,
        prices: Arc<dyn PriceSource>,
        notifications: NotificationService,
    ) -> Self {
        Self {
            alerts,
            holdings,
            prices,
            notifications,
            history: None,
            events: None,
        }
    }

    /// Adds daily history and the events calendar, for technical and
    /// calendar alerts.
    pub fn with_market(mut self, history: Arc<dyn HistorySource>, events: Arc<dyn EventSource>) -> Self {
        self.history = Some(history);
        self.events = Some(events);
        self
    }

    /// Closes, oldest first, ending with the live `price` (today's close
    /// so far) when known.
    async fn closes(&self, ticker: &TickerSymbol, price: Option<Decimal>, today: &str) -> Option<Vec<f64>> {
        let history = self.history.as_ref()?.daily_closes(ticker).await.ok()?;
        let mut closes: Vec<f64> = history.iter().map(|(_, c)| *c).collect();
        if let Some(p) = price.and_then(|p| p.to_f64()) {
            match history.last() {
                Some((date, _)) if date.as_str() >= today => *closes.last_mut()? = p,
                _ => closes.push(p),
            }
        }
        Some(closes)
    }

    /// Days until the next event the alert watches, if one is known.
    fn days_until(alert: &Alert, events: &[dtos::market::CalendarEvent], today: &str) -> Option<i64> {
        let kind = match alert.kind {
            AlertKind::EarningsWithin => EventKind::Earnings,
            AlertKind::ExDividendWithin => EventKind::ExDividend,
            _ => return None,
        };
        events
            .iter()
            .filter(|e| e.kind == kind && e.ticker.eq_ignore_ascii_case(alert.ticker.as_str()))
            .filter_map(|e| days_between(today, &e.date))
            .filter(|d| *d >= 0)
            .min()
    }

    /// Runs [`check_once`](Self::check_once) every minute, forever.
    pub fn spawn(self) {
        tokio::spawn(async move {
            loop {
                match self.check_once().await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!("{n} alert(s) fired"),
                    Err(e) => tracing::warn!("alert check failed: {e}"),
                }
                tokio::time::sleep(INTERVAL).await;
            }
        });
    }

    /// Fires every active alert whose condition holds; returns how many.
    pub async fn check_once(&self) -> Result<usize, String> {
        let active = self.alerts.active_alerts()?;
        if active.is_empty() {
            return Ok(0);
        }
        let dashboard = self.holdings.dashboard()?;
        let needs_holdings = active
            .iter()
            .any(|a| a.kind == AlertKind::WeightAbove || a.kind.is_portfolio());
        let mut tickers: HashSet<TickerSymbol> = active
            .iter()
            .filter(|a| !a.kind.is_portfolio())
            .map(|a| a.ticker.clone())
            .collect();
        if needs_holdings {
            tickers.extend(dashboard.transactions.iter().filter(|t| !t.is_cash()).map(|t| t.ticker.clone()));
        }
        // (price, day change %) per ticker; unreachable prices are skipped.
        let mut prices: HashMap<TickerSymbol, (Decimal, Decimal)> = HashMap::new();
        for t in tickers {
            if let Ok((price, previous)) = self.prices.price(&t).await {
                let day = if previous.is_zero() {
                    Decimal::ZERO
                } else {
                    (price / previous - Decimal::ONE) * Decimal::ONE_HUNDRED
                };
                prices.insert(t, (price, day));
            }
        }
        let positions = compute_positions(&dashboard, &prices);
        let (total, _, _, day_change) = portfolio_summary(&positions);
        let weight = |t: &TickerSymbol| {
            let p = positions.iter().find(|p| &p.ticker == t)?;
            (total > Decimal::ZERO).then(|| p.market_value() / total * Decimal::ONE_HUNDRED)
        };
        // Whole-portfolio figures only when every holding is priced; a
        // missing price would look like a crash.
        let all_priced = !positions.is_empty() && positions.iter().all(|p| p.current_price > Decimal::ZERO);
        let portfolio_value = (all_priced && total > Decimal::ZERO).then_some(total);
        let portfolio_day = portfolio_value.and_then(|v| {
            let before = v - day_change;
            (before > Decimal::ZERO).then(|| day_change / before * Decimal::ONE_HUNDRED)
        });

        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let event_tickers: Vec<TickerSymbol> = active
            .iter()
            .filter(|a| a.kind.is_event())
            .map(|a| a.ticker.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let events = match (&self.events, event_tickers.is_empty()) {
            (Some(source), false) => source.events(event_tickers).await.unwrap_or_default(),
            _ => vec![],
        };
        let mut history: HashMap<TickerSymbol, Option<Vec<f64>>> = HashMap::new();

        let mut fired = 0;
        for mut alert in active {
            if alert.kind.is_technical() || alert.kind.is_event() {
                let price = prices.get(&alert.ticker).map(|(p, _)| *p);
                let met = if alert.kind.is_event() {
                    let days = Self::days_until(&alert, &events, &today);
                    alert.is_met_by_event(days).then(|| {
                        let date = dtos::planning::add_days(&today, days.unwrap_or(0)).unwrap_or_default();
                        format!("On {date}")
                    })
                } else {
                    if !history.contains_key(&alert.ticker) {
                        let closes = self.closes(&alert.ticker, price, &today).await;
                        history.insert(alert.ticker.clone(), closes);
                    }
                    let closes = history.get(&alert.ticker).cloned().flatten().unwrap_or_default();
                    alert.is_met_by_history(&closes).then(|| {
                        price.map(|p| format!("Now ${:.2}", p.round_dp(2))).unwrap_or_default()
                    })
                };
                let Some(body) = met else { continue };
                if !self.alerts.mark_fired(alert.id)? {
                    continue;
                }
                fired += 1;
                if let Err(e) = self.notifications.notify(&format!("🔔 {}", alert.describe()), &body).await {
                    tracing::warn!("couldn't record notification: {e}");
                }
                continue;
            }
            if alert.kind.is_portfolio() {
                if alert.kind == AlertKind::PortfolioDrawdown {
                    if let Some(v) = portfolio_value {
                        if alert.peak.is_none_or(|p| v > p) {
                            alert.peak = Some(v);
                            self.alerts.save_peak(alert.id, v)?;
                        }
                    }
                }
                if !alert.is_met_by_portfolio(portfolio_day, portfolio_value) {
                    continue;
                }
                if !self.alerts.mark_fired(alert.id)? {
                    continue;
                }
                fired += 1;
                let body = match (portfolio_value, portfolio_day) {
                    (Some(v), Some(d)) => format!("All holdings now ${:.2} ({d:+.2}% today)", v.round_dp(2)),
                    _ => String::new(),
                };
                if let Err(e) = self.notifications.notify(&format!("🔔 {}", alert.describe()), &body).await {
                    tracing::warn!("couldn't record notification: {e}");
                }
                continue;
            }
            let (price, day) = prices
                .get(&alert.ticker)
                .map(|(p, d)| (Some(*p), Some(*d)))
                .unwrap_or((None, None));
            if !alert.is_met(price, day, weight(&alert.ticker)) {
                continue;
            }
            if !self.alerts.mark_fired(alert.id)? {
                continue;
            }
            fired += 1;
            let body = match (price, day) {
                (Some(p), Some(d)) => format!("Now ${:.2} ({d:+.2}% today)", p.round_dp(2)),
                _ => String::new(),
            };
            if let Err(e) = self.notifications.notify(&format!("🔔 {}", alert.describe()), &body).await {
                tracing::warn!("couldn't record notification: {e}");
            }
        }
        Ok(fired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::repositories::{NotificationRepository, Pusher};
    use async_trait::async_trait;
    use dtos::{
        notifications::{Channels, Notification},
        portfolio::GetDashBoardResponse,
        watch::{Alert, AlertKind},
    };
    use rust_decimal_macros::dec;
    use std::sync::Mutex;
    use uuid::Uuid;

    #[derive(Default)]
    struct Store {
        alerts: Mutex<Vec<Alert>>,
    }

    impl AlertStore for Store {
        fn active_alerts(&self) -> Result<Vec<Alert>, String> {
            Ok(self.alerts.lock().unwrap().iter().filter(|a| a.is_active()).cloned().collect())
        }
        fn mark_fired(&self, id: Uuid) -> Result<bool, String> {
            let mut alerts = self.alerts.lock().unwrap();
            let a = alerts.iter_mut().find(|a| a.id == id && a.is_active());
            Ok(a.map(|a| a.triggered_at = Some("now".into())).is_some())
        }
        fn save_peak(&self, id: Uuid, peak: Decimal) -> Result<(), String> {
            let mut alerts = self.alerts.lock().unwrap();
            if let Some(a) = alerts.iter_mut().find(|a| a.id == id) {
                a.peak = Some(peak);
            }
            Ok(())
        }
    }

    struct NoHoldings;
    impl HoldingsSource for NoHoldings {
        fn dashboard(&self) -> Result<GetDashBoardResponse, String> {
            Ok(GetDashBoardResponse::default())
        }
    }

    /// NVDA at 120, up from 100.
    struct Prices;
    #[async_trait]
    impl PriceSource for Prices {
        async fn price(&self, t: &TickerSymbol) -> Result<(Decimal, Decimal), String> {
            match t.as_str() {
                "NVDA" => Ok((dec!(120), dec!(100))),
                _ => Err("unknown".into()),
            }
        }
    }

    #[derive(Default)]
    struct Memory {
        items: Mutex<Vec<Notification>>,
        channels: Mutex<Channels>,
    }
    impl NotificationRepository for Memory {
        fn recent(&self, _: usize) -> Result<Vec<Notification>, crate::shared::RepositoryError> {
            Ok(self.items.lock().unwrap().clone())
        }
        fn add(&self, n: &Notification) -> Result<(), crate::shared::RepositoryError> {
            self.items.lock().unwrap().push(n.clone());
            Ok(())
        }
        fn mark_all_read(&self) -> Result<(), crate::shared::RepositoryError> {
            Ok(())
        }
        fn channels(&self) -> Result<Channels, crate::shared::RepositoryError> {
            Ok(self.channels.lock().unwrap().clone())
        }
        fn save_channels(&self, c: &Channels) -> Result<(), crate::shared::RepositoryError> {
            *self.channels.lock().unwrap() = c.clone();
            Ok(())
        }
    }

    #[derive(Default)]
    struct CountingPusher {
        sent: Mutex<Vec<String>>,
    }
    #[async_trait]
    impl Pusher for CountingPusher {
        async fn push(&self, _: &Channels, title: &str, _: &str) -> Vec<(String, Result<(), String>)> {
            self.sent.lock().unwrap().push(title.to_string());
            vec![("ntfy".into(), Ok(()))]
        }
    }

    fn alert(kind: AlertKind, value: Decimal) -> Alert {
        Alert {
            id: Uuid::new_v4(),
            ticker: TickerSymbol::new("NVDA").unwrap(),
            kind,
            value,
            created_at: String::new(),
            triggered_at: None,
            peak: None,
        }
    }

    #[tokio::test]
    async fn fires_met_alerts_once_and_pushes() {
        let store = Arc::new(Store::default());
        *store.alerts.lock().unwrap() = vec![
            alert(AlertKind::PriceAbove, dec!(110)), // met
            alert(AlertKind::PriceAbove, dec!(130)), // not yet
            alert(AlertKind::DayMove, dec!(15)),     // +20% today: met
        ];
        let repo = Arc::new(Memory::default());
        repo.save_channels(&Channels { ntfy_topic: "t".into(), ..Default::default() }).unwrap();
        let pusher = Arc::new(CountingPusher::default());
        let monitor = AlertMonitor::new(
            store.clone(),
            Arc::new(NoHoldings),
            Arc::new(Prices),
            NotificationService::new(repo.clone(), pusher.clone()),
        );
        assert_eq!(monitor.check_once().await.unwrap(), 2);
        assert_eq!(monitor.check_once().await.unwrap(), 0, "fired alerts don't repeat");
        assert_eq!(repo.items.lock().unwrap().len(), 2);
        assert_eq!(pusher.sent.lock().unwrap().len(), 2);
        assert!(repo.items.lock().unwrap()[0].body.contains("+20.00% today"));
    }

    /// 10 NVDA bought at $100.
    struct TenNvda;
    impl HoldingsSource for TenNvda {
        fn dashboard(&self) -> Result<GetDashBoardResponse, String> {
            Ok(GetDashBoardResponse {
                portfolios: vec![],
                transactions: vec![dtos::Transaction {
                    id: Uuid::nil(),
                    portfolio_id: Uuid::nil(),
                    ticker: TickerSymbol::new("NVDA").unwrap(),
                    transaction_type: types::transaction_type::TransactionType::Buy,
                    shares: dec!(10),
                    price: dec!(100),
                    date: "2026-01-02".into(),
                    fee: dec!(0),
                    currency: "USD".into(),
                    fx_to_usd: dec!(1),
                }],
            })
        }
    }

    /// NVDA at a price that can be changed, with its previous close.
    struct Moving(Mutex<(Decimal, Decimal)>);
    #[async_trait]
    impl PriceSource for Moving {
        async fn price(&self, _: &TickerSymbol) -> Result<(Decimal, Decimal), String> {
            Ok(*self.0.lock().unwrap())
        }
    }

    /// NVDA rising 1 a day for 30 days, to 130.
    struct Rising;
    #[async_trait]
    impl HistorySource for Rising {
        async fn daily_closes(&self, _: &TickerSymbol) -> Result<Vec<(String, f64)>, String> {
            Ok((1..=30).map(|i| (format!("2020-01-{i:02}"), 100.0 + i as f64)).collect())
        }
    }

    /// NVDA reports in two days.
    struct Soon;
    #[async_trait]
    impl EventSource for Soon {
        async fn events(&self, _: Vec<TickerSymbol>) -> Result<Vec<dtos::market::CalendarEvent>, String> {
            let today = chrono::Utc::now().date_naive();
            Ok(vec![dtos::market::CalendarEvent {
                date: (today + chrono::Duration::days(2)).format("%Y-%m-%d").to_string(),
                ticker: "NVDA".into(),
                name: "NVIDIA".into(),
                kind: EventKind::Earnings,
                annual_dividend: None,
                estimated: false,
            }])
        }
    }

    #[tokio::test]
    async fn technical_and_calendar_alerts() {
        let store = Arc::new(Store::default());
        *store.alerts.lock().unwrap() = vec![
            alert(AlertKind::Near52WeekHigh, dec!(10)),   // 120 vs a 130 high: met
            alert(AlertKind::CrossBelowSma, dec!(10)),    // live 120 < 10-day avg of ~126: met
            alert(AlertKind::EarningsWithin, dec!(3)),    // in 2 days: met
            alert(AlertKind::ExDividendWithin, dec!(30)), // none known
        ];
        let repo = Arc::new(Memory::default());
        let monitor = AlertMonitor::new(
            store.clone(),
            Arc::new(NoHoldings),
            Arc::new(Prices),
            NotificationService::new(repo.clone(), Arc::new(CountingPusher::default())),
        );
        assert_eq!(monitor.check_once().await.unwrap(), 0, "no history source, nothing fires");
        let monitor = monitor.with_market(Arc::new(Rising), Arc::new(Soon));
        assert_eq!(monitor.check_once().await.unwrap(), 3);
        let titles: Vec<String> = repo.items.lock().unwrap().iter().map(|n| n.title.clone()).collect();
        assert!(titles.iter().any(|t| t.contains("earnings within 3 days")), "{titles:?}");
    }

    #[tokio::test]
    async fn portfolio_alerts_follow_the_whole_portfolio() {
        let store = Arc::new(Store::default());
        *store.alerts.lock().unwrap() = vec![
            alert(AlertKind::PortfolioDayDrop, dec!(5)),
            alert(AlertKind::PortfolioDrawdown, dec!(10)),
        ];
        let prices = Arc::new(Moving(Mutex::new((dec!(120), dec!(100)))));
        let repo = Arc::new(Memory::default());
        let monitor = AlertMonitor::new(
            store.clone(),
            Arc::new(TenNvda),
            prices.clone(),
            NotificationService::new(repo.clone(), Arc::new(CountingPusher::default())),
        );
        // Up 20% today, at its peak: nothing fires, the peak is remembered.
        assert_eq!(monitor.check_once().await.unwrap(), 0);
        assert_eq!(store.alerts.lock().unwrap()[1].peak, Some(dec!(1200)));

        // Next day: 120 → 100 is −16.7% on the day and from the peak.
        *prices.0.lock().unwrap() = (dec!(100), dec!(120));
        assert_eq!(monitor.check_once().await.unwrap(), 2);
        assert!(repo.items.lock().unwrap()[0].title.contains("All holdings"));
    }
}
