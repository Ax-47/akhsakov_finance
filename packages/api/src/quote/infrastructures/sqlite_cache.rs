//! SQLite adapter for [`QuoteCache`]: quotes and candles stored as JSON.

use crate::{database::Database, quote::repositories::quote_cache::QuoteCache};
use rusqlite::{params, OptionalExtension};
use types::{candle::Candle, quote::Quote, ticker_symbol::TickerSymbol};

pub struct SqliteQuoteCache {
    db: Database,
}

impl SqliteQuoteCache {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn save(&self, sql: &str, key: &str, json: String) {
        if let Err(e) = self.db.with(|c| c.execute(sql, params![key, json])) {
            tracing::warn!("couldn't save {key} for offline use: {e}");
        }
    }

    fn load(&self, sql: &str, key: &str) -> Option<String> {
        self.db
            .with(|c| c.query_row(sql, [key], |r| r.get(0)).optional())
            .ok()
            .flatten()
    }
}

impl QuoteCache for SqliteQuoteCache {
    fn save_quote(&self, quote: &Quote) {
        if let Ok(json) = serde_json::to_string(quote) {
            self.save(
                "INSERT OR REPLACE INTO quote_cache (ticker, json, updated_at) VALUES (?1, ?2, datetime('now'))",
                quote.ticker_symbol.as_str(),
                json,
            );
        }
    }

    fn load_quote(&self, ticker: &TickerSymbol) -> Option<Quote> {
        let json = self.load("SELECT json FROM quote_cache WHERE ticker = ?1", ticker.as_str())?;
        serde_json::from_str(&json).ok()
    }

    fn save_chart(&self, key: &str, candles: &[Candle]) {
        if let Ok(json) = serde_json::to_string(candles) {
            self.save(
                "INSERT OR REPLACE INTO chart_cache (key, json, updated_at) VALUES (?1, ?2, datetime('now'))",
                key,
                json,
            );
        }
    }

    fn load_chart(&self, key: &str) -> Option<Vec<Candle>> {
        let json = self.load("SELECT json FROM chart_cache WHERE key = ?1", key)?;
        serde_json::from_str(&json).ok()
    }
}
