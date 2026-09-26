//! The tools the connector offers: read your portfolios and theses, save a
//! thesis, and add to its journal. Everything the AI writes is marked as
//! written by AI.

use crate::{portfolio::PortfolioService, thesis::ThesisService, watchlist::WatchlistService};
use dtos::{
    compute_positions,
    portfolio::{GetDashBoardResponse, GetPortfolioResponse},
    thesis::{Author, Thesis, ThesisEntry, ThesisStatus},
    Position,
};
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};
use serde_json::{json, Value};
use std::{collections::HashMap, str::FromStr};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

type ToolResult = Result<Value, String>;

/// Journal entries shown per thesis when listing many.
const LIST_JOURNAL: usize = 3;

pub fn definitions() -> Value {
    let portfolio = json!({
        "type": "string",
        "description": "Portfolio name or id. May be left out when there is only one portfolio, or only one holds the ticker."
    });
    let ticker = json!({ "type": "string", "description": "Ticker symbol as the app lists it, e.g. NVDA, PTT.BK." });
    let read_only = json!({ "readOnlyHint": true, "openWorldHint": false });
    let writes = json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false });
    json!([
        {
            "name": "list_portfolios",
            "title": "List portfolios and holdings",
            "description": "Every portfolio with its current holdings (shares, average cost, cost basis, weight by cost, first buy date) and the status of each holding's thesis. Amounts are USD.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": read_only,
        },
        {
            "name": "list_theses",
            "title": "List theses",
            "description": "The theses of one portfolio (or all), with the latest journal entries, plus the holdings that have no thesis yet. Use this to review a whole portfolio or find reviews that are due.",
            "inputSchema": { "type": "object", "properties": { "portfolio": portfolio } },
            "annotations": read_only,
        },
        {
            "name": "get_thesis",
            "title": "Get a thesis",
            "description": "The user's full thesis on one holding: why they own it, what would make them sell, target, conviction, status, review date, the whole journal, the position, and their general note on the stock. Call this before discussing the holding.",
            "inputSchema": {
                "type": "object",
                "properties": { "portfolio": portfolio, "ticker": ticker },
                "required": ["ticker"],
            },
            "annotations": read_only,
        },
        {
            "name": "save_thesis",
            "title": "Save a thesis",
            "description": "Creates or updates the thesis on a holding. Only the fields you pass change; pass null to clear one. Only do this when the user asks or agrees. The change is logged in the journal as written by AI, with `note` if given.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "portfolio": portfolio,
                    "ticker": ticker,
                    "thesis": { "type": ["string", "null"], "description": "Why the user owns it." },
                    "sell_if": { "type": ["string", "null"], "description": "What would prove the thesis wrong or make them sell." },
                    "target_price": { "type": ["number", "null"], "description": "What they think it's worth per share, in USD." },
                    "conviction": { "type": ["integer", "null"], "minimum": 1, "maximum": 5, "description": "1 (low) to 5 (high)." },
                    "review_on": { "type": ["string", "null"], "description": "Date to review it again, YYYY-MM-DD." },
                    "status": { "type": "string", "enum": ["on_track", "at_risk", "broken"] },
                    "note": { "type": "string", "description": "Why it changed, one or two sentences for the journal." },
                },
                "required": ["ticker"],
            },
            "annotations": writes,
        },
        {
            "name": "add_thesis_note",
            "title": "Add to a thesis journal",
            "description": "Adds a dated entry to the journal of a holding's thesis: what you learned (earnings, news, a review with the user) and what it means for the thesis. Starts an empty thesis if there's none.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "portfolio": portfolio,
                    "ticker": ticker,
                    "text": { "type": "string", "description": "One or two sentences, in the user's language." },
                },
                "required": ["ticker", "text"],
            },
            "annotations": writes,
        },
    ])
}

#[derive(Clone)]
pub struct Tools {
    theses: ThesisService,
    portfolios: PortfolioService,
    watchlist: WatchlistService,
}

/// Everything the tools look at, read once per call.
struct Book {
    dash: GetDashBoardResponse,
    theses: Vec<Thesis>,
    today: String,
}

impl Book {
    fn positions(&self, portfolio: Uuid) -> Vec<Position> {
        let scoped = GetDashBoardResponse {
            portfolios: vec![],
            transactions: self
                .dash
                .transactions
                .iter()
                .filter(|t| t.portfolio_id == portfolio)
                .cloned()
                .collect(),
        };
        let mut positions = compute_positions(&scoped, &HashMap::new());
        positions.sort_by_key(|p| std::cmp::Reverse(p.cost_basis()));
        positions
    }

