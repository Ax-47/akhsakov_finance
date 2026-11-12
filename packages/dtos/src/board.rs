//! The live board: one row per instrument with what a trading screen
//! shows (last, change, bid and ask, volume, the day's and the year's
//! range), in the instrument's own currency, kept current by streamed
//! trades between full refreshes.

use serde::{Deserialize, Serialize};

/// Which part of the trading day an instrument is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Session {
    /// Before the open, with pre-market trading.
    Pre,
    Regular,
    /// After the close, with after-hours trading.
    Post,
    #[default]
    Closed,
}

impl Session {
    /// From Yahoo's `marketState`.
    pub fn from_state(state: &str) -> Self {
        match state {
            "REGULAR" => Self::Regular,
            "PRE" => Self::Pre,
            "POST" => Self::Post,
            _ => Self::Closed,
        }
    }

    /// Short badge beside the price; nothing during regular hours.
    pub fn badge(self) -> Option<&'static str> {
        match self {
            Self::Pre => Some("PRE"),
            Self::Post => Some("POST"),
            Self::Closed => Some("CLOSED"),
            Self::Regular => None,
        }
    }
}

/// One instrument on the board. Money is in `currency`, except
/// `market_cap_usd`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BoardQuote {
    pub ticker: String,
    pub name: String,
    pub currency: String,
    #[serde(default)]
    pub exchange: Option<String>,
    /// Sector, when the board is an index.
    #[serde(default)]
    pub sector: Option<String>,
    /// Last regular-session trade.
    pub price: f64,
    #[serde(default)]
    pub previous_close: Option<f64>,
    #[serde(default)]
    pub open: Option<f64>,
    #[serde(default)]
    pub day_high: Option<f64>,
    #[serde(default)]
    pub day_low: Option<f64>,
    /// Shares traded so far today.
    #[serde(default)]
    pub volume: Option<f64>,
    /// Average daily volume over three months.
    #[serde(default)]
    pub avg_volume: Option<f64>,
    #[serde(default)]
    pub bid: Option<f64>,
    #[serde(default)]
    pub ask: Option<f64>,
    #[serde(default)]
    pub high_52w: Option<f64>,
    #[serde(default)]
    pub low_52w: Option<f64>,
    #[serde(default)]
    pub market_cap_usd: Option<f64>,
    #[serde(default)]
    pub pe: Option<f64>,
    #[serde(default)]
    pub session: Session,
    /// Last pre-market or after-hours trade, outside regular hours.
    #[serde(default)]
    pub ext_price: Option<f64>,
    /// Unix seconds of the last trade.
    #[serde(default)]
    pub time: Option<i64>,
}

/// A streamed trade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    pub price: f64,
    /// Shares traded so far today, when the stream says.
    pub volume: Option<f64>,
    pub time: i64,
}

/// Which way the last trade moved the price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
}

impl BoardQuote {
    /// Change since the previous close, in money.
    pub fn change(&self) -> Option<f64> {
        self.previous_close.filter(|p| *p > 0.0).map(|p| self.price - p)
    }

    /// Change since the previous close, percent.
    pub fn change_pct(&self) -> Option<f64> {
        self.previous_close.filter(|p| *p > 0.0).map(|p| (self.price / p - 1.0) * 100.0)
    }

    /// The pre-market or after-hours move from the regular close, percent.
    pub fn ext_change_pct(&self) -> Option<f64> {
        let ext = self.ext_price.filter(|_| self.session != Session::Regular)?;
        (self.price > 0.0).then(|| (ext / self.price - 1.0) * 100.0)
    }

    /// Today's volume against a normal day's: 1.0 is average. Early in the
    /// session this is low for every stock; it compares rows, not hours.
    pub fn relative_volume(&self) -> Option<f64> {
        let avg = self.avg_volume.filter(|a| *a > 0.0)?;
        Some(self.volume? / avg)
    }

    /// Money traded today, in `currency`.
    pub fn turnover(&self) -> Option<f64> {
        Some(self.volume? * self.price)
    }

