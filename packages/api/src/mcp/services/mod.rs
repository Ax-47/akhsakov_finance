//! The connector's key, and the MCP protocol itself: JSON-RPC 2.0 messages
//! in, replies out. The tools live in [`tools`]; AI paper portfolios in
//! [`trading`].

pub mod race_gate;
pub mod tools;
pub mod trading;

use crate::{mcp::repositories::{ConnectionRecord, ConnectionRepository}, shared::ServiceError};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use dtos::mcp::{McpAccessPreset, McpAuditEvent, McpConnection, McpConnectionSecret, McpPortfolioScope};
use serde_json::{json, Value};
use std::sync::Arc;
use tools::Tools;
use trading::Trading;
use uuid::Uuid;

/// Newest first; the first is offered to clients asking for another.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Given to the model when it connects.
const INSTRUCTIONS: &str = "Akhsakov Finance is the user's portfolio tracker. It keeps an investment \
thesis for each holding in each portfolio: why they own it, what would make them sell (sell_if), a \
target price, conviction from 1 to 5, a review date, a status (on_track, at_risk, broken), and a dated \
journal. Use it as long-term memory of the user's reasoning.\n\
- Before discussing, analysing or recommending anything about a holding, call get_thesis (or \
list_theses for a whole portfolio) to recall why the user owns it.\n\
- When you learn something that bears on a thesis (earnings, news, a review with the user), record it \
with add_thesis_note in one or two sentences, in the user's language.\n\
- Change the thesis fields with save_thesis only when the user asks or agrees. Every change you make is \
labelled as written by AI and logged in the journal.\n\
- Holdings with no thesis are listed by list_theses; offer to write one with the user.\n\
- Amounts are in USD. The user's holdings come without live prices; get_quote looks one up.\n\
- The user may have given you portfolios of your own, each with its own paper money, to manage \
yourself (get_my_portfolio). Decide what to buy and sell in each, and trade with place_order (naming the \
portfolio when you have more than one), giving a short reason each time; the reason is kept in that \
holding's journal. The user may have set goals for those portfolios (get_my_goals): aim for them, \
but you can't change them. Invest for the long run, spread the risk, and \
don't trade just to be busy. place_order can't touch the user's other portfolios; never present your \
own portfolio's trades as advice to copy.\n\
- A portfolio of yours may be entered in an AI race; get_my_portfolio then shows `race`. Trade it only \
while `race.window_open` is true, before `race.deadline`: prices are frozen for the round and orders \
outside the window are rejected. Otherwise check back around `race.next_round_at`.";

