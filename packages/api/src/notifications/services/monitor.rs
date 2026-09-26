//! Checks active price alerts against live prices every minute.

use super::NotificationService;
use crate::notifications::repositories::{AlertStore, HoldingsSource, PriceSource};
use dtos::position::{compute_positions, portfolio_summary};
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
        }
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
        let needs_weights = active.iter().any(|a| a.kind == dtos::watch::AlertKind::WeightAbove);
        let mut tickers: HashSet<TickerSymbol> = active.iter().map(|a| a.ticker.clone()).collect();
        if needs_weights {
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
        let (total, ..) = portfolio_summary(&positions);
        let weight = |t: &TickerSymbol| {
            let p = positions.iter().find(|p| &p.ticker == t)?;
            (total > Decimal::ZERO).then(|| p.market_value() / total * Decimal::ONE_HUNDRED)
        };

        let mut fired = 0;
        for alert in active {
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
}
