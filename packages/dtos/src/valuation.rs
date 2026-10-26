//! Is the price reasonable? Three plain checks from the fundamentals:
//! today's P/E against the stock's own history, the Graham number, and a
//! discounted cash flow whose assumptions the reader can change.

use crate::fundamentals::KeyStats;

/// Today's P/E within the range of the stock's P/E at past period ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeBand {
    pub current: f64,
    pub low: f64,
    pub median: f64,
    pub high: f64,
    /// How many past P/Es the range is drawn from.
    pub samples: usize,
}

/// Fewer past P/Es than this and a "usual range" says little.
const MIN_PE_SAMPLES: usize = 4;

impl PeBand {
    /// `history` holds P/Es at past dates; losses (P/E ≤ 0) are left out,
    /// as is a band with too few points to call a range.
    pub fn new(current: Option<f64>, history: impl IntoIterator<Item = f64>) -> Option<Self> {
        let current = current.filter(|pe| *pe > 0.0)?;
        let mut past: Vec<f64> = history.into_iter().filter(|pe| *pe > 0.0 && pe.is_finite()).collect();
        if past.len() < MIN_PE_SAMPLES {
            return None;
        }
        past.sort_by(f64::total_cmp);
        let middle = past.len() / 2;
        let median = if past.len() % 2 == 0 { (past[middle - 1] + past[middle]) / 2.0 } else { past[middle] };
        Some(Self { current, low: past[0], median, high: past[past.len() - 1], samples: past.len() })
    }

    /// Today's P/E against the median: −0.2 is 20% cheaper than usual.
    pub fn against_median(&self) -> f64 {
        self.current / self.median - 1.0
    }
}

/// Benjamin Graham's most a defensive investor should pay for a share:
/// √(22.5 × EPS × book value per share). Only for a profitable company
/// with positive book value.
pub fn graham_number(eps: f64, book_value_per_share: f64) -> Option<f64> {
    (eps > 0.0 && book_value_per_share > 0.0).then(|| (22.5 * eps * book_value_per_share).sqrt())
}

/// What the discounted cash flow assumes, as fractions a year.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DcfAssumptions {
    /// Free cash flow growth for the first five years.
    pub growth: f64,
    /// Growth for ever after year ten; at most the economy's long-run pace.
    pub terminal_growth: f64,
    /// The return you'd want for the risk: future cash is discounted by it.
    pub discount_rate: f64,
}

/// Fallback first-five-years growth when analysts give no estimate.
const DEFAULT_GROWTH: f64 = 0.05;
/// Analyst growth estimates are capped: few companies keep up more for
/// five years, and the model is very sensitive to this number.
const MAX_SUGGESTED_GROWTH: f64 = 0.20;
const DEFAULT_TERMINAL_GROWTH: f64 = 0.025;
const DEFAULT_DISCOUNT_RATE: f64 = 0.09;
const HIGH_GROWTH_YEARS: u32 = 5;
const FORECAST_YEARS: u32 = 10;

impl DcfAssumptions {
    /// A starting point: analysts' 5-year EPS growth (capped), long-run
    /// growth of 2.5% and a 9% required return.
    pub fn suggested(stats: &KeyStats) -> Self {
        Self {
            growth: stats
                .eps_growth_5y
                .map_or(DEFAULT_GROWTH, |g| g.clamp(0.0, MAX_SUGGESTED_GROWTH)),
            terminal_growth: DEFAULT_TERMINAL_GROWTH,
            discount_rate: DEFAULT_DISCOUNT_RATE,
        }
    }
}

/// Why there's no discounted cash flow value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DcfError {
    /// Free cash flow is missing, zero or negative: nothing to project.
    NoCashFlow,
    NoShareCount,
    /// A perpetuity growing as fast as it's discounted is worth infinity.
    DiscountNotAboveTerminalGrowth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DcfValue {
    pub per_share: f64,
    /// Share of the company's value that comes after year ten: the higher,
    /// the more the result rests on the long-run guesses.
    pub terminal_share: f64,
}

