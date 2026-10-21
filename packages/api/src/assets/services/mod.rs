//! Asset use cases: describing assets, entering their prices, and fetching
//! prices for the ones with a feed.

use crate::{
    assets::{feeds::PriceSource, repositories::AssetRepository},
    shared::ServiceError,
};
use dtos::assets::{AssetInfo, FeedUpdate, ManualPrice, PriceFeed};
use rust_decimal::Decimal;
use std::{sync::Arc, time::Duration};
use types::ticker_symbol::TickerSymbol;

/// Name of the SEC open data API key in the secret store.
const SEC_KEY: &str = "sec_api_key";
/// How often fetched prices are updated.
const FEED_EVERY: Duration = Duration::from_secs(3 * 60 * 60);
/// How far back to look for a fund's latest NAV (long holidays included).
const NAV_DAYS_BACK: i64 = 10;

/// Today in Thailand (UTC+7), `YYYY-MM-DD`: the date Thai prices are for.
pub fn thai_today() -> String {
    (chrono::Utc::now() + chrono::Duration::hours(7)).format("%Y-%m-%d").to_string()
}

#[derive(Clone)]
pub struct AssetService {
    repo: Arc<dyn AssetRepository>,
    feeds: Option<Arc<dyn PriceSource>>,
}

impl AssetService {
    pub fn new(repo: Arc<dyn AssetRepository>) -> Self {
        Self { repo, feeds: None }
    }

    pub fn with_feeds(mut self, feeds: Arc<dyn PriceSource>) -> Self {
        self.feeds = Some(feeds);
        self
    }

    pub fn has_sec_key(&self) -> Result<bool, ServiceError> {
        Ok(self.repo.secret(SEC_KEY)?.is_some())
    }

    /// Saves the SEC API key; an empty one forgets it.
    pub fn set_sec_key(&self, key: &str) -> Result<(), ServiceError> {
        let key = key.trim();
        if key.len() > 200 || key.chars().any(char::is_whitespace) {
            return Err(ServiceError::Validation("That doesn't look like an API key".into()));
        }
        Ok(self.repo.set_secret(SEC_KEY, Some(key).filter(|k| !k.is_empty()))?)
    }

