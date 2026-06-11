use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use types::ticker_symbol::TickerSymbol;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ConcentrationRisk {
    Low,
    Moderate,
    High,
}

impl ConcentrationRisk {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
        }
    }
    pub fn color(&self) -> &'static str {
        match self {
            Self::Low => "text-ctp-green",
            Self::Moderate => "text-ctp-yellow",
            Self::High => "text-ctp-red",
        }
    }
    pub fn bar_color(&self) -> &'static str {
        match self {
            Self::Low => "bg-ctp-green",
            Self::Moderate => "bg-ctp-yellow",
            Self::High => "bg-ctp-red",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MptAnalysis {
    /// Herfindahl-Hirschman Index — sum of squared weights (0–1).
    /// Lower = more diversified; >0.25 is highly concentrated.
    pub hhi: Decimal,

    /// Effective number of independent bets (1 / HHI).
    /// A 10-stock portfolio with HHI=0.5 behaves like 2 equal-weight stocks.
    pub effective_n: Decimal,

    /// Qualitative concentration bucket derived from HHI.
    pub concentration_risk: ConcentrationRisk,

    /// Ticker with the largest portfolio weight + that weight (%).
    pub top_holding: (TickerSymbol, Decimal),

    /// % of open positions currently showing an unrealized gain.
    pub win_rate: Decimal,

    /// Weighted-average unrealized return across all positions (%).
    /// Each position's return is weighted by its share of total portfolio value.
    pub weighted_avg_return: Decimal,

    /// Cross-sectional standard deviation of per-position unrealized returns (%).
    /// High dispersion = some positions diverging strongly from the pack.
    pub return_dispersion: Decimal,

    /// 0–100 score: how close the current weights are to the equal-weight ideal.
    /// 100 = perfectly balanced; 0 = single-asset portfolio.
    pub diversification_score: Decimal,

    /// Per-position weight breakdown for the bar chart: (ticker, weight_pct).
    pub weights: Vec<(TickerSymbol, Decimal)>,

    /// Total number of live-priced positions used in the analysis.
    pub live_positions: usize,
}

// ─── Main computation ─────────────────────────────────────────────────────────

/// Compute MPT-flavoured portfolio analytics.
///
/// Returns `None` when no live-priced positions are available (e.g. prices not
/// loaded yet or the portfolio is empty).
pub fn compute_mpt(positions: &[Position], total_value: Decimal) -> Option<MptAnalysis> {
    if positions.is_empty() || total_value <= Decimal::ZERO {
        return None;
    }

    // Only analyse positions that have a live market price.
    let live: Vec<&Position> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .collect();

    if live.is_empty() {
        return None;
    }

    let n = live.len();

    // ── Weights ────────────────────────────────────────────────────────────────
    let weights_decimal: Vec<Decimal> = live
        .iter()
        .map(|p| p.market_value() / total_value)
        .collect();

    // ── HHI ───────────────────────────────────────────────────────────────────
    // HHI = Σ wᵢ²  (range 1/N … 1; lower is better)
    let hhi: Decimal = weights_decimal.iter().map(|w| w * w).sum();

    let effective_n = if hhi > Decimal::ZERO {
        (Decimal::ONE / hhi).round_dp(2)
    } else {
        Decimal::ZERO
    };

    let concentration_risk = if hhi < dec!(0.15) {
        ConcentrationRisk::Low
    } else if hhi < dec!(0.25) {
        ConcentrationRisk::Moderate
    } else {
        ConcentrationRisk::High
    };

    // ── Top holding ────────────────────────────────────────────────────────────
    let top_holding = live
        .iter()
        .zip(weights_decimal.iter())
        .max_by(|a, b| a.1.cmp(b.1))
        .map(|(p, w)| (p.ticker.clone(), (w * dec!(100)).round_dp(1)))
        .unwrap_or_default();

    // ── Win rate ───────────────────────────────────────────────────────────────
    let profitable = live
        .iter()
        .filter(|p| p.unrealized_pnl() >= Decimal::ZERO)
        .count();
    let win_rate = (Decimal::from(profitable) / Decimal::from(n) * dec!(100)).round_dp(1);

    // ── Returns ────────────────────────────────────────────────────────────────
    let returns: Vec<Decimal> = live.iter().map(|p| p.unrealized_pnl_pct()).collect();

    // Weighted average return  Σ (wᵢ * rᵢ)
    let weighted_avg_return: Decimal = returns
        .iter()
        .zip(weights_decimal.iter())
        .map(|(r, w)| r * w)
        .sum::<Decimal>()
        .round_dp(2);

    // Cross-sectional return dispersion (population std-dev)
    let mean_ret: Decimal = returns.iter().copied().sum::<Decimal>() / Decimal::from(n);
    let variance: Decimal = returns
        .iter()
        .map(|r| {
            let d = r - mean_ret;
            d * d
        })
        .sum::<Decimal>()
        / Decimal::from(n);

    let return_dispersion = {
        let v_f64 = variance.to_f64().unwrap_or(0.0).sqrt();
        Decimal::try_from(v_f64)
            .unwrap_or(Decimal::ZERO)
            .round_dp(2)
    };

    // ── Diversification score ─────────────────────────────────────────────────
    // Normalise HHI: score = (HHI_max - HHI) / (HHI_max - HHI_min)
    //   where HHI_max = 1  (single asset)
    //         HHI_min = 1/N (equal weight)
    //
    // → 100 when perfectly equal-weighted, 0 when single-asset.
    let equal_hhi = Decimal::ONE / Decimal::from(n);
    let diversification_score = if n > 1 {
        let raw = (Decimal::ONE - hhi) / (Decimal::ONE - equal_hhi);
        (raw.max(Decimal::ZERO).min(Decimal::ONE) * dec!(100)).round_dp(1)
    } else {
        Decimal::ZERO
    };

    // ── Weight breakdown for chart ─────────────────────────────────────────────
    let mut weights: Vec<(TickerSymbol, Decimal)> = live
        .iter()
        .zip(weights_decimal.iter())
        .map(|(p, w)| (p.ticker.clone(), (w * dec!(100)).round_dp(1)))
        .collect();
    weights.sort_by(|a, b| b.1.cmp(&a.1));

    Some(MptAnalysis {
        hhi: hhi.round_dp(4),
        effective_n,
        concentration_risk,
        top_holding,
        win_rate,
        weighted_avg_return,
        return_dispersion,
        diversification_score,
        weights,
        live_positions: n,
    })
}
