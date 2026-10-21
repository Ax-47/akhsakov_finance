//! SQLite adapter for [`ManualQuotes`], reading the assets context's tables.

use crate::{
    database::Database,
    quote::repositories::manual_quotes::{ManualQuotes, ManualSeries},
};
use dtos::assets::ManualPrice;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;

pub struct SqliteManualQuotes {
    db: Database,
}

impl SqliteManualQuotes {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl ManualQuotes for SqliteManualQuotes {
    fn manual(&self, ticker: &TickerSymbol) -> Option<ManualSeries> {
        self.db
            .with(|c| {
                let Some((currency, rate)) = c
                    .query_row(
                        "SELECT currency, yearly_rate FROM assets WHERE ticker = ?1 AND manual = 1",
                        [ticker.as_str()],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                    )
                    .optional()?
                else {
                    return Ok(None);
                };
                let prices = c
                    .prepare("SELECT date, price FROM manual_prices WHERE ticker = ?1 ORDER BY date")?
                    .query_map([ticker.as_str()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                    .filter_map(|row| {
                        let (date, price) = row.ok()?;
                        Some(ManualPrice { date, price: Decimal::from_str(&price).ok()? })
                    })
                    .collect();
                let yearly_rate = rate.and_then(|r| Decimal::from_str(&r).ok());
                Ok(Some(ManualSeries { currency, prices, yearly_rate }))
            })
            .ok()
            .flatten()
    }
}
