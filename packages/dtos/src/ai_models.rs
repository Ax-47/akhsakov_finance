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
}
