//! Adapters: SQLite storage, HTTP push (ntfy, Telegram, webhook), and the
//! monitor's views of the watchlist, portfolio and quote contexts.

use crate::{
    database::Database,
    notifications::repositories::{
        AlertStore, HoldingsSource, NotificationRepository, PriceSource, Pusher,
    },
    portfolio::PortfolioService,
    quote::services::quote::QuoteService,
    shared::{RepositoryError, ServiceError},
    watchlist::WatchlistService,
};
use async_trait::async_trait;
use dtos::{
    notifications::{Channels, Notification},
    portfolio::GetDashBoardResponse,
    watch::Alert,
};
use rusqlite::params;
use rust_decimal::Decimal;
use serde_json::json;
use std::{collections::HashMap, time::Duration};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub struct SqliteNotificationRepository {
    db: Database,
}

impl SqliteNotificationRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl NotificationRepository for SqliteNotificationRepository {
    fn recent(&self, limit: usize) -> Result<Vec<Notification>, RepositoryError> {
        let rows: Vec<(String, String, String, String, bool)> = self.db.with(|c| {
            c.prepare(
                "SELECT id, title, body, created_at, read FROM notifications
                 ORDER BY created_at DESC, rowid DESC LIMIT ?1",
            )?
            .query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect()
        })?;
        rows.into_iter()
            .map(|(id, title, body, created_at, read)| {
                Ok(Notification {
                    id: Uuid::parse_str(&id)
                        .map_err(|_| RepositoryError::Corrupt(format!("notification id \"{id}\"")))?,
                    title,
                    body,
                    created_at,
                    read,
                })
            })
            .collect()
    }

    fn add(&self, n: &Notification) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO notifications (id, title, body, read) VALUES (?1, ?2, ?3, ?4)",
                params![n.id.to_string(), n.title, n.body, n.read],
            )
        })?;
        Ok(())
    }

    fn mark_all_read(&self) -> Result<(), RepositoryError> {
        self.db
            .with(|c| c.execute("UPDATE notifications SET read = 1 WHERE read = 0", []))?;
        Ok(())
    }

    fn channels(&self) -> Result<Channels, RepositoryError> {
        let stored: HashMap<String, String> = self.db.with(|c| {
            c.prepare("SELECT key, value FROM notify_config")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?;
        let d = Channels::default();
        let get = |key: &str, fallback: String| stored.get(key).cloned().unwrap_or(fallback);
        Ok(Channels {
            ntfy_topic: get("ntfy_topic", d.ntfy_topic),
            ntfy_server: get("ntfy_server", d.ntfy_server),
            telegram_token: get("telegram_token", d.telegram_token),
            telegram_chat_id: get("telegram_chat_id", d.telegram_chat_id),
            webhook_url: get("webhook_url", d.webhook_url),
        })
    }

    fn save_channels(&self, c: &Channels) -> Result<(), RepositoryError> {
        self.db.transaction(|t| {
            let mut upsert =
                t.prepare("INSERT OR REPLACE INTO notify_config (key, value) VALUES (?1, ?2)")?;
            for (key, value) in [
                ("ntfy_topic", &c.ntfy_topic),
                ("ntfy_server", &c.ntfy_server),
                ("telegram_token", &c.telegram_token),
                ("telegram_chat_id", &c.telegram_chat_id),
                ("webhook_url", &c.webhook_url),
            ] {
                upsert.execute([key, value.as_str()])?;
            }
            Ok(())
        })?;
        Ok(())
    }
}

/// Sends to every configured channel over HTTPS.
pub struct HttpPusher {
    http: reqwest::Client,
}

impl HttpPusher {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("akhsakov-finance/1.0")
                .timeout(Duration::from_secs(15))
                // The addresses are user-entered: don't let a redirect bounce
                // the request onto the server's own network.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("build the HTTP client"),
        }
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<(), String> {
        let response = request.send().await.map_err(|e| e.to_string())?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("{status}: {}", body.chars().take(200).collect::<String>()))
        }
    }
}

#[async_trait]
impl Pusher for HttpPusher {
    async fn push(&self, channels: &Channels, title: &str, body: &str) -> Vec<(String, Result<(), String>)> {
        let c = channels.trimmed();
        let mut results = Vec::new();
        if !c.ntfy_topic.is_empty() {
            let request = self
                .http
                .post(format!("{}/{}", c.ntfy_server, c.ntfy_topic))
                // Header values must be ASCII; the body carries the full text.
                .header("Title", title.chars().filter(char::is_ascii).collect::<String>())
                .header("Tags", "chart_with_upwards_trend")
                .body(format!("{title}\n{body}"));
            results.push(("ntfy".to_string(), self.send(request).await));
        }
        if !c.telegram_token.is_empty() {
            let request = self
                .http
                .post(format!("https://api.telegram.org/bot{}/sendMessage", c.telegram_token))
                .json(&json!({"chat_id": c.telegram_chat_id, "text": format!("{title}\n{body}")}));
            results.push(("Telegram".to_string(), self.send(request).await));
        }
        if !c.webhook_url.is_empty() {
            let text = format!("{title}\n{body}");
            let request = self
                .http
                .post(&c.webhook_url)
                .json(&json!({"text": text, "content": text, "title": title, "body": body}));
            results.push(("Webhook".to_string(), self.send(request).await));
        }
        results
    }
}

fn message(e: ServiceError) -> String {
    e.to_string()
}

/// Alerts from the watchlist context.
pub struct WatchlistAlerts(pub WatchlistService);

impl AlertStore for WatchlistAlerts {
    fn active_alerts(&self) -> Result<Vec<Alert>, String> {
        Ok(self
            .0
            .alerts()
            .map_err(message)?
            .into_iter()
            .filter(Alert::is_active)
            .collect())
    }

    fn mark_fired(&self, id: Uuid) -> Result<bool, String> {
        match self.0.mark_triggered(id) {
            Ok(()) => Ok(true),
            Err(ServiceError::NotFound(_)) => Ok(false),
            Err(e) => Err(message(e)),
        }
    }
}

/// Holdings from the portfolio context.
pub struct PortfolioHoldings(pub PortfolioService);

impl HoldingsSource for PortfolioHoldings {
    fn dashboard(&self) -> Result<GetDashBoardResponse, String> {
        self.0.dashboard().map_err(message)
    }
}

/// USD prices from the quote context.
pub struct QuotePrices(pub QuoteService);

#[async_trait]
impl PriceSource for QuotePrices {
    async fn price(&self, ticker: &TickerSymbol) -> Result<(Decimal, Decimal), String> {
        let q = self
            .0
            .get_quote(ticker.clone())
            .await
            .map_err(|e| e.to_string())?;
        if q.stale {
            return Err("offline".into());
        }
        Ok((q.current_price, q.previous_close_price))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_notifications_and_channels() {
        let repo = SqliteNotificationRepository::new(Database::in_memory().unwrap());
        assert_eq!(repo.channels().unwrap(), Channels::default());
        let c = Channels { ntfy_topic: "t".into(), ..Default::default() };
        repo.save_channels(&c).unwrap();
        assert_eq!(repo.channels().unwrap(), c);

        let n = Notification {
            id: Uuid::new_v4(),
            title: "🔔 NVDA".into(),
            body: "Now $1".into(),
            created_at: String::new(),
            read: false,
        };
        repo.add(&n).unwrap();
        assert!(!repo.recent(10).unwrap()[0].read);
        repo.mark_all_read().unwrap();
        assert!(repo.recent(10).unwrap()[0].read);
    }
}
