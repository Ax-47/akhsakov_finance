//! Stock research data beyond prices: news, analyst rating changes,
//! ownership, corporate actions and options.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsItem {
    pub title: String,
    pub publisher: Option<String>,
    pub link: Option<String>,
    /// `YYYY-MM-DD HH:MM` (UTC).
    pub published_at: String,
}

/// An analyst firm changing its rating, e.g. "Hold → Buy".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatingChange {
    pub date: String,
    pub firm: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// e.g. "Upgrade", "Downgrade", "Initiated", "Maintained".
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Holders {
    /// e.g. ("Institutions", 0.67) — fractions.
    pub breakdown: Vec<(String, f64)>,
    pub institutions: Vec<Institution>,
    pub insider_trades: Vec<InsiderTrade>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Institution {
    pub name: String,
    pub shares: Option<f64>,
    /// Fraction of shares outstanding.
    pub pct_held: Option<f64>,
    pub value: Option<f64>,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsiderTrade {
    pub name: String,
    pub position: String,
    /// e.g. "Buy", "Sell", "Award".
    pub kind: String,
    pub shares: Option<f64>,
    pub value: Option<f64>,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CorporateAction {
    Dividend {
        date: String,
        amount: f64,
    },
    /// `numerator`-for-`denominator`, e.g. 4-for-1.
    Split {
        date: String,
        numerator: u32,
        denominator: u32,
    },
    CapitalGain {
        date: String,
        amount: f64,
    },
}

impl CorporateAction {
    pub fn date(&self) -> &str {
        match self {
            Self::Dividend { date, .. }
            | Self::Split { date, .. }
            | Self::CapitalGain { date, .. } => date,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionQuote {
    pub strike: f64,
    pub last: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub volume: Option<u64>,
    pub open_interest: Option<u64>,
    /// Implied volatility as a fraction.
    pub implied_volatility: Option<f64>,
    pub in_the_money: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OptionChainView {
    /// Available expirations as unix seconds, soonest first.
    pub expirations: Vec<i64>,
    /// The expiration shown.
    pub expiration: Option<i64>,
    /// By strike, ascending.
    pub calls: Vec<OptionQuote>,
    pub puts: Vec<OptionQuote>,
}

/// `StrongBuy` → `Strong buy`.
pub fn humanize(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes_enum_names() {
        assert_eq!(humanize("StrongBuy"), "Strong buy");
        assert_eq!(humanize("Hold"), "Hold");
        assert_eq!(humanize("ChiefExecutiveOfficer"), "Chief executive officer");
    }
}
