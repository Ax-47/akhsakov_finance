/// Capital Asset Pricing Model (CAPM) analytics.
///
/// CAPM formula:  E(rᵢ) = Rf + βᵢ · (Rm − Rf)
///
/// Metrics computed here:
///   - Portfolio beta          βₚ = Σ wᵢβᵢ
///   - Expected return (CAPM)  E(rᵢ) per position and for the whole portfolio
///   - Jensen's Alpha          αᵢ = actual_rᵢ − E(rᵢ)
///   - Treynor Ratio           (rᵢ − Rf) / βᵢ  (per position + portfolio)
///   - Sharpe Ratio            (rₚ − Rf) / σₚ   (uses cross-sectional σ as proxy)
///   - Information Ratio       αₚ / σ(αᵢ)        (portfolio alpha / alpha dispersion)
///
/// Beta source: the caller supplies a `beta_map`.  If a ticker is absent the
/// default beta is 1.0 (the market itself), which is a reasonable fallback
/// until the price-feed or a separate endpoint provides real betas.
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use std::collections::HashMap;

// ─── Public inputs / outputs ──────────────────────────────────────────────────

/// Parameters that the user can tune interactively.
#[derive(Clone, Debug, PartialEq)]
pub struct CAPMInputs {
    /// Annualised risk-free rate in % (e.g. 5.25 for 5.25 %).
    pub rf: Decimal,
    /// Annualised expected market return in % (e.g. 10.0 for 10 %).
    pub rm: Decimal,
}

impl Default for CAPMInputs {
    fn default() -> Self {
        Self {
            rf: dec!(5.25),
            rm: dec!(10.0),
        }
    }
}

impl CAPMInputs {
    /// Market risk premium  Rm − Rf.
    pub fn market_premium(&self) -> Decimal {
        self.rm - self.rf
    }
}

/// CAPM metrics for a single position.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionCAPM {
    pub ticker: String,
    /// Portfolio weight (0–1).
    pub weight: Decimal,
    /// Beta used (1.0 if not supplied in beta_map).
    pub beta: Decimal,
    /// CAPM expected return %:  Rf + β(Rm − Rf).
    pub expected_return: Decimal,
    /// Actual unrealized return % for this position.
    pub actual_return: Decimal,
    /// Jensen's Alpha %:  actual − expected.
    pub alpha: Decimal,
    /// Treynor Ratio:  (actual − Rf) / β.
    /// `None` when β ≈ 0 (division undefined).
    pub treynor: Option<Decimal>,
}

/// Aggregate CAPM metrics for the whole portfolio.
#[derive(Clone, Debug, PartialEq)]
pub struct PortfolioCAPM {
    pub inputs: CAPMInputs,
    /// Weighted average beta  Σ wᵢβᵢ.
    pub portfolio_beta: Decimal,
    /// CAPM expected return for the portfolio.
    pub portfolio_expected_return: Decimal,
    /// Weighted average of position alphas.
    pub portfolio_alpha: Decimal,
    /// Sharpe Ratio:  (rₚ − Rf) / σₚ  (σ = cross-sectional return std-dev).
    /// `None` when σ ≈ 0 (single position or all returns identical).
    pub sharpe_ratio: Option<Decimal>,
    /// Portfolio Treynor Ratio:  (rₚ − Rf) / βₚ.
    pub treynor_ratio: Option<Decimal>,
    /// Information Ratio:  αₚ / σ(αᵢ).
    pub information_ratio: Option<Decimal>,
    /// Actual weighted-average portfolio return (unrealized).
    pub portfolio_actual_return: Decimal,
    /// Per-position breakdown, sorted by alpha descending.
    pub positions: Vec<PositionCAPM>,
}

// ─── Main computation ─────────────────────────────────────────────────────────

