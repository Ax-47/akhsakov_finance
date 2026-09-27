//! The tools the connector offers: read your portfolios and theses, save a
//! thesis, and add to its journal; and, when you've given Claude a
//! portfolio of its own, look up prices and trade in it. Everything the AI
//! writes is marked as written by AI.

use super::trading::{Book as MyBook, Side, Size, Trading};
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

/// Trades listed by get_my_portfolio.
const RECENT_TRADES: usize = 10;

const NO_PORTFOLIO: &str = "You don't have a portfolio of your own yet. The user can give you one, with \
starting cash, in the app under Settings → Connect Claude.";

pub fn definitions() -> Value {
    let portfolio = json!({
        "type": "string",
        "description": "Portfolio name or id. May be left out when there is only one portfolio, or only one holds the ticker."
    });
    let ticker = json!({ "type": "string", "description": "Ticker symbol as the app lists it, e.g. NVDA, PTT.BK." });
    let mine = json!({
        "type": "string",
        "description": "Which of your own portfolios, by name or id. May be left out when you have only one."
    });
    let read_only = json!({ "readOnlyHint": true, "openWorldHint": false });
    let writes = json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false });
    json!([
        {
            "name": "list_portfolios",
            "title": "List portfolios and holdings",
            "description": "Every portfolio with its current holdings (shares, average cost, cost basis, weight by cost, first buy date) and the status of each holding's thesis. `yours` marks the one you manage yourself. Amounts are USD.",
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
        {
            "name": "get_my_portfolio",
            "title": "Your own portfolios",
            "description": "The portfolios the user gave you to manage yourself, each with its own paper money: cash, holdings at live prices, total value, profit against the money you were given, and your recent trades. All of them, or the one named. Amounts are USD.",
            "inputSchema": { "type": "object", "properties": { "portfolio": mine } },
            "annotations": { "readOnlyHint": true, "openWorldHint": true },
        },
        {
            "name": "get_quote",
            "title": "Get a live price",
            "description": "The latest price of a stock, ETF or fund, in its own currency and in USD, with the change since the previous close.",
            "inputSchema": { "type": "object", "properties": { "ticker": ticker }, "required": ["ticker"] },
            "annotations": { "readOnlyHint": true, "openWorldHint": true },
        },
        {
            "name": "place_order",
            "title": "Buy or sell in your portfolio",
            "description": "Buys or sells in one of your own portfolios (see get_my_portfolio) at the live price, with no fee, using that portfolio's cash. It can't trade in the user's portfolios. Give either `shares` (fractions allowed) or `amount_usd`; to sell everything, pass shares \"all\". The reason goes in the holding's thesis journal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "portfolio": mine,
                    "ticker": ticker,
                    "side": { "type": "string", "enum": ["buy", "sell"] },
                    "shares": { "type": ["number", "string"], "description": "How many shares, or \"all\" to sell the whole holding." },
                    "amount_usd": { "type": "number", "description": "How many dollars' worth, instead of shares." },
                    "reason": { "type": "string", "description": "Why, in one or two sentences, in the user's language." },
                },
                "required": ["ticker", "side", "reason"],
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": true },
        },
    ])
}

#[derive(Clone)]
pub struct Tools {
    theses: ThesisService,
    portfolios: PortfolioService,
    watchlist: WatchlistService,
    trading: Trading,
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
    pub fn new(
        theses: ThesisService,
        portfolios: PortfolioService,
        watchlist: WatchlistService,
        trading: Trading,
    ) -> Self {
        Self {
            theses,
            portfolios,
            watchlist,
            trading,
        }
    }