/// What a request's key allows.
#[derive(Debug, PartialEq)]
pub enum Access {
    Granted(AccessContext),
    Denied,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessContext {
    pub connection_id: Uuid,
    pub preset: McpAccessPreset,
    pub portfolio_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct McpService {
    connections: Arc<dyn ConnectionRepository>,
    tools: Tools,
    trading: Trading,
}

fn hash_secret(secret: &str) -> Result<String, ServiceError> {
    let salt=SaltString::encode_b64(Uuid::new_v4().as_bytes()).map_err(|e|ServiceError::Storage(e.to_string()))?;
    Argon2::default().hash_password(secret.as_bytes(), &salt)
        .map(|v| v.to_string()).map_err(|e| ServiceError::Storage(e.to_string()))
}

fn reply(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

impl McpService {
    pub fn new(connections: Arc<dyn ConnectionRepository>, tools: Tools, trading: Trading) -> Self {
        Self { connections, tools, trading }
    }

    /// AI paper portfolios, which Settings sets up and funds.
    pub fn trading(&self) -> &Trading {
        &self.trading
    }

    pub(crate) fn tools(&self) -> &Tools {
        &self.tools
    }

    /// Which portfolios are racing, and their round windows.
    pub(crate) fn race_gate(&self) -> &race_gate::RaceGate {
        self.tools.race_gate()
    }

    pub fn connections(&self) -> Result<Vec<McpConnection>, ServiceError> { Ok(self.connections.list()?) }
    pub fn portfolio_scopes(&self) -> Result<Vec<McpPortfolioScope>, ServiceError> { self.tools.portfolio_scopes() }

    pub fn create_connection(&self, name: &str, preset: McpAccessPreset, portfolios: Vec<Uuid>) -> Result<McpConnectionSecret, ServiceError> {
        let name = name.trim();
        if name.is_empty() { return Err(ServiceError::Validation("Enter a connection name.".into())); }
        self.validate_portfolios(&portfolios)?;
        let id = Uuid::new_v4();
        let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let token = format!("akf_mcp_{}_{}", id.simple(), secret);
        let connection = McpConnection { id, name:name.into(), enabled:true, preset, portfolio_ids:portfolios, created_at:String::new(), updated_at:String::new(), last_used_at:None };
        self.connections.create(&ConnectionRecord { connection, token_hash: hash_secret(&secret)? })?;
        let connection = self.connections.find(id)?.ok_or_else(|| ServiceError::Storage("connection was not saved".into()))?.connection;
        Ok(McpConnectionSecret { connection, token })
    }

    fn validate_portfolios(&self, ids: &[Uuid]) -> Result<(), ServiceError> {
        if ids.is_empty() { return Err(ServiceError::Validation("Select at least one portfolio.".into())); }
        let known = self.tools.portfolio_scopes()?;
        if ids.iter().any(|id| !known.iter().any(|p| p.id == *id)) { return Err(ServiceError::Validation("One or more selected portfolios no longer exist.".into())); }
        Ok(())
    }
    pub fn update_connection(&self,id:Uuid,name:&str,preset:McpAccessPreset,enabled:bool,portfolios:Vec<Uuid>)->Result<(),ServiceError>{if self.connections.find(id)?.is_none(){return Err(ServiceError::NotFound("MCP connection".into()));}self.ensure_not_racing(id,"changed")?;self.validate_portfolios(&portfolios)?;if name.trim().is_empty(){return Err(ServiceError::Validation("Enter a connection name.".into()));}self.connections.update(id,name.trim(),preset,enabled,&portfolios)?;Ok(())}
    pub fn rotate_connection(&self,id:Uuid)->Result<McpConnectionSecret,ServiceError>{if self.connections.find(id)?.is_none(){return Err(ServiceError::NotFound("MCP connection".into()));}let secret=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());self.connections.rotate(id,&hash_secret(&secret)?)?;let connection=self.connections.find(id)?.unwrap().connection;Ok(McpConnectionSecret{connection,token:format!("akf_mcp_{}_{}",id.simple(),secret)})}
    pub fn delete_connection(&self,id:Uuid)->Result<(),ServiceError>{if self.connections.find(id)?.is_none(){return Err(ServiceError::NotFound("MCP connection".into()));}self.ensure_not_racing(id,"deleted")?;self.connections.delete(id)?;Ok(())}
    pub fn audit_events(&self,id:Uuid)->Result<Vec<McpAuditEvent>,ServiceError>{Ok(self.connections.events(id,20)?) }

    fn ensure_not_racing(&self, id: Uuid, action: &str) -> Result<(), ServiceError> {
        if self.connections.in_active_race(id)? {
            return Err(ServiceError::Validation(format!("MCP connections that drive a contestant in an active race cannot be {action}.")));
        }
        Ok(())
    }

    fn token_parts(token:&str)->Option<(Uuid,&str)>{let rest=token.trim().strip_prefix("akf_mcp_")?;let (raw,secret)=rest.split_once('_')?;Some((Uuid::parse_str(raw).ok()?,secret))}
    pub fn authorize(&self, presented: Option<&str>) -> Access {
        let Some((id,secret))=presented.and_then(Self::token_parts) else { return Access::Denied; };
        match self.connections.find(id) {
            Err(e)=>Access::Error(e.to_string()),
            Ok(None)=>Access::Denied,
            Ok(Some(record)) if !record.connection.enabled=>Access::Denied,
            Ok(Some(record))=>match PasswordHash::new(&record.token_hash).ok().and_then(|h|Argon2::default().verify_password(secret.as_bytes(),&h).ok()) {
                Some(())=>{let _=self.connections.touch(id);Access::Granted(AccessContext{connection_id:id,preset:record.connection.preset,portfolio_ids:record.connection.portfolio_ids})},
                None=>Access::Denied,
            }
        }
    }

    /// The reply to a body that isn't JSON.
    pub fn parse_error() -> Value {
        error(Value::Null, PARSE_ERROR, "Parse error")
    }

    /// Answers one JSON-RPC message, or a batch of them. `None` when
    /// nothing needs a reply (notifications, responses).
    /// Trusted internal dispatch used by configured model traders and tests.
    pub async fn handle(&self, message: Value) -> Option<Value> {
        let access=AccessContext { connection_id:Uuid::nil(), preset:McpAccessPreset::Trader, portfolio_ids:self.tools.portfolio_scopes().unwrap_or_default().into_iter().map(|p|p.id).collect() };
        self.handle_scoped(message,&access).await
    }

    pub async fn handle_scoped(&self, message: Value, access: &AccessContext) -> Option<Value> {
        match message {
            Value::Array(batch) if batch.is_empty() => Some(error(Value::Null, INVALID_REQUEST, "Empty batch")),
            Value::Array(batch) => {
                let mut replies = Vec::with_capacity(batch.len());
                for m in batch {
                    replies.extend(self.handle_one(m, access).await);
                }
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            message => self.handle_one(message, access).await,
        }
    }

    async fn handle_one(&self, message: Value, access: &AccessContext) -> Option<Value> {
        if !message.is_object() {
            return Some(error(Value::Null, INVALID_REQUEST, "Invalid request"));
        }
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            let is_response = message.get("result").is_some() || message.get("error").is_some();
            return (!is_response).then(|| error(id.unwrap_or(Value::Null), INVALID_REQUEST, "Invalid request"));
        };
        // Notifications (no id), e.g. notifications/initialized, need no reply.
        let id = id?;
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let tool=params.get("name").and_then(Value::as_str);
        let targets=self.tools.audit_targets(access,tool,&params);
        let dispatched=self.dispatch(method, &params, access).await;
        let success=dispatched.as_ref().is_ok_and(|value| value.get("isError") != Some(&Value::Bool(true)));
        let category=if !success { Some(match dispatched.as_ref().err() { Some((code,_)) if *code==INVALID_PARAMS=>"invalid_request",Some(_)=>"protocol_error",None=>"tool_error" }) } else { None };
        let _=self.connections.audit(access.connection_id,method,tool,&targets,success,category);
        Some(match dispatched { Ok(result)=>reply(id,result), Err((code,message))=>error(id,code,&message) })
    }

    async fn dispatch(&self, method: &str, params: &Value, access: &AccessContext) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str);
                let version = asked
                    .filter(|v| PROTOCOL_VERSIONS.contains(v))
                    .unwrap_or(PROTOCOL_VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "akhsakov-finance",
                        "title": "Akhsakov Finance",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools::definitions_for(access.preset) })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or((INVALID_PARAMS, "Missing the tool name".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                // A tool that fails says why as its result, for the model
                // to read and correct; only unknown tools are protocol errors.
                let (text, is_error) = match self.tools.call_scoped(name, &args, access).await {
                    None => return Err((INVALID_PARAMS, format!("Unknown tool: {name}"))),
                    Some(Ok(value)) => (serde_json::to_string_pretty(&value).unwrap_or_default(), false),
                    Some(Err(message)) => (message, true),
                };
                Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
            }
            _ => Err((METHOD_NOT_FOUND, format!("Method not found: {method}"))),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        database::Database,
        mcp::{
            infrastructures::{SqliteAiPortfolioRepository, SqliteConnectionRepository},
            repositories::{LivePrices, LiveQuote},
        },
        planning::planning_services_setup,
        portfolio::portfolio_services_setup,
        settings::settings_services_setup,
        thesis::thesis_services_setup,
        watchlist::watchlist_services_setup,
    };
    use rust_decimal_macros::dec;

