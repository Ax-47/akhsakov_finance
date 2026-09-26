//! Market-wide views: index and watchlist heatmaps.

use serde::{Deserialize, Serialize};

/// Indices with a heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketIndex {
    Sp500,
    Nasdaq100,
    Dow30,
    Set50,
}

impl MarketIndex {
    pub const ALL: [Self; 4] = [Self::Sp500, Self::Nasdaq100, Self::Dow30, Self::Set50];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sp500 => "S&P 500",
            Self::Nasdaq100 => "Nasdaq-100",
            Self::Dow30 => "Dow Jones",
            Self::Set50 => "SET50",
        }
    }

    /// Yahoo symbol of the index itself.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Sp500 => "^GSPC",
            Self::Nasdaq100 => "^NDX",
            Self::Dow30 => "^DJI",
            Self::Set50 => "^SET50.BK",
        }
    }

    /// Stable key, e.g. for storage.
    pub fn key(self) -> &'static str {
        match self {
            Self::Sp500 => "sp500",
            Self::Nasdaq100 => "ndx",
            Self::Dow30 => "dow",
            Self::Set50 => "set50",
        }
    }
}

/// One stock on a heatmap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeatmapItem {
    pub ticker: String,
    pub name: String,
    /// GICS sector; `None` when unknown (e.g. watchlist stocks).
    pub sector: Option<String>,
    /// Last price, USD.
    pub price: f64,
    /// Change since the previous close, percent.
    pub change_pct: Option<f64>,
    /// Market capitalisation, USD.
    pub market_cap: Option<f64>,
    #[serde(default)]
    pub pe: Option<f64>,
    #[serde(default)]
    pub forward_pe: Option<f64>,
    /// Fraction, e.g. 0.012 for 1.2%.
    #[serde(default)]
    pub dividend_yield: Option<f64>,
    #[serde(default)]
    pub high_52w: Option<f64>,
    #[serde(default)]
    pub low_52w: Option<f64>,
}

impl HeatmapItem {
    /// As a table row.
    pub fn row(&self) -> StockRow {
        StockRow {
            ticker: self.ticker.clone(),
            name: self.name.clone(),
            price: self.price,
            change_pct: self.change_pct,
            market_cap: self.market_cap,
            pe: self.pe,
            forward_pe: self.forward_pe,
            dividend_yield: self.dividend_yield,
            volume: None,
            high_52w: self.high_52w,
            low_52w: self.low_52w,
        }
    }
}

// ─── Screener ─────────────────────────────────────────────────────────────────

/// Markets the screener covers, as (Yahoo region code, name).
pub const REGIONS: [(&str, &str); 10] = [
    ("us", "United States"),
    ("th", "Thailand"),
    ("gb", "United Kingdom"),
    ("jp", "Japan"),
    ("hk", "Hong Kong"),
    ("de", "Germany"),
    ("in", "India"),
    ("sg", "Singapore"),
    ("au", "Australia"),
    ("ca", "Canada"),
];

/// Currency a region's stocks trade in, for converting money filters.
pub fn region_currency(region: &str) -> &'static str {
    match region {
        "th" => "THB",
        "gb" => "GBP",
        "jp" => "JPY",
        "hk" => "HKD",
        "de" => "EUR",
        "in" => "INR",
        "sg" => "SGD",
        "au" => "AUD",
        "ca" => "CAD",
        _ => "USD",
    }
}

/// Yahoo's sector names, as the screener filters them.
pub const SECTORS: [&str; 11] = [
    "Technology",
    "Healthcare",
    "Financial Services",
    "Consumer Cyclical",
    "Communication Services",
    "Industrials",
    "Consumer Defensive",
    "Energy",
    "Basic Materials",
    "Real Estate",
    "Utilities",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScreenSort {
    MarketCap,
    Change,
    Volume,
    Pe,
    DividendYield,
}

impl ScreenSort {
    pub const ALL: [Self; 5] = [
        Self::MarketCap,
        Self::Change,
        Self::Volume,
        Self::Pe,
        Self::DividendYield,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::MarketCap => "Market cap",
            Self::Change => "Change today",
            Self::Volume => "Volume",
            Self::Pe => "P/E",
            Self::DividendYield => "Dividend yield",
        }
    }
}

/// Stock screener criteria. Money amounts are USD; ranges are inclusive
/// and `None` means no limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenFilter {
    /// Yahoo region code, see [`REGIONS`].
    pub region: String,
    /// One of [`SECTORS`].
    pub sector: Option<String>,
    pub min_market_cap: Option<f64>,
    pub max_market_cap: Option<f64>,
    pub min_pe: Option<f64>,
    pub max_pe: Option<f64>,
    /// Percent, e.g. 3 for 3%.
    pub min_dividend_yield: Option<f64>,
    /// Today's change, percent.
    pub min_change: Option<f64>,
    pub max_change: Option<f64>,
    pub sort: ScreenSort,
    pub descending: bool,
    /// Zero-based page of [`SCREEN_PAGE_SIZE`] rows.
    pub page: u32,
}

pub const SCREEN_PAGE_SIZE: u32 = 50;

impl Default for ScreenFilter {
    fn default() -> Self {
        Self {
            region: "us".into(),
            sector: None,
            min_market_cap: Some(10e9),
            max_market_cap: None,
            min_pe: None,
            max_pe: None,
            min_dividend_yield: None,
            min_change: None,
            max_change: None,
            sort: ScreenSort::MarketCap,
            descending: true,
            page: 0,
        }
    }
}

