//! Provider-neutral model connections and audited paper-trading runs.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_TRADER_STRATEGY: &str = "Invest for the long run. Diversify, respect the investment thesis, keep enough cash for opportunities, and do not trade merely to be busy.";
pub const DEFAULT_MEMORY_CHAR_LIMIT: u32 = 8_000;
pub const DEFAULT_CONTEXT_TOKEN_LIMIT: u32 = 32_768;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraderConfig {
    pub portfolio_id: Uuid,
    pub profile_id: Option<Uuid>,
    /// An MCP connection that drives this portfolio instead of a model
    /// profile. At most one of `profile_id` and this is set.
    #[serde(default)]
    pub mcp_connection_id: Option<Uuid>,
    pub strategy: String,
    pub memory_char_limit: u32,
    pub context_token_limit: u32,
}

impl TraderConfig {
    pub fn new(portfolio_id: Uuid) -> Self {
        Self {
            portfolio_id,
            profile_id: None,
            mcp_connection_id: None,
            strategy: DEFAULT_TRADER_STRATEGY.into(),
            memory_char_limit: DEFAULT_MEMORY_CHAR_LIMIT,
            context_token_limit: DEFAULT_CONTEXT_TOKEN_LIMIT,
        }
    }
}

/// A model connection typed in while creating an AI portfolio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewModelConnection {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

/// Everything needed to set up an AI portfolio in one step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewAiTrader {
    /// Blank picks "AI", "AI 2", …
    pub name: String,
    pub starting_cash: rust_decimal::Decimal,
    /// An existing connection; ignored when `connection` is given.
    pub profile_id: Option<Uuid>,
    /// A new connection to save and use.
    pub connection: Option<NewModelConnection>,
    pub strategy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraderMemory {
    pub portfolio_id: Uuid,
    pub decision_summary: String,
    pub unresolved_questions: String,
    pub updated_at: Option<String>,
    pub source_run_id: Option<Uuid>,
}

impl TraderMemory {
    pub fn empty(portfolio_id: Uuid) -> Self {
        Self {
            portfolio_id,
            decision_summary: String::new(),
            unresolved_questions: String::new(),
            updated_at: None,
            source_run_id: None,
        }
    }

