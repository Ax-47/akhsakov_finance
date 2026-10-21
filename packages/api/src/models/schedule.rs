//! Scheduled trader runs: an AI portfolio can run on its own every few
//! hours, on chosen weekdays, and only while a market is open if asked.

use super::ModelService;
use crate::shared::ServiceError;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, SecondsFormat, TimeZone, Utc};
use dtos::ai_models::{MarketHours, TraderSchedule, AI_STOPPED_MESSAGE};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

const MAX_EVERY_HOURS: u32 = 168;
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(60);
/// How far ahead to look for a time the schedule allows.
const SEARCH_DAYS: i64 = 8;
const STEP_MINUTES: i64 = 15;

fn storage(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Storage(e.to_string())
}

fn invalid(message: impl Into<String>) -> ServiceError {
    ServiceError::Validation(message.into())
}

fn stamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn market_name(hours: MarketHours) -> &'static str {
    match hours {
        MarketHours::Any => "any",
        MarketHours::Us => "us",
        MarketHours::Thai => "thai",
    }
}

fn parse_market(value: &str) -> MarketHours {
    match value {
        "us" => MarketHours::Us,
        "thai" => MarketHours::Thai,
        _ => MarketHours::Any,
    }
}

/// The `n`th Sunday of a month.
fn nth_sunday(year: i32, month: u32, n: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("a valid month");
    let to_sunday = (7 - first.weekday().num_days_from_sunday()) % 7;
    first + Duration::days(i64::from(to_sunday + 7 * (n - 1)))
}

/// New York's offset from UTC: daylight time runs from 2:00 on the second
/// Sunday in March to 2:00 on the first Sunday in November.
fn new_york_offset(t: DateTime<Utc>) -> Duration {
    let year = t.year();
    let starts = Utc.from_utc_datetime(&nth_sunday(year, 3, 2).and_hms_opt(7, 0, 0).expect("a time"));
    let ends = Utc.from_utc_datetime(&nth_sunday(year, 11, 1).and_hms_opt(6, 0, 0).expect("a time"));
    if t >= starts && t < ends {
        Duration::hours(-4)
    } else {
        Duration::hours(-5)
    }
}

/// Whether the schedule lets a run start at `t`.
fn allowed(t: DateTime<Utc>, schedule: &TraderSchedule) -> bool {
    let (local, open) = match schedule.market_hours {
        MarketHours::Any => (t.with_timezone(&chrono::Local).naive_local(), None),
        MarketHours::Us => (
            t.naive_utc() + new_york_offset(t),
            Some((NaiveTime::from_hms_opt(9, 30, 0), NaiveTime::from_hms_opt(16, 0, 0))),
        ),
        MarketHours::Thai => (
            t.naive_utc() + Duration::hours(7),
            Some((NaiveTime::from_hms_opt(10, 0, 0), NaiveTime::from_hms_opt(16, 30, 0))),
        ),
    };
    let weekday = local.weekday().number_from_monday() as u8;
    if !schedule.weekdays.contains(&weekday) {
        return false;
    }
    match open {
        None => true,
        Some((Some(from), Some(to))) => weekday <= 5 && local.time() >= from && local.time() < to,
        Some(_) => false,
    }
}

/// The first time at least `every_hours` after `after` that the schedule
/// allows, to the quarter hour; `None` if nothing in the next week does.
pub(super) fn next_run(after: DateTime<Utc>, schedule: &TraderSchedule) -> Option<DateTime<Utc>> {
    let mut t = after + Duration::hours(i64::from(schedule.every_hours));
    for _ in 0..SEARCH_DAYS * 24 * 60 / STEP_MINUTES {
        if allowed(t, schedule) {
            return Some(t);
        }
        t += Duration::minutes(STEP_MINUTES);
    }
    None
}

