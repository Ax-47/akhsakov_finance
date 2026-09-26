//! Storage port for target weights and goals.

use crate::shared::RepositoryError;
use dtos::{
    dca::DcaPlan,
    planning::{Goal, TargetWeight},
    Transaction,
};
use uuid::Uuid;

pub trait PlanningRepository: Send + Sync {
    fn targets(&self, portfolio_id: Uuid) -> Result<Vec<TargetWeight>, RepositoryError>;
    /// Replaces all of the portfolio's targets.
    fn replace_targets(
        &self,
        portfolio_id: Uuid,
        targets: &[TargetWeight],
    ) -> Result<(), RepositoryError>;

    fn goals(&self) -> Result<Vec<Goal>, RepositoryError>;
    /// Inserts, or replaces the goal with the same id.
    fn save_goal(&self, goal: &Goal) -> Result<(), RepositoryError>;
    fn delete_goal(&self, id: Uuid) -> Result<(), RepositoryError>;
}

pub trait DcaRepository: Send + Sync {
    /// Ordered by day of the month.
    fn plans(&self) -> Result<Vec<DcaPlan>, RepositoryError>;
    /// Inserts, or replaces the plan with the same id (keeping when it
    /// last reminded you).
    fn save_plan(&self, plan: &DcaPlan) -> Result<(), RepositoryError>;
    fn delete_plan(&self, id: Uuid) -> Result<(), RepositoryError>;
    fn mark_done(&self, id: Uuid, month: &str) -> Result<(), RepositoryError>;
    /// `YYYY-MM` of the last reminder sent.
    fn last_reminded(&self, id: Uuid) -> Result<Option<String>, RepositoryError>;
    fn mark_reminded(&self, id: Uuid, month: &str) -> Result<(), RepositoryError>;
}

/// Records a purchase in a portfolio (the portfolio context).
#[async_trait::async_trait]
pub trait TradeRecorder: Send + Sync {
    async fn record(&self, tx: Transaction) -> Result<(), String>;
}

/// Sends a reminder (the notifications context).
#[async_trait::async_trait]
pub trait Reminder: Send + Sync {
    async fn remind(&self, title: &str, body: &str) -> Result<(), String>;
}
