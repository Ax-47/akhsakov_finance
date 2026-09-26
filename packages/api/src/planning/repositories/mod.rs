//! Storage port for target weights and goals.

use crate::shared::RepositoryError;
use dtos::planning::{Goal, TargetWeight};
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