    struct NoFx;

    #[async_trait::async_trait]
    impl crate::shared::FxRates for NoFx {
        async fn usd_per_unit(&self, _: &str, _: &str) -> Result<rust_decimal::Decimal, String> {
            Ok(rust_decimal::Decimal::ONE)
        }
        async fn usd_per_unit_now(&self, _: &str) -> Result<rust_decimal::Decimal, String> {
            Ok(rust_decimal::Decimal::ONE)
        }
        async fn currency_of(&self, _: &types::ticker_symbol::TickerSymbol) -> Result<String, String> {
            Ok("USD".into())
        }
    }

    /// NVDA at $200, PTT.BK at ฿35 (3¢ a baht), and OLD with only a saved
    /// price.
    struct FakePrices;

    #[async_trait::async_trait]
    impl LivePrices for FakePrices {
        async fn quote(&self, ticker: &types::ticker_symbol::TickerSymbol) -> Result<LiveQuote, String> {
            let (price, currency, usd_per_unit, stale) = match ticker.as_str() {
                "NVDA" => (dec!(200), "USD", dec!(1), false),
                "PTT.BK" => (dec!(35), "THB", dec!(0.03), false),
                "OLD" => (dec!(10), "USD", dec!(1), true),
                other => return Err(format!("no such ticker {other}")),
            };
            Ok(LiveQuote {
                price,
                previous_close: price * dec!(0.98),
                currency: currency.into(),
                usd_per_unit,
                timestamp: 1_767_600_000,
                stale,
            })
        }
    }

