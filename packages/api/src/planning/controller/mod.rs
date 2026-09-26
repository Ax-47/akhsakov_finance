//! Server functions for target weights and goals. Each delegates to
//! [`PlanningService`](super::PlanningService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::{
    dca::DcaPlan,
    planning::{Goal, TargetWeight},
};
use rust_decimal::Decimal;
use uuid::Uuid;

#[cfg(feature = "server")]
use super::{DcaService, PlanningService};
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

/// Monthly investment plans, by day of the month.
#[get("/api/planning/dca", service: Extension<DcaService>)]
pub async fn get_dca_plans() -> Result<Vec<DcaPlan>, ServerFnError> {
    Ok(service.plans()?)
}

#[post("/api/planning/dca/save", service: Extension<DcaService>)]
pub async fn save_dca_plan(plan: DcaPlan) -> Result<(), ServerFnError> {
    Ok(service.save_plan(plan)?)
}

#[post("/api/planning/dca/delete", service: Extension<DcaService>)]
pub async fn delete_dca_plan(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_plan(id)?)
}

/// Records this month's purchase for plan `id` and marks it done.
#[post("/api/planning/dca/record", service: Extension<DcaService>)]
pub async fn record_dca(
    id: Uuid,
    date: String,
    shares: Decimal,
    price: Decimal,
    fee: Decimal,
) -> Result<(), ServerFnError> {
    Ok(service.record(id, date, shares, price, fee).await?)
}
