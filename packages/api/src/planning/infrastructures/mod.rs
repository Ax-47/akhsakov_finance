//! SQLite adapter for [`PlanningRepository`].

use crate::{
    database::Database,
    notifications::NotificationService,
    planning::repositories::{DcaRepository, PlanningRepository, Reminder, TradeRecorder},
    portfolio::PortfolioService,
    shared::RepositoryError,
};
use dtos::{
    dca::DcaPlan,
    planning::{Goal, TargetWeight},
    Transaction,
};
use rusqlite::OptionalExtension;
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

impl DcaRepository for SqlitePlanningRepository {
    fn plans(&self) -> Result<Vec<DcaPlan>, RepositoryError> {
        type Row = (String, String, String, String, String, u32, bool, String, Option<String>);
        let rows: Vec<Row> = self.db.with(|c| {
            c.prepare(
                "SELECT id, portfolio_id, ticker, amount, currency, day, active, start, last_done
                 FROM dca_plans ORDER BY day, ticker",
            )?
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
            })?
            .collect()
        })?;
        rows.into_iter()
            .map(|(id, portfolio_id, ticker, amount, currency, day, active, start, last_done)| {
                Ok(DcaPlan {
                    id: Uuid::parse_str(&id).map_err(|_| corrupt("plan id", &id))?,
                    portfolio_id: Uuid::parse_str(&portfolio_id).map_err(|_| corrupt("plan portfolio", &portfolio_id))?,
                    ticker: TickerSymbol::new(&ticker).map_err(|_| corrupt("ticker", &ticker))?,
                    amount: Decimal::from_str(&amount).map_err(|_| corrupt("plan amount", &amount))?,
                    currency,
                    day,
                    active,
                    start,
                    last_done,
                })
            })
            .collect()
    }

    fn save_plan(&self, p: &DcaPlan) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO dca_plans (id, portfolio_id, ticker, amount, currency, day, active, start, last_done)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET portfolio_id = ?2, ticker = ?3, amount = ?4, currency = ?5,
                     day = ?6, active = ?7, start = ?8, last_done = ?9",
                params![
                    p.id.to_string(),
                    p.portfolio_id.to_string(),
                    p.ticker.as_str(),
                    p.amount.to_string(),
                    p.currency,
                    p.day,
                    p.active,
                    p.start,
                    p.last_done
                ],
            )
        })?;
        Ok(())
    }

    fn delete_plan(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.db.with(|c| c.execute("DELETE FROM dca_plans WHERE id = ?1", [id.to_string()]))?;
        Ok(())
    }

    fn mark_done(&self, id: Uuid, month: &str) -> Result<(), RepositoryError> {
        let n = self.db.with(|c| {
            c.execute("UPDATE dca_plans SET last_done = ?2 WHERE id = ?1", params![id.to_string(), month])
        })?;
        if n == 0 {
            return Err(RepositoryError::NotFound(format!("plan {id}")));
        }
        Ok(())
    }

    fn last_reminded(&self, id: Uuid) -> Result<Option<String>, RepositoryError> {
        Ok(self
            .db
            .with(|c| {
                c.query_row("SELECT last_reminded FROM dca_plans WHERE id = ?1", [id.to_string()], |r| r.get(0))
                    .optional()
            })?
            .flatten())
    }

    fn mark_reminded(&self, id: Uuid, month: &str) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute("UPDATE dca_plans SET last_reminded = ?2 WHERE id = ?1", params![id.to_string(), month])
        })?;
        Ok(())
    }
}

/// Purchases go through the portfolio context (validation, FX rate).
pub struct PortfolioTrades(pub PortfolioService);

#[async_trait::async_trait]
impl TradeRecorder for PortfolioTrades {
    async fn record(&self, tx: Transaction) -> Result<(), String> {
        self.0.save_transaction(tx).await.map_err(|e| e.to_string())
    }
}

/// Reminders go out like alerts: in the app and to your push channels.
pub struct NotifyReminder(pub NotificationService);

#[async_trait::async_trait]
impl Reminder for NotifyReminder {
    async fn remind(&self, title: &str, body: &str) -> Result<(), String> {
        self.0.notify(title, body).await.map_err(|e| e.to_string())
    }
}