    pub fn used_chars(&self) -> usize {
        self.decision_summary.chars().count() + self.unresolved_questions.chars().count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiRunStatus {
    Running,
    Completed,
    Failed,
}

impl AiRunStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRunEvent {
    pub sequence: i64,
    pub tool: String,
    pub arguments: String,
    pub success: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRun {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub status: AiRunStatus,
    pub profile_name: String,
    pub model: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub final_response: Option<String>,
    pub error: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub events: Vec<AiRunEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiRaceStatus {
    Draft,
    Running,
    Paused,
    Completed,
    Stopped,
}

impl AiRaceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Stopped => "Stopped",
        }
    }
}

/// Configuration accepted when a race is created. Contestants must be AI
/// portfolios with a configured model connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewAiRace {
    pub name: String,
    pub contestant_ids: Vec<Uuid>,
    pub starting_capital: f64,
    pub rounds: u32,
    pub trading_frequency_minutes: u32,
    pub round_timeout_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRaceContestant {
    pub portfolio_id: Uuid,
    pub name: String,
    /// True when an external MCP client trades this contestant.
    #[serde(default)]
    pub mcp: bool,
    pub profile_name: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRaceStanding {
    pub rank: u32,
    pub portfolio_id: Uuid,
    pub name: String,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub volatility_pct: f64,
    pub risk_adjusted_return: f64,
    pub fees: f64,
    pub turnover: f64,
    pub cash_allocation_pct: f64,
    pub failed_model_runs: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRaceAuditEvent {
    pub sequence: i64,
    pub at: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRace {
    pub id: Uuid,
    pub name: String,
    pub status: AiRaceStatus,
    pub starting_capital: f64,
    pub rounds: u32,
    pub completed_rounds: u32,
    pub trading_frequency_minutes: u32,
    pub round_timeout_seconds: u32,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub contestants: Vec<AiRaceContestant>,
    pub leaderboard: Vec<AiRaceStanding>,
    pub audit: Vec<AiRaceAuditEvent>,
    /// Every contestant's value at the end of each completed round.
    #[serde(default)]
    pub history: Vec<AiRacePoint>,
    /// The app's benchmark over the same rounds, if its price was recorded.
    #[serde(default)]
    pub benchmark: Option<AiRaceBenchmark>,
}

impl AiRace {
    /// The leader once the race is over; `None` while it runs or if
    /// nobody has a result.
    pub fn winner(&self) -> Option<&AiRaceStanding> {
        matches!(self.status, AiRaceStatus::Completed | AiRaceStatus::Stopped)
            .then(|| self.leaderboard.first())
            .flatten()
            .filter(|_| self.completed_rounds > 0)
    }

    /// The benchmark's return (%) from the start to the last completed round.
    pub fn benchmark_return_pct(&self) -> Option<f64> {
        let points = &self.benchmark.as_ref()?.points;
        let (_, last) = points.last()?;
        (points.len() > 1 && self.starting_capital > 0.0)
            .then(|| (last / self.starting_capital - 1.0) * 100.0)
    }

    /// Values by round for one contestant, starting from the capital.
    pub fn values_of(&self, portfolio_id: Uuid) -> Vec<(u32, f64)> {
        let mut out = vec![(0, self.starting_capital)];
        out.extend(
            self.history
                .iter()
                .filter(|p| p.portfolio_id == portfolio_id)
                .map(|p| (p.round, p.value)),
        );
        out
    }
}

/// A contestant's value (USD) at the end of a round.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRacePoint {
    pub round: u32,
    pub portfolio_id: Uuid,
    pub value: f64,
}

/// What the starting capital would be worth in the benchmark at the end of
/// each round; round 0 is the start of the race.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRaceBenchmark {
    pub ticker: String,
    pub points: Vec<(u32, f64)>,
}

/// One contestant's run in one round of a race.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRaceRun {
    pub round: u32,
    pub run: AiRun,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn race(status: AiRaceStatus, rounds_done: u32) -> AiRace {
        let a = Uuid::from_u128(1);
        AiRace {
            id: Uuid::nil(),
            name: "Test".into(),
            status,
            starting_capital: 1_000.0,
            rounds: 3,
            completed_rounds: rounds_done,
            trading_frequency_minutes: 1,
            round_timeout_seconds: 60,
            created_at: String::new(),
            started_at: None,
            finished_at: None,
            contestants: vec![],
            leaderboard: vec![AiRaceStanding {
                rank: 1,
                portfolio_id: a,
                name: "A".into(),
                total_return_pct: 5.0,
                max_drawdown_pct: 0.0,
                volatility_pct: 0.0,
                risk_adjusted_return: 0.0,
                fees: 0.0,
                turnover: 0.0,
                cash_allocation_pct: 0.0,
                failed_model_runs: 0,
            }],
            audit: vec![],
            history: vec![
                AiRacePoint { round: 1, portfolio_id: a, value: 1_020.0 },
                AiRacePoint { round: 1, portfolio_id: Uuid::from_u128(2), value: 990.0 },
                AiRacePoint { round: 2, portfolio_id: a, value: 1_050.0 },
            ],
            benchmark: Some(AiRaceBenchmark { ticker: "SPY".into(), points: vec![(0, 1_000.0), (1, 1_010.0), (2, 1_030.0)] }),
        }
    }

    #[test]
    fn winner_only_once_the_race_is_over() {
        assert!(race(AiRaceStatus::Running, 2).winner().is_none());
        assert!(race(AiRaceStatus::Stopped, 0).winner().is_none(), "no round, no winner");
        assert_eq!(race(AiRaceStatus::Completed, 3).winner().unwrap().name, "A");
    }

    #[test]
    fn values_start_from_the_capital() {
        let r = race(AiRaceStatus::Running, 2);
        assert_eq!(r.values_of(Uuid::from_u128(1)), vec![(0, 1_000.0), (1, 1_020.0), (2, 1_050.0)]);
        assert_eq!(r.values_of(Uuid::from_u128(9)), vec![(0, 1_000.0)]);
    }

    #[test]
    fn benchmark_return_needs_two_points() {
        let r = race(AiRaceStatus::Running, 2);
        assert!((r.benchmark_return_pct().unwrap() - 3.0).abs() < 1e-9);
        let mut only_start = r.clone();
        only_start.benchmark.as_mut().unwrap().points.truncate(1);
        assert_eq!(only_start.benchmark_return_pct(), None);
    }
}