    /// Ask minus bid as a share of the mid price, percent. `None` when the
    /// book is empty or crossed (a closed market often reports that).
    pub fn spread_pct(&self) -> Option<f64> {
        let (bid, ask) = (self.bid.filter(|b| *b > 0.0)?, self.ask.filter(|a| *a > 0.0)?);
        (ask >= bid).then(|| (ask - bid) / ((ask + bid) / 2.0) * 100.0)
    }

    /// Where the price sits in today's range, 0 (low) to 1 (high).
    pub fn day_position(&self) -> Option<f64> {
        position(self.price, self.day_low?, self.day_high?)
    }

    /// Where the price sits in the 52-week range, 0 (low) to 1 (high).
    pub fn year_position(&self) -> Option<f64> {
        position(self.price, self.low_52w?, self.high_52w?)
    }

    /// At (or within 0.1% of) the 52-week high.
    pub fn at_year_high(&self) -> bool {
        self.high_52w.is_some_and(|h| h > 0.0 && self.price >= h * 0.999)
    }

    /// At (or within 0.1% of) the 52-week low.
    pub fn at_year_low(&self) -> bool {
        self.low_52w.is_some_and(|l| l > 0.0 && self.price <= l * 1.001)
    }

    /// Applies a streamed trade and says which way the price moved. In
    /// regular hours it moves the last price and stretches the day's
    /// range; outside them it moves the extended-hours price only, so the
    /// day's change keeps meaning the regular session. Trades older than
    /// the one shown are ignored.
    pub fn apply(&mut self, tick: Tick) -> Option<Move> {
        if tick.price <= 0.0 || self.time.is_some_and(|t| tick.time < t) {
            return None;
        }
        let shown = match self.session {
            Session::Regular => self.price,
            _ => self.ext_price.unwrap_or(self.price),
        };
        match self.session {
            Session::Regular => {
                self.price = tick.price;
                self.day_high = Some(self.day_high.map_or(tick.price, |h| h.max(tick.price)));
                self.day_low = Some(self.day_low.map_or(tick.price, |l| l.min(tick.price)));
                // The count only grows within a day; a smaller one is a
                // late message.
                if let Some(v) = tick.volume.filter(|v| self.volume.is_none_or(|old| *v > old)) {
                    self.volume = Some(v);
                }
            }
            _ => self.ext_price = Some(tick.price),
        }
        self.time = Some(tick.time);
        if tick.price > shown {
            Some(Move::Up)
        } else if tick.price < shown {
            Some(Move::Down)
        } else {
            None
        }
    }
}

fn position(price: f64, low: f64, high: f64) -> Option<f64> {
    (high > low).then(|| ((price - low) / (high - low)).clamp(0.0, 1.0))
}

/// How many rows rose, fell and stood still, and how many sit at their
/// 52-week high or low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Breadth {
    pub up: usize,
    pub down: usize,
    pub flat: usize,
    pub year_highs: usize,
    pub year_lows: usize,
}

impl Breadth {
    pub fn of<'a>(rows: impl IntoIterator<Item = &'a BoardQuote>) -> Self {
        let mut b = Self::default();
        for row in rows {
            match row.change_pct() {
                Some(c) if c > 0.0 => b.up += 1,
                Some(c) if c < 0.0 => b.down += 1,
                Some(_) => b.flat += 1,
                None => {}
            }
            b.year_highs += row.at_year_high() as usize;
            b.year_lows += row.at_year_low() as usize;
        }
        b
    }

    pub fn counted(&self) -> usize {
        self.up + self.down + self.flat
    }
}

/// Column the board can be sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoardSort {
    Ticker,
    Price,
    ChangePct,
    Volume,
    RelativeVolume,
    Turnover,
    DayPosition,
    YearPosition,
    MarketCap,
}

impl BoardSort {
    /// The number rows are ordered by; `None` sorts last either way.
    fn value(self, q: &BoardQuote) -> Option<f64> {
        match self {
            Self::Ticker => None,
            Self::Price => Some(q.price),
            Self::ChangePct => q.change_pct(),
            Self::Volume => q.volume,
            Self::RelativeVolume => q.relative_volume(),
            Self::Turnover => q.turnover(),
            Self::DayPosition => q.day_position(),
            Self::YearPosition => q.year_position(),
            Self::MarketCap => q.market_cap_usd,
        }
    }
}

