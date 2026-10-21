//! Tells the user about AI runs and races through the app's notifications,
//! as much as they asked for: a failed run, a run that traded, a race
//! round where contestants failed, a race that finished or was paused.

use super::ModelService;
use crate::{notifications::NotificationService, shared::ServiceError};
use dtos::ai_models::{AiNotifyLevel, AiRunStatus};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

/// The longest a notification's text gets, in characters.
const BODY_CHARS: usize = 300;
const STOPPED: &str = "Stopped by the emergency stop.";

fn storage(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Storage(e.to_string())
}

/// The first line of `text`, at most `BODY_CHARS` long.
fn short(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    if line.chars().count() <= BODY_CHARS {
        line.to_string()
    } else {
        let cut: String = line.chars().take(BODY_CHARS - 1).collect();
        format!("{cut}…")
    }
}

impl ModelService {
    pub fn with_notifications(mut self, notifications: NotificationService) -> Self {
        self.notifications = Some(notifications);
        for id in self.paused_on_start.clone().iter() {
            self.announce_race_paused(*id, "The app restarted during the race; resume it when you're ready.");
        }
        self
    }

    pub fn notify_level(&self) -> Result<AiNotifyLevel, ServiceError> {
        let stored: Option<String> = self
            .db
            .with(|c| {
                c.query_row("SELECT value FROM ai_preferences WHERE key = 'notify'", [], |r| r.get(0))
                    .optional()
            })
            .map_err(storage)?;
        Ok(match stored.as_deref() {
            Some("off") => AiNotifyLevel::Off,
            Some("all") => AiNotifyLevel::All,
            _ => AiNotifyLevel::Failures,
        })
    }

    pub fn set_notify_level(&self, level: AiNotifyLevel) -> Result<AiNotifyLevel, ServiceError> {
        let value = match level {
            AiNotifyLevel::Off => "off",
            AiNotifyLevel::Failures => "failures",
            AiNotifyLevel::All => "all",
        };
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_preferences (key, value) VALUES ('notify', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![value],
                )
            })
            .map_err(storage)?;
        self.notify_level()
    }

    /// Sends a notification in the background if the user's level takes it.
    fn announce(&self, failure: bool, title: String, body: String) {
        let Some(notifications) = self.notifications.clone() else { return };
        let wanted = match self.notify_level().unwrap_or_default() {
            AiNotifyLevel::Off => false,
            AiNotifyLevel::Failures => failure,
            AiNotifyLevel::All => true,
        };
        if wanted {
            tokio::spawn(async move {
                if let Err(e) = notifications.notify(&title, &body).await {
                    tracing::warn!("couldn't record an AI notification: {e}");
                }
            });
        }
    }

    fn portfolio_name(&self, id: Uuid) -> String {
        self.mcp
            .trading()
            .book(Some(&id.to_string()))
            .map(|b| b.name)
            .unwrap_or_else(|_| "AI portfolio".into())
    }

    /// A finished run outside a race: its failure, or the trades it made.
    /// Race runs are summed up per round instead.
    pub(super) fn announce_run(&self, id: Uuid) {
        if self.notifications.is_none() {
            return;
        }
        let Ok(run) = self.run(id) else { return };
        let in_race = self
            .db
            .with(|c| {
                c.query_row("SELECT race_id IS NOT NULL FROM ai_runs WHERE id = ?1", [id.to_string()], |r| {
                    r.get::<_, bool>(0)
                })
            })
            .unwrap_or(true);
        if in_race {
            return;
        }
        let name = self.portfolio_name(run.portfolio_id);
        match run.status {
            AiRunStatus::Failed => {
                let error = run.error.unwrap_or_default();
                // The user stopped it themselves.
                if error != STOPPED {
                    self.announce(true, format!("🤖 AI run failed: {name}"), short(&error));
                }
            }
            AiRunStatus::Completed => {
                let trades = run.events.iter().filter(|e| e.tool == "place_order" && e.success).count();
                if trades > 0 {
                    let summary = short(run.final_response.as_deref().unwrap_or_default());
                    let count = if trades == 1 { "1 trade".to_string() } else { format!("{trades} trades") };
                    self.announce(false, format!("🤖 AI run traded: {name}"), format!("{count}. {summary}"));
                }
            }
            AiRunStatus::Running => {}
        }
    }

    /// One message per race round in which contestants failed.
    pub(super) fn announce_round(&self, race_id: Uuid, round: u32) {
        if self.notifications.is_none() {
            return;
        }
        let Ok((failed, total)) = self.db.with(|c| {
            c.query_row(
                "SELECT COALESCE(SUM(status = 'failed'), 0), COUNT(*) FROM ai_runs WHERE race_id = ?1 AND race_round = ?2",
                params![race_id.to_string(), round],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )
        }) else {
            return;
        };
        if failed == 0 {
            return;
        }
        let name = self.race(race_id).map(|r| r.name).unwrap_or_default();
        self.announce(
            true,
            format!("🏁 AI race {name}: round {round}"),
            format!("{failed} of {total} contestants failed this round. The race goes on."),
        );
    }

    pub(super) fn announce_race_end(&self, race_id: Uuid) {
        let Ok(race) = self.race(race_id) else { return };
        let body = match race.winner() {
            Some(winner) => format!("{} won with {:+.2}%.", winner.name, winner.total_return_pct),
            None => "It finished without a result.".to_string(),
        };
        self.announce(false, format!("🏁 AI race finished: {}", race.name), body);
    }

    pub(super) fn announce_race_paused(&self, race_id: Uuid, why: &str) {
        let name = self.race(race_id).map(|r| r.name).unwrap_or_default();
        self.announce(true, format!("🏁 AI race paused: {name}"), short(why));
    }
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn notification_text_is_one_short_line() {
        assert_eq!(short("\n  First line.\nSecond."), "First line.");
        let long = "x".repeat(400);
        assert_eq!(short(&long).chars().count(), 300);
        assert!(short(&long).ends_with('…'));
    }
}