impl ScreenFilter {
    /// Rejects unknown regions / sectors and inverted ranges.
    pub fn validate(&self) -> Result<(), String> {
        if !REGIONS.iter().any(|(code, _)| *code == self.region) {
            return Err(format!("Unknown market {}", self.region));
        }
        if let Some(sector) = &self.sector {
            if !SECTORS.contains(&sector.as_str()) {
                return Err(format!("Unknown sector {sector}"));
            }
        }
        for (lo, hi, what) in [
            (self.min_market_cap, self.max_market_cap, "market cap"),
            (self.min_pe, self.max_pe, "P/E"),
            (self.min_change, self.max_change, "change"),
        ] {
            if let (Some(lo), Some(hi)) = (lo, hi) {
                if lo > hi {
                    return Err(format!("The {what} minimum is above the maximum"));
                }
            }
        }
        Ok(())
    }
}

/// A stock with the figures screens, peers and calendars show. Money is
/// USD (converted from the stock's currency).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockRow {
    pub ticker: String,
    pub name: String,
    pub price: f64,
    pub change_pct: Option<f64>,
    pub market_cap: Option<f64>,
    pub pe: Option<f64>,
    pub forward_pe: Option<f64>,
    /// Fraction, e.g. 0.012 for 1.2%.
    pub dividend_yield: Option<f64>,
    pub volume: Option<f64>,
    pub high_52w: Option<f64>,
    pub low_52w: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScreenResult {
    /// Matches across all pages.
    pub total: u32,
    pub rows: Vec<StockRow>,
}

/// A stock's sector peers within the first index that lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerGroup {
    pub index: MarketIndex,
    pub sector: String,
    /// Largest first; includes the stock itself.
    pub rows: Vec<StockRow>,
}

// ─── Calendar ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventKind {
    Earnings,
    ExDividend,
    DividendPayment,
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Earnings => "Earnings",
            Self::ExDividend => "Ex-dividend",
            Self::DividendPayment => "Dividend paid",
        }
    }
}

/// An earnings report or dividend date for one stock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEvent {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub ticker: String,
    pub name: String,
    pub kind: EventKind,
    /// Dividends: forecast annual dividend per share, USD.
    pub annual_dividend: Option<f64>,
    /// Earnings: the date is Yahoo's estimate, not confirmed.
    pub estimated: bool,
}

/// A sector's size and market-cap-weighted move.
#[derive(Debug, Clone, PartialEq)]
pub struct SectorMove {
    pub sector: String,
    pub market_cap: f64,
    pub change_pct: f64,
    pub count: usize,
}

/// Market-cap-weighted change of `items`, percent; `None` without caps.
pub fn weighted_change<'a>(items: impl IntoIterator<Item = &'a HeatmapItem>) -> Option<f64> {
    let (weighted, total) = items
        .into_iter()
        .filter_map(|i| Some((i.change_pct?, i.market_cap?)))
        .fold((0.0, 0.0), |(w, t), (c, cap)| (w + c * cap, t + cap));
    (total > 0.0).then(|| weighted / total)
}

/// Sectors, largest first.
pub fn sector_moves(items: &[HeatmapItem]) -> Vec<SectorMove> {
    let mut sectors: Vec<String> = items.iter().filter_map(|i| i.sector.clone()).collect();
    sectors.sort();
    sectors.dedup();
    let mut moves: Vec<SectorMove> = sectors
        .into_iter()
        .filter_map(|sector| {
            let members: Vec<&HeatmapItem> = items
                .iter()
                .filter(|i| i.sector.as_deref() == Some(sector.as_str()))
                .collect();
            Some(SectorMove {
                market_cap: members.iter().filter_map(|i| i.market_cap).sum(),
                change_pct: weighted_change(members.iter().copied())?,
                count: members.len(),
                sector,
            })
        })
        .collect();
    moves.sort_by(|a, b| b.market_cap.total_cmp(&a.market_cap));
    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(ticker: &str, sector: &str, change: f64, cap: f64) -> HeatmapItem {
        HeatmapItem {
            ticker: ticker.into(),
            name: ticker.into(),
            sector: Some(sector.into()),
            price: 100.0,
            change_pct: Some(change),
            market_cap: Some(cap),
            pe: None,
            forward_pe: None,
            dividend_yield: None,
            high_52w: None,
            low_52w: None,
        }
    }

    #[test]
    fn weights_moves_by_market_cap() {
        let items = [
            item("A", "Tech", 2.0, 300.0),
            item("B", "Tech", -1.0, 100.0),
            item("C", "Energy", -3.0, 50.0),
        ];
        // (2·300 − 1·100) / 400 = 1.25
        let sectors = sector_moves(&items);
        assert_eq!(sectors[0].sector, "Tech");
        assert!((sectors[0].change_pct - 1.25).abs() < 1e-9);
        assert_eq!(sectors[0].count, 2);
        assert_eq!(sectors[1].sector, "Energy");
        // (600 − 100 − 150) / 450
        let all = weighted_change(&items).unwrap();
        assert!((all - 350.0 / 450.0).abs() < 1e-9);
        assert_eq!(weighted_change(&[]), None);
    }

    #[test]
    fn screen_filters_are_checked() {
        assert!(ScreenFilter::default().validate().is_ok());
        let bad = |f: ScreenFilter| f.validate().is_err();
        assert!(bad(ScreenFilter { region: "mars".into(), ..Default::default() }));
        assert!(bad(ScreenFilter { sector: Some("Tech".into()), ..Default::default() }));
        assert!(bad(ScreenFilter { min_pe: Some(30.0), max_pe: Some(10.0), ..Default::default() }));
    }
}
