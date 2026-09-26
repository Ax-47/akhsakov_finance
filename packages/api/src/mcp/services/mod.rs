//! The connector's key, and the MCP protocol itself: JSON-RPC 2.0 messages
//! in, replies out. The tools live in [`tools`]; Claude's own portfolio in
//! [`trading`].

pub mod tools;
pub mod trading;

use crate::{mcp::repositories::KeyRepository, shared::ServiceError};
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
- The user may have given you a portfolio of your own, with paper money, to manage yourself \
(get_my_portfolio). Decide what to buy and sell there, and trade with place_order, giving a short reason \
each time; the reason is kept in that holding's journal. Invest for the long run, spread the risk, and \
don't trade just to be busy. place_order can't touch the user's other portfolios; never present your \
own portfolio's trades as advice to copy.";

/// What a request's key allows.
#[derive(Debug, PartialEq)]
pub enum Access {
    Granted,
    Denied,
    /// No key has been made: the connector is off.
    Off,
    Error(String),
}

#[derive(Clone)]
pub struct McpService {
    keys: Arc<dyn KeyRepository>,
    tools: Tools,
    trading: Trading,
}

/// Compares without stopping at the first difference, so response timing
/// doesn't reveal how much of a guess was right.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn reply(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

impl McpService {
    pub fn new(keys: Arc<dyn KeyRepository>, tools: Tools, trading: Trading) -> Self {
        Self { keys, tools, trading }
    }

    /// Claude's own portfolio, which Settings sets up and funds.
    pub fn trading(&self) -> &Trading {
        &self.trading
    }

    /// `None` while the connector is off.
    pub fn key(&self) -> Result<Option<String>, ServiceError> {
        Ok(self.keys.key()?)
    }

    /// Turns the connector on with a fresh key; any old key stops working.
    pub fn new_key(&self) -> Result<String, ServiceError> {
        // 244 random bits, hex: URL-safe, so it can go in a path.
        let key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        self.keys.set_key(Some(&key))?;
        Ok(key)
    }

    pub fn turn_off(&self) -> Result<(), ServiceError> {
        Ok(self.keys.set_key(None)?)
    }

    pub fn authorize(&self, presented: Option<&str>) -> Access {
        match self.keys.key() {
            Err(e) => Access::Error(e.to_string()),
            Ok(None) => Access::Off,
            Ok(Some(key)) if presented.is_some_and(|p| same(p.trim(), &key)) => Access::Granted,
            Ok(Some(_)) => Access::Denied,
        }
    }

    /// The reply to a body that isn't JSON.
    pub fn parse_error() -> Value {
        error(Value::Null, PARSE_ERROR, "Parse error")
    }

    /// Answers one JSON-RPC message, or a batch of them. `None` when
    /// nothing needs a reply (notifications, responses).
    pub async fn handle(&self, message: Value) -> Option<Value> {
        match message {
            Value::Array(batch) if batch.is_empty() => Some(error(Value::Null, INVALID_REQUEST, "Empty batch")),
            Value::Array(batch) => {
                let mut replies = Vec::with_capacity(batch.len());
                for m in batch {
                    replies.extend(self.handle_one(m).await);
                }
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            message => self.handle_one(message).await,
        }
    }

    async fn handle_one(&self, message: Value) -> Option<Value> {
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
        Some(match self.dispatch(method, &params).await {
            Ok(result) => reply(id, result),
            Err((code, message)) => error(id, code, &message),
        })
    }

    async fn dispatch(&self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
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
            "tools/list" => Ok(json!({ "tools": tools::definitions() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or((INVALID_PARAMS, "Missing the tool name".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                // A tool that fails says why as its result, for the model
                // to read and correct; only unknown tools are protocol errors.
                let (text, is_error) = match self.tools.call(name, &args).await {
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
            infrastructures::{SqliteAiPortfolioRepository, SqliteKeyRepository},
            repositories::{LivePrices, LiveQuote},
        },
        portfolio::portfolio_services_setup,
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
        );
        McpService::new(Arc::new(SqliteKeyRepository::new(db)), tools, trading)
    }

    #[test]
    fn keys() {
        let s = service();
        assert_eq!(s.authorize(Some("anything")), Access::Off);
        let key = s.new_key().unwrap();
        assert_eq!(key.len(), 64);
        assert_eq!(s.authorize(Some(&key)), Access::Granted);
        assert_eq!(s.authorize(Some(&key[..63])), Access::Denied);
        assert_eq!(s.authorize(None), Access::Denied);
        let newer = s.new_key().unwrap();
        assert_eq!(s.authorize(Some(&key)), Access::Denied, "a new key replaces the old");
        assert_eq!(s.key().unwrap(), Some(newer));
        s.turn_off().unwrap();
        assert_eq!(s.authorize(Some(&key)), Access::Off);
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
            ["list_portfolios", "list_theses", "get_thesis", "save_thesis", "add_thesis_note", "get_my_portfolio", "get_quote", "place_order"]
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