    /// A service over a database with one portfolio, "Main", holding NVDA.
    pub(crate) fn service() -> McpService {
        let db = Database::in_memory().unwrap();
        service_with_database(db)
    }

    pub(crate) fn service_with_database(db: Database) -> McpService {
        db.with(|c| {
            c.execute("INSERT INTO portfolios (id, name) VALUES ('11111111-1111-1111-1111-111111111111', 'Main')", [])?;
            c.execute(
                "INSERT INTO transactions (id, portfolio_id, ticker, kind, shares, price, fee, date)
                 VALUES ('22222222-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111',
                         'NVDA', 'Buy', '2', '100', '0', '2026-01-05')",
                [],
            )
        })
        .unwrap();
        let portfolios = portfolio_services_setup(db.clone(), Arc::new(NoFx));
        let trading = Trading::new(
            Arc::new(SqliteAiPortfolioRepository::new(db.clone())),
            portfolios.clone(),
            Arc::new(FakePrices),
        );
        let tools = Tools::new(
            thesis_services_setup(db.clone()),
            portfolios,
            watchlist_services_setup(db.clone()),
            trading.clone(),
            planning_services_setup(db.clone()),
            settings_services_setup(db.clone()),
        );
        McpService::new(Arc::new(SqliteConnectionRepository::new(db)), tools, trading)
    }

    #[test]
    fn named_connections_are_independent() {
        let s = service();
        let portfolio=Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        assert_eq!(s.authorize(Some("anything")), Access::Denied);
        let first=s.create_connection("Reader",McpAccessPreset::ReadOnly,vec![portfolio]).unwrap();
        let second=s.create_connection("Editor",McpAccessPreset::ThesisEditor,vec![portfolio]).unwrap();
        assert!(matches!(s.authorize(Some(&first.token)),Access::Granted(_)));
        assert!(matches!(s.authorize(Some(&second.token)),Access::Granted(_)));
        assert_eq!(s.authorize(Some(&first.token[..first.token.len()-1])), Access::Denied);
        assert_eq!(s.authorize(None), Access::Denied);
        let rotated=s.rotate_connection(first.connection.id).unwrap();
        assert_eq!(s.authorize(Some(&first.token)),Access::Denied);
        assert!(matches!(s.authorize(Some(&rotated.token)),Access::Granted(_)));
        assert!(matches!(s.authorize(Some(&second.token)),Access::Granted(_)),"rotation is per connection");
    }