impl ModelService {
    pub fn schedule(&self, portfolio_id: Uuid) -> Result<TraderSchedule, ServiceError> {
        self.config(portfolio_id)?;
        let stored = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT enabled, every_hours, weekdays, market_hours, next_run_at, last_run_at, last_note
                     FROM ai_trader_schedules WHERE portfolio_id = ?1",
                    [portfolio_id.to_string()],
                    |r| {
                        let weekdays: String = r.get(2)?;
                        let market: String = r.get(3)?;
                        Ok(TraderSchedule {
                            portfolio_id,
                            enabled: r.get(0)?,
                            every_hours: r.get(1)?,
                            weekdays: weekdays.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect(),
                            market_hours: parse_market(&market),
                            next_run_at: r.get(4)?,
                            last_run_at: r.get(5)?,
                            last_note: r.get(6)?,
                        })
                    },
                )
                .optional()
            })
            .map_err(storage)?;
        Ok(stored.unwrap_or_else(|| TraderSchedule::new(portfolio_id)))
    }

    /// Saves a schedule; an enabled one gets its next run from now.
    pub fn save_schedule(&self, schedule: TraderSchedule) -> Result<TraderSchedule, ServiceError> {
        let config = self.config(schedule.portfolio_id)?;
        if !(1..=MAX_EVERY_HOURS).contains(&schedule.every_hours) {
            return Err(invalid(format!("Run every 1 to {MAX_EVERY_HOURS} hours.")));
        }
        let mut weekdays = schedule.weekdays.clone();
        weekdays.sort_unstable();
        weekdays.dedup();
        if weekdays.is_empty() || weekdays.iter().any(|d| !(1..=7).contains(d)) {
            return Err(invalid("Choose at least one day of the week."));
        }
        let schedule = TraderSchedule { weekdays, ..schedule };
        let next_run_at = if schedule.enabled {
            if config.profile_id.is_none() || config.mcp_connection_id.is_some() {
                return Err(invalid(
                    "Only portfolios run by a model profile can run on a schedule; MCP clients trade when they connect.",
                ));
            }
            let next = next_run(Utc::now(), &schedule)
                .ok_or_else(|| invalid("No time in the next week fits this schedule."))?;
            Some(stamp(next))
        } else {
            None
        };
        let days: String = schedule.weekdays.iter().map(|d| char::from(b'0' + d)).collect();
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_trader_schedules (portfolio_id, enabled, every_hours, weekdays, market_hours, next_run_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(portfolio_id) DO UPDATE SET enabled=excluded.enabled, every_hours=excluded.every_hours,
                     weekdays=excluded.weekdays, market_hours=excluded.market_hours, next_run_at=excluded.next_run_at",
                    params![
                        schedule.portfolio_id.to_string(),
                        schedule.enabled,
                        schedule.every_hours,
                        days,
                        market_name(schedule.market_hours),
                        next_run_at
                    ],
                )
            })
            .map_err(storage)?;
        self.schedule(schedule.portfolio_id)
    }

    /// Starts every scheduled run due at `now` and moves each schedule on to
    /// its next slot first, so slots missed while the app was off run once
    /// and a restart never repeats one. Returns how many runs started.
    pub(super) async fn run_due_schedules(&self, now: DateTime<Utc>) -> Result<usize, ServiceError> {
        let due: Vec<String> = self
            .db
            .with(|c| {
                c.prepare(
                    "SELECT portfolio_id FROM ai_trader_schedules
                     WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= ?1",
                )?
                .query_map([stamp(now)], |r| r.get(0))?
                .collect()
            })
            .map_err(storage)?;
        let mut started = 0;
        for id in due {
            let Ok(portfolio) = Uuid::parse_str(&id) else { continue };
            let schedule = self.schedule(portfolio)?;
            let next = next_run(now, &schedule).map(stamp);
            let claimed = self
                .db
                .with(|c| {
                    c.execute(
                        "UPDATE ai_trader_schedules SET next_run_at = ?3 WHERE portfolio_id = ?1 AND next_run_at = ?2",
                        params![id, schedule.next_run_at, next],
                    )
                })
                .map_err(storage)?;
            if claimed == 0 {
                continue;
            }
            let note = if *self.stop.borrow() {
                Some(format!("Skipped: {AI_STOPPED_MESSAGE}"))
            } else if self.race_active_for(portfolio)? {
                Some("Skipped: this portfolio is in an active race.".to_string())
            } else {
                match self.start_run(portfolio).await {
                    Ok(_) => {
                        started += 1;
                        None
                    }
                    Err(e) => Some(format!("Couldn't start: {e}")),
                }
            };
            let ran = note.is_none().then(|| stamp(now));
            self.db
                .with(|c| {
                    c.execute(
                        "UPDATE ai_trader_schedules SET last_note = ?2, last_run_at = COALESCE(?3, last_run_at)
                         WHERE portfolio_id = ?1",
                        params![id, note, ran],
                    )
                })
                .map_err(storage)?;
        }
        Ok(started)
    }

    /// Checks for due scheduled runs every minute, forever.
    pub fn spawn_scheduler(self) {
        tokio::spawn(async move {
            loop {
                match self.run_due_schedules(Utc::now()).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!("{n} scheduled trader run(s) started"),
                    Err(e) => tracing::warn!("scheduled trader runs failed: {e}"),
                }
                tokio::time::sleep(CHECK_EVERY).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn schedule(every_hours: u32, market_hours: MarketHours) -> TraderSchedule {
        TraderSchedule { enabled: true, every_hours, market_hours, ..TraderSchedule::new(Uuid::nil()) }
    }

    #[test]
    fn next_runs_follow_the_hours_days_and_market() {
        let any = schedule(6, MarketHours::Any);
        assert_eq!(next_run(at("2026-10-21T05:00:00Z"), &any), Some(at("2026-10-21T11:00:00Z")));

        // Friday 16:00 in New York (daylight time) is closed until Monday 9:30.
        let us = schedule(1, MarketHours::Us);
        assert_eq!(next_run(at("2026-10-23T20:00:00Z"), &us), Some(at("2026-10-26T13:30:00Z")));
        // In January New York is UTC-5.
        assert_eq!(next_run(at("2027-01-12T12:00:00Z"), &us), Some(at("2027-01-12T14:30:00Z")));
        // Bangkok is UTC+7: 16:00 there plus an hour is past the close.
        let thai = schedule(1, MarketHours::Thai);
        assert_eq!(next_run(at("2026-10-21T09:00:00Z"), &thai), Some(at("2026-10-22T03:00:00Z")));

        // Only Mondays, in Bangkok: from a Tuesday, the next Monday's open.
        let mondays = TraderSchedule { weekdays: vec![1], ..thai.clone() };
        assert_eq!(next_run(at("2026-10-20T03:00:00Z"), &mondays), Some(at("2026-10-26T03:00:00Z")));
    }

    #[test]
    fn new_york_changes_clocks_on_the_right_sundays() {
        assert_eq!(nth_sunday(2026, 3, 2), NaiveDate::from_ymd_opt(2026, 3, 8).unwrap());
        assert_eq!(nth_sunday(2026, 11, 1), NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(new_york_offset(at("2026-03-08T06:59:00Z")), Duration::hours(-5));
        assert_eq!(new_york_offset(at("2026-03-08T07:00:00Z")), Duration::hours(-4));
        assert_eq!(new_york_offset(at("2026-11-01T06:00:00Z")), Duration::hours(-5));
    }
}