    /// Fetches the latest price of every asset with a feed and saves it as
    /// a price on its date. Each asset reports what happened; one failing
    /// doesn't stop the others.
    pub async fn refresh_feeds(&self, today: &str) -> Result<Vec<FeedUpdate>, ServiceError> {
        let Some(source) = self.feeds.clone() else {
            return Ok(vec![]);
        };
        let assets: Vec<AssetInfo> = self.repo.assets()?.into_iter().filter(|a| a.manual && a.feed.is_some()).collect();
        let mut gold: Option<Result<ManualPrice, String>> = None;
        let mut updates = Vec::new();
        for asset in assets {
            let outcome = match asset.feed {
                Some(PriceFeed::ThaiGold) => {
                    if gold.is_none() {
                        gold = Some(source.thai_gold().await);
                    }
                    gold.clone().expect("fetched above")
                }
                Some(PriceFeed::ThaiFundNav) => self.latest_nav(source.as_ref(), &asset.ticker, today).await,
                None => continue,
            };
            let outcome = match outcome {
                Ok(price) => self.repo.save_price(&asset.ticker, &price).map(|()| price).map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            updates.push(FeedUpdate { ticker: asset.ticker, outcome });
        }
        Ok(updates)
    }

    /// The fund's latest NAV, from the last [`NAV_DAYS_BACK`] days up to
    /// `today`, saved on the day it's for.
    async fn latest_nav(&self, source: &dyn PriceSource, ticker: &TickerSymbol, today: &str) -> Result<ManualPrice, String> {
        let key = self
            .repo
            .secret(SEC_KEY)
            .map_err(|e| e.to_string())?
            .ok_or("Add your SEC API key in Settings to fetch fund prices")?;
        let fund = source.find_fund(&key, ticker.as_str()).await?;
        let from = dtos::planning::add_days(today, -NAV_DAYS_BACK).ok_or("Bad date")?;
        source
            .latest_nav(&key, &fund, &from, today)
            .await?
            .ok_or_else(|| format!("The SEC has no NAV for {ticker} in the last {NAV_DAYS_BACK} days"))
    }

    /// Updates fetched prices every few hours, while any asset has a feed.
    pub fn spawn_feed_updates(self) {
        tokio::spawn(async move {
            loop {
                match self.refresh_feeds(&thai_today()).await {
                    Ok(updates) => {
                        for u in updates.iter().filter_map(|u| u.outcome.as_ref().err().map(|e| (&u.ticker, e))) {
                            tracing::warn!("price feed for {}: {}", u.0, u.1);
                        }
                    }
                    Err(e) => tracing::warn!("price feeds failed: {e}"),
                }
                tokio::time::sleep(FEED_EVERY).await;
            }
        });
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
    use crate::{
        assets::{feeds::SecFund, infrastructures::SqliteAssetRepository},
        database::Database,
    };
    use dtos::assets::TaxWrapper;
    use rust_decimal_macros::dec;
    use types::asset_class::AssetClass;

    /// Gold at ฿41,950; fund KFSSF (SEC id F1) last priced on 2026-10-17.
    struct FakeSource;

    #[async_trait::async_trait]
    impl PriceSource for FakeSource {
        async fn thai_gold(&self) -> Result<ManualPrice, String> {
            Ok(ManualPrice { date: "2026-10-20".into(), price: dec!(41950) })
        }
        async fn find_fund(&self, key: &str, name: &str) -> Result<SecFund, String> {
            assert_eq!(key, "sec-key");
            (name == "KFSSF").then(|| SecFund { proj_id: "F1".into(), class: None }).ok_or_else(|| format!("no fund {name}"))
        }
        async fn latest_nav(&self, _: &str, fund: &SecFund, from: &str, to: &str) -> Result<Option<ManualPrice>, String> {
            assert_eq!((fund.proj_id.as_str(), from, to), ("F1", "2026-10-10", "2026-10-20"));
            Ok(Some(ManualPrice { date: "2026-10-17".into(), price: dec!(12.34) }))
        }
    }

    #[tokio::test]
    async fn fetches_gold_and_fund_prices() {
        let s = AssetService::new(Arc::new(SqliteAssetRepository::new(Database::in_memory().unwrap())))
            .with_feeds(Arc::new(FakeSource));
        let asset = |t: &str, feed| AssetInfo {
            manual: true,
            currency: "THB".into(),
            feed: Some(feed),
            ..AssetInfo::new(TickerSymbol::new(t).unwrap())
        };
        s.save_asset(asset("GOLD965", PriceFeed::ThaiGold)).unwrap();
        s.save_asset(asset("KFSSF", PriceFeed::ThaiFundNav)).unwrap();
        s.save_asset(asset("NOPE", PriceFeed::ThaiFundNav)).unwrap();
        // Without a key, funds say what's missing; gold still updates.
        let first = s.refresh_feeds("2026-10-20").await.unwrap();
        let by = |u: &[FeedUpdate], t: &str| u.iter().find(|u| u.ticker.as_str() == t).unwrap().outcome.clone();
        assert_eq!(by(&first, "GOLD965"), Ok(ManualPrice { date: "2026-10-20".into(), price: dec!(41950) }));
        assert!(by(&first, "KFSSF").unwrap_err().contains("SEC API key"));

        assert!(!s.has_sec_key().unwrap());
        s.set_sec_key(" sec-key ").unwrap();
        assert!(s.has_sec_key().unwrap());
        let second = s.refresh_feeds("2026-10-20").await.unwrap();
        // The latest NAV is saved on its own date.
        assert_eq!(by(&second, "KFSSF"), Ok(ManualPrice { date: "2026-10-17".into(), price: dec!(12.34) }));
        assert!(by(&second, "NOPE").unwrap_err().contains("no fund"));
        assert_eq!(s.prices(TickerSymbol::new("KFSSF").unwrap()).unwrap()[0].price, dec!(12.34));
        assert!(s.set_sec_key("has space").is_err());
        s.set_sec_key("").unwrap();
        assert!(!s.has_sec_key().unwrap());
    }

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
            yearly_rate: Some(dec!(1.25)),
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