    #[test]
    fn raw_connection_secret_is_never_stored_or_returned_as_metadata() {
        let db=Database::in_memory().unwrap();
        let s=service_with_database(db.clone());
        let portfolio=Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let made=s.create_connection("Reader",McpAccessPreset::ReadOnly,vec![portfolio]).unwrap();
        let stored:String=db.with(|c|c.query_row("SELECT token_hash FROM mcp_connections WHERE id=?1",[made.connection.id.to_string()],|r|r.get(0))).unwrap();
        assert!(stored.starts_with("$argon2")&&!stored.contains(&made.token));
        assert_eq!(s.connections().unwrap(),vec![made.connection]);
    }

    #[tokio::test]
    async fn presets_and_portfolio_scopes_apply_to_listing_and_direct_calls() {
        let s=service();
        let main=Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let ai=s.trading().start("Hidden AI",rust_decimal_macros::dec!(1000)).await.unwrap();
        let made=s.create_connection("Main reader",McpAccessPreset::ReadOnly,vec![main]).unwrap();
        let Access::Granted(access)=s.authorize(Some(&made.token)) else { panic!("connection should authenticate") };

        let listed=s.handle_scoped(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),&access).await.unwrap();
        let names:Vec<&str>=listed["result"]["tools"].as_array().unwrap().iter().filter_map(|v|v["name"].as_str()).collect();
        assert!(!names.contains(&"save_thesis")&&!names.contains(&"place_order"));
        let hidden=s.handle_scoped(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"save_thesis","arguments":{"portfolio":main,"ticker":"NVDA"}}}),&access).await.unwrap();
        assert_eq!(hidden["error"]["code"],INVALID_PARAMS,"hidden tools cannot be called directly");

        let portfolios=s.handle_scoped(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_portfolios","arguments":{}}}),&access).await.unwrap();
        let text=portfolios["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Main")&&!text.contains(&ai.name));
        let events=s.audit_events(made.connection.id).unwrap();
        assert!(events.iter().any(|e|e.tool.as_deref()==Some("list_portfolios")));
    }

    #[tokio::test]
    async fn protocol() {
        let s = service();
        let init = s
            .handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).await
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(init["result"]["capabilities"]["tools"].is_object());
        let init = s.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}})).await.unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);

        assert_eq!(s.handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await, None);
        assert_eq!(s.handle(json!({"jsonrpc":"2.0","id":9,"result":{}})).await, None, "responses need no reply");
        assert_eq!(s.handle(json!({"jsonrpc":"2.0","id":"p","method":"ping"})).await.unwrap()["result"], json!({}));
        assert_eq!(s.handle(json!({"jsonrpc":"2.0","id":2,"method":"nope"})).await.unwrap()["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(s.handle(json!(42)).await.unwrap()["error"]["code"], INVALID_REQUEST);

        let list = s.handle(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})).await.unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["list_portfolios", "list_theses", "get_thesis", "save_thesis", "add_thesis_note", "get_my_portfolio", "get_my_goals", "get_quote", "place_order"]
        );

        let unknown = s.handle(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"rm_rf"}})).await.unwrap();
        assert_eq!(unknown["error"]["code"], INVALID_PARAMS);

        let batch = s
            .handle(json!([
                {"jsonrpc":"2.0","id":5,"method":"ping"},
                {"jsonrpc":"2.0","method":"notifications/initialized"}
            ])).await
            .unwrap();
        assert_eq!(batch.as_array().unwrap().len(), 1);
    }
}
