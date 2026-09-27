//! Provider-neutral model connections and audited paper-trading runs.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_TRADER_STRATEGY: &str = "Invest for the long run. Diversify, respect the investment thesis, keep enough cash for opportunities, and do not trade merely to be busy.";

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
    pub strategy: String,
}

impl TraderConfig {
    pub fn new(portfolio_id: Uuid) -> Self {
        Self {
            portfolio_id,
            profile_id: None,
            strategy: DEFAULT_TRADER_STRATEGY.into(),
        }
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
