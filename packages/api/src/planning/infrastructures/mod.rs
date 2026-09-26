//! SQLite adapter for [`PlanningRepository`].

use crate::{
    database::Database, planning::repositories::PlanningRepository, shared::RepositoryError,
};
use dtos::planning::{Goal, TargetWeight};
use rusqlite::params;
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub struct SqlitePlanningRepository {
    db: Database,
}

impl SqlitePlanningRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

fn corrupt(what: &str, value: &str) -> RepositoryError {
    RepositoryError::Corrupt(format!("{what} \"{value}\""))
}

impl PlanningRepository for SqlitePlanningRepository {
    fn targets(&self, portfolio_id: Uuid) -> Result<Vec<TargetWeight>, RepositoryError> {
        let rows: Vec<(String, String)> = self.db.with(|c| {
            c.prepare("SELECT ticker, weight FROM targets WHERE portfolio_id = ?1 ORDER BY CAST(weight AS REAL) DESC")?
                .query_map([portfolio_id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(t, w)| {
                Ok(TargetWeight {
                    ticker: TickerSymbol::new(&t).map_err(|_| corrupt("ticker", &t))?,
                    weight: Decimal::from_str(&w).map_err(|_| corrupt("weight", &w))?,
                })
            })
            .collect()
    }

    fn replace_targets(
        &self,
        portfolio_id: Uuid,
        targets: &[TargetWeight],
    ) -> Result<(), RepositoryError> {
        self.db.transaction(|t| {
            t.execute(
                "DELETE FROM targets WHERE portfolio_id = ?1",
                [portfolio_id.to_string()],
            )?;
            let mut insert = t.prepare(
                "INSERT INTO targets (portfolio_id, ticker, weight) VALUES (?1, ?2, ?3)",
            )?;
            for target in targets {
                insert.execute(params![
                    portfolio_id.to_string(),
                    target.ticker.as_str(),
                    target.weight.to_string()
                ])?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn goals(&self) -> Result<Vec<Goal>, RepositoryError> {
        type Row = (String, String, String, String, String, Option<String>);
        let rows: Vec<Row> = self.db.with(|c| {
            c.prepare(
                "SELECT id, name, target, date, monthly, portfolio_id FROM goals ORDER BY date",
            )?
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect()
        })?;
        rows.into_iter()
            .map(|(id, name, target, date, monthly, portfolio_id)| {
                Ok(Goal {
                    id: Uuid::parse_str(&id).map_err(|_| corrupt("goal id", &id))?,
                    name,
                    target: Decimal::from_str(&target)
                        .map_err(|_| corrupt("goal target", &target))?,
                    date,
                    monthly: Decimal::from_str(&monthly)
                        .map_err(|_| corrupt("monthly amount", &monthly))?,
                    portfolio_id: portfolio_id
                        .map(|p| Uuid::parse_str(&p).map_err(|_| corrupt("goal portfolio", &p)))
                        .transpose()?,
                })
            })
            .collect()
    }

    fn save_goal(&self, goal: &Goal) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO goals (id, name, target, date, monthly, portfolio_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    goal.id.to_string(),
                    goal.name,
                    goal.target.to_string(),
                    goal.date,
                    goal.monthly.to_string(),
                    goal.portfolio_id.map(|p| p.to_string())
                ],
            )
        })?;
        Ok(())
    }

    fn delete_goal(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("DELETE FROM goals WHERE id = ?1", [id.to_string()]))?;
        Ok(())
    }
}
