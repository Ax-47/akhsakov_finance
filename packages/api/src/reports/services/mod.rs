//! Report use cases: building a month's report, sending it, and the
//! schedule that sends last month's on the 1st.

use crate::{
    reports::repositories::{ReportConfig, ReportData, ReportSender},
    shared::ServiceError,
};
use dtos::report::{monthly_report, previous_month, report_text, MonthlyReport};
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::Arc, time::Duration};
use types::{range::Range, ticker_symbol::TickerSymbol};
use uuid::Uuid;

const CHECK_EVERY: Duration = Duration::from_secs(3600);

#[derive(Clone)]
pub struct ReportService {
    data: Arc<dyn ReportData>,
    config: Arc<dyn ReportConfig>,
    sender: Arc<dyn ReportSender>,
}

/// History long enough to reach back to `month` from `today`.
fn range_for(month: &str, today: &str) -> Range {
    let months = |m: &str| -> Option<i32> { Some(m.get(..4)?.parse::<i32>().ok()? * 12 + m.get(5..7)?.parse::<i32>().ok()?) };
    match (months(today), months(month)) {
        (Some(t), Some(m)) if t - m <= 1 => Range::M3,
        (Some(t), Some(m)) if t - m <= 10 => Range::Y1,
        (Some(t), Some(m)) if t - m <= 22 => Range::Y2,
        (Some(t), Some(m)) if t - m <= 58 => Range::Y5,
        _ => Range::Max,
    }
}

impl ReportService {
    pub fn new(data: Arc<dyn ReportData>, config: Arc<dyn ReportConfig>, sender: Arc<dyn ReportSender>) -> Self {
        Self { data, config, sender }
    }

    /// The report for `month` (`YYYY-MM`), for one portfolio or all.
    pub async fn build(&self, month: &str, portfolio: Option<Uuid>, today: &str) -> Result<MonthlyReport, ServiceError> {
        if dtos::report::month_bounds(month).is_none() || month.len() != 7 {
            return Err(ServiceError::Validation("Month must be YYYY-MM".into()));
        }
        let transactions: Vec<_> = self
            .data
            .transactions()
            .map_err(ServiceError::Storage)?
            .into_iter()
            .filter(|t| portfolio.is_none_or(|p| t.portfolio_id == p))
            .collect();
        let mut tickers: Vec<TickerSymbol> = transactions.iter().filter(|t| !t.is_cash()).map(|t| t.ticker.clone()).collect();
        tickers.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        tickers.dedup();
        let range = range_for(month, today);
        let fetched = futures::future::join_all(tickers.into_iter().map(|t| async move {
            let closes = self.data.closes(&t, range).await.unwrap_or_default();
            (t, closes)
        }))
        .await;
        let history: HashMap<TickerSymbol, Vec<(String, Decimal)>> = fetched.into_iter().collect();
        let price_on = |t: &TickerSymbol, date: &str| {
            let closes = history.get(t)?;
            let i = closes.partition_point(|(d, _)| d.as_str() <= date);
            closes.get(i.checked_sub(1)?).map(|(_, c)| *c)
        };
        monthly_report(&transactions, month, price_on).ok_or_else(|| ServiceError::Validation("Month must be YYYY-MM".into()))
    }

    /// Builds the report for all holdings and sends it.
    pub async fn send(&self, month: &str, today: &str) -> Result<(), ServiceError> {
        let report = self.build(month, None, today).await?;
        let (symbol, rate) = self.data.display().await;
        let body = report_text(&report, |v| money(v * rate, &symbol));
        self.sender
            .send(&format!("📊 Monthly report {month}"), &body)
            .await
            .map_err(ServiceError::Upstream)
    }

    pub fn enabled(&self) -> Result<bool, ServiceError> {
        self.config.enabled().map_err(ServiceError::Storage)
    }

    pub fn set_enabled(&self, on: bool) -> Result<(), ServiceError> {
        self.config.set_enabled(on).map_err(ServiceError::Storage)
    }

    /// From the 1st of a month: sends last month's report once, if the
    /// schedule is on. Returns whether it sent.
    pub async fn tick(&self, today: &str) -> Result<bool, ServiceError> {
        if !self.enabled()? {
            return Ok(false);
        }
        let Some(month) = today.get(..7).and_then(previous_month) else { return Ok(false) };
        if self.config.last_sent().map_err(ServiceError::Storage)?.as_deref() >= Some(month.as_str()) {
            return Ok(false);
        }
        self.send(&month, today).await?;
        self.config.set_last_sent(&month).map_err(ServiceError::Storage)?;
        Ok(true)
    }

