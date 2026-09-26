//! Notifications (e.g. fired alerts) and where to push them.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    /// `YYYY-MM-DD HH:MM:SS`, UTC.
    pub created_at: String,
    pub read: bool,
}

/// Where alerts are pushed besides the app itself. Empty fields are off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channels {
    /// ntfy topic; subscribe to it in the ntfy phone app.
    pub ntfy_topic: String,
    /// ntfy server, `https://ntfy.sh` unless you host your own.
    pub ntfy_server: String,
    pub telegram_token: String,
    pub telegram_chat_id: String,
    /// Receives `{"text": …, "content": …}` (Slack / Discord style) as JSON.
    pub webhook_url: String,
}

impl Default for Channels {
    fn default() -> Self {
        Self {
            ntfy_topic: String::new(),
            ntfy_server: "https://ntfy.sh".into(),
            telegram_token: String::new(),
            telegram_chat_id: String::new(),
            webhook_url: String::new(),
        }
    }
}

impl Channels {
    /// Trims every field.
    pub fn trimmed(&self) -> Self {
        Self {
            ntfy_topic: self.ntfy_topic.trim().into(),
            ntfy_server: self.ntfy_server.trim().trim_end_matches('/').into(),
            telegram_token: self.telegram_token.trim().into(),
            telegram_chat_id: self.telegram_chat_id.trim().into(),
            webhook_url: self.webhook_url.trim().into(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let c = self.trimmed();
        let https = |url: &str| url.starts_with("https://") || url.starts_with("http://");
        if !c.ntfy_topic.is_empty()
            && !(c.ntfy_topic.len() <= 64
                && c.ntfy_topic
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        {
            return Err("ntfy topics use letters, digits, - and _ (at most 64)".into());
        }
        if !c.ntfy_topic.is_empty() && !https(&c.ntfy_server) {
            return Err("The ntfy server must be an http(s) address".into());
        }
        if c.telegram_token.is_empty() != c.telegram_chat_id.is_empty() {
            return Err("Telegram needs both the bot token and your chat id".into());
        }
        if !c.telegram_token.is_empty() && !c.telegram_token.contains(':') {
            return Err("That doesn't look like a Telegram bot token (123456:ABC…)".into());
        }
        if !c.webhook_url.is_empty() && !https(&c.webhook_url) {
            return Err("The webhook must be an http(s) address".into());
        }
        Ok(())
    }

    pub fn any(&self) -> bool {
        let c = self.trimmed();
        !c.ntfy_topic.is_empty() || !c.telegram_token.is_empty() || !c.webhook_url.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_channels() {
        assert!(Channels::default().validate().is_ok());
        assert!(!Channels::default().any());
        let ntfy = Channels { ntfy_topic: " my-alerts_1 ".into(), ..Default::default() };
        assert!(ntfy.validate().is_ok() && ntfy.any());
        let bad = |c: Channels| c.validate().is_err();
        assert!(bad(Channels { ntfy_topic: "has space".into(), ..Default::default() }));
        assert!(bad(Channels { telegram_token: "123:abc".into(), ..Default::default() }));
        assert!(bad(Channels { telegram_token: "nocolon".into(), telegram_chat_id: "1".into(), ..Default::default() }));
        assert!(bad(Channels { webhook_url: "ftp://x".into(), ..Default::default() }));
    }
}
