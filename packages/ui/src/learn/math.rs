//! The arithmetic behind the lesson calculators. Rates and returns are
//! fractions (0.08 = 8%) and volatility is yearly, as in the risk report.

use crate::components::analysis::stats::TRADING_DAYS;

/// z-scores of a normal distribution for one-sided 95% and 99%.
pub const Z_95: f64 = 1.645;
pub const Z_99: f64 = 2.326;
/// Average of the worst 5% of a standard normal, in standard deviations
/// (φ(1.645) ÷ 0.05): expected shortfall at 95%.
pub const ES_95: f64 = 2.063;

/// Price ÷ earnings per share; `None` without positive earnings.
pub fn pe(price: f64, eps: f64) -> Option<f64> {
    (price > 0.0 && eps > 0.0).then(|| price / eps)
}

/// P/E ÷ yearly EPS growth in percent; `None` without positive growth.
pub fn peg(pe: f64, growth_pct: f64) -> Option<f64> {
    (growth_pct > 0.0).then(|| pe / growth_pct)
}

/// Typical one-day move for a yearly volatility.
pub fn daily_vol(yearly: f64) -> f64 {
    yearly / TRADING_DAYS.sqrt()
}

/// One-year return range `sigmas` standard deviations either side of
/// `mean` (bell-curve assumption). You can't lose more than everything.
pub fn yearly_range(mean: f64, vol: f64, sigmas: f64) -> (f64, f64) {
    ((mean - sigmas * vol).max(-1.0), mean + sigmas * vol)
}

/// Gain needed to get back to even after losing `loss` (0–1).
pub fn recovery_gain(loss: f64) -> f64 {
    1.0 / (1.0 - loss.min(0.999_9)) - 1.0
}

/// Years to make up `loss` growing at `rate` a year; `None` if it never does.
pub fn years_to_recover(loss: f64, rate: f64) -> Option<f64> {
    (rate > 0.0).then(|| (1.0 + recovery_gain(loss)).ln() / (1.0 + rate).ln())
}

/// (Return − risk-free) ÷ volatility; `None` without volatility.
pub fn sharpe(ret: f64, vol: f64, risk_free: f64) -> Option<f64> {
    (vol > 0.0).then(|| (ret - risk_free) / vol)
}

/// Volatility of `weight` in A and the rest in B.
pub fn two_asset_vol(weight: f64, vol_a: f64, vol_b: f64, corr: f64) -> f64 {
    let (a, b) = (weight * vol_a, (1.0 - weight) * vol_b);
    (a * a + b * b + 2.0 * corr.clamp(-1.0, 1.0) * a * b).max(0.0).sqrt()
}

/// Largest position whose plausible `fall` costs at most `accept` of the
/// portfolio, capped at the whole portfolio.
pub fn max_position(portfolio: f64, accept: f64, fall: f64) -> Option<f64> {
    (fall > 0.0 && accept >= 0.0).then(|| (portfolio * accept / fall).min(portfolio))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn valuation() {
        assert_eq!(pe(150.0, 6.0), Some(25.0));
        assert_eq!(pe(150.0, 0.0), None);
        assert_eq!(pe(150.0, -2.0), None);
        // The valuation lesson's quiz: A has the lower PEG.
        assert_eq!(peg(30.0, 30.0), Some(1.0));
        assert_eq!(peg(15.0, 5.0), Some(3.0));
        assert_eq!(peg(15.0, 0.0), None);
    }

    #[test]
    fn volatility() {
        // A 1% typical day is about 16% a year.
        assert!(close(daily_vol(0.01 * TRADING_DAYS.sqrt()), 0.01));
        assert!(close(0.01 * TRADING_DAYS.sqrt(), 0.1587));
        // The volatility lesson's quiz.
        let (lo, hi) = yearly_range(0.08, 0.40, 1.0);
        assert!(close(lo, -0.32) && close(hi, 0.48));
        assert_eq!(yearly_range(0.08, 0.60, 2.0).0, -1.0);
    }

    #[test]
    fn drawdowns() {
        assert!(close(recovery_gain(0.2), 0.25));
        assert!(close(recovery_gain(0.5), 1.0));
        assert!(close(recovery_gain(0.8), 4.0));
        assert!(recovery_gain(1.0).is_finite());
        // Doubling at 7.18% a year takes about ten years.
        assert!((years_to_recover(0.5, 0.0718).unwrap() - 10.0).abs() < 0.01);
        assert_eq!(years_to_recover(0.3, 0.0), None);
    }

    #[test]
    fn risk_adjusted() {
        // The risk-adjusted lesson's quiz: B has the higher Sharpe.
        assert!(close(sharpe(0.15, 0.25, 0.04).unwrap(), 0.44));
        assert!(close(sharpe(0.10, 0.10, 0.04).unwrap(), 0.60));
        assert_eq!(sharpe(0.1, 0.0, 0.04), None);
        // Normal tail constants.
        const { assert!(Z_95 < ES_95 && ES_95 < Z_99) };
    }

    #[test]
    fn diversification() {
        // Perfectly correlated: just the weighted average.
        assert!(close(two_asset_vol(0.5, 0.3, 0.2, 1.0), 0.25));
        // Uncorrelated and opposite.
        assert!(close(two_asset_vol(0.5, 0.3, 0.2, 0.0), 0.1803));
        assert!(close(two_asset_vol(0.5, 0.3, 0.2, -1.0), 0.05));
        // Lower correlation, lower risk.
        assert!(two_asset_vol(0.5, 0.3, 0.2, 0.3) < two_asset_vol(0.5, 0.3, 0.2, 0.6));
    }

    #[test]
    fn position_sizing() {
        // The lesson's example: 2% of 1,000,000 on a stock that could fall 40%.
        assert!(close(max_position(1_000_000.0, 0.02, 0.40).unwrap(), 50_000.0));
        assert_eq!(max_position(100.0, 0.5, 0.2), Some(100.0));
        assert_eq!(max_position(100.0, 0.02, 0.0), None);
    }
}
