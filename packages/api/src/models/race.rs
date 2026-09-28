//! Synchronized competitions between independently configured AI portfolios.

use super::{
    service::{RaceRunContext, Usage},
    ModelService,
};
use crate::mcp::services::{race_gate::RaceInfo, tools::FrozenMarket};
use crate::shared::ServiceError;
use dtos::ai_models::{
    AiRace, AiRaceAuditEvent, AiRaceContestant, AiRaceStanding, AiRaceStatus, NewAiRace,
};
use rusqlite::{params, OptionalExtension};
use rust_decimal::prelude::ToPrimitive;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

const MIN_CONTESTANTS: usize = 2;
const MAX_CONTESTANTS: usize = 12;
const MAX_ROUNDS: u32 = 365;
const MAX_FREQUENCY_MINUTES: u32 = 10_080;
const MIN_TIMEOUT_SECONDS: u32 = 15;
const MAX_TIMEOUT_SECONDS: u32 = 600;

fn storage(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Storage(e.to_string())
}

fn invalid(message: impl Into<String>) -> ServiceError {
    ServiceError::Validation(message.into())
}

fn status(value: &str) -> AiRaceStatus {
    match value {
        "running" => AiRaceStatus::Running,
        "paused" => AiRaceStatus::Paused,
        "completed" => AiRaceStatus::Completed,
        "stopped" => AiRaceStatus::Stopped,
        _ => AiRaceStatus::Draft,
    }
}

