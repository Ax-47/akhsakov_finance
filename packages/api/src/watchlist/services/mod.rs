//! Watchlist and alert use cases.

use crate::{shared::ServiceError, watchlist::repositories::WatchlistRepository};
use dtos::watch::{Alert, AlertKind, WatchItem};
use rust_decimal::Decimal;
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[derive(Clone)]
pub struct WatchlistService {
    repo: Arc<dyn WatchlistRepository>,
}

impl WatchlistService {
    pub fn new(repo: Arc<dyn WatchlistRepository>) -> Self {
        Self { repo }
    }

    pub fn watchlist(&self) -> Result<Vec<WatchItem>, ServiceError> {
        Ok(self.repo.watchlist()?)
    }

    pub fn watch(&self, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.watch(ticker)?)
    }

    pub fn unwatch(&self, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.unwatch(ticker)?)
    }

    pub fn alerts(&self) -> Result<Vec<Alert>, ServiceError> {
        Ok(self.repo.alerts()?)
    }

    /// Creates an active alert; its ticker is watched too.
    pub fn create_alert(
        &self,
        ticker: TickerSymbol,
        kind: AlertKind,
        value: Decimal,
    ) -> Result<Alert, ServiceError> {
        if value <= Decimal::ZERO {
            return Err(ServiceError::Validation("Enter a value above zero".into()));
        }
        if matches!(kind, AlertKind::WeightAbove) && value > Decimal::ONE_HUNDRED {
            return Err(ServiceError::Validation(
                "A weight can't be over 100%".into(),
            ));
        }
        let alert = Alert {
            id: Uuid::new_v4(),
            ticker,
            kind,
            value,
            created_at: String::new(),
            triggered_at: None,
        };
        self.repo.save_alert(&alert)?;
        self.repo.watch(&alert.ticker)?;
        Ok(alert)
    }

    pub fn delete_alert(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_alert(id)?)
    }

    pub fn mark_triggered(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.mark_triggered(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, watchlist::infrastructures::SqliteWatchlistRepository};
    use rust_decimal_macros::dec;

    fn service() -> WatchlistService {
        WatchlistService::new(Arc::new(SqliteWatchlistRepository::new(
            Database::in_memory().unwrap(),
        )))
    }

    fn sym(s: &str) -> TickerSymbol {
        TickerSymbol::new(s).unwrap()
    }

    #[test]
    fn watch_is_idempotent() {
        let s = service();
        s.watch(&sym("NVDA")).unwrap();
        s.watch(&sym("NVDA")).unwrap();
        s.watch(&sym("AMD")).unwrap();
        assert_eq!(s.watchlist().unwrap().len(), 2);
        s.unwatch(&sym("NVDA")).unwrap();
        assert_eq!(s.watchlist().unwrap()[0].ticker.as_str(), "AMD");
    }

    #[test]
    fn alerts_fire_once() {
        let s = service();
        assert!(matches!(
            s.create_alert(sym("NVDA"), AlertKind::PriceAbove, dec!(0)),
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.create_alert(sym("NVDA"), AlertKind::WeightAbove, dec!(120)),
            Err(ServiceError::Validation(_))
        ));

        let alert = s
            .create_alert(sym("TSM"), AlertKind::PriceBelow, dec!(150.5))
            .unwrap();
        assert_eq!(
            s.watchlist().unwrap()[0].ticker.as_str(),
            "TSM",
            "alerting watches the ticker"
        );
        let stored = &s.alerts().unwrap()[0];
        assert_eq!(
            (stored.kind, stored.value, stored.is_active()),
            (AlertKind::PriceBelow, dec!(150.5), true)
        );

        s.mark_triggered(alert.id).unwrap();
        assert!(!s.alerts().unwrap()[0].is_active());
        assert!(
            matches!(s.mark_triggered(alert.id), Err(ServiceError::NotFound(_))),
            "can't fire twice"
        );

        s.delete_alert(alert.id).unwrap();
        assert!(s.alerts().unwrap().is_empty());
    }
}