/// Compute CAPM analytics.
///
/// Returns `None` when no live-priced positions are available.
///
/// * `positions`  — from `PortfolioState`
/// * `total_value`— from `PortfolioState`
/// * `beta_map`   — ticker → beta (missing tickers default to 1.0)
/// * `inputs`     — Rf and Rm the user has selected
pub fn compute_capm(
    positions: &[Position],
    total_value: Decimal,
    beta_map: &HashMap<String, Decimal>,
    inputs: &CAPMInputs,
) -> Option<PortfolioCAPM> {
    if positions.is_empty() || total_value <= Decimal::ZERO {
        return None;
    }

    let live: Vec<&Position> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .collect();

    if live.is_empty() {
        return None;
    }

    let rf = inputs.rf;
    let premium = inputs.market_premium();

    // ── Per-position metrics ───────────────────────────────────────────────────
    let mut pos_capm: Vec<PositionCAPM> = live
        .iter()
        .map(|p| {
            let weight = p.market_value() / total_value;
            let beta = *beta_map.get(&p.ticker).unwrap_or(&Decimal::ONE);
            let expected_return = (rf + beta * premium).round_dp(4);
            let actual_return = p.unrealized_pnl_pct().round_dp(4);
            let alpha = (actual_return - expected_return).round_dp(4);
            let treynor = if beta.abs() > dec!(0.0001) {
                Some(((actual_return - rf) / beta).round_dp(4))
            } else {
                None
            };
            PositionCAPM {
                ticker: p.ticker.clone(),
                weight,
                beta,
                expected_return,
                actual_return,
                alpha,
                treynor,
            }
        })
        .collect();

    // Sort by alpha descending (best alpha-generators first).
    pos_capm.sort_by(|a, b| {
        b.alpha
            .partial_cmp(&a.alpha)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // ── Portfolio-level aggregates ─────────────────────────────────────────────
    let portfolio_beta: Decimal = pos_capm
        .iter()
        .map(|p| p.weight * p.beta)
        .sum::<Decimal>()
        .round_dp(4);

    let portfolio_expected_return = (rf + portfolio_beta * premium).round_dp(4);

    let portfolio_actual_return: Decimal = pos_capm
        .iter()
        .map(|p| p.weight * p.actual_return)
        .sum::<Decimal>()
        .round_dp(4);

    let portfolio_alpha: Decimal = pos_capm
        .iter()
        .map(|p| p.weight * p.alpha)
        .sum::<Decimal>()
        .round_dp(4);

    // Sharpe  = (rₚ − Rf) / σ   where σ = population std-dev of actual returns
    let mean_ret: Decimal =
        pos_capm.iter().map(|p| p.actual_return).sum::<Decimal>() / Decimal::from(pos_capm.len());
    let variance: Decimal = pos_capm
        .iter()
        .map(|p| {
            let d = p.actual_return - mean_ret;
            d * d
        })
        .sum::<Decimal>()
        / Decimal::from(pos_capm.len());

    let sigma_f64 = variance.to_f64().unwrap_or(0.0).sqrt();
    let sharpe_ratio = if sigma_f64 > 1e-6 {
        let sigma = Decimal::try_from(sigma_f64).unwrap_or(Decimal::ZERO);
        Some(((portfolio_actual_return - rf) / sigma).round_dp(4))
    } else {
        None
    };

    // Treynor  = (rₚ − Rf) / βₚ
    let treynor_ratio = if portfolio_beta.abs() > dec!(0.0001) {
        Some(((portfolio_actual_return - rf) / portfolio_beta).round_dp(4))
    } else {
        None
    };

    // Information Ratio  = αₚ / σ(αᵢ)
    let alpha_mean: Decimal =
        pos_capm.iter().map(|p| p.alpha).sum::<Decimal>() / Decimal::from(pos_capm.len());
    let alpha_variance: Decimal = pos_capm
        .iter()
        .map(|p| {
            let d = p.alpha - alpha_mean;
            d * d
        })
        .sum::<Decimal>()
        / Decimal::from(pos_capm.len());
    let alpha_sigma_f64 = alpha_variance.to_f64().unwrap_or(0.0).sqrt();
    let information_ratio = if alpha_sigma_f64 > 1e-6 {
        let alpha_sigma = Decimal::try_from(alpha_sigma_f64).unwrap_or(Decimal::ZERO);
        Some((portfolio_alpha / alpha_sigma).round_dp(4))
    } else {
        None
    };

    Some(PortfolioCAPM {
        inputs: inputs.clone(),
        portfolio_beta,
        portfolio_expected_return,
        portfolio_alpha,
        sharpe_ratio,
        treynor_ratio,
        information_ratio,
        portfolio_actual_return,
        positions: pos_capm,
    })
}
