//! Authenticated server functions for model profiles and AI trader runs.

use dioxus::prelude::*;
use dtos::{
    ai_models::{
        AiNotifyLevel, AiRace, AiRaceRun, AiRun, AiStopState, ModelProfile, ModelProfileSettings, NewAiRace,
        NewAiTrader, TraderConfig, TraderMemory, TraderSchedule,
    },
    ai_portfolio::AiPortfolioInfo,
};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::ModelService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Which AI events notify the user.
#[get("/api/models/notify", service: Extension<ModelService>)]
pub async fn get_ai_notify_level() -> Result<AiNotifyLevel, ServerFnError> {
    Ok(service.notify_level()?)
}

#[post("/api/models/notify/set", service: Extension<ModelService>)]
pub async fn set_ai_notify_level(level: AiNotifyLevel) -> Result<AiNotifyLevel, ServerFnError> {
    Ok(service.set_notify_level(level)?)
}

/// Whether the emergency stop is on, and who flipped it lately.
#[get("/api/models/stop", service: Extension<ModelService>)]
pub async fn get_ai_stop() -> Result<AiStopState, ServerFnError> {
    Ok(service.ai_stop()?)
}

/// Turns the emergency stop for all AI trading on or off.
#[post(
    "/api/models/stop/set",
    service: Extension<ModelService>,
    auth: Extension<crate::auth::AuthService>,
    headers: dioxus::server::axum::http::HeaderMap
)]
pub async fn set_ai_stop(active: bool) -> Result<AiStopState, ServerFnError> {
    let actor = auth
        .user_for(crate::auth::bearer(&headers))?
        .unwrap_or_else(|| "local user".into());
    Ok(service.set_ai_stop(active, &actor)?)
}

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
    settings: ModelProfileSettings,
) -> Result<ModelProfile, ServerFnError> {
    Ok(service.save_profile_with(
        id,
        &name,
        &base_url,
        &model,
        api_key.as_deref(),
        Some(&settings),
    )?)
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

/// Creates an AI portfolio with its model connection and strategy.
#[post("/api/models/trader/create", service: Extension<ModelService>)]
pub async fn create_ai_trader(input: NewAiTrader) -> Result<AiPortfolioInfo, ServerFnError> {
    Ok(service.create_trader(input).await?)
}

#[post("/api/models/trader/config/save", service: Extension<ModelService>)]
pub async fn save_trader_config(config: TraderConfig) -> Result<TraderConfig, ServerFnError> {
    Ok(service.save_config(config)?)
}

#[post("/api/models/trader/schedule", service: Extension<ModelService>)]
pub async fn get_trader_schedule(portfolio_id: Uuid) -> Result<TraderSchedule, ServerFnError> {
    Ok(service.schedule(portfolio_id)?)
}

#[post("/api/models/trader/schedule/save", service: Extension<ModelService>)]
pub async fn save_trader_schedule(schedule: TraderSchedule) -> Result<TraderSchedule, ServerFnError> {
    Ok(service.save_schedule(schedule)?)
}

#[post("/api/models/trader/memory", service: Extension<ModelService>)]
pub async fn get_trader_memory(portfolio_id: Uuid) -> Result<TraderMemory, ServerFnError> {
    Ok(service.memory(portfolio_id)?)
}

#[post("/api/models/trader/memory/save", service: Extension<ModelService>)]
pub async fn save_trader_memory(
    portfolio_id: Uuid,
    decision_summary: String,
    unresolved_questions: String,
) -> Result<TraderMemory, ServerFnError> {
    Ok(service.save_memory(portfolio_id, &decision_summary, &unresolved_questions)?)
}

#[post("/api/models/trader/memory/clear", service: Extension<ModelService>)]
pub async fn clear_trader_memory(portfolio_id: Uuid) -> Result<TraderMemory, ServerFnError> {
    Ok(service.clear_memory(portfolio_id)?)
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

#[get("/api/models/races", service: Extension<ModelService>)]
pub async fn get_ai_races() -> Result<Vec<AiRace>, ServerFnError> {
    Ok(service.races()?)
}

#[post("/api/models/races/create", service: Extension<ModelService>)]
pub async fn create_ai_race(input: NewAiRace) -> Result<AiRace, ServerFnError> {
    Ok(service.create_race(input)?)
}

#[post("/api/models/races/get", service: Extension<ModelService>)]
pub async fn get_ai_race(id: Uuid) -> Result<AiRace, ServerFnError> {
    Ok(service.race(id)?)
}

#[post("/api/models/races/start", service: Extension<ModelService>)]
pub async fn start_ai_race(id: Uuid) -> Result<AiRace, ServerFnError> {
    Ok(service.start_race(id)?)
}

#[post("/api/models/races/pause", service: Extension<ModelService>)]
pub async fn pause_ai_race(id: Uuid) -> Result<AiRace, ServerFnError> {
    Ok(service.pause_race(id)?)
}

#[post("/api/models/races/resume", service: Extension<ModelService>)]
pub async fn resume_ai_race(id: Uuid) -> Result<AiRace, ServerFnError> {
    Ok(service.resume_race(id)?)
}

#[post("/api/models/races/stop", service: Extension<ModelService>)]
pub async fn stop_ai_race(id: Uuid) -> Result<AiRace, ServerFnError> {
    Ok(service.stop_race(id)?)
}

/// One contestant's runs in a race, by round, with every tool call.
#[post("/api/models/races/runs", service: Extension<ModelService>)]
pub async fn get_ai_race_runs(race_id: Uuid, portfolio_id: Uuid) -> Result<Vec<AiRaceRun>, ServerFnError> {
    Ok(service.race_runs(race_id, portfolio_id)?)
}

#[post("/api/models/races/results", service: Extension<ModelService>)]
pub async fn download_ai_race_results(id: Uuid) -> Result<String, ServerFnError> {
    Ok(service.race_csv(id)?)
}
