//! The emergency stop: one switch that ends every AI trader run, pauses
//! every race and refuses every AI order until it is turned off.

use super::ModelService;
use crate::shared::ServiceError;
use dtos::ai_models::{AiRaceStatus, AiStopEvent, AiStopState, AI_STOPPED_MESSAGE};
use rusqlite::params;

const HISTORY: i64 = 10;

fn storage(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Storage(e.to_string())
}

impl ModelService {
    pub fn ai_stop(&self) -> Result<AiStopState, ServiceError> {
        let history = self
            .db
            .with(|c| {
                c.prepare("SELECT active, actor, at FROM ai_stop_log ORDER BY id DESC LIMIT ?1")?
                    .query_map([HISTORY], |r| {
                        Ok(AiStopEvent { active: r.get(0)?, actor: r.get(1)?, at: r.get(2)? })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        Ok(AiStopState {
            active: history.first().is_some_and(|e| e.active),
            history,
        })
    }

    /// Turns the emergency stop on or off. Turning it on fails every
    /// running trader run and pauses every running race; races stay
    /// paused when it is turned off again.
    pub fn set_ai_stop(&self, active: bool, actor: &str) -> Result<AiStopState, ServiceError> {
        if self.ai_stop()?.active == active {
            return self.ai_stop();
        }
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_stop_log (active, actor) VALUES (?1, ?2)",
                    params![active, actor.trim()],
                )
            })
            .map_err(storage)?;
        self.stop.send_replace(active);
        if active {
            self.db
                .with(|c| {
                    c.execute(
                        "UPDATE ai_runs SET status='failed', finished_at=datetime('now'),
                         error='Stopped by the emergency stop.' WHERE status='running'",
                        [],
                    )
                })
                .map_err(storage)?;
            for race in self.races()? {
                if race.status == AiRaceStatus::Running {
                    self.pause_race(race.id)?;
                }
            }
        }
        self.ai_stop()
    }

    pub(super) fn ensure_not_halted(&self) -> Result<(), ServiceError> {
        if *self.stop.borrow() {
            return Err(ServiceError::Validation(AI_STOPPED_MESSAGE.into()));
        }
        Ok(())
    }
}
