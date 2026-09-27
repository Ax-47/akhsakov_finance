//! SQLite adapters for [`KeyRepository`] and [`AiPortfolioRepository`],
//! live prices from the quote service, and the HTTP endpoint.

pub mod http;

use crate::{
    database::Database,
    mcp::repositories::{AiPortfolioRepository, KeyRepository, LiveQuote, LivePrices},
    quote::services::quote::QuoteService,
    shared::RepositoryError,
};
use rusqlite::OptionalExtension;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub struct SqliteKeyRepository {
    db: Database,
}

impl SqliteKeyRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl KeyRepository for SqliteKeyRepository {
    fn key(&self) -> Result<Option<String>, RepositoryError> {
        Ok(self.db.with(|c| {
            c.query_row("SELECT token FROM mcp_access WHERE id = 1", [], |r| r.get(0))
                .optional()
        })?)
    }

    fn set_key(&self, key: Option<&str>) -> Result<(), RepositoryError> {
        self.db.with(|c| match key {
            Some(key) => c.execute(
                "INSERT OR REPLACE INTO mcp_access (id, token, created_at) VALUES (1, ?1, datetime('now'))",
                [key],
            ),
            None => c.execute("DELETE FROM mcp_access", []),
        })?;
        Ok(())
    }
}

pub struct SqliteAiPortfolioRepository {
    db: Database,
}

impl SqliteAiPortfolioRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl AiPortfolioRepository for SqliteAiPortfolioRepository {
    fn ai_portfolios(&self) -> Result<Vec<Uuid>, RepositoryError> {
        let ids: Vec<String> = self.db.with(|c| {
            c.prepare("SELECT portfolio_id FROM ai_portfolios ORDER BY created_at, rowid")?
                .query_map([], |r| r.get(0))?
                .collect()
        })?;
        Ok(ids.iter().filter_map(|id| Uuid::parse_str(id).ok()).collect())
    }

    fn add_ai_portfolio(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db.with(|c| c.execute("INSERT OR IGNORE INTO ai_portfolios (portfolio_id) VALUES (?1)", [id.to_string()]))?;
        Ok(())
    }

    fn remove_ai_portfolio(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db.with(|c| c.execute("DELETE FROM ai_portfolios WHERE portfolio_id = ?1", [id.to_string()]))?;
        Ok(())
    }
}

/// [`LivePrices`] from the app's quote service (Yahoo, or prices entered by
/// hand).
pub struct QuotePrices(pub QuoteService);

#[async_trait::async_trait]
impl LivePrices for QuotePrices {
    async fn quote(&self, ticker: &TickerSymbol) -> Result<LiveQuote, String> {
        let q = self.0.native_quote(ticker.clone()).await.map_err(|e| e.to_string())?;
        let usd_per_unit = if q.currency == "USD" {
            rust_decimal::Decimal::ONE
        } else {
            self.0.usd_rate(&q.currency).await.map_err(|e| e.to_string())?
        };
        Ok(LiveQuote {
            price: q.current_price,
            previous_close: q.previous_close_price,
            currency: q.currency,
            usd_per_unit,
            timestamp: q.timestamp,
            stale: q.stale,
        })
    }
}