    /// `None` for an unknown tool. `Err` is shown to the model as a failed
    /// call, so it says what to fix.
    pub async fn call(&self, name: &str, args: &Value) -> Option<ToolResult> {
        Some(match name {
            "list_portfolios" => self.list_portfolios(),
            "list_theses" => self.list_theses(args),
            "get_thesis" => self.get_thesis(args),
            "save_thesis" => self.save_thesis(args),
            "add_thesis_note" => self.add_note(args),
            "get_my_portfolio" => self.my_portfolio(args).await,
            "get_quote" => self.quote(args).await,
            "place_order" => self.place_order(args).await,
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
                json!({ "id": p.id, "name": p.name, "yours": p.ai, "holdings": holdings })
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

    async fn my_portfolio(&self, args: &Value) -> ToolResult {
        let wanted = args.get("portfolio").and_then(Value::as_str).filter(|s| !s.trim().is_empty());
        let books = match wanted {
            Some(_) => vec![self.trading.book(wanted).map_err(|e| e.to_string())?],
            None => self.trading.books().map_err(|e| e.to_string())?,
        };
        if books.is_empty() {
            return Err(NO_PORTFOLIO.into());
        }
        let mut portfolios = Vec::with_capacity(books.len());
        for book in &books {
            portfolios.push(self.book_json(book).await);
        }
        Ok(json!({
            "portfolios": portfolios,
            "note": "Paper money the user gave you to manage; each portfolio has its own cash. Orders fill at the latest price with no fee, so results are a little better than a real broker's.",
        }))
    }

    async fn book_json(&self, book: &MyBook) -> Value {
        let positions = book.positions();
        let mut holdings = Vec::with_capacity(positions.len());
        let mut invested = Decimal::ZERO;
        let mut unpriced = vec![];
        for pos in &positions {
            let price = match self.trading.quote(&pos.ticker).await {
                Ok(q) => Some(q.price * q.usd_per_unit),
                Err(_) => {
                    unpriced.push(pos.ticker.as_str());
                    None
                }
            };
            // Without a price it's counted at cost.
            let value = price.map_or(pos.cost_basis(), |p| p * pos.shares);
            invested += value;
            let mut h = position_json(pos, None);
            h["ticker"] = json!(pos.ticker.as_str());
            h["price"] = json!(price.map(|p| num(p, 4)));
            h["market_value"] = num(value, 2);
            h["unrealized_pnl"] = num(value - pos.cost_basis(), 2);
            holdings.push(h);
        }
        let (cash, funded) = (book.cash(), book.funded());
        let total = cash + invested;
        let profit = total - funded;
        let recent: Vec<Value> = book
            .trades()
            .rev()
            .take(RECENT_TRADES)
            .map(|t| {
                json!({
                    "date": t.date,
                    "side": if t.transaction_type == TransactionType::Buy { "buy" } else { "sell" },
                    "ticker": t.ticker.as_str(),
                    "shares": num(t.shares, 6),
                    "price": num(t.price, 4),
                    "currency": t.currency,
                    "total_usd": num(t.shares * t.usd_price(), 2),
                })
            })
            .collect();
        let mut out = json!({
            "portfolio": { "id": book.id, "name": book.name },
            "funded": num(funded, 2),
            "cash": num(cash, 2),
            "invested_value": num(invested, 2),
            "total_value": num(total, 2),
            "profit": num(profit, 2),
            "profit_pct": if funded > Decimal::ZERO { num(profit / funded * Decimal::ONE_HUNDRED, 2) } else { Value::Null },
            "realized_pnl": num(dtos::position::realized_pnl(&book.transactions), 2),
            "holdings": holdings,
            "trades": book.trades().count(),
            "recent_trades": recent,
        });
        if !unpriced.is_empty() {
            out["unpriced_at_cost"] = json!(unpriced);
        }
        out
    }

    async fn quote(&self, args: &Value) -> ToolResult {
        let ticker = ticker(args)?;
        let q = self.trading.quote(&ticker).await.map_err(|e| e.to_string())?;
        let change = if q.previous_close > Decimal::ZERO {
            Some(num((q.price / q.previous_close - Decimal::ONE) * Decimal::ONE_HUNDRED, 2))
        } else {
            None
        };
        Ok(json!({
            "ticker": ticker.as_str(),
            "price": num(q.price, 4),
            "currency": q.currency,
            "price_usd": num(q.price * q.usd_per_unit, 4),
            "previous_close": num(q.previous_close, 4),
            "change_pct": change,
            "as_of": chrono::DateTime::from_timestamp(q.timestamp, 0).map(|t| t.to_rfc3339()),
            "stale": q.stale,
        }))
    }

    async fn place_order(&self, args: &Value) -> ToolResult {
        let ticker = ticker(args)?;
        let side = match args.get("side").and_then(Value::as_str).map(str::to_lowercase).as_deref() {
            Some("buy") => Side::Buy,
            Some("sell") => Side::Sell,
            _ => return Err("`side` must be buy or sell.".into()),
        };
        let reason = args.get("reason").and_then(Value::as_str).map(str::trim).unwrap_or_default();
        if reason.is_empty() {
            return Err("Give a `reason` for the trade.".into());
        }
        let amount = |v: &Value, field: &str| -> Result<Decimal, String> {
            match v {
                Value::String(s) => Decimal::from_str(s.trim()).ok(),
                v => v.as_f64().and_then(Decimal::from_f64),
            }
            .filter(|d| *d > Decimal::ZERO)
            .ok_or_else(|| format!("`{field}` must be a number more than zero."))
        };
        let size = match (args.get("shares").filter(|v| !v.is_null()), args.get("amount_usd").filter(|v| !v.is_null())) {
            (Some(_), Some(_)) => return Err("Give `shares` or `amount_usd`, not both.".into()),
            (Some(Value::String(s)), None) if s.trim().eq_ignore_ascii_case("all") => Size::All,
            (Some(v), None) => Size::Shares(amount(v, "shares")?),
            (None, Some(v)) => Size::Usd(amount(v, "amount_usd")?),
            (None, None) => return Err("Say how much: `shares` or `amount_usd`.".into()),
        };
        let wanted = args.get("portfolio").and_then(Value::as_str);
        let fill = self.trading.order(wanted, &ticker, side, size).await.map_err(|e| e.to_string())?;
        let tx = &fill.tx;
        let verb = if side == Side::Buy { "Bought" } else { "Sold" };
        let mut entry = format!(
            "{verb} {} at {} {} (${}). {reason}",
            tx.shares.normalize(),
            tx.price.round_dp(4).normalize(),
            tx.currency,
            fill.usd_total.round_dp(2)
        );
        if entry.chars().count() > dtos::thesis::MAX_ENTRY_LEN {
            entry = entry.chars().take(dtos::thesis::MAX_ENTRY_LEN).collect();
        }
        // The trade stands even if the journal can't be written.
        let journaled = self.theses.add_entry(tx.portfolio_id, &ticker, &entry, Author::Ai).is_ok();
        Ok(json!({
            "filled": true,
            "side": if side == Side::Buy { "buy" } else { "sell" },
            "ticker": ticker.as_str(),
            "shares": num(tx.shares, 6),
            "price": num(tx.price, 4),
            "currency": tx.currency,
            "total_usd": num(fill.usd_total, 2),
            "cash_after": num(fill.cash_after, 2),
            "date": tx.date,
            "journaled": journaled,
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::services::tests::service;
    use rust_decimal_macros::dec;
    use serde_json::{json, Value};

    /// Calls a tool; `Err` holds a failed call's message.
    async fn call(s: &crate::mcp::McpService, name: &str, args: Value) -> Result<Value, String> {
        let reply = s
            .handle(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}))
            .await
            .unwrap();
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        if result["isError"] == json!(true) {
            Err(text)
        } else {
            Ok(serde_json::from_str(&text).unwrap())
        }
    }

    #[tokio::test]
    async fn remembers_theses_through_the_tools() {
        let s = service();
        let list = call(&s, "list_portfolios", json!({})).await.unwrap();
        let nvda = &list["portfolios"][0]["holdings"][0];
        assert_eq!((nvda["ticker"].as_str(), nvda["shares"].as_f64()), (Some("NVDA"), Some(2.0)));
        assert_eq!(nvda["first_bought"], "2026-01-05");
        assert_eq!(nvda["thesis_status"], Value::Null);

        let missing = call(&s, "list_theses", json!({})).await.unwrap();
        assert_eq!(missing["portfolios"][0]["held_without_thesis"], json!(["NVDA"]));
        let empty = call(&s, "get_thesis", json!({"ticker":"nvda"})).await.unwrap();
        assert!(empty["thesis"].is_null() && empty["hint"].is_string());

        // Only one portfolio: it needn't be named.
        let saved = call(
            &s,
            "save_thesis",
            json!({"ticker":"NVDA","thesis":"Owns AI training","sell_if":"Hyperscaler capex falls","target_price":250.5,"conviction":4,"note":"Agreed with the user."}),
        )
        .await
        .unwrap();
        assert_eq!(saved["thesis"]["target_price"], json!(250.5));
        assert_eq!(saved["thesis"]["last_edited_by"], "ai");
        assert_eq!(saved["position"]["avg_cost"], json!(100.0));
        let journal = saved["thesis"]["journal"][0]["text"].as_str().unwrap();
        assert!(journal.starts_with("Agreed with the user.\nUpdated: thesis, sell if, target none → 250.5"), "{journal}");

        // Partial updates keep the other fields; null clears.
        let updated = call(&s, "save_thesis", json!({"portfolio":"main","ticker":"NVDA","status":"at_risk","target_price":null})).await.unwrap();
        assert_eq!(updated["thesis"]["thesis"], "Owns AI training");
        assert_eq!((updated["thesis"]["status"].as_str(), &updated["thesis"]["target_price"]), (Some("at_risk"), &Value::Null));

        call(&s, "add_thesis_note", json!({"ticker":"NVDA","text":"Q3: data-centre revenue +60%."})).await.unwrap();
        let got = call(&s, "get_thesis", json!({"ticker":"NVDA"})).await.unwrap();
        assert_eq!(got["thesis"]["journal"][0]["text"], "Q3: data-centre revenue +60%.");
        assert_eq!(got["thesis"]["journal_entries"], 3);

        assert!(call(&s, "get_thesis", json!({"ticker":"NVDA","portfolio":"Other"})).await.unwrap_err().contains("Main"));
        assert!(call(&s, "save_thesis", json!({"ticker":"NVDA","conviction":9})).await.is_err());
        assert!(call(&s, "save_thesis", json!({"ticker":"NVDA","status":"great"})).await.is_err());
        assert!(call(&s, "add_thesis_note", json!({"ticker":"NVDA","text":" "})).await.is_err());
    }

    #[tokio::test]
    async fn trades_only_in_its_own_portfolio() {
        let s = service();
        let none = call(&s, "get_my_portfolio", json!({})).await.unwrap_err();
        assert!(none.contains("Settings"), "{none}");
        let refused = call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":1,"reason":"x"})).await;
        assert!(refused.is_err(), "no orders before it has a portfolio");

        let info = s.trading().start("", dec!(1000)).await.unwrap();
        assert_eq!((info.name.as_str(), info.funded, info.cash), ("Claude", dec!(1000), dec!(1000)));
        assert!(s.trading().start("", dec!(0)).await.is_err(), "needs cash");

        let quote = call(&s, "get_quote", json!({"ticker":"PTT.BK"})).await.unwrap();
        assert_eq!((quote["price"].as_f64(), quote["price_usd"].as_f64()), (Some(35.0), Some(1.05)));

        let buy = call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":2,"reason":"AI capex keeps growing."}))
            .await
            .unwrap();
        assert_eq!((buy["total_usd"].as_f64(), buy["cash_after"].as_f64()), (Some(400.0), Some(600.0)));
        // In baht, converted at the live rate; by amount, rounded down.
        let thai = call(&s, "place_order", json!({"ticker":"PTT.BK","side":"buy","amount_usd":"100","reason":"Dividend"}))
            .await
            .unwrap();
        assert_eq!((thai["shares"].as_f64(), thai["currency"].as_str()), (Some(95.238095), Some("THB")));

        let broke = call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":3,"reason":"More"})).await.unwrap_err();
        assert!(broke.contains("Not enough cash"), "{broke}");
        assert!(call(&s, "place_order", json!({"ticker":"NVDA","side":"sell","shares":5,"reason":"x"})).await.unwrap_err().contains("only 2"));
        assert!(call(&s, "place_order", json!({"ticker":"OLD","side":"buy","shares":1,"reason":"x"})).await.is_err(), "stale price");
        assert!(call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":1})).await.is_err(), "needs a reason");
        assert!(call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":1,"amount_usd":5,"reason":"x"})).await.is_err());
        assert!(call(&s, "place_order", json!({"ticker":"^GSPC","side":"buy","shares":1,"reason":"x"})).await.is_err());

        let sold = call(&s, "place_order", json!({"ticker":"NVDA","side":"sell","shares":"all","reason":"Taking profit."}))
            .await
            .unwrap();
        assert_eq!(sold["shares"].as_f64(), Some(2.0));

        let all = call(&s, "get_my_portfolio", json!({})).await.unwrap();
        let mine = &all["portfolios"][0];
        assert_eq!(mine["portfolio"]["name"], "Claude");
        assert_eq!(mine["trades"], 3);
        assert_eq!(mine["recent_trades"][0]["side"], "sell");
        assert_eq!(mine["holdings"].as_array().unwrap().len(), 1);
        assert_eq!(mine["funded"].as_f64(), Some(1000.0));
        // Priced at cost and sold at the same price: nothing gained or lost
        // beyond rounding the baht position.
        assert!(mine["profit"].as_f64().unwrap().abs() < 0.01, "{mine}");

        // The reason is journaled on the holding in Claude's portfolio.
        let thesis = call(&s, "get_thesis", json!({"ticker":"NVDA","portfolio":"Claude"})).await.unwrap();
        let journal = thesis["thesis"]["journal"].as_array().unwrap();
        assert!(
            journal.iter().any(|e| e["text"] == "Bought 2 at 200 USD ($400). AI capex keeps growing."),
            "{journal:?}"
        );
        // The user's own portfolio is untouched.
        let list = call(&s, "list_portfolios", json!({})).await.unwrap();
        let main = list["portfolios"].as_array().unwrap().iter().find(|p| p["name"] == "Main").unwrap();
        assert_eq!(main["holdings"][0]["shares"].as_f64(), Some(2.0));
        assert_eq!(main["yours"], false);
        assert!(list["portfolios"].as_array().unwrap().iter().any(|p| p["name"] == "Claude" && p["yours"] == true));

        let more = s.trading().add_funds(info.portfolio_id, dec!(500)).await.unwrap();
        assert_eq!(more.funded, dec!(1500));
        s.trading().stop(info.portfolio_id).unwrap();
        assert!(s.trading().infos().unwrap().is_empty());
        assert!(call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":1,"reason":"x"})).await.is_err());
        assert!(s.trading().add_funds(info.portfolio_id, dec!(5)).await.is_err(), "not Claude's any more");
    }

    #[tokio::test]
    async fn each_portfolio_trades_with_its_own_cash() {
        let s = service();
        let us = s.trading().start("US growth", dec!(500)).await.unwrap();
        let thai = s.trading().start("", dec!(100)).await.unwrap();
        assert_eq!(thai.name, "Claude");
        assert!(s.trading().start("main", dec!(5)).await.is_err(), "names stay unique");

        let unsure = call(&s, "place_order", json!({"ticker":"NVDA","side":"buy","shares":1,"reason":"x"})).await.unwrap_err();
        assert!(unsure.contains("US growth") && unsure.contains("Claude"), "{unsure}");
        assert!(call(&s, "place_order", json!({"portfolio":"Main","ticker":"NVDA","side":"buy","shares":1,"reason":"x"})).await.is_err(), "not the user's");

        // $200 fits the US portfolio's $500 but not the other's $100.
        call(&s, "place_order", json!({"portfolio":"us growth","ticker":"NVDA","side":"buy","shares":1,"reason":"x"})).await.unwrap();
        let broke = call(&s, "place_order", json!({"portfolio":"Claude","ticker":"NVDA","side":"buy","shares":1,"reason":"x"})).await.unwrap_err();
        assert!(broke.contains("you have $100"), "{broke}");
        let sell = call(&s, "place_order", json!({"portfolio":"Claude","ticker":"NVDA","side":"sell","shares":1,"reason":"x"})).await.unwrap_err();
        assert!(sell.contains("don't hold"), "holdings are per portfolio too: {sell}");

        let all = call(&s, "get_my_portfolio", json!({})).await.unwrap();
        let cash = |name: &str| {
            let p = all["portfolios"].as_array().unwrap().iter().find(|p| p["portfolio"]["name"] == name).unwrap();
            p["cash"].as_f64().unwrap()
        };
        assert_eq!((cash("US growth"), cash("Claude")), (300.0, 100.0));
        let one = call(&s, "get_my_portfolio", json!({"portfolio": us.portfolio_id.to_string()})).await.unwrap();
        assert_eq!(one["portfolios"].as_array().unwrap().len(), 1);

        s.trading().stop(us.portfolio_id).unwrap();
        // One left: it needn't be named.
        call(&s, "place_order", json!({"ticker":"PTT.BK","side":"buy","amount_usd":50,"reason":"x"})).await.unwrap();
        assert_eq!(s.trading().infos().unwrap().len(), 1);
    }
}
