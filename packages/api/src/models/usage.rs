//! Token use and estimated cost of AI runs, from the prices the user
//! entered on each model profile. Nothing here knows any provider's prices.

use super::ModelService;
use crate::shared::ServiceError;
use dtos::ai_models::AiUsageRow;
use uuid::Uuid;

fn storage(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Storage(e.to_string())
}

impl ModelService {
    /// The portfolio's estimated model cost since the start of this
    /// calendar month (UTC), in USD.
    pub(super) fn month_cost(&self, portfolio_id: Uuid) -> Result<f64, ServiceError> {
        self.db
            .with(|c| {
                c.query_row(
                    "SELECT COALESCE(SUM(cost_usd), 0) FROM ai_runs
                     WHERE portfolio_id = ?1 AND started_at >= datetime('now', 'start of month')",
                    [portfolio_id.to_string()],
                    |r| r.get(0),
                )
            })
            .map_err(storage)
    }

    /// The estimated model cost of a contestant's runs in a race, if any
    /// of them was priced.
    pub(super) fn race_cost(&self, race_id: Uuid, portfolio_id: Uuid) -> Result<Option<f64>, ServiceError> {
        self.db
            .with(|c| {
                c.query_row(
                    "SELECT SUM(cost_usd) FROM ai_runs WHERE race_id = ?1 AND portfolio_id = ?2",
                    [race_id.to_string(), portfolio_id.to_string()],
                    |r| r.get(0),
                )
            })
            .map_err(storage)
    }

    /// Tokens and estimated cost per AI portfolio, per model profile and
    /// per race, over 7 days, 30 days and all time.
    pub fn usage(&self) -> Result<Vec<AiUsageRow>, ServiceError> {
        let groups = [
            ("portfolio", "COALESCE(p.name, 'Removed portfolio')", "r.portfolio_id"),
            ("profile", "r.profile_name", "r.profile_name"),
            ("race", "COALESCE(a.name, 'Removed race')", "r.race_id"),
        ];
        let mut rows = Vec::new();
        for (kind, name, key) in groups {
            let race_only = if kind == "race" { "WHERE r.race_id IS NOT NULL" } else { "" };
            let sql = format!(
                "SELECT {name},
                    COALESCE(SUM(CASE WHEN r.started_at >= datetime('now', '-7 days') THEN r.total_tokens END), 0),
                    COALESCE(SUM(CASE WHEN r.started_at >= datetime('now', '-7 days') THEN r.cost_usd END), 0),
                    COALESCE(SUM(CASE WHEN r.started_at >= datetime('now', '-30 days') THEN r.total_tokens END), 0),
                    COALESCE(SUM(CASE WHEN r.started_at >= datetime('now', '-30 days') THEN r.cost_usd END), 0),
                    COALESCE(SUM(r.total_tokens), 0),
                    COALESCE(SUM(r.cost_usd), 0),
                    SUM(r.cost_usd IS NULL AND r.status != 'running')
                 FROM ai_runs r
                 LEFT JOIN portfolios p ON p.id = r.portfolio_id
                 LEFT JOIN ai_races a ON a.id = r.race_id
                 {race_only}
                 GROUP BY {key}
                 ORDER BY 7 DESC, 6 DESC"
            );
            let found: Vec<AiUsageRow> = self
                .db
                .with(|c| {
                    c.prepare(&sql)?
                        .query_map([], |r| {
                            Ok(AiUsageRow {
                                kind: kind.to_string(),
                                name: r.get(0)?,
                                tokens_7d: r.get::<_, i64>(1)? as u64,
                                cost_7d: r.get(2)?,
                                tokens_30d: r.get::<_, i64>(3)? as u64,
                                cost_30d: r.get(4)?,
                                tokens_all: r.get::<_, i64>(5)? as u64,
                                cost_all: r.get(6)?,
                                unpriced_runs: r.get::<_, Option<i64>>(7)?.unwrap_or(0) as u32,
                            })
                        })?
                        .collect()
                })
                .map_err(storage)?;
            rows.extend(found);
        }
        Ok(rows)
    }
}
