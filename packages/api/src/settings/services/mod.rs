//! Settings use cases: defaults for anything unset, validation on save.

use crate::{settings::repositories::SettingsRepository, shared::ServiceError};
use dtos::settings::Settings;
use rust_decimal::Decimal;
use std::{str::FromStr, sync::Arc};
use types::ticker_symbol::TickerSymbol;

#[derive(Clone)]
pub struct SettingsService {
    repo: Arc<dyn SettingsRepository>,
}

impl SettingsService {
    pub fn new(repo: Arc<dyn SettingsRepository>) -> Self {
        Self { repo }
    }

    /// Stored settings, with defaults for missing or unreadable values.
    pub fn get(&self) -> Result<Settings, ServiceError> {
        let stored = self.repo.all()?;
        let d = Settings::default();
        let dec = |key: &str, fallback: Decimal| {
            stored
                .get(key)
                .and_then(|v| Decimal::from_str(v).ok())
                .unwrap_or(fallback)
        };
        Ok(Settings {
            currency: stored.get("currency").cloned().unwrap_or(d.currency),
            risk_free: dec("risk_free", d.risk_free),
            benchmark: stored
                .get("benchmark")
                .and_then(|b| TickerSymbol::new(b).ok())
                .unwrap_or(d.benchmark),
            assumed_return: dec("assumed_return", d.assumed_return),
        })
    }

    pub fn save(&self, settings: Settings) -> Result<(), ServiceError> {
        settings.validate().map_err(ServiceError::Validation)?;
        Ok(self.repo.set_all(&[
            ("currency", settings.currency),
            ("risk_free", settings.risk_free.to_string()),
            ("benchmark", settings.benchmark.to_string()),
            ("assumed_return", settings.assumed_return.to_string()),
        ])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, settings::infrastructures::SqliteSettingsRepository};
    use rust_decimal_macros::dec;

    #[test]
    fn defaults_then_round_trip() {
        let s = SettingsService::new(Arc::new(SqliteSettingsRepository::new(
            Database::in_memory().unwrap(),
        )));
        assert_eq!(s.get().unwrap(), Settings::default());
        let custom = Settings {
            currency: "THB".into(),
            risk_free: dec!(2.5),
            ..Settings::default()
        };
        s.save(custom.clone()).unwrap();
        assert_eq!(s.get().unwrap(), custom);
        assert!(matches!(
            s.save(Settings {
                currency: "XYZ".into(),
                ..custom
            }),
            Err(ServiceError::Validation(_))
        ));
    }
}
