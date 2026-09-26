//! Monthly investment plans: storing them, recording each month's
//! purchase, and reminding you on the day (checked hourly on the server).

use crate::{
    planning::repositories::{DcaRepository, Reminder, TradeRecorder},
    shared::ServiceError,
};
use dtos::{dca::DcaPlan, Transaction};
use rust_decimal::Decimal;
use std::{sync::Arc, time::Duration};
use types::transaction_type::TransactionType;
use uuid::Uuid;

const CHECK_EVERY: Duration = Duration::from_secs(3600);

#[derive(Clone)]
pub struct DcaService {
    repo: Arc<dyn DcaRepository>,
    trades: Arc<dyn TradeRecorder>,
    reminder: Arc<dyn Reminder>,
}

impl DcaService {
    pub fn new(repo: Arc<dyn DcaRepository>, trades: Arc<dyn TradeRecorder>, reminder: Arc<dyn Reminder>) -> Self {
        Self { repo, trades, reminder }
    }

    pub fn plans(&self) -> Result<Vec<DcaPlan>, ServiceError> {
        Ok(self.repo.plans()?)
    }

    pub fn save_plan(&self, plan: DcaPlan) -> Result<(), ServiceError> {
        plan.validate().map_err(ServiceError::Validation)?;
        Ok(self.repo.save_plan(&plan)?)
    }

    pub fn delete_plan(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_plan(id)?)
    }

    /// Records this month's purchase: a buy of `shares` at `price` on
    /// `date` in the plan's portfolio, then marks the month done.
    pub async fn record(&self, id: Uuid, date: String, shares: Decimal, price: Decimal, fee: Decimal) -> Result<(), ServiceError> {
        let plan = self
            .repo
            .plans()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| ServiceError::NotFound(format!("plan {id}")))?;
        let month = date.get(..7).ok_or_else(|| ServiceError::Validation("Date must be YYYY-MM-DD".into()))?.to_string();
        let tx = Transaction {
            id: Uuid::new_v4(),
            portfolio_id: plan.portfolio_id,
            ticker: plan.ticker.clone(),
            transaction_type: TransactionType::Buy,
            shares,
            price,
            date,
            fee,
            currency: plan.currency.clone(),
            // The portfolio context looks up the day's rate.
            fx_to_usd: if plan.currency == "USD" { Decimal::ONE } else { Decimal::ZERO },
        };
        self.trades.record(tx).await.map_err(ServiceError::Validation)?;
        Ok(self.repo.mark_done(id, &month)?)
    }

    /// Reminds once per month about each plan due on `today`; returns how
    /// many reminders went out.
    pub async fn remind_due(&self, today: &str) -> Result<usize, ServiceError> {
        let Some(month) = today.get(..7) else { return Ok(0) };
        let mut sent = 0;
        for plan in self.repo.plans()?.into_iter().filter(|p| p.is_due(today)) {
            if self.repo.last_reminded(plan.id)?.as_deref() == Some(month) {
                continue;
            }
            let title = format!("📅 Time to invest: {} {} of {}", plan.amount.normalize(), plan.currency, plan.ticker);
            let body = "Your monthly plan is due. Record the purchase in Portfolio → Plan.";
            if let Err(e) = self.reminder.remind(&title, body).await {
                tracing::warn!("couldn't send the DCA reminder: {e}");
                continue;
            }
            self.repo.mark_reminded(plan.id, month)?;
            sent += 1;
        }
        Ok(sent)
    }

    /// Runs [`Self::remind_due`] every hour, forever, on the server's date.
    pub fn spawn_reminders(self) {
        tokio::spawn(async move {
            loop {
                let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
                match self.remind_due(&today).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!("{n} DCA reminder(s) sent"),
                    Err(e) => tracing::warn!("DCA check failed: {e}"),
                }
                tokio::time::sleep(CHECK_EVERY).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, planning::infrastructures::SqlitePlanningRepository};
    use rust_decimal_macros::dec;
    use std::sync::Mutex;
    use types::ticker_symbol::TickerSymbol;

    #[derive(Default)]
    struct Recorded(Mutex<Vec<Transaction>>);
    #[async_trait::async_trait]
    impl TradeRecorder for Recorded {
        async fn record(&self, tx: Transaction) -> Result<(), String> {
            self.0.lock().unwrap().push(tx);
            Ok(())
        }
    }

    #[derive(Default)]
    struct Sent(Mutex<Vec<String>>);
    #[async_trait::async_trait]
    impl Reminder for Sent {
        async fn remind(&self, title: &str, _: &str) -> Result<(), String> {
            self.0.lock().unwrap().push(title.to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn reminds_once_and_records_the_purchase() {
        let db = Database::in_memory().unwrap();
        let portfolio = Uuid::new_v4();
        db.with(|c| c.execute("INSERT INTO portfolios (id, name) VALUES (?1, 'P')", [portfolio.to_string()]))
            .unwrap();
        let (trades, sent) = (Arc::new(Recorded::default()), Arc::new(Sent::default()));
        let s = DcaService::new(Arc::new(SqlitePlanningRepository::new(db)), trades.clone(), sent.clone());
        let plan = DcaPlan {
            id: Uuid::new_v4(),
            portfolio_id: portfolio,
            ticker: TickerSymbol::new("PTT.BK").unwrap(),
            amount: dec!(3000),
            currency: "THB".into(),
            day: 10,
            active: true,
            start: "2026-01-01".into(),
            last_done: None,
        };
        assert!(s.save_plan(DcaPlan { day: 31, ..plan.clone() }).is_err());
        s.save_plan(plan.clone()).unwrap();

        assert_eq!(s.remind_due("2026-03-09").await.unwrap(), 0, "not yet");
        assert_eq!(s.remind_due("2026-03-10").await.unwrap(), 1);
        assert_eq!(s.remind_due("2026-03-11").await.unwrap(), 0, "once a month");
        assert!(sent.0.lock().unwrap()[0].contains("3000 THB of PTT.BK"));

        s.record(plan.id, "2026-03-11".into(), dec!(100), dec!(30), dec!(5)).await.unwrap();
        let tx = trades.0.lock().unwrap()[0].clone();
        assert_eq!((tx.shares, tx.price, tx.currency.as_str(), tx.fx_to_usd), (dec!(100), dec!(30), "THB", dec!(0)));
        assert_eq!(s.plans().unwrap()[0].last_done.as_deref(), Some("2026-03"));
        // Editing the plan keeps its reminder state.
        s.save_plan(DcaPlan { amount: dec!(4000), last_done: Some("2026-03".into()), ..plan.clone() }).unwrap();
        assert_eq!(s.remind_due("2026-03-12").await.unwrap(), 0);
        assert_eq!(s.remind_due("2026-04-10").await.unwrap(), 1);

        s.delete_plan(plan.id).unwrap();
        assert!(s.plans().unwrap().is_empty());
    }
}
