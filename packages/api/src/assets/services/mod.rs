//! Asset use cases: describing assets and entering their prices.

use crate::{assets::repositories::AssetRepository, shared::ServiceError};
use dtos::assets::{AssetInfo, ManualPrice};
use rust_decimal::Decimal;
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;

#[derive(Clone)]
pub struct AssetService {
    repo: Arc<dyn AssetRepository>,
}

impl AssetService {
    pub fn new(repo: Arc<dyn AssetRepository>) -> Self {
        Self { repo }
    }

    pub fn assets(&self) -> Result<Vec<AssetInfo>, ServiceError> {
        Ok(self.repo.assets()?)
    }

    pub fn save_asset(&self, asset: AssetInfo) -> Result<(), ServiceError> {
        let asset = asset.validated().map_err(ServiceError::Validation)?;
        Ok(self.repo.save_asset(&asset)?)
    }

    pub fn delete_asset(&self, ticker: TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.delete_asset(&ticker)?)
    }

    pub fn prices(&self, ticker: TickerSymbol) -> Result<Vec<ManualPrice>, ServiceError> {
        Ok(self.repo.prices(&ticker)?)
    }

    /// Only for assets marked as priced by hand.
    pub fn save_price(&self, ticker: TickerSymbol, price: ManualPrice) -> Result<(), ServiceError> {
        if price.price <= Decimal::ZERO {
            return Err(ServiceError::Validation("The price must be above zero".into()));
        }
        if chrono::NaiveDate::parse_from_str(&price.date, "%Y-%m-%d").is_err() || price.date.len() != 10 {
            return Err(ServiceError::Validation("Date must be YYYY-MM-DD".into()));
        }
        let manual = self.repo.assets()?.iter().any(|a| a.ticker == ticker && a.manual);
        if !manual {
            return Err(ServiceError::Validation(format!("{ticker} is priced by the market, not by hand")));
        }
        Ok(self.repo.save_price(&ticker, &price)?)
    }

    pub fn delete_price(&self, ticker: TickerSymbol, date: String) -> Result<(), ServiceError> {
        Ok(self.repo.delete_price(&ticker, &date)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assets::infrastructures::SqliteAssetRepository, database::Database};
    use dtos::assets::TaxWrapper;
    use rust_decimal_macros::dec;
    use types::asset_class::AssetClass;

    #[test]
    fn describes_assets_and_prices_them() {
        let s = AssetService::new(Arc::new(SqliteAssetRepository::new(Database::in_memory().unwrap())));
        let t = TickerSymbol::new("KFSSF").unwrap();
        let price = |d: &str, p| ManualPrice { date: d.into(), price: p };
        assert!(matches!(s.save_price(t.clone(), price("2026-01-01", dec!(10))), Err(ServiceError::Validation(_))));

        let fund = AssetInfo {
            class: AssetClass::Fund,
            name: "Krungsri SSF".into(),
            currency: "THB".into(),
            manual: true,
            wrapper: Some(TaxWrapper::Ssf),
            ..AssetInfo::new(t.clone())
        };
        s.save_asset(fund.clone()).unwrap();
        assert_eq!(s.assets().unwrap(), vec![fund]);
        s.save_price(t.clone(), price("2026-01-01", dec!(10))).unwrap();
        s.save_price(t.clone(), price("2026-02-01", dec!(10.5))).unwrap();
        s.save_price(t.clone(), price("2026-02-01", dec!(10.7))).unwrap();
        assert_eq!(s.prices(t.clone()).unwrap(), vec![price("2026-02-01", dec!(10.7)), price("2026-01-01", dec!(10))]);
        assert!(s.save_price(t.clone(), price("2026-13-01", dec!(1))).is_err());
        assert!(s.save_price(t.clone(), price("2026-03-01", dec!(0))).is_err());

        s.delete_asset(t.clone()).unwrap();
        assert!(s.assets().unwrap().is_empty() && s.prices(t).unwrap().is_empty());
    }
}
