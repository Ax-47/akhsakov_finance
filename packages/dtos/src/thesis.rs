//! Investment theses: why you hold each stock in each portfolio, what
//! would make you sell, and a dated journal that you and an AI assistant
//! (through the MCP connector) both add to.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

pub const MAX_THESIS_LEN: usize = 5000;
pub const MAX_EXIT_LEN: usize = 2000;
pub const MAX_ENTRY_LEN: usize = 2000;

/// Who wrote a thesis or journal entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Author {
    #[default]
    You,
    /// An AI assistant, through the MCP connector.
    Ai,
}

impl Author {
    pub fn label(self) -> &'static str {
        match self {
            Self::You => "You",
            Self::Ai => "AI",
        }
    }
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::You => "You",
            Self::Ai => "Ai",
        })
    }
}

impl FromStr for Author {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "You" => Ok(Self::You),
            "Ai" => Ok(Self::Ai),
            other => Err(format!("unknown author {other}")),
        }
    }
}

/// How the thesis is holding up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThesisStatus {
    #[default]
    OnTrack,
    AtRisk,
    Broken,
}

impl ThesisStatus {
    pub const ALL: [Self; 3] = [Self::OnTrack, Self::AtRisk, Self::Broken];

    pub fn label(self) -> &'static str {
        match self {
            Self::OnTrack => "On track",
            Self::AtRisk => "At risk",
            Self::Broken => "Broken",
        }
    }

    /// The name used by the MCP tools: `on_track`, `at_risk`, `broken`.
    pub fn key(self) -> &'static str {
        match self {
            Self::OnTrack => "on_track",
            Self::AtRisk => "at_risk",
            Self::Broken => "broken",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key.trim().to_lowercase())
    }
}

impl fmt::Display for ThesisStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl FromStr for ThesisStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|v| v.to_string() == s)
            .ok_or_else(|| format!("unknown thesis status {s}"))
    }
}

/// The editable part of a thesis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ThesisDraft {
    /// Why you own it.
    pub thesis: String,
    /// What would prove the thesis wrong, or make you sell.
    pub exit_if: String,
    /// Price you think it's worth, in USD.
    pub target_price: Option<Decimal>,
    /// 1 (low) to 5 (high).
    pub conviction: Option<u8>,
    /// When to look at it again, YYYY-MM-DD.
    pub review_on: Option<String>,
    pub status: ThesisStatus,
}

/// A dated journal line: an earnings check, a news reaction, a review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThesisEntry {
    pub id: Uuid,
    pub author: Author,
    pub text: String,
    /// `YYYY-MM-DD HH:MM:SS`, UTC.
    pub created_at: String,
}

/// Your thesis on one holding of one portfolio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thesis {
    pub portfolio_id: Uuid,
    pub ticker: TickerSymbol,
    pub draft: ThesisDraft,
    /// Who last changed the fields in `draft`.
    pub updated_by: Author,
    /// `YYYY-MM-DD HH:MM:SS`, UTC.
    pub updated_at: String,
    /// Newest first.
    pub log: Vec<ThesisEntry>,
}

impl Thesis {
    /// Whether the review date has come (`today` is YYYY-MM-DD).
    pub fn review_due(&self, today: &str) -> bool {
        self.draft
            .review_on
            .as_deref()
            .is_some_and(|d| !today.is_empty() && d <= today)
    }
}

/// `YYYY-MM-DD` with a real month and day.
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
    matches!(
        (num(0..4), num(5..7), num(8..10)),
        (Some(_), Some(1..=12), Some(1..=31))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_names_round_trip() {
        for s in ThesisStatus::ALL {
            assert_eq!(ThesisStatus::from_key(s.key()), Some(s));
            assert_eq!(s.to_string().parse::<ThesisStatus>(), Ok(s));
        }
        assert_eq!(ThesisStatus::from_key(" At_Risk "), Some(ThesisStatus::AtRisk));
        for a in [Author::You, Author::Ai] {
            assert_eq!(a.to_string().parse::<Author>(), Ok(a));
        }
    }

    #[test]
    fn dates() {
        assert!(is_iso_date("2026-09-30"));
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date("2026-9-30"));
        assert!(!is_iso_date("next week"));
    }
}