    fn first_bought(&self, portfolio: Uuid, ticker: &TickerSymbol) -> Option<String> {
        self.dash
            .transactions
            .iter()
            .filter(|t| t.portfolio_id == portfolio && t.ticker == *ticker)
            .filter(|t| t.transaction_type == TransactionType::Buy)
            .map(|t| t.date.clone())
            .min()
    }

    fn thesis(&self, portfolio: Uuid, ticker: &TickerSymbol) -> Option<&Thesis> {
        self.theses
            .iter()
            .find(|t| t.portfolio_id == portfolio && t.ticker == *ticker)
    }

    fn names(&self) -> String {
        let names: Vec<&str> = self.dash.portfolios.iter().map(|p| p.name.as_str()).collect();
        names.join(", ")
    }

    /// The portfolio `args` names, or the only one it can be.
    fn portfolio(&self, args: &Value, ticker: Option<&TickerSymbol>) -> Result<&GetPortfolioResponse, String> {
        let all = &self.dash.portfolios;
        if all.is_empty() {
            return Err("There are no portfolios yet.".into());
        }
        if let Some(wanted) = args
            .get("portfolio")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return all
                .iter()
                .find(|p| p.id.to_string() == wanted || p.name.eq_ignore_ascii_case(wanted))
                .ok_or_else(|| format!("No portfolio called “{wanted}”. The portfolios are: {}.", self.names()));
        }
        let candidates: Vec<&GetPortfolioResponse> = match ticker {
            _ if all.len() == 1 => all.iter().collect(),
            Some(t) => all
                .iter()
                .filter(|p| p.assets.iter().any(|a| a.ticker_symbol == *t) || self.thesis(p.id, t).is_some())
                .collect(),
            None => vec![],
        };
        match candidates.as_slice() {
            [only] => Ok(only),
            _ => Err(format!("Say which portfolio: {}.", self.names())),
        }
    }
}

fn num(d: Decimal, dp: u32) -> Value {
    json!(d.round_dp(dp).to_f64())
}

fn ticker(args: &Value) -> Result<TickerSymbol, String> {
    let raw = args.get("ticker").and_then(Value::as_str).unwrap_or_default();
    TickerSymbol::new(raw).map_err(|_| format!("“{raw}” isn't a ticker symbol."))
}

fn entry_json(e: &ThesisEntry) -> Value {
    json!({
        "at": e.created_at,
        "by": if e.author == Author::Ai { "ai" } else { "user" },
        "text": e.text,
    })
}

fn thesis_json(t: &Thesis, today: &str, journal: usize) -> Value {
    let d = &t.draft;
    json!({
        "thesis": d.thesis,
        "sell_if": d.exit_if,
        "target_price": d.target_price.map(|p| num(p, 4)),
        "conviction": d.conviction,
        "review_on": d.review_on,
        "review_due": t.review_due(today),
        "status": d.status.key(),
        "last_edited_by": if t.updated_by == Author::Ai { "ai" } else { "user" },
        "last_edited_at": t.updated_at,
        "journal_entries": t.log.len(),
        "journal": t.log.iter().take(journal).map(entry_json).collect::<Vec<_>>(),
    })
}

fn position_json(p: &Position, first_bought: Option<String>) -> Value {
    json!({
        "shares": num(p.shares, 6),
        "avg_cost": num(p.avg_cost, 4),
        "cost_basis": num(p.cost_basis(), 2),
        "first_bought": first_bought,
    })
}

/// A text field: `null` clears it.
fn text(v: &Value, field: &str) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        _ => Err(format!("`{field}` must be text or null.")),
    }
}

impl Tools {
    pub fn new(theses: ThesisService, portfolios: PortfolioService, watchlist: WatchlistService) -> Self {
        Self {
            theses,
            portfolios,
            watchlist,
        }
    }

    /// `None` for an unknown tool. `Err` is shown to the model as a failed
    /// call, so it says what to fix.
    pub fn call(&self, name: &str, args: &Value) -> Option<ToolResult> {
        Some(match name {
            "list_portfolios" => self.list_portfolios(),
            "list_theses" => self.list_theses(args),
            "get_thesis" => self.get_thesis(args),
            "save_thesis" => self.save_thesis(args),
            "add_thesis_note" => self.add_note(args),
            _ => return None,
        })
    }

    fn book(&self) -> Result<Book, String> {
        Ok(Book {
            dash: self.portfolios.dashboard().map_err(|e| e.to_string())?,
            theses: self.theses.theses(None).map_err(|e| e.to_string())?,
            today: chrono::Local::now().format("%Y-%m-%d").to_string(),
        })
    }

