//! Watchlist, note and alert use cases.

use crate::{shared::ServiceError, watchlist::repositories::WatchlistRepository};
use dtos::watch::{
    normalize_tags, Alert, AlertKind, Note, WatchItem, Watchlist, MAX_NOTE_LEN,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

const MAX_LIST_NAME: usize = 40;

#[derive(Clone)]
pub struct WatchlistService {
    repo: Arc<dyn WatchlistRepository>,
}

impl WatchlistService {
    pub fn new(repo: Arc<dyn WatchlistRepository>) -> Self {
        Self { repo }
    }

    pub fn watchlists(&self) -> Result<Vec<Watchlist>, ServiceError> {
        Ok(self.repo.watchlists()?)
    }

    /// Every watched stock once, across all lists.
    pub fn watchlist(&self) -> Result<Vec<WatchItem>, ServiceError> {
        let mut items: Vec<WatchItem> = Vec::new();
        for item in self.repo.watchlists()?.into_iter().flat_map(|l| l.items) {
            if !items.iter().any(|i| i.ticker == item.ticker) {
                items.push(item);
            }
        }
        Ok(items)
    }

    /// Adds to the first list (creating one if there are none).
    pub fn watch(&self, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        let list = self.default_list()?;
        Ok(self.repo.watch(list, ticker)?)
    }

    pub fn watch_in(&self, list: Uuid, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        self.ensure_list(list)?;
        Ok(self.repo.watch(list, ticker)?)
    }

    /// From every list.
    pub fn unwatch(&self, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.unwatch(None, ticker)?)
    }

    pub fn unwatch_from(&self, list: Uuid, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.unwatch(Some(list), ticker)?)
    }

    pub fn create_list(&self, name: &str) -> Result<Uuid, ServiceError> {
        let name = self.valid_name(name, None)?;
        let id = Uuid::new_v4();
        self.repo.create_list(id, &name)?;
        Ok(id)
    }

    pub fn rename_list(&self, id: Uuid, name: &str) -> Result<(), ServiceError> {
        let name = self.valid_name(name, Some(id))?;
        Ok(self.repo.rename_list(id, &name)?)
    }

    /// The last list can't be deleted.
    pub fn delete_list(&self, id: Uuid) -> Result<(), ServiceError> {
        let lists = self.repo.watchlists()?;
        if !lists.iter().any(|l| l.id == id) {
            return Err(ServiceError::NotFound(format!("watchlist {id}")));
        }
        if lists.len() == 1 {
            return Err(ServiceError::Validation("You need at least one watchlist".into()));
        }
        Ok(self.repo.delete_list(id)?)
    }

    pub fn notes(&self) -> Result<Vec<Note>, ServiceError> {
        Ok(self.repo.notes()?)
    }

    /// Saves your note and tags on `ticker`; clearing both deletes it.
    pub fn save_note(
        &self,
        ticker: &TickerSymbol,
        text: &str,
        tags: &[String],
    ) -> Result<(), ServiceError> {
        let text = text.trim();
        if text.chars().count() > MAX_NOTE_LEN {
            return Err(ServiceError::Validation(format!(
                "Notes can be at most {MAX_NOTE_LEN} characters"
            )));
        }
        let tags = normalize_tags(tags);
        if text.is_empty() && tags.is_empty() {
            return Ok(self.repo.delete_note(ticker.as_str())?);
        }
        Ok(self.repo.save_note(&Note {
            ticker: ticker.to_string(),
            text: text.to_string(),
            tags,
            updated_at: String::new(),
        })?)
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
        if !self.watchlist()?.iter().any(|i| i.ticker == alert.ticker) {
            self.watch(&alert.ticker)?;
        }
        Ok(alert)
    }

    pub fn delete_alert(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_alert(id)?)
    }

    pub fn mark_triggered(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.mark_triggered(id)?)
    }

    fn default_list(&self) -> Result<Uuid, ServiceError> {
        match self.repo.watchlists()?.first() {
            Some(list) => Ok(list.id),
            None => self.create_list("Watchlist"),
        }
    }

    fn ensure_list(&self, id: Uuid) -> Result<(), ServiceError> {
        if self.repo.watchlists()?.iter().any(|l| l.id == id) {
            Ok(())
        } else {
            Err(ServiceError::NotFound(format!("watchlist {id}")))
        }
    }

    /// Trimmed, non-empty, not too long and unique (ignoring `except`).
    fn valid_name(&self, name: &str, except: Option<Uuid>) -> Result<String, ServiceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ServiceError::Validation("Give the list a name".into()));
        }
        if name.chars().count() > MAX_LIST_NAME {
            return Err(ServiceError::Validation(format!(
                "Names can be at most {MAX_LIST_NAME} characters"
            )));
        }
        let taken = self
            .repo
            .watchlists()?
            .iter()
            .any(|l| Some(l.id) != except && l.name.eq_ignore_ascii_case(name));
        if taken {
            return Err(ServiceError::Validation(format!(
                "You already have a list called “{name}”"
            )));
        }
        Ok(name.to_string())
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
    fn several_lists() {
        let s = service();
        assert_eq!(s.watchlists().unwrap().len(), 1, "a default list exists");
        let div = s.create_list(" Dividends ").unwrap();
        assert!(matches!(s.create_list("dividends"), Err(ServiceError::Validation(_))));
        s.watch_in(div, &sym("KO")).unwrap();
        s.watch(&sym("KO")).unwrap();
        s.watch(&sym("NVDA")).unwrap();
        let lists = s.watchlists().unwrap();
        assert_eq!(lists[1].name, "Dividends");
        assert_eq!(lists[1].items.len(), 1);
        assert_eq!(s.watchlist().unwrap().len(), 2, "KO counted once");

        s.unwatch_from(div, &sym("KO")).unwrap();
        assert!(s.watchlists().unwrap()[1].items.is_empty());
        assert_eq!(s.watchlist().unwrap().len(), 2, "still in the first list");

        s.rename_list(div, "Income").unwrap();
        assert_eq!(s.watchlists().unwrap()[1].name, "Income");
        s.delete_list(div).unwrap();
        let only = s.watchlists().unwrap()[0].id;
        assert!(matches!(s.delete_list(only), Err(ServiceError::Validation(_))));
        assert!(matches!(
            s.watch_in(Uuid::new_v4(), &sym("X")),
            Err(ServiceError::NotFound(_))
        ));
    }

    #[test]
    fn notes_save_normalize_and_clear() {
        let s = service();
        let tags = ["Dividend".to_string(), "#dividend".into(), "long term".into()];
        s.save_note(&sym("KO"), "  Steady payer. ", &tags).unwrap();
        let notes = s.notes().unwrap();
        assert_eq!(notes[0].text, "Steady payer.");
        assert_eq!(notes[0].tags, vec!["dividend", "long term"]);
        s.save_note(&sym("KO"), "", &[]).unwrap();
        assert!(s.notes().unwrap().is_empty());
        let long = "x".repeat(MAX_NOTE_LEN + 1);
        assert!(matches!(s.save_note(&sym("KO"), &long, &[]), Err(ServiceError::Validation(_))));
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
