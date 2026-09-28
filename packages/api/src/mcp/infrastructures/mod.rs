//! SQLite adapters for named MCP connections and [`AiPortfolioRepository`],
//! live prices from the quote service, and the HTTP endpoint.

pub mod http;

use crate::{
    database::Database,
    mcp::repositories::{AiPortfolioRepository, ConnectionRecord, ConnectionRepository, LiveQuote, LivePrices},
    quote::services::quote::QuoteService,
    shared::RepositoryError,
};
use rusqlite::{OptionalExtension, params};
use dtos::mcp::{McpAccessPreset, McpAuditEvent, McpConnection};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub struct SqliteConnectionRepository {
    db: Database,
}

impl SqliteConnectionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

fn preset(raw: &str) -> McpAccessPreset {
    match raw { "trader" => McpAccessPreset::Trader, "thesis_editor" => McpAccessPreset::ThesisEditor, _ => McpAccessPreset::ReadOnly }
}

impl ConnectionRepository for SqliteConnectionRepository {
    fn list(&self) -> Result<Vec<McpConnection>, RepositoryError> {
        let rows: Vec<(String,String,i64,String,String,String,Option<String>)> = self.db.with(|c| c.prepare(
            "SELECT id,name,enabled,preset,created_at,updated_at,last_used_at FROM mcp_connections ORDER BY created_at,id"
        )?.query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?.collect())?;
        rows.into_iter().map(|r| {
            let id = Uuid::parse_str(&r.0).map_err(|e| RepositoryError::Corrupt(e.to_string()))?;
            let portfolio_ids = self.db.with(|c| c.prepare("SELECT portfolio_id FROM mcp_connection_portfolios WHERE connection_id=?1 ORDER BY portfolio_id")?
                .query_map([&r.0], |row| row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>())?
                .into_iter().filter_map(|v| Uuid::parse_str(&v).ok()).collect();
            Ok(McpConnection { id, name:r.1, enabled:r.2!=0, preset:preset(&r.3), portfolio_ids, created_at:r.4, updated_at:r.5, last_used_at:r.6 })
        }).collect()
    }

    fn find(&self, id: Uuid) -> Result<Option<ConnectionRecord>, RepositoryError> {
        let hash: Option<String> = self.db.with(|c| c.query_row("SELECT token_hash FROM mcp_connections WHERE id=?1", [id.to_string()], |r| r.get(0)).optional())?;
        Ok(match hash { Some(token_hash) => self.list()?.into_iter().find(|v| v.id==id).map(|connection| ConnectionRecord{connection,token_hash}), None => None })
    }

    fn create(&self, record: &ConnectionRecord) -> Result<(), RepositoryError> {
        let c = &record.connection;
        self.db.transaction(|tx| { tx.execute("INSERT INTO mcp_connections(id,name,enabled,preset,token_hash) VALUES(?1,?2,?3,?4,?5)", params![c.id.to_string(),c.name,c.enabled,c.preset.key(),record.token_hash])?;
            for id in &c.portfolio_ids { tx.execute("INSERT INTO mcp_connection_portfolios(connection_id,portfolio_id) VALUES(?1,?2)", params![c.id.to_string(),id.to_string()])?; } Ok(()) })?;
        Ok(())
    }

    fn update(&self, id: Uuid, name: &str, access: McpAccessPreset, enabled: bool, portfolios: &[Uuid]) -> Result<(), RepositoryError> {
        self.db.transaction(|tx| { tx.execute("UPDATE mcp_connections SET name=?2,preset=?3,enabled=?4,updated_at=datetime('now') WHERE id=?1", params![id.to_string(),name,access.key(),enabled])?; tx.execute("DELETE FROM mcp_connection_portfolios WHERE connection_id=?1",[id.to_string()])?; for p in portfolios { tx.execute("INSERT INTO mcp_connection_portfolios(connection_id,portfolio_id) VALUES(?1,?2)",params![id.to_string(),p.to_string()])?; } Ok(()) })?; Ok(())
    }
    fn rotate(&self,id:Uuid,hash:&str)->Result<(),RepositoryError>{self.db.with(|c|c.execute("UPDATE mcp_connections SET token_hash=?2,updated_at=datetime('now') WHERE id=?1",params![id.to_string(),hash]))?;Ok(())}
    fn delete(&self,id:Uuid)->Result<(),RepositoryError>{self.db.with(|c|c.execute("DELETE FROM mcp_connections WHERE id=?1",[id.to_string()]))?;Ok(())}
    fn touch(&self,id:Uuid)->Result<(),RepositoryError>{self.db.with(|c|c.execute("UPDATE mcp_connections SET last_used_at=datetime('now') WHERE id=?1",[id.to_string()]))?;Ok(())}
    fn audit(&self,id:Uuid,method:&str,tool:Option<&str>,portfolios:&[Uuid],success:bool,error:Option<&str>)->Result<(),RepositoryError>{let ids=serde_json::to_string(portfolios).unwrap_or_else(|_|"[]".into());self.db.transaction(|tx|{tx.execute("INSERT INTO mcp_audit(connection_id,method,tool,portfolio_ids,success,error_category) VALUES(?1,?2,?3,?4,?5,?6)",params![id.to_string(),method,tool,ids,success,error])?;tx.execute("DELETE FROM mcp_audit WHERE connection_id=?1 AND id NOT IN (SELECT id FROM mcp_audit WHERE connection_id=?1 ORDER BY id DESC LIMIT 500)",[id.to_string()])?;Ok(())})?;Ok(())}
    fn events(&self,id:Uuid,limit:usize)->Result<Vec<McpAuditEvent>,RepositoryError>{Ok(self.db.with(|c|c.prepare("SELECT id,at,method,tool,portfolio_ids,success,error_category FROM mcp_audit WHERE connection_id=?1 ORDER BY id DESC LIMIT ?2")?.query_map(params![id.to_string(),limit as i64],|r|{let raw:String=r.get(4)?;Ok(McpAuditEvent{id:r.get(0)?,at:r.get(1)?,method:r.get(2)?,tool:r.get(3)?,portfolio_ids:serde_json::from_str(&raw).unwrap_or_default(),success:r.get::<_,i64>(5)?!=0,error_category:r.get(6)?})})?.collect())?)}
    fn in_active_race(&self, id: Uuid) -> Result<bool, RepositoryError> {
        Ok(self.db.with(|c| c.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM ai_trader_configs tc
                JOIN ai_race_contestants rc ON rc.portfolio_id=tc.portfolio_id
                JOIN ai_races r ON r.id=rc.race_id
                WHERE tc.mcp_connection_id=?1 AND r.status IN ('running','paused')
            )",
            [id.to_string()],
            |r| r.get(0),
        ))?)
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