    fn list_portfolios(&self) -> ToolResult {
        let book = self.book()?;
        let portfolios: Vec<Value> = book
            .dash
            .portfolios
            .iter()
            .map(|p| {
                let positions = book.positions(p.id);
                let total: Decimal = positions.iter().map(Position::cost_basis).sum();
                let holdings: Vec<Value> = positions
                    .iter()
                    .map(|pos| {
                        let mut h = position_json(pos, book.first_bought(p.id, &pos.ticker));
                        h["ticker"] = json!(pos.ticker.as_str());
                        if total > Decimal::ZERO {
                            h["weight_by_cost_pct"] = num(pos.cost_basis() / total * Decimal::ONE_HUNDRED, 1);
                        }
                        h["thesis_status"] = json!(book.thesis(p.id, &pos.ticker).map(|t| t.draft.status.key()));
                        h
                    })
                    .collect();
                json!({ "id": p.id, "name": p.name, "holdings": holdings })
            })
            .collect();
        Ok(json!({
            "portfolios": portfolios,
            "note": "Amounts are USD, converted at each trade's exchange rate. Live prices aren't included.",
        }))
    }

    fn list_theses(&self, args: &Value) -> ToolResult {
        let book = self.book()?;
        let scope: Vec<&GetPortfolioResponse> = match args.get("portfolio").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => vec![book.portfolio(args, None)?],
            _ => book.dash.portfolios.iter().collect(),
        };
        let portfolios: Vec<Value> = scope
            .into_iter()
            .map(|p| {
                let positions = book.positions(p.id);
                let theses: Vec<Value> = book
                    .theses
                    .iter()
                    .filter(|t| t.portfolio_id == p.id)
                    .map(|t| {
                        let mut v = thesis_json(t, &book.today, LIST_JOURNAL);
                        v["ticker"] = json!(t.ticker.as_str());
                        v["held"] = json!(positions.iter().any(|pos| pos.ticker == t.ticker));
                        v
                    })
                    .collect();
                let missing: Vec<&str> = positions
                    .iter()
                    .filter(|pos| book.thesis(p.id, &pos.ticker).is_none())
                    .map(|pos| pos.ticker.as_str())
                    .collect();
                json!({ "portfolio": p.name, "id": p.id, "theses": theses, "held_without_thesis": missing })
            })
            .collect();
        Ok(json!({ "today": book.today, "portfolios": portfolios }))
    }

    fn get_thesis(&self, args: &Value) -> ToolResult {
        let book = self.book()?;
        let ticker = ticker(args)?;
        let p = book.portfolio(args, Some(&ticker))?;
        Ok(self.holding_json(&book, p, &ticker))
    }

    fn holding_json(&self, book: &Book, p: &GetPortfolioResponse, ticker: &TickerSymbol) -> Value {
        let position = book
            .positions(p.id)
            .into_iter()
            .find(|pos| pos.ticker == *ticker)
            .map(|pos| position_json(&pos, book.first_bought(p.id, ticker)));
        let note = self
            .watchlist
            .notes()
            .ok()
            .and_then(|notes| notes.into_iter().find(|n| n.ticker == ticker.as_str()))
            .map(|n| json!({ "text": n.text, "tags": n.tags }));
        let thesis = book.thesis(p.id, ticker);
        let mut out = json!({
            "portfolio": { "id": p.id, "name": p.name },
            "ticker": ticker.as_str(),
            "position": position,
            "thesis": thesis.map(|t| thesis_json(t, &book.today, usize::MAX)),
            "stock_note": note,
        });
        if thesis.is_none() {
            out["hint"] = json!("No thesis yet. Ask the user why they own it and what would make them sell, then save it with save_thesis.");
        }
        out
    }

    fn save_thesis(&self, args: &Value) -> ToolResult {
        let book = self.book()?;
        let ticker = ticker(args)?;
        let p = book.portfolio(args, Some(&ticker))?;
        let mut draft = book.thesis(p.id, &ticker).map(|t| t.draft.clone()).unwrap_or_default();
        if let Some(v) = args.get("thesis") {
            draft.thesis = text(v, "thesis")?;
        }
        if let Some(v) = args.get("sell_if") {
            draft.exit_if = text(v, "sell_if")?;
        }
        if let Some(v) = args.get("target_price") {
            draft.target_price = match v {
                Value::Null => None,
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => Some(Decimal::from_str(s.trim()).map_err(|_| "`target_price` must be a number.")?),
                v => Some(
                    v.as_f64()
                        .and_then(Decimal::from_f64)
                        .ok_or("`target_price` must be a number.")?
                        .round_dp(4),
                ),
            };
        }
        if let Some(v) = args.get("conviction") {
            draft.conviction = match v {
                Value::Null => None,
                v => Some(
                    v.as_u64()
                        .and_then(|c| u8::try_from(c).ok())
                        .ok_or("`conviction` must be a whole number from 1 to 5.")?,
                ),
            };
        }
        if let Some(v) = args.get("review_on") {
            draft.review_on = Some(text(v, "review_on")?);
        }
        if let Some(v) = args.get("status") {
            draft.status = v
                .as_str()
                .and_then(ThesisStatus::from_key)
                .ok_or("`status` must be on_track, at_risk or broken.")?;
        }
        let note = args.get("note").and_then(Value::as_str);
        self.theses
            .save(p.id, &ticker, draft, Author::Ai, note)
            .map_err(|e| e.to_string())?;
        Ok(self.holding_json(&self.book()?, p, &ticker))
    }

    fn add_note(&self, args: &Value) -> ToolResult {
        let book = self.book()?;
        let ticker = ticker(args)?;
        let p = book.portfolio(args, Some(&ticker))?;
        let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
        let entry = self
            .theses
            .add_entry(p.id, &ticker, text, Author::Ai)
            .map_err(|e| e.to_string())?;
        Ok(json!({ "added": true, "portfolio": p.name, "ticker": ticker.as_str(), "text": entry.text }))
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::services::tests::service;
    use serde_json::{json, Value};

    /// Calls a tool; `Err` holds a failed call's message.
    fn call(s: &crate::mcp::McpService, name: &str, args: Value) -> Result<Value, String> {
        let reply = s
            .handle(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}))
            .unwrap();
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        if result["isError"] == json!(true) {
            Err(text)
        } else {
            Ok(serde_json::from_str(&text).unwrap())
        }
    }

    #[test]
    fn remembers_theses_through_the_tools() {
        let s = service();
        let list = call(&s, "list_portfolios", json!({})).unwrap();
        let nvda = &list["portfolios"][0]["holdings"][0];
        assert_eq!((nvda["ticker"].as_str(), nvda["shares"].as_f64()), (Some("NVDA"), Some(2.0)));
        assert_eq!(nvda["first_bought"], "2026-01-05");
        assert_eq!(nvda["thesis_status"], Value::Null);

        let missing = call(&s, "list_theses", json!({})).unwrap();
        assert_eq!(missing["portfolios"][0]["held_without_thesis"], json!(["NVDA"]));
        let empty = call(&s, "get_thesis", json!({"ticker":"nvda"})).unwrap();
        assert!(empty["thesis"].is_null() && empty["hint"].is_string());

        // Only one portfolio: it needn't be named.
        let saved = call(
            &s,
            "save_thesis",
            json!({"ticker":"NVDA","thesis":"Owns AI training","sell_if":"Hyperscaler capex falls","target_price":250.5,"conviction":4,"note":"Agreed with the user."}),
        )
        .unwrap();
        assert_eq!(saved["thesis"]["target_price"], json!(250.5));
        assert_eq!(saved["thesis"]["last_edited_by"], "ai");
        assert_eq!(saved["position"]["avg_cost"], json!(100.0));
        let journal = saved["thesis"]["journal"][0]["text"].as_str().unwrap();
        assert!(journal.starts_with("Agreed with the user.\nUpdated: thesis, sell if, target none → 250.5"), "{journal}");

        // Partial updates keep the other fields; null clears.
        let updated = call(&s, "save_thesis", json!({"portfolio":"main","ticker":"NVDA","status":"at_risk","target_price":null})).unwrap();
        assert_eq!(updated["thesis"]["thesis"], "Owns AI training");
        assert_eq!((updated["thesis"]["status"].as_str(), &updated["thesis"]["target_price"]), (Some("at_risk"), &Value::Null));

        call(&s, "add_thesis_note", json!({"ticker":"NVDA","text":"Q3: data-centre revenue +60%."})).unwrap();
        let got = call(&s, "get_thesis", json!({"ticker":"NVDA"})).unwrap();
        assert_eq!(got["thesis"]["journal"][0]["text"], "Q3: data-centre revenue +60%.");
        assert_eq!(got["thesis"]["journal_entries"], 3);

        assert!(call(&s, "get_thesis", json!({"ticker":"NVDA","portfolio":"Other"})).unwrap_err().contains("Main"));
        assert!(call(&s, "save_thesis", json!({"ticker":"NVDA","conviction":9})).is_err());
        assert!(call(&s, "save_thesis", json!({"ticker":"NVDA","status":"great"})).is_err());
        assert!(call(&s, "add_thesis_note", json!({"ticker":"NVDA","text":" "})).is_err());
    }
}
