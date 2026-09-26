//! Notification use cases, and the alert monitor that checks price alerts
//! on the server so they fire even when no app is open.

mod monitor;

pub use monitor::AlertMonitor;

use crate::{
    notifications::repositories::{NotificationRepository, Pusher},
    shared::ServiceError,
};
use dtos::notifications::{Channels, Notification};
use std::sync::Arc;
use uuid::Uuid;

const MAX_LISTED: usize = 50;

#[derive(Clone)]
pub struct NotificationService {
    repo: Arc<dyn NotificationRepository>,
    pusher: Arc<dyn Pusher>,
}

impl NotificationService {
    pub fn new(repo: Arc<dyn NotificationRepository>, pusher: Arc<dyn Pusher>) -> Self {
        Self { repo, pusher }
    }

    /// Records a notification and pushes it to every configured channel.
    /// Push failures are logged; the app still shows it.
    pub async fn notify(&self, title: &str, body: &str) -> Result<(), ServiceError> {
        self.repo.add(&Notification {
            id: Uuid::new_v4(),
            title: title.to_string(),
            body: body.to_string(),
            created_at: String::new(),
            read: false,
        })?;
        let channels = self.repo.channels()?;
        if channels.any() {
            for (channel, result) in self.pusher.push(&channels, title, body).await {
                if let Err(e) = result {
                    tracing::warn!("push via {channel} failed: {e}");
                }
            }
        }
        Ok(())
    }

    pub fn recent(&self) -> Result<Vec<Notification>, ServiceError> {
        Ok(self.repo.recent(MAX_LISTED)?)
    }

    pub fn mark_all_read(&self) -> Result<(), ServiceError> {
        Ok(self.repo.mark_all_read()?)
    }

    pub fn channels(&self) -> Result<Channels, ServiceError> {
        Ok(self.repo.channels()?)
    }

    pub fn save_channels(&self, channels: Channels) -> Result<(), ServiceError> {
        channels.validate().map_err(ServiceError::Validation)?;
        Ok(self.repo.save_channels(&channels.trimmed())?)
    }

    /// Sends a test message through `channels` (not saved); one result per
    /// channel.
    pub async fn test(&self, channels: Channels) -> Result<Vec<(String, Result<(), String>)>, ServiceError> {
        channels.validate().map_err(ServiceError::Validation)?;
        if !channels.any() {
            return Err(ServiceError::Validation("Set up a channel first".into()));
        }
        Ok(self
            .pusher
            .push(&channels, "Akhsakov Finance", "Test notification — alerts will arrive like this.")
            .await)
    }
}