    /// Runs [`Self::tick`] every hour, forever, on the server's date.
    pub fn spawn_schedule(self) {
        tokio::spawn(async move {
            loop {
                let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
                match self.tick(&today).await {
                    Ok(true) => tracing::info!("monthly report sent"),
                    Ok(false) => {}
                    Err(e) => tracing::warn!("monthly report failed: {e}"),
                }
                tokio::time::sleep(CHECK_EVERY).await;
            }
        });
    }
}

/// `฿1,234.56` style.
fn money(v: Decimal, symbol: &str) -> String {
    let abs = v.abs().round_dp(2);
    let whole = abs.trunc().to_string();
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let cents = ((abs - abs.trunc()) * Decimal::ONE_HUNDRED).round();
    let sign = if v.is_sign_negative() && !abs.is_zero() { "-" } else { "" };
    format!("{sign}{symbol}{grouped}.{cents:0>2}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dtos::Transaction;
    use rust_decimal_macros::dec;
    use std::sync::Mutex;
    use types::transaction_type::TransactionType;

    struct Data;
    #[async_trait]
    impl ReportData for Data {
        fn transactions(&self) -> Result<Vec<Transaction>, String> {
            Ok(vec![Transaction {
                id: Uuid::nil(),
                portfolio_id: Uuid::nil(),
                ticker: TickerSymbol::new("VOO").unwrap(),
                transaction_type: TransactionType::Buy,
                shares: dec!(10),
                price: dec!(100),
                date: "2026-01-05".into(),
                fee: dec!(0),
                currency: "USD".into(),
                fx_to_usd: dec!(1),
            }])
        }
        async fn closes(&self, _: &TickerSymbol, _: Range) -> Result<Vec<(String, Decimal)>, String> {
            Ok(vec![("2026-01-30".into(), dec!(100)), ("2026-02-27".into(), dec!(110))])
        }
        async fn display(&self) -> (String, Decimal) {
            ("฿".into(), dec!(35))
        }
    }

    #[derive(Default)]
    struct Config(Mutex<(bool, Option<String>)>);
    impl ReportConfig for Config {
        fn enabled(&self) -> Result<bool, String> {
            Ok(self.0.lock().unwrap().0)
        }
        fn set_enabled(&self, on: bool) -> Result<(), String> {
            self.0.lock().unwrap().0 = on;
            Ok(())
        }
        fn last_sent(&self) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().1.clone())
        }
        fn set_last_sent(&self, m: &str) -> Result<(), String> {
            self.0.lock().unwrap().1 = Some(m.into());
            Ok(())
        }
    }

    #[derive(Default)]
    struct Outbox(Mutex<Vec<(String, String)>>);
    #[async_trait]
    impl ReportSender for Outbox {
        async fn send(&self, title: &str, body: &str) -> Result<(), String> {
            self.0.lock().unwrap().push((title.into(), body.into()));
            Ok(())
        }
    }

    #[tokio::test]
    async fn sends_last_month_once_when_enabled() {
        let outbox = Arc::new(Outbox::default());
        let s = ReportService::new(Arc::new(Data), Arc::new(Config::default()), outbox.clone());
        assert!(!s.tick("2026-03-01").await.unwrap(), "off by default");
        s.set_enabled(true).unwrap();
        assert!(s.tick("2026-03-01").await.unwrap());
        assert!(!s.tick("2026-03-02").await.unwrap(), "once a month");
        let (title, body) = outbox.0.lock().unwrap()[0].clone();
        assert_eq!(title, "📊 Monthly report 2026-02");
        // 10 × 100 → 10 × 110, at 35 baht per dollar.
        assert!(body.contains("Value: ฿35,000.00 → ฿38,500.00"), "{body}");
        assert!(body.contains("(+10"), "{body}");

        let r = s.build("2026-02", None, "2026-03-01").await.unwrap();
        assert_eq!((r.start_value, r.end_value), (dec!(1000), dec!(1100)));
        assert!(s.build("Feb", None, "2026-03-01").await.is_err());
        assert_eq!(range_for("2026-02", "2026-03-01"), Range::M3);
        assert_eq!(range_for("2024-02", "2026-03-01"), Range::Y5);
    }
}
