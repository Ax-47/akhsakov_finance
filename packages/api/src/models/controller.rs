//! Authenticated server functions for model profiles and AI trader runs.

use dioxus::prelude::*;
use dtos::ai_models::{AiRun, ModelProfile, TraderConfig};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::ModelService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

#[get("/api/models/profiles", service: Extension<ModelService>)]
pub async fn get_model_profiles() -> Result<Vec<ModelProfile>, ServerFnError> {
    Ok(service.profiles()?)
}

#[post("/api/models/profiles/save", service: Extension<ModelService>)]
pub async fn save_model_profile(
    id: Option<Uuid>,
    name: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
) -> Result<ModelProfile, ServerFnError> {
    Ok(service.save_profile(id, &name, &base_url, &model, api_key.as_deref())?)
}

#[post("/api/models/profiles/delete", service: Extension<ModelService>)]
pub async fn delete_model_profile(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.delete_profile(id)?)
}

#[post("/api/models/profiles/test", service: Extension<ModelService>)]
pub async fn test_model_profile(id: Uuid) -> Result<(), ServerFnError> {
    Ok(service.test_profile(id).await?)
}

#[post("/api/models/trader/config", service: Extension<ModelService>)]
pub async fn get_trader_config(portfolio_id: Uuid) -> Result<TraderConfig, ServerFnError> {
    Ok(service.config(portfolio_id)?)
}

#[post("/api/models/trader/config/save", service: Extension<ModelService>)]
pub async fn save_trader_config(config: TraderConfig) -> Result<TraderConfig, ServerFnError> {
    Ok(service.save_config(config)?)
}

#[post("/api/models/trader/run", service: Extension<ModelService>)]
pub async fn start_ai_trader_run(portfolio_id: Uuid) -> Result<AiRun, ServerFnError> {
    Ok(service.start_run(portfolio_id).await?)
}

#[post("/api/models/trader/runs", service: Extension<ModelService>)]
pub async fn get_ai_trader_runs(portfolio_id: Uuid) -> Result<Vec<AiRun>, ServerFnError> {
    Ok(service.runs(portfolio_id)?)
}

#[post("/api/models/trader/run/get", service: Extension<ModelService>)]
pub async fn get_ai_trader_run(id: Uuid) -> Result<AiRun, ServerFnError> {
    Ok(service.run(id)?)
}
