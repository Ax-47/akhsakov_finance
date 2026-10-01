use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAccessPreset {
    ReadOnly,
    ThesisEditor,
    Trader,
}

impl McpAccessPreset {
    pub fn key(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::ThesisEditor => "thesis_editor",
            Self::Trader => "trader",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPortfolioScope {
    pub id: Uuid,
    pub name: String,
    pub ai: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConnection {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub preset: McpAccessPreset,
    pub portfolio_ids: Vec<Uuid>,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConnectionSecret {
    pub connection: McpConnection,
    /// Returned only by create and rotate. It is never persisted.
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpAuditEvent {
    pub id: i64,
    pub at: String,
    pub method: String,
    pub tool: Option<String>,
    pub portfolio_ids: Vec<Uuid>,
    pub success: bool,
    pub error_category: Option<String>,
}
