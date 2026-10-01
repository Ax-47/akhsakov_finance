//! Which AI portfolios are racing, and when an external MCP client may
//! trade them. A racing portfolio is closed to every MCP connection except
//! while its round window is open, and then only to the connection that
//! drives it, at the round's frozen prices.

use super::tools::FrozenMarket;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

/// Longest tool result kept in a round's event log.
const DETAIL_CHARS: usize = 1_000;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RaceInfo {
    pub name: String,
    pub round: u32,
    pub rounds: u32,
}

/// One tool call an MCP client made inside its round window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecordedCall {
    pub tool: String,
    pub args: Value,
    pub success: bool,
    pub detail: String,
}

struct Window {
    connection_id: Uuid,
    deadline: String,
    market: FrozenMarket,
    calls: Vec<RecordedCall>,
}

struct Gate {
    info: RaceInfo,
    next_round_at: Option<String>,
    window: Option<Window>,
}

#[derive(Clone, Default)]
pub(crate) struct RaceGate(Arc<Mutex<HashMap<Uuid, Gate>>>);

impl RaceGate {
    fn with<T>(&self, f: impl FnOnce(&mut HashMap<Uuid, Gate>) -> T) -> T {
        let mut gates = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut gates)
    }

    /// Closes `portfolio` to MCP trading until its next window opens.
    pub fn lock(&self, portfolio: Uuid, info: RaceInfo, next_round_at: Option<String>) {
        self.with(|g| {
            g.insert(portfolio, Gate { info, next_round_at, window: None });
        });
    }

    /// Lets `connection_id` trade `portfolio` at `market` until closed.
    pub fn open(&self, portfolio: Uuid, info: RaceInfo, connection_id: Uuid, deadline: String, market: FrozenMarket) {
        self.with(|g| {
            g.insert(portfolio, Gate {
                info,
                next_round_at: None,
                window: Some(Window { connection_id, deadline, market, calls: vec![] }),
            });
        });
    }

    /// Ends the window, keeping the portfolio closed, and returns the calls
    /// made in it. `None` if no window was open (e.g. the race stopped).
    pub fn close(&self, portfolio: Uuid) -> Option<Vec<RecordedCall>> {
        self.with(|g| g.get_mut(&portfolio)?.window.take().map(|w| w.calls))
    }

    /// The race is over for `portfolio`; MCP trading works as usual again.
    pub fn release(&self, portfolio: Uuid) {
        self.with(|g| {
            g.remove(&portfolio);
        });
    }

    /// The frozen market of `portfolio`'s open window.
    pub fn market_for(&self, portfolio: Uuid) -> Option<FrozenMarket> {
        self.with(|g| g.get(&portfolio)?.window.as_ref().map(|w| w.market.clone()))
    }

    /// The frozen market of any window open for `connection`. Every window
    /// of one round shares a market.
    pub fn connection_market(&self, connection: Uuid) -> Option<FrozenMarket> {
        self.with(|g| {
            g.values()
                .filter_map(|gate| gate.window.as_ref())
                .find(|w| w.connection_id == connection)
                .map(|w| w.market.clone())
        })
    }

    /// Whether `connection` may place an order in `portfolio`: `Ok(None)`
    /// for a portfolio that isn't racing (live prices), `Ok(Some)` inside
    /// its window (frozen prices), otherwise why not.
    pub fn order_market(&self, portfolio: Uuid, connection: Uuid) -> Result<Option<FrozenMarket>, String> {
        self.with(|g| match g.get(&portfolio) {
            None => Ok(None),
            Some(Gate { window: Some(w), .. }) if w.connection_id == connection => Ok(Some(w.market.clone())),
            Some(Gate { window: Some(_), .. }) => Err("This portfolio is in an AI race and is traded by another connection.".into()),
            Some(Gate { info, next_round_at, .. }) => Err(match next_round_at {
                Some(at) => format!("This portfolio is in the AI race \"{}\"; the round window is closed. The next round starts around {at}.", info.name),
                None => format!("This portfolio is in the AI race \"{}\"; the round window is closed. Check get_my_portfolio later.", info.name),
            }),
        })
    }

    /// What get_my_portfolio tells an MCP client about the race.
    pub fn status_json(&self, portfolio: Uuid) -> Option<Value> {
        self.with(|g| {
            let gate = g.get(&portfolio)?;
            let mut out = json!({
                "name": gate.info.name,
                "round": gate.info.round,
                "rounds": gate.info.rounds,
                "window_open": gate.window.is_some(),
            });
            match &gate.window {
                Some(w) => {
                    out["deadline"] = json!(w.deadline);
                    out["note"] = json!("Your round window is open: prices are frozen for this round and orders fill at them. Finish before the deadline.");
                }
                None => {
                    out["next_round_at"] = json!(gate.next_round_at);
                    out["note"] = json!("Orders are rejected until the next round window opens.");
                }
            }
            Some(out)
        })
    }

    /// Logs a call made by `connection` in its open windows: `target`'s
    /// alone when the call named one, otherwise all of them.
    pub fn record(&self, connection: Uuid, target: Option<Uuid>, tool: &str, args: &Value, result: &Result<Value, String>) {
        self.with(|g| {
            for (portfolio, gate) in g.iter_mut() {
                let Some(window) = gate.window.as_mut() else { continue };
                if window.connection_id != connection || target.is_some_and(|t| t != *portfolio) {
                    continue;
                }
                let (success, detail) = match result {
                    Ok(value) if tool == "place_order" => (true, value.to_string()),
                    Ok(_) => (true, "Completed".to_string()),
                    Err(error) => (false, error.clone()),
                };
                window.calls.push(RecordedCall {
                    tool: tool.into(),
                    args: args.clone(),
                    success,
                    detail: detail.chars().take(DETAIL_CHARS).collect(),
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> RaceInfo {
        RaceInfo { name: "Cup".into(), round: 1, rounds: 3 }
    }

    #[test]
    fn only_the_driving_connection_trades_inside_its_window() {
        let gate = RaceGate::default();
        let (portfolio, driver, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        assert!(gate.order_market(portfolio, driver).unwrap().is_none(), "not racing");

        gate.lock(portfolio, info(), Some("soon".into()));
        assert!(gate.order_market(portfolio, driver).err().unwrap().contains("soon"));

        gate.open(portfolio, info(), driver, "deadline".into(), FrozenMarket::new());
        assert!(gate.order_market(portfolio, driver).unwrap().is_some());
        assert!(gate.order_market(portfolio, other).is_err());
        assert!(gate.connection_market(driver).is_some() && gate.connection_market(other).is_none());
        assert_eq!(gate.status_json(portfolio).unwrap()["window_open"], true);

        gate.record(other, None, "get_quote", &json!({}), &Ok(json!({})));
        gate.record(driver, Some(portfolio), "place_order", &json!({}), &Ok(json!({"filled": true})));
        let calls = gate.close(portfolio).unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].success && calls[0].detail.contains("filled"));
        assert!(gate.order_market(portfolio, driver).is_err(), "closed again after the window");
        assert!(gate.close(portfolio).is_none());

        gate.release(portfolio);
        assert!(gate.order_market(portfolio, other).unwrap().is_none(), "released");
    }
}
