//! Market-wide views: index and watchlist heatmaps.

use serde::{Deserialize, Serialize};

/// Indices with a heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketIndex {
    Sp500,
}

impl MarketIndex {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sp500 => "S&P 500",
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
}