/// Sorts `rows` by `key`. Rows without the value go last whichever way
/// round, and equal rows keep their order.
pub fn sort_board(rows: &mut [BoardQuote], key: BoardSort, descending: bool) {
    use std::cmp::Ordering;
    rows.sort_by(|a, b| {
        if key == BoardSort::Ticker {
            let o = a.ticker.cmp(&b.ticker);
            return if descending { o.reverse() } else { o };
        }
        match (key.value(a), key.value(b)) {
            (Some(x), Some(y)) => {
                let o = x.total_cmp(&y);
                if descending { o.reverse() } else { o }
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    });
}

/// Rows whose ticker or name contains `needle` (any case).
pub fn matches(row: &BoardQuote, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    needle.is_empty() || row.ticker.to_lowercase().contains(&needle) || row.name.to_lowercase().contains(&needle)
}

/// Markets around the world for the strip above the board, as (Yahoo
/// symbol, name): indices, volatility, rates, the dollar, commodities and
/// crypto.
pub const WORLD: [(&str, &str); 16] = [
    ("^GSPC", "S&P 500"),
    ("^IXIC", "Nasdaq"),
    ("^DJI", "Dow Jones"),
    ("^RUT", "Russell 2000"),
    ("^VIX", "VIX"),
    ("^SET.BK", "SET"),
    ("^N225", "Nikkei 225"),
    ("^HSI", "Hang Seng"),
    ("^STOXX50E", "Euro Stoxx 50"),
    ("^FTSE", "FTSE 100"),
    ("^TNX", "US 10-year yield"),
    ("DX-Y.NYB", "US dollar index"),
    ("THB=X", "USD/THB"),
    ("GC=F", "Gold"),
    ("CL=F", "Crude oil"),
    ("BTC-USD", "Bitcoin"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn row(ticker: &str, price: f64, previous: f64) -> BoardQuote {
        BoardQuote {
            ticker: ticker.into(),
            name: format!("{ticker} Inc."),
            currency: "USD".into(),
            price,
            previous_close: Some(previous),
            session: Session::Regular,
            ..Default::default()
        }
    }

    #[test]
    fn change_is_from_the_previous_close() {
        let q = row("AMD", 110.0, 100.0);
        assert_eq!(q.change(), Some(10.0));
        assert!((q.change_pct().unwrap() - 10.0).abs() < 1e-9);
        assert_eq!(BoardQuote { previous_close: None, ..q.clone() }.change_pct(), None);
        assert_eq!(BoardQuote { previous_close: Some(0.0), ..q }.change(), None, "no close, no change");
    }

    #[test]
    fn a_trade_in_regular_hours_moves_the_price_and_the_range() {
        let mut q = BoardQuote { day_high: Some(111.0), day_low: Some(105.0), volume: Some(1_000.0), time: Some(100), ..row("AMD", 110.0, 100.0) };
        assert_eq!(q.apply(Tick { price: 112.0, volume: Some(1_500.0), time: 101 }), Some(Move::Up));
        assert_eq!((q.price, q.day_high, q.volume, q.time), (112.0, Some(112.0), Some(1_500.0), Some(101)));
        assert_eq!(q.apply(Tick { price: 104.0, volume: Some(1_400.0), time: 102 }), Some(Move::Down));
        assert_eq!((q.day_low, q.volume), (Some(104.0), Some(1_500.0)), "the day's volume never shrinks");
        assert_eq!(q.apply(Tick { price: 104.0, volume: None, time: 103 }), None, "same price, no move");
    }

    #[test]
    fn late_and_empty_trades_are_ignored() {
        let mut q = BoardQuote { time: Some(100), ..row("AMD", 110.0, 100.0) };
        assert_eq!(q.apply(Tick { price: 90.0, volume: None, time: 99 }), None);
        assert_eq!(q.apply(Tick { price: 0.0, volume: None, time: 200 }), None);
        assert_eq!((q.price, q.time), (110.0, Some(100)));
    }

    #[test]
    fn a_trade_after_hours_leaves_the_days_close_alone() {
        let mut q = BoardQuote { session: Session::Post, day_high: Some(111.0), ..row("AMD", 110.0, 100.0) };
        assert_eq!(q.apply(Tick { price: 121.0, volume: Some(9.0), time: 5 }), Some(Move::Up));
        assert_eq!((q.price, q.day_high, q.ext_price), (110.0, Some(111.0), Some(121.0)));
        assert!((q.change_pct().unwrap() - 10.0).abs() < 1e-9, "the day's change is the regular session's");
        assert!((q.ext_change_pct().unwrap() - 10.0).abs() < 1e-9, "after hours: 110 → 121");
        assert_eq!(q.apply(Tick { price: 115.5, volume: None, time: 6 }), Some(Move::Down), "against the last after-hours trade");
        assert_eq!(BoardQuote { session: Session::Regular, ..q }.ext_change_pct(), None);
    }

    #[test]
    fn spread_needs_both_sides_of_the_book() {
        let q = BoardQuote { bid: Some(99.0), ask: Some(101.0), ..row("AMD", 100.0, 100.0) };
        assert!((q.spread_pct().unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(BoardQuote { bid: Some(0.0), ..q.clone() }.spread_pct(), None);
        assert_eq!(BoardQuote { ask: Some(98.0), ..q }.spread_pct(), None, "a crossed book has no spread");
    }

    #[test]
    fn ranges_and_volume_compare_rows() {
        let q = BoardQuote {
            day_low: Some(90.0),
            day_high: Some(110.0),
            low_52w: Some(50.0),
            high_52w: Some(100.0),
            volume: Some(3_000.0),
            avg_volume: Some(2_000.0),
            ..row("AMD", 100.0, 95.0)
        };
        assert_eq!(q.day_position(), Some(0.5));
        assert_eq!(q.year_position(), Some(1.0));
        assert!(q.at_year_high() && !q.at_year_low());
        assert_eq!(q.relative_volume(), Some(1.5));
        assert_eq!(q.turnover(), Some(300_000.0));
        assert_eq!(BoardQuote { day_high: Some(90.0), ..q }.day_position(), None, "no range yet");
    }

    #[test]
    fn breadth_counts_each_row_once() {
        let rows = vec![
            row("A", 11.0, 10.0),
            row("B", 9.0, 10.0),
            row("C", 10.0, 10.0),
            BoardQuote { previous_close: None, ..row("D", 5.0, 0.0) },
            BoardQuote { high_52w: Some(12.0), low_52w: Some(6.0), ..row("E", 12.0, 11.0) },
        ];
        let b = Breadth::of(&rows);
        assert_eq!((b.up, b.down, b.flat, b.year_highs, b.year_lows), (2, 1, 1, 1, 0));
        assert_eq!(b.counted(), 4);
    }

    #[test]
    fn sorting_puts_unknowns_last_both_ways() {
        let mut rows = vec![
            row("B", 10.0, 10.0),
            BoardQuote { previous_close: None, ..row("X", 1.0, 0.0) },
            row("A", 12.0, 10.0),
            row("C", 9.0, 10.0),
        ];
        let order = |rows: &[BoardQuote]| rows.iter().map(|r| r.ticker.clone()).collect::<Vec<_>>().join("");
        sort_board(&mut rows, BoardSort::ChangePct, true);
        assert_eq!(order(&rows), "ABCX");
        sort_board(&mut rows, BoardSort::ChangePct, false);
        assert_eq!(order(&rows), "CBAX");
        sort_board(&mut rows, BoardSort::Ticker, false);
        assert_eq!(order(&rows), "ABCX");
    }

    #[test]
    fn rows_are_found_by_ticker_or_name() {
        let q = BoardQuote { name: "Advanced Micro Devices".into(), ..row("AMD", 1.0, 1.0) };
        assert!(matches(&q, "amd") && matches(&q, " micro ") && matches(&q, ""));
        assert!(!matches(&q, "nvda"));
    }

    #[test]
    fn sessions_come_from_yahoos_market_state() {
        assert_eq!(Session::from_state("REGULAR"), Session::Regular);
        assert_eq!(Session::from_state("PRE"), Session::Pre);
        assert_eq!(Session::from_state("POST"), Session::Post);
        for closed in ["CLOSED", "PREPRE", "POSTPOST", ""] {
            assert_eq!(Session::from_state(closed), Session::Closed);
        }
        assert_eq!(Session::Regular.badge(), None);
    }
}
