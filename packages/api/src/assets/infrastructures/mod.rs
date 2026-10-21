//! SQLite adapter for [`AssetRepository`].

use crate::{assets::repositories::AssetRepository, database::Database, shared::RepositoryError};
use dtos::assets::{AssetInfo, ManualPrice, PriceFeed, TaxWrapper};
use rusqlite::{params, OptionalExtension};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::{asset_class::AssetClass, ticker_symbol::TickerSymbol};

pub struct SqliteAssetRepository {
    db: Database,
}

impl SqliteAssetRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

fn corrupt(what: &str, value: &str) -> RepositoryError {
    RepositoryError::Corrupt(format!("{what} \"{value}\""))
}

impl AssetRepository for SqliteAssetRepository {
    fn assets(&self) -> Result<Vec<AssetInfo>, RepositoryError> {
        type Row = (String, String, String, String, bool, Option<String>, Option<String>, Option<String>);
        let rows: Vec<Row> = self.db.with(|c| {
            c.prepare("SELECT ticker, class, name, currency, manual, wrapper, yearly_rate, feed FROM assets ORDER BY ticker")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(ticker, class, name, currency, manual, wrapper, rate, feed)| {
                Ok(AssetInfo {
                    ticker: TickerSymbol::new(&ticker).map_err(|_| corrupt("ticker", &ticker))?,
                    class: AssetClass::from_key(&class),
                    name,
                    currency,
                    manual,
                    wrapper: wrapper.as_deref().and_then(TaxWrapper::from_key),
                    yearly_rate: rate.map(|r| Decimal::from_str(&r).map_err(|_| corrupt("interest rate", &r))).transpose()?,
                    feed: feed.as_deref().and_then(PriceFeed::from_key),
                })
            })
            .collect()
    }

    fn save_asset(&self, a: &AssetInfo) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO assets (ticker, class, name, currency, manual, wrapper, yearly_rate, feed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    a.ticker.as_str(),
                    a.class.key(),
                    a.name,
                    a.currency,
                    a.manual,
                    a.wrapper.map(|w| w.key()),
                    a.yearly_rate.map(|r| r.to_string()),
                    a.feed.map(|f| f.key())
                ],
            )
        })?;
        Ok(())
    }

    fn delete_asset(&self, ticker: &TickerSymbol) -> Result<(), RepositoryError> {
        self.db.transaction(|t| {
            t.execute("DELETE FROM manual_prices WHERE ticker = ?1", [ticker.as_str()])?;
            t.execute("DELETE FROM assets WHERE ticker = ?1", [ticker.as_str()])
        })?;
        Ok(())
    }

    fn prices(&self, ticker: &TickerSymbol) -> Result<Vec<ManualPrice>, RepositoryError> {
        let rows: Vec<(String, String)> = self.db.with(|c| {
            c.prepare("SELECT date, price FROM manual_prices WHERE ticker = ?1 ORDER BY date DESC")?
                .query_map([ticker.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(date, price)| {
                Ok(ManualPrice { price: Decimal::from_str(&price).map_err(|_| corrupt("price", &price))?, date })
            })
            .collect()
    }

    fn save_price(&self, ticker: &TickerSymbol, p: &ManualPrice) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO manual_prices (ticker, date, price) VALUES (?1, ?2, ?3)",
                params![ticker.as_str(), p.date, p.price.to_string()],
            )
        })?;
        Ok(())
    }

    fn delete_price(&self, ticker: &TickerSymbol, date: &str) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute("DELETE FROM manual_prices WHERE ticker = ?1 AND date = ?2", params![ticker.as_str(), date])
        })?;
        Ok(())
    }

    fn secret(&self, name: &str) -> Result<Option<String>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.query_row("SELECT value FROM app_secrets WHERE name = ?1", [name], |r| r.get(0)).optional()
        })?)
    }

    fn set_secret(&self, name: &str, value: Option<&str>) -> Result<(), RepositoryError> {
        self.db.with(|c| match value {
            Some(v) => c.execute("INSERT OR REPLACE INTO app_secrets (name, value) VALUES (?1, ?2)", params![name, v]),
            None => c.execute("DELETE FROM app_secrets WHERE name = ?1", [name]),
        })?;
        Ok(())
    }
}
