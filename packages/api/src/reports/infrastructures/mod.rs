//! Adapters: the portfolio, quote and settings contexts as report data,
//! SQLite for the schedule, and notifications for delivery.

use crate::{
    database::Database,
    notifications::NotificationService,
    portfolio::PortfolioService,
    quote::services::quote::QuoteService,
    reports::repositories::{ReportConfig, ReportData, ReportSender},
    settings::SettingsService,
};
use async_trait::async_trait;
use dtos::Transaction;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

pub struct AppReportData {
    pub portfolio: PortfolioService,
    pub quotes: QuoteService,
    pub settings: SettingsService,
}

#[async_trait]
impl ReportData for AppReportData {
    fn transactions(&self) -> Result<Vec<Transaction>, String> {
        Ok(self.portfolio.dashboard().map_err(|e| e.to_string())?.transactions)
    }

    async fn closes(&self, ticker: &TickerSymbol, range: Range) -> Result<Vec<(String, Decimal)>, String> {
        let candles = self
            .quotes
            .get_chart(ticker.clone(), range, Interval::D1, false)
            .await
            .map_err(|e| e.to_string())?;
        Ok(candles.into_iter().map(|c| (c.ts.format("%Y-%m-%d").to_string(), c.close)).collect())
    }

    async fn display(&self) -> (String, Decimal) {
        let Ok(settings) = self.settings.get() else {
            return ("$".into(), Decimal::ONE);
        };
        match self.quotes.usd_rate(&settings.currency).await {
            Ok(usd_per_unit) if usd_per_unit > Decimal::ZERO => {
                (settings.currency_symbol().to_string(), Decimal::ONE / usd_per_unit)
            }
            _ => ("$".into(), Decimal::ONE),
        }
    }
}

/// Stored with the push channels' settings.
pub struct SqliteReportConfig {
    db: Database,
}

impl SqliteReportConfig {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        self.db
            .with(|c| c.query_row("SELECT value FROM notify_config WHERE key = ?1", [key], |r| r.get(0)).optional())
            .map_err(|e| e.to_string())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.db
            .with(|c| c.execute("INSERT OR REPLACE INTO notify_config (key, value) VALUES (?1, ?2)", [key, value]))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

impl ReportConfig for SqliteReportConfig {
    fn enabled(&self) -> Result<bool, String> {
        Ok(self.get("monthly_report")?.as_deref() == Some("1"))
    }

    fn set_enabled(&self, on: bool) -> Result<(), String> {
        self.set("monthly_report", if on { "1" } else { "0" })
    }

    fn last_sent(&self) -> Result<Option<String>, String> {
        self.get("monthly_report_sent")
    }

    fn set_last_sent(&self, month: &str) -> Result<(), String> {
        self.set("monthly_report_sent", month)
    }
}

pub struct NotifySender(pub NotificationService);

#[async_trait]
impl ReportSender for NotifySender {
    async fn send(&self, title: &str, body: &str) -> Result<(), String> {
        self.0.notify(title, body).await.map_err(|e| e.to_string())
    }
}
