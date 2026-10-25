//! Lines drawn on a stock's chart: price levels, trend lines and Fibonacci
//! retracements, kept with the stock so they show on every device.

use serde::{Deserialize, Serialize};

/// Retracement levels, as shares of the move from the first point to the
/// second.
pub const FIB_LEVELS: [f64; 7] = [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];

/// The most drawings kept for one stock.
pub const MAX_PER_TICKER: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawingKind {
    /// A horizontal line at one price.
    Level,
    /// A line through two points.
    Trend,
    /// Retracement levels between a swing's two ends.
    Fibonacci,
}

impl DrawingKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Level => "level",
            Self::Trend => "trend",
            Self::Fibonacci => "fibonacci",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        [Self::Level, Self::Trend, Self::Fibonacci].into_iter().find(|k| k.key() == key)
    }

    /// How many points it's drawn through.
    pub fn points(self) -> usize {
        match self {
            Self::Level => 1,
            Self::Trend | Self::Fibonacci => 2,
        }
    }
}

/// A point on a chart: when (Unix seconds) and at what price, in the
/// stock's own currency.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChartPoint {
    pub ts: i64,
    pub price: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChartDrawing {
    pub id: i64,
    pub kind: DrawingKind,
    pub points: Vec<ChartPoint>,
}

/// Checks a drawing before it's saved.
pub fn validate(kind: DrawingKind, points: &[ChartPoint]) -> Result<(), String> {
    if points.len() != kind.points() {
        return Err(format!("A {} needs {} point(s)", kind.key(), kind.points()));
    }
    if points.iter().any(|p| !p.price.is_finite() || p.price <= 0.0 || p.ts <= 0) {
        return Err("Points need a time and a price above zero".into());
    }
    Ok(())
}

/// The price of each retracement level between `from` and `to`, paired
/// with its level.
pub fn fib_prices(from: f64, to: f64) -> Vec<(f64, f64)> {
    FIB_LEVELS.iter().map(|l| (*l, to - (to - from) * l)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_points() {
        let p = |ts, price| ChartPoint { ts, price };
        assert!(validate(DrawingKind::Level, &[p(1, 10.0)]).is_ok());
        assert!(validate(DrawingKind::Trend, &[p(1, 10.0)]).is_err());
        assert!(validate(DrawingKind::Fibonacci, &[p(1, 10.0), p(2, f64::NAN)]).is_err());
        assert!(validate(DrawingKind::Trend, &[p(1, 10.0), p(2, 0.0)]).is_err());
        assert_eq!(DrawingKind::from_key("fibonacci"), Some(DrawingKind::Fibonacci));
        assert_eq!(DrawingKind::from_key("x"), None);
    }

    #[test]
    fn retracements_run_back_from_the_swing_end() {
        // A rise from 100 to 200: 0% at the top, 61.8% at 138.2, 100% at the start.
        let levels = fib_prices(100.0, 200.0);
        assert_eq!(levels[0], (0.0, 200.0));
        assert!((levels[4].1 - 138.2).abs() < 1e-9);
        assert_eq!(levels[6], (1.0, 100.0));
    }
}
