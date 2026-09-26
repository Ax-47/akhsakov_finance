//! Ports for notifications and the alert monitor.

use crate::shared::RepositoryError;
use async_trait::async_trait;
use dtos::{
    notifications::{Channels, Notification},
    portfolio::GetDashBoardResponse,
    watch::Alert,
};
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub trait NotificationRepository: Send + Sync {
    /// Newest first, at most `limit`.
    fn recent(&self, limit: usize) -> Result<Vec<Notification>, RepositoryError>;
    fn add(&self, n: &Notification) -> Result<(), RepositoryError>;
    fn mark_all_read(&self) -> Result<(), RepositoryError>;
    fn channels(&self) -> Result<Channels, RepositoryError>;
    fn save_channels(&self, channels: &Channels) -> Result<(), RepositoryError>;
}

/// Delivers a message outside the app (phone push, chat, webhook).
#[async_trait]
pub trait Pusher: Send + Sync {
    /// One result per configured channel, named.
    async fn push(&self, channels: &Channels, title: &str, body: &str) -> Vec<(String, Result<(), String>)>;
}

/// Alerts to check, and recording that one fired.
pub trait AlertStore: Send + Sync {
    fn active_alerts(&self) -> Result<Vec<Alert>, String>;
    /// `Ok(false)` if it had already fired (e.g. another check got there first).
    fn mark_fired(&self, id: Uuid) -> Result<bool, String>;
}

/// Holdings, for weight alerts.
pub trait HoldingsSource: Send + Sync {
    fn dashboard(&self) -> Result<GetDashBoardResponse, String>;
}

/// Latest (price, previous close) in USD.
#[async_trait]
pub trait PriceSource: Send + Sync {
    async fn price(&self, ticker: &TickerSymbol) -> Result<(Decimal, Decimal), String>;
}