impl ModelService {
    pub(super) fn profile_active_in_race(&self, profile_id: Uuid) -> Result<bool, ServiceError> {
        self.db
            .with(|c| {
                c.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM ai_race_contestants rc
                        JOIN ai_races r ON r.id=rc.race_id
                        JOIN ai_trader_configs tc ON tc.portfolio_id=rc.portfolio_id
                        WHERE tc.profile_id=?1 AND r.status IN ('running','paused')
                    )",
                    [profile_id.to_string()],
                    |r| r.get(0),
                )
            })
            .map_err(storage)
    }

    pub(super) fn race_active_for(&self, portfolio_id: Uuid) -> Result<bool, ServiceError> {
        self.db
            .with(|c| {
                c.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM ai_race_contestants rc JOIN ai_races r ON r.id=rc.race_id
                        WHERE rc.portfolio_id=?1 AND r.status IN ('running','paused')
                    )",
                    [portfolio_id.to_string()],
                    |r| r.get(0),
                )
            })
            .map_err(storage)
    }

    pub fn create_race(&self, input: NewAiRace) -> Result<AiRace, ServiceError> {
        let name = input.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(invalid("Race name must be between 1 and 80 characters."));
        }
        let mut contestants = input.contestant_ids;
        contestants.sort_unstable();
        contestants.dedup();
        if !(MIN_CONTESTANTS..=MAX_CONTESTANTS).contains(&contestants.len()) {
            return Err(invalid("Choose between 2 and 12 different AI portfolios."));
        }
        if !input.starting_capital.is_finite()
            || input.starting_capital <= 0.0
            || input.starting_capital > 1_000_000_000.0
        {
            return Err(invalid(
                "Starting capital must be more than zero and at most $1 billion.",
            ));
        }
        if input.rounds == 0 || input.rounds > MAX_ROUNDS {
            return Err(invalid("Race duration must be between 1 and 365 rounds."));
        }
        if input.trading_frequency_minutes == 0
            || input.trading_frequency_minutes > MAX_FREQUENCY_MINUTES
        {
            return Err(invalid(
                "Trading frequency must be between 1 minute and 7 days.",
            ));
        }
        if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&input.round_timeout_seconds) {
            return Err(invalid(
                "Round deadline must be between 15 and 600 seconds.",
            ));
        }
        for id in &contestants {
            let config = self.config(*id)?;
            if config.profile_id.is_none() && config.mcp_connection_id.is_none() {
                return Err(invalid(format!(
                    "Portfolio {id} needs a model or MCP connection before it can race."
                )));
            }
            if let Some(connection) = config.mcp_connection_id {
                self.check_mcp_driver(connection, *id)?;
            }
            if self.race_active_for(*id)? {
                return Err(invalid(format!(
                    "Portfolio {id} is already in an active race."
                )));
            }
        }
        let id = Uuid::new_v4();
        self.db
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO ai_races
                     (id,name,starting_capital,rounds,trading_frequency_minutes,round_timeout_seconds)
                     VALUES (?1,?2,?3,?4,?5,?6)",
                    params![id.to_string(), name, input.starting_capital, input.rounds, input.trading_frequency_minutes, input.round_timeout_seconds],
                )?;
                for portfolio in &contestants {
                    tx.execute(
                        "INSERT INTO ai_race_contestants (race_id,portfolio_id) VALUES (?1,?2)",
                        params![id.to_string(), portfolio.to_string()],
                    )?;
                }
                Ok(())
            })
            .map_err(storage)?;
        self.audit(
            id,
            "created",
            &format!(
                "Created with {} contestants and {} rounds.",
                contestants.len(),
                input.rounds
            ),
        )?;
        self.race(id)
    }

    pub fn races(&self) -> Result<Vec<AiRace>, ServiceError> {
        let ids: Vec<Uuid> = self
            .db
            .with(|c| {
                c.prepare("SELECT id FROM ai_races ORDER BY created_at DESC LIMIT 20")?
                    .query_map([], |r| {
                        let id: String = r.get(0)?;
                        Ok(Uuid::parse_str(&id).unwrap_or_default())
                    })?
                    .collect()
            })
            .map_err(storage)?;
        ids.into_iter().map(|id| self.race(id)).collect()
    }

    pub fn race(&self, id: Uuid) -> Result<AiRace, ServiceError> {
        let mut race = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT name,status,starting_capital,rounds,completed_rounds,
                            trading_frequency_minutes,round_timeout_seconds,created_at,started_at,finished_at
                     FROM ai_races WHERE id=?1",
                    [id.to_string()],
                    |r| {
                        let state: String = r.get(1)?;
                        Ok(AiRace {
                            id,
                            name: r.get(0)?,
                            status: status(&state),
                            starting_capital: r.get(2)?,
                            rounds: r.get::<_, i64>(3)? as u32,
                            completed_rounds: r.get::<_, i64>(4)? as u32,
                            trading_frequency_minutes: r.get::<_, i64>(5)? as u32,
                            round_timeout_seconds: r.get::<_, i64>(6)? as u32,
                            created_at: r.get(7)?,
                            started_at: r.get(8)?,
                            finished_at: r.get(9)?,
                            contestants: vec![],
                            leaderboard: vec![],
                            audit: vec![],
                        })
                    },
                )
                .optional()
            })
            .map_err(storage)?
            .ok_or_else(|| ServiceError::NotFound("AI race".into()))?;
        race.contestants = self.race_contestants(id)?;
        race.leaderboard = self.leaderboard(&race)?;
        race.audit = self.race_audit(id)?;
        Ok(race)
    }

    pub fn start_race(&self, id: Uuid) -> Result<AiRace, ServiceError> {
        let race = self.race(id)?;
        if race.status != AiRaceStatus::Draft {
            return Err(invalid("Only a draft race can be started."));
        }
        let stored_count: i64 = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT COUNT(*) FROM ai_race_contestants WHERE race_id=?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
            })
            .map_err(storage)?;
        if race.contestants.len() != stored_count as usize
            || race.contestants.len() < MIN_CONTESTANTS
        {
            return Err(invalid(
                "Every contestant needs an available model connection before the race can start.",
            ));
        }
        for contestant in &race.contestants {
            let other_active: bool = self
                .db
                .with(|c| {
                    c.query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM ai_race_contestants rc JOIN ai_races r ON r.id=rc.race_id
                            WHERE rc.portfolio_id=?1 AND r.id<>?2 AND r.status IN ('running','paused')
                        )",
                        params![contestant.portfolio_id.to_string(), id.to_string()],
                        |r| r.get(0),
                    )
                })
                .map_err(storage)?;
            if other_active {
                return Err(invalid(format!(
                    "{} is already competing in another active race.",
                    contestant.name
                )));
            }
        }
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.db
            .transaction(|tx| {
                for contestant in &race.contestants {
                    tx.execute("DELETE FROM transactions WHERE portfolio_id=?1", [contestant.portfolio_id.to_string()])?;
                    tx.execute(
                        "INSERT INTO transactions
                         (id,portfolio_id,ticker,kind,shares,price,fee,date,currency,fx_to_usd)
                         VALUES (?1,?2,'$CASH','Deposit',?3,'1','0',?4,'USD','1')",
                        params![Uuid::new_v4().to_string(), contestant.portfolio_id.to_string(), race.starting_capital.to_string(), date],
                    )?;
                }
                tx.execute(
                    "UPDATE ai_races SET status='running',started_at=datetime('now') WHERE id=?1 AND status='draft'",
                    [id.to_string()],
                )?;
                Ok(())
            })
            .map_err(storage)?;
        self.audit(
            id,
            "started",
            "Contestant portfolios were reset to identical starting capital.",
        )?;
        let race = self.race(id)?;
        self.lock_gates(&race, None);
        self.spawn_race(id);
        Ok(race)
    }

    pub fn pause_race(&self, id: Uuid) -> Result<AiRace, ServiceError> {
        self.change_race_state(
            id,
            "running",
            "paused",
            "paused",
            "Paused after the current round.",
        )
    }

    pub fn resume_race(&self, id: Uuid) -> Result<AiRace, ServiceError> {
        let race = self.change_race_state(id, "paused", "running", "resumed", "Race resumed.")?;
        self.lock_gates(&race, None);
        self.spawn_race(id);
        Ok(race)
    }

    pub fn stop_race(&self, id: Uuid) -> Result<AiRace, ServiceError> {
        let state = self.race(id)?.status;
        if !matches!(state, AiRaceStatus::Running | AiRaceStatus::Paused) {
            return Err(invalid("Only a running or paused race can be stopped."));
        }
        self.db
            .with(|c| {
                c.execute(
                    "UPDATE ai_races SET status='stopped',finished_at=datetime('now') WHERE id=?1",
                    [id.to_string()],
                )
            })
            .map_err(storage)?;
        self.db.with(|c| c.execute(
            "UPDATE ai_runs SET status='failed',finished_at=datetime('now'),error='The race was stopped.'
             WHERE race_id=?1 AND status='running'",
            [id.to_string()],
        )).map_err(storage)?;
        self.audit(
            id,
            "stopped",
            "Stopped by the user; no new rounds will start.",
        )?;
        let race = self.race(id)?;
        self.release_gates(&race);
        Ok(race)
    }

    fn race_info(race: &AiRace, round: u32) -> RaceInfo {
        RaceInfo {
            name: race.name.clone(),
            round,
            rounds: race.rounds,
        }
    }

    /// Closes every contestant to outside MCP trading until its round
    /// window opens.
    fn lock_gates(&self, race: &AiRace, next_round_at: Option<String>) {
        let info = Self::race_info(race, race.completed_rounds + 1);
        for contestant in &race.contestants {
            self.mcp
                .race_gate()
                .lock(contestant.portfolio_id, info.clone(), next_round_at.clone());
        }
    }

    fn release_gates(&self, race: &AiRace) {
        for contestant in &race.contestants {
            self.mcp.race_gate().release(contestant.portfolio_id);
        }
    }

    /// After a restart every active race is paused; its portfolios stay
    /// closed to outside MCP trading until it is resumed or stopped.
    pub(super) fn relock_active_races(&self) {
        let ids: Vec<String> = self
            .db
            .with(|c| {
                c.prepare("SELECT id FROM ai_races WHERE status IN ('running','paused')")?
                    .query_map([], |r| r.get(0))?
                    .collect()
            })
            .unwrap_or_default();
        for id in ids.iter().filter_map(|id| Uuid::parse_str(id).ok()) {
            if let Ok(race) = self.race(id) {
                self.lock_gates(&race, None);
            }
        }
    }

    fn change_race_state(
        &self,
        id: Uuid,
        from: &str,
        to: &str,
        kind: &str,
        detail: &str,
    ) -> Result<AiRace, ServiceError> {
        let changed = self
            .db
            .with(|c| {
                c.execute(
                    "UPDATE ai_races SET status=?2 WHERE id=?1 AND status=?3",
                    params![id.to_string(), to, from],
                )
            })
            .map_err(storage)?;
        if changed == 0 {
            return Err(invalid(format!("Race must be {from} to do that.")));
        }
        self.audit(id, kind, detail)?;
        self.race(id)
    }

    fn spawn_race(&self, id: Uuid) {
        let runner = self.clone();
        tokio::spawn(async move {
            if let Err(error) = runner.run_race(id).await {
                let _ = runner.audit(id, "scheduler_error", &error.to_string());
                let _ = runner.db.with(|c| {
                    c.execute(
                        "UPDATE ai_races SET status='paused' WHERE id=?1 AND status='running'",
                        [id.to_string()],
                    )
                });
            }
        });
    }

    async fn run_race(&self, id: Uuid) -> Result<(), ServiceError> {
        loop {
            let race = self.race(id)?;
            if race.status != AiRaceStatus::Running {
                return Ok(());
            }
            if race.completed_rounds >= race.rounds {
                self.complete_race(id)?;
                return Ok(());
            }
            let round = race.completed_rounds + 1;
            let claimed = self.db.with(|c| c.execute(
                "INSERT OR IGNORE INTO ai_race_rounds (race_id,round_number,status) VALUES (?1,?2,'running')",
                params![id.to_string(), round],
            )).map_err(storage)?;
            if claimed == 0 {
                return Ok(());
            }
            let market = FrozenMarket::new();
            self.preload_market(&race, &market).await?;
            let mut snapshot = market.snapshot().await;
            snapshot["frozen_at"] = json!(chrono::Utc::now().to_rfc3339());
            let frozen_at = snapshot["frozen_at"].clone();
            self.db
                .with(|c| {
                    c.execute(
                "UPDATE ai_race_rounds SET market_snapshot=?3 WHERE race_id=?1 AND round_number=?2",
                params![id.to_string(), round, snapshot.to_string()],
            )
                })
                .map_err(storage)?;
            self.audit(id, "round_started", &format!("Round {round} started; all contestants share one deadline and market snapshot."))?;
            let timeout = Duration::from_secs(u64::from(race.round_timeout_seconds));
            let mut run_ids = Vec::new();
            let mut mcp_runs = Vec::new();
            for contestant in &race.contestants {
                let context = RaceRunContext {
                    race_id: id,
                    round,
                    market: market.clone(),
                };
                if contestant.mcp {
                    match self.open_mcp_window(&race, round, contestant.portfolio_id, &market, timeout) {
                        Ok(run) => mcp_runs.push((contestant.portfolio_id, run)),
                        Err(error) => self.audit(
                            id,
                            "contestant_start_failed",
                            &format!("{}: {error}", contestant.name),
                        )?,
                    }
                    continue;
                }
                match self
                    .start_run_for(contestant.portfolio_id, Some(context), timeout)
                    .await
                {
                    Ok(run) => run_ids.push(run.id),
                    Err(error) => self.audit(
                        id,
                        "contestant_start_failed",
                        &format!("{}: {error}", contestant.name),
                    )?,
                }
            }
            let deadline = tokio::time::Instant::now() + timeout + Duration::from_secs(2);
            loop {
                let remaining: i64 = self.db.with(|c| c.query_row(
                    "SELECT COUNT(*) FROM ai_runs WHERE race_id=?1 AND race_round=?2 AND status='running'",
                    params![id.to_string(), round], |r| r.get(0),
                )).map_err(storage)?;
                if remaining == 0 || tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            for (portfolio, run_id) in &mcp_runs {
                self.close_mcp_window(*portfolio, *run_id)?;
                run_ids.push(*run_id);
            }
            let mut snapshot = market.snapshot().await;
            snapshot["frozen_at"] = frozen_at;
            let snapshot = self.capture_metrics(&race, round, snapshot).await?;
            self.db.transaction(|tx| {
                tx.execute(
                    "UPDATE ai_race_rounds SET status='completed',market_snapshot=?3,finished_at=datetime('now')
                     WHERE race_id=?1 AND round_number=?2",
                    params![id.to_string(), round, snapshot.to_string()],
                )?;
                tx.execute(
                    "UPDATE ai_races SET completed_rounds=?2 WHERE id=?1 AND completed_rounds<?2",
                    params![id.to_string(), round],
                )?;
                Ok(())
            }).map_err(storage)?;
            self.audit(
                id,
                "round_completed",
                &format!(
                    "Round {round} completed; {} model runs were launched.",
                    run_ids.len()
                ),
            )?;
            if round >= race.rounds {
                self.complete_race(id)?;
                return Ok(());
            }
            let delay = Duration::from_secs(u64::from(race.trading_frequency_minutes) * 60);
            let current = self.race(id)?;
            if matches!(current.status, AiRaceStatus::Running | AiRaceStatus::Paused) {
                let next = chrono::Utc::now() + chrono::Duration::seconds(delay.as_secs() as i64);
                let next = (current.status == AiRaceStatus::Running).then(|| next.to_rfc3339());
                self.lock_gates(&current, next);
            }
            let until = tokio::time::Instant::now() + delay;
            while tokio::time::Instant::now() < until {
                tokio::time::sleep(
                    Duration::from_secs(1)
                        .min(until.saturating_duration_since(tokio::time::Instant::now())),
                )
                .await;
                if self.race(id)?.status != AiRaceStatus::Running {
                    return Ok(());
                }
            }
        }
    }

    async fn preload_market(
        &self,
        race: &AiRace,
        market: &FrozenMarket,
    ) -> Result<(), ServiceError> {
        for contestant in &race.contestants {
            let book = self
                .mcp
                .trading()
                .book(Some(&contestant.portfolio_id.to_string()))?;
            for position in book.positions() {
                self.mcp
                    .tools()
                    .call_frozen(
                        "get_quote",
                        &json!({"ticker": position.ticker.as_str()}),
                        market,
                    )
                    .await;
            }
        }
        Ok(())
    }

    async fn capture_metrics(
        &self,
        race: &AiRace,
        round: u32,
        mut snapshot: Value,
    ) -> Result<Value, ServiceError> {
        let quotes = snapshot["prices"]
            .as_object_mut()
            .expect("race snapshots have a price object");
        for contestant in &race.contestants {
            let book = self
                .mcp
                .trading()
                .book(Some(&contestant.portfolio_id.to_string()))?;
            let mut value = book.cash().to_f64().unwrap_or_default();
            for position in book.positions() {
                let key = position.ticker.as_str().to_string();
                let price = if let Some(item) = quotes.get(&key) {
                    item["price"].as_f64().unwrap_or_default()
                        * item["usd_per_unit"].as_f64().unwrap_or(1.0)
                } else {
                    let quote = self.mcp.trading().quote(&position.ticker).await?;
                    let price = (quote.price * quote.usd_per_unit)
                        .to_f64()
                        .unwrap_or_default();
                    quotes.insert(key.clone(), json!({"price": quote.price, "currency": quote.currency, "usd_per_unit": quote.usd_per_unit, "timestamp": quote.timestamp}));
                    price
                };
                value += position.shares.to_f64().unwrap_or_default() * price;
            }
            let fees: f64 = book
                .transactions
                .iter()
                .map(|t| t.usd_fee().to_f64().unwrap_or_default())
                .sum();
            let turnover: f64 = book
                .trades()
                .map(|t| {
                    (t.shares * t.usd_price())
                        .abs()
                        .to_f64()
                        .unwrap_or_default()
                })
                .sum();
            let failed: i64 = self.db.with(|c| c.query_row(
                "SELECT COUNT(*) FROM ai_runs WHERE race_id=?1 AND portfolio_id=?2 AND status='failed'",
                params![race.id.to_string(), contestant.portfolio_id.to_string()], |r| r.get(0),
            )).map_err(storage)?;
            self.db.with(|c| c.execute(
                "INSERT OR REPLACE INTO ai_race_metrics
                 (race_id,round_number,portfolio_id,portfolio_value,cash,fees,turnover,failed_model_runs)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![race.id.to_string(), round, contestant.portfolio_id.to_string(), value,
                    book.cash().to_f64().unwrap_or_default(), fees, turnover, failed],
            )).map_err(storage)?;
        }
        Ok(snapshot)
    }

    fn complete_race(&self, id: Uuid) -> Result<(), ServiceError> {
        self.db.with(|c| c.execute(
            "UPDATE ai_races SET status='completed',finished_at=datetime('now') WHERE id=?1 AND status='running'",
            [id.to_string()],
        )).map_err(storage)?;
        self.release_gates(&self.race(id)?);
        self.audit(id, "completed", "All scheduled rounds completed.")
    }

    /// Records a run for an MCP contestant and lets its connection trade
    /// at the round's frozen prices until the shared deadline.
    fn open_mcp_window(
        &self,
        race: &AiRace,
        round: u32,
        portfolio: Uuid,
        market: &FrozenMarket,
        timeout: Duration,
    ) -> Result<Uuid, ServiceError> {
        let connection_id = self
            .config(portfolio)?
            .mcp_connection_id
            .ok_or_else(|| invalid("The contestant no longer has an MCP connection."))?;
        let connection = self.check_mcp_driver(connection_id, portfolio)?;
        let run_id = Uuid::new_v4();
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_runs (id, portfolio_id, status, profile_name, model, race_id, race_round)
                     VALUES (?1, ?2, 'running', ?3, 'MCP client', ?4, ?5)",
                    params![
                        run_id.to_string(),
                        portfolio.to_string(),
                        connection.name,
                        race.id.to_string(),
                        round
                    ],
                )
            })
            .map_err(storage)?;
        let deadline = chrono::Utc::now() + chrono::Duration::seconds(timeout.as_secs() as i64);
        self.mcp.race_gate().open(
            portfolio,
            Self::race_info(race, round),
            connection_id,
            deadline.to_rfc3339(),
            market.clone(),
        );
        Ok(run_id)
    }

    /// Ends an MCP contestant's window and settles its run from the calls
    /// the client made in it.
    pub(super) fn close_mcp_window(&self, portfolio: Uuid, run_id: Uuid) -> Result<(), ServiceError> {
        let calls = self.mcp.race_gate().close(portfolio).unwrap_or_default();
        for (index, call) in calls.iter().enumerate() {
            self.record_event(
                run_id,
                index as i64 + 1,
                &call.tool,
                &call.args,
                call.success,
                &call.detail,
            )?;
        }
        if !calls.iter().any(|c| c.success) {
            self.finish(
                run_id,
                None,
                Some("The MCP client did not act before the round deadline.".into()),
                Usage::default(),
            );
        } else {
            let orders = calls
                .iter()
                .filter(|c| c.success && c.tool == "place_order")
                .count();
            self.finish(
                run_id,
                Some(format!(
                    "The MCP client made {} tool calls and {orders} orders in this round.",
                    calls.len()
                )),
                None,
                Usage::default(),
            );
        }
        Ok(())
    }

    fn race_contestants(&self, id: Uuid) -> Result<Vec<AiRaceContestant>, ServiceError> {
        self.db
            .with(|c| {
                c.prepare(
                    "SELECT rc.portfolio_id,p.name,COALESCE(mc.name,mp.name),
                            CASE WHEN mc.id IS NOT NULL THEN 'MCP client' ELSE mp.model END,
                            mc.id IS NOT NULL
             FROM ai_race_contestants rc
             JOIN portfolios p ON p.id=rc.portfolio_id
             JOIN ai_trader_configs tc ON tc.portfolio_id=rc.portfolio_id
             LEFT JOIN ai_model_profiles mp ON mp.id=tc.profile_id
             LEFT JOIN mcp_connections mc ON mc.id=tc.mcp_connection_id
             WHERE rc.race_id=?1 AND (mp.id IS NOT NULL OR mc.id IS NOT NULL)
             ORDER BY lower(p.name)",
                )?
                .query_map([id.to_string()], |r| {
                    let raw: String = r.get(0)?;
                    Ok(AiRaceContestant {
                        portfolio_id: Uuid::parse_str(&raw).unwrap_or_default(),
                        name: r.get(1)?,
                        profile_name: r.get(2)?,
                        model: r.get(3)?,
                        mcp: r.get(4)?,
                    })
                })?
                .collect()
            })
            .map_err(storage)
    }

    fn leaderboard(&self, race: &AiRace) -> Result<Vec<AiRaceStanding>, ServiceError> {
        let mut out = Vec::new();
        for contestant in &race.contestants {
            let rows: Vec<(f64,f64,f64,f64,i64)> = self.db.with(|c| c.prepare(
                "SELECT portfolio_value,cash,fees,turnover,failed_model_runs FROM ai_race_metrics
                 WHERE race_id=?1 AND portfolio_id=?2 ORDER BY round_number"
            )?.query_map(params![race.id.to_string(), contestant.portfolio_id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.collect()).map_err(storage)?;
            let values: Vec<f64> = rows.iter().map(|r| r.0).collect();
            let last = values.last().copied().unwrap_or(race.starting_capital);
            let returns: Vec<f64> = values
                .windows(2)
                .filter_map(|v| (v[0] > 0.0).then_some(v[1] / v[0] - 1.0))
                .collect();
            let mean = if returns.is_empty() {
                0.0
            } else {
                returns.iter().sum::<f64>() / returns.len() as f64
            };
            let volatility = if returns.len() < 2 {
                0.0
            } else {
                (returns.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / returns.len() as f64)
                    .sqrt()
            };
            let mut peak = race.starting_capital;
            let mut drawdown = 0.0_f64;
            for value in &values {
                peak = peak.max(*value);
                if peak > 0.0 {
                    drawdown = drawdown.max((peak - *value) / peak);
                }
            }
            let last_row = rows.last().copied().unwrap_or((last, last, 0.0, 0.0, 0));
            out.push(AiRaceStanding {
                rank: 0,
                portfolio_id: contestant.portfolio_id,
                name: contestant.name.clone(),
                total_return_pct: (last / race.starting_capital - 1.0) * 100.0,
                max_drawdown_pct: drawdown * 100.0,
                volatility_pct: volatility * 100.0,
                risk_adjusted_return: if volatility > 0.0 {
                    mean / volatility
                } else {
                    0.0
                },
                fees: last_row.2,
                turnover: last_row.3,
                cash_allocation_pct: if last > 0.0 {
                    last_row.1 / last * 100.0
                } else {
                    0.0
                },
                failed_model_runs: last_row.4 as u32,
            });
        }
        out.sort_by(|a, b| b.total_return_pct.total_cmp(&a.total_return_pct));
        for (index, row) in out.iter_mut().enumerate() {
            row.rank = index as u32 + 1;
        }
        Ok(out)
    }

    fn audit(&self, id: Uuid, kind: &str, detail: &str) -> Result<(), ServiceError> {
        self.db.with(|c| c.execute(
            "INSERT INTO ai_race_audit (race_id,sequence,kind,detail)
             VALUES (?1,COALESCE((SELECT MAX(sequence)+1 FROM ai_race_audit WHERE race_id=?1),1),?2,?3)",
            params![id.to_string(), kind, detail],
        )).map_err(storage)?;
        Ok(())
    }

    fn race_audit(&self, id: Uuid) -> Result<Vec<AiRaceAuditEvent>, ServiceError> {
        self.db.with(|c| c.prepare(
            "SELECT sequence,at,kind,detail FROM ai_race_audit WHERE race_id=?1 ORDER BY sequence DESC LIMIT 100"
        )?.query_map([id.to_string()], |r| Ok(AiRaceAuditEvent { sequence:r.get(0)?, at:r.get(1)?, kind:r.get(2)?, detail:r.get(3)? }))?.collect()).map_err(storage)
    }

    pub fn race_csv(&self, id: Uuid) -> Result<String, ServiceError> {
        let race = self.race(id)?;
        let mut csv = String::from("rank,portfolio,total_return_pct,max_drawdown_pct,volatility_pct,risk_adjusted_return,fees,turnover,cash_allocation_pct,failed_model_runs\n");
        for s in race.leaderboard {
            let name = s.name.replace('"', "\"\"");
            csv.push_str(&format!(
                "{},\"{}\",{:.6},{:.6},{:.6},{:.6},{:.2},{:.2},{:.6},{}\n",
                s.rank,
                name,
                s.total_return_pct,
                s.max_drawdown_pct,
                s.volatility_pct,
                s.risk_adjusted_return,
                s.fees,
                s.turnover,
                s.cash_allocation_pct,
                s.failed_model_runs
            ));
        }
        Ok(csv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ModelService {
        let db = crate::database::Database::in_memory().unwrap();
        let connector = crate::mcp::services::tests::service_with_database(db.clone());
        ModelService::new(db, connector)
    }

    #[test]
    fn race_status_parser_is_provider_neutral() {
        assert_eq!(status("running"), AiRaceStatus::Running);
        assert_eq!(status("something-new"), AiRaceStatus::Draft);
    }

    #[test]
    fn leaderboard_return_math_handles_no_rounds() {
        let start = 10_000.0;
        let last = 10_000.0;
        assert_eq!((last / start - 1.0) * 100.0, 0.0);
    }

    #[tokio::test]
    async fn race_start_equalizes_capital_and_locks_configs() {
        let service = service();
        let profile = service
            .save_profile(
                None,
                "Test model",
                "https://api.example.invalid/v1",
                "test-model",
                Some("test-key"),
            )
            .unwrap();
        let first = service
            .mcp
            .trading()
            .start("First", rust_decimal::Decimal::from(500))
            .await
            .unwrap();
        let second = service
            .mcp
            .trading()
            .start("Second", rust_decimal::Decimal::from(900))
            .await
            .unwrap();
        for id in [first.portfolio_id, second.portfolio_id] {
            let mut config = service.config(id).unwrap();
            config.profile_id = Some(profile.id);
            service.save_config(config).unwrap();
        }
        let race = service
            .create_race(NewAiRace {
                name: "Fair start".into(),
                contestant_ids: vec![first.portfolio_id, second.portfolio_id],
                starting_capital: 10_000.0,
                rounds: 2,
                trading_frequency_minutes: 1,
                round_timeout_seconds: 15,
            })
            .unwrap();
        assert_eq!(race.status, AiRaceStatus::Draft);
        let started = service.start_race(race.id).unwrap();
        assert_eq!(started.status, AiRaceStatus::Running);
        service.pause_race(race.id).unwrap();

        for id in [first.portfolio_id, second.portfolio_id] {
            let book = service.mcp.trading().book(Some(&id.to_string())).unwrap();
            assert_eq!(book.cash(), rust_decimal::Decimal::from(10_000));
            assert_eq!(book.transactions.len(), 1);
            assert!(service.save_config(service.config(id).unwrap()).is_err());
        }
        assert!(service
            .save_profile(
                Some(profile.id),
                "Changed",
                &profile.base_url,
                &profile.model,
                None,
            )
            .is_err());
        assert!(service.delete_profile(profile.id).is_err());
        assert!(service.start_run(first.portfolio_id).await.is_err());
        service.stop_race(race.id).unwrap();
        assert!(service
            .save_config(service.config(first.portfolio_id).unwrap())
            .is_ok());
    }

    #[tokio::test]
    async fn mcp_contestants_trade_only_inside_their_round_window() {
        use dtos::mcp::McpAccessPreset;
        let service = service();
        let profile = service
            .save_profile(None, "Model", "https://api.example.invalid/v1", "m", Some("k"))
            .unwrap();
        let trading = service.mcp.trading();
        let first = trading.start("Model racer", rust_decimal::Decimal::from(500)).await.unwrap();
        let second = trading.start("MCP racer", rust_decimal::Decimal::from(500)).await.unwrap();
        let (a, b) = (first.portfolio_id, second.portfolio_id);
        let bot = service
            .mcp
            .create_connection("Bot", McpAccessPreset::Trader, vec![b])
            .unwrap();
        let reader = service
            .mcp
            .create_connection("Reader", McpAccessPreset::ReadOnly, vec![b])
            .unwrap();

        let mut config = service.config(a).unwrap();
        config.profile_id = Some(profile.id);
        service.save_config(config).unwrap();
        let mut config = service.config(b).unwrap();
        config.profile_id = Some(profile.id);
        config.mcp_connection_id = Some(bot.connection.id);
        assert!(service.save_config(config.clone()).is_err(), "only one driver");
        config.profile_id = None;
        config.mcp_connection_id = Some(reader.connection.id);
        assert!(service.save_config(config.clone()).is_err(), "needs the Trader preset");
        let mut elsewhere = service.config(a).unwrap();
        elsewhere.profile_id = None;
        elsewhere.mcp_connection_id = Some(bot.connection.id);
        assert!(service.save_config(elsewhere).is_err(), "needs access to the portfolio");
        config.mcp_connection_id = Some(bot.connection.id);
        service.save_config(config).unwrap();
        assert!(service.start_run(b).await.is_err(), "MCP portfolios have no manual run");

        let race = service
            .create_race(NewAiRace {
                name: "Mixed".into(),
                contestant_ids: vec![a, b],
                starting_capital: 10_000.0,
                rounds: 3,
                trading_frequency_minutes: 1,
                round_timeout_seconds: 15,
            })
            .unwrap();
        assert_eq!(race.contestants.iter().filter(|c| c.mcp).count(), 1);
        service.start_race(race.id).unwrap();
        service.pause_race(race.id).unwrap();
        assert!(service
            .mcp
            .update_connection(bot.connection.id, "Bot", McpAccessPreset::Trader, true, vec![b])
            .is_err());
        assert!(service.mcp.delete_connection(bot.connection.id).is_err());

        let crate::mcp::services::Access::Granted(access) = service.mcp.authorize(Some(&bot.token)) else {
            panic!("the connection should authenticate")
        };
        let call = |id: i64, name: &str, arguments: Value| {
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})
        };
        let order = json!({"ticker":"NVDA","side":"buy","amount_usd":1000,"reason":"test"});
        let closed = service.mcp.handle_scoped(call(1, "place_order", order.clone()), &access).await.unwrap();
        assert_eq!(closed["result"]["isError"], true, "closed between rounds");

        let race = service.race(race.id).unwrap();
        let market = FrozenMarket::new();
        let run = service
            .open_mcp_window(&race, 1, b, &market, Duration::from_secs(15))
            .unwrap();
        let mine = service.mcp.handle_scoped(call(2, "get_my_portfolio", json!({})), &access).await.unwrap();
        assert!(mine["result"]["content"][0]["text"].as_str().unwrap().contains("\"window_open\": true"));
        let filled = service.mcp.handle_scoped(call(3, "place_order", order.clone()), &access).await.unwrap();
        assert_eq!(filled["result"]["isError"], false, "{filled}");
        assert!(market.snapshot().await["prices"]["NVDA"].is_object(), "filled at the frozen price");
        service.close_mcp_window(b, run).unwrap();
        let settled = service.run(run).unwrap();
        assert_eq!(settled.status, dtos::ai_models::AiRunStatus::Completed);
        assert!(settled.events.iter().any(|e| e.tool == "place_order" && e.success));
        let again = service.mcp.handle_scoped(call(4, "place_order", order.clone()), &access).await.unwrap();
        assert_eq!(again["result"]["isError"], true, "closed after the window");

        let idle = service
            .open_mcp_window(&race, 2, b, &FrozenMarket::new(), Duration::from_secs(15))
            .unwrap();
        service.close_mcp_window(b, idle).unwrap();
        assert_eq!(service.run(idle).unwrap().status, dtos::ai_models::AiRunStatus::Failed);

        service.stop_race(race.id).unwrap();
        let free = service.mcp.handle_scoped(call(5, "place_order", order), &access).await.unwrap();
        assert_eq!(free["result"]["isError"], false, "released when the race ends");
    }
}
