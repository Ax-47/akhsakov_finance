//! Server functions for target weights and goals. Each delegates to
//! [`PlanningService`](super::PlanningService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::planning::{Goal, TargetWeight};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::PlanningService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

#[post("/api/planning/targets", service: Extension<PlanningService>)]
pub async fn get_targets(portfolio_id: Uuid) -> Result<Vec<TargetWeight>, ServerFnError> {
    Ok(service.targets(portfolio_id)?)
}

#[post("/api/planning/targets/save", service: Extension<PlanningService>)]
pub async fn set_targets(
    portfolio_id: Uuid,
    targets: Vec<TargetWeight>,
) -> Result<(), ServerFnError> {
    Ok(service.set_targets(portfolio_id, targets)?)
}

#[get("/api/planning/goals", service: Extension<PlanningService>)]
pub async fn get_goals() -> Result<Vec<Goal>, ServerFnError> {
    Ok(service.goals()?)
}

#[post("/api/planning/goals/save", service: Extension<PlanningService>)]
pub async fn save_goal(goal: Goal) -> Result<(), ServerFnError> {
    Ok(service.save_goal(goal)?)
}

#[post("/api/planning/goals/delete", service: Extension<PlanningService>)]
pub async fn delete_goal(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_goal(id)?)
}