/// Ten years of free cash flow, growing at `growth` for five years then
/// easing in equal steps to `terminal_growth`; after that a perpetuity
/// growing at `terminal_growth`. Everything is discounted at
/// `discount_rate`, net debt is taken off, and the rest split per share.
pub fn discounted_cash_flow(
    free_cash_flow: Option<f64>,
    net_debt: f64,
    shares: Option<f64>,
    assumptions: DcfAssumptions,
) -> Result<DcfValue, DcfError> {
    let DcfAssumptions { growth, terminal_growth, discount_rate } = assumptions;
    let mut cash = free_cash_flow.filter(|f| *f > 0.0).ok_or(DcfError::NoCashFlow)?;
    let shares = shares.filter(|s| *s > 0.0).ok_or(DcfError::NoShareCount)?;
    if discount_rate <= terminal_growth {
        return Err(DcfError::DiscountNotAboveTerminalGrowth);
    }
    let fade_years = (FORECAST_YEARS - HIGH_GROWTH_YEARS) as f64;
    let mut forecast_value = 0.0;
    for year in 1..=FORECAST_YEARS {
        let rate = if year <= HIGH_GROWTH_YEARS {
            growth
        } else {
            let step = (year - HIGH_GROWTH_YEARS) as f64 / fade_years;
            growth + (terminal_growth - growth) * step
        };
        cash *= 1.0 + rate;
        forecast_value += cash / (1.0 + discount_rate).powi(year as i32);
    }
    let terminal = cash * (1.0 + terminal_growth) / (discount_rate - terminal_growth);
    let terminal_value = terminal / (1.0 + discount_rate).powi(FORECAST_YEARS as i32);
    let company = forecast_value + terminal_value;
    Ok(DcfValue {
        per_share: (company - net_debt) / shares,
        terminal_share: terminal_value / company,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pe_band_takes_the_median_and_skips_losses() {
        let band = PeBand::new(Some(15.0), [20.0, 10.0, -5.0, 30.0, 25.0]).unwrap();
        assert_eq!((band.low, band.median, band.high, band.samples), (10.0, 22.5, 30.0, 4));
        assert!((band.against_median() - (15.0 / 22.5 - 1.0)).abs() < 1e-12);
        assert_eq!(PeBand::new(Some(15.0), [20.0, 10.0, 30.0]), None, "too few to call a range");
        assert_eq!(PeBand::new(Some(-3.0), [20.0, 10.0, 30.0, 25.0]), None, "no P/E today");
    }

    #[test]
    fn graham_number_needs_profit_and_book_value() {
        assert_eq!(graham_number(2.0, 20.0), Some(30.0)); // √900
        assert_eq!(graham_number(-1.0, 20.0), None);
        assert_eq!(graham_number(2.0, 0.0), None);
    }

    #[test]
    fn flat_cash_flow_is_worth_a_perpetuity() {
        // No growth at all: ten years plus the tail add up to F / r.
        let flat = DcfAssumptions { growth: 0.0, terminal_growth: 0.0, discount_rate: 0.1 };
        let value = discounted_cash_flow(Some(100.0), 0.0, Some(10.0), flat).unwrap();
        assert!((value.per_share - 100.0).abs() < 1e-9); // 1000 / 10 shares
        // Net debt comes off before splitting per share.
        let with_debt = discounted_cash_flow(Some(100.0), 200.0, Some(10.0), flat).unwrap();
        assert!((with_debt.per_share - 80.0).abs() < 1e-9);
    }

    #[test]
    fn more_growth_or_a_lower_discount_rate_is_worth_more() {
        let base = DcfAssumptions { growth: 0.05, terminal_growth: 0.02, discount_rate: 0.09 };
        let value = |a| discounted_cash_flow(Some(100.0), 0.0, Some(10.0), a).unwrap().per_share;
        assert!(value(DcfAssumptions { growth: 0.10, ..base }) > value(base));
        assert!(value(DcfAssumptions { discount_rate: 0.08, ..base }) > value(base));
        let share = discounted_cash_flow(Some(100.0), 0.0, Some(10.0), base).unwrap().terminal_share;
        assert!(share > 0.5 && share < 1.0, "{share}");
    }

    #[test]
    fn dcf_says_why_it_has_no_value() {
        let a = DcfAssumptions { growth: 0.05, terminal_growth: 0.03, discount_rate: 0.03 };
        assert_eq!(discounted_cash_flow(Some(100.0), 0.0, Some(1.0), a), Err(DcfError::DiscountNotAboveTerminalGrowth));
        assert_eq!(discounted_cash_flow(Some(-5.0), 0.0, Some(1.0), a), Err(DcfError::NoCashFlow));
        assert_eq!(discounted_cash_flow(Some(5.0), 0.0, None, a), Err(DcfError::NoShareCount));
    }

    #[test]
    fn suggested_growth_comes_from_analysts_capped() {
        let stats = |g| KeyStats { eps_growth_5y: g, ..Default::default() };
        assert_eq!(DcfAssumptions::suggested(&stats(Some(0.12))).growth, 0.12);
        assert_eq!(DcfAssumptions::suggested(&stats(Some(0.45))).growth, MAX_SUGGESTED_GROWTH);
        assert_eq!(DcfAssumptions::suggested(&stats(None)).growth, DEFAULT_GROWTH);
    }
}
