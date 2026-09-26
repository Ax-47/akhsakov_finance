/// Capital Asset Pricing Model (CAPM) analytics.
///
/// CAPM formula:  E(rᵢ) = Rf + βᵢ · (Rm − Rf), a return *per year*.
///
/// Each holding's actual return is since purchase, so it is put on the same
/// footing first: holdings held a year or more are annualised over their
/// (cost-weighted) holding period; shorter ones are not annualised (per
/// GIPS) and are compared with what CAPM expects over the same stretch.
///
/// Metrics computed here:
///   - Portfolio beta          βₚ = Σ wᵢβᵢ
///   - Expected return (CAPM)  per position and for the whole portfolio
///   - Jensen's Alpha          αᵢ = actualᵢ − expectedᵢ (same horizon)
///   - Treynor Ratio           (rᵢ − Rf) / βᵢ, for annualised returns
///
/// Beta source: the caller supplies a `beta_map`. If a ticker is absent the
/// default beta is 1.0 (the market itself).
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::ticker_symbol::TickerSymbol;

/// Holdings held at least this long (years) get annualised returns.
pub const ANNUALISE_AFTER_YEARS: f64 = 1.0;

// ─── Public inputs / outputs ──────────────────────────────────────────────────

/// Parameters that the user can tune interactively.
#[derive(Clone, Debug, PartialEq)]
pub struct CAPMInputs {
    /// Annualised risk-free rate in % (e.g. 4.0 for 4 %).
    pub rf: Decimal,
    /// Annualised expected market return in % (e.g. 7.0 for 7 %).
    pub rm: Decimal,
}

impl Default for CAPMInputs {
    fn default() -> Self {
        Self {
            rf: dec!(4),
            rm: dec!(7),
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
    pub ticker: TickerSymbol,
    /// Portfolio weight (0–1).
    pub weight: Decimal,
    /// Beta used (1.0 if not supplied in beta_map).
    pub beta: Decimal,
    /// Cost-weighted years the current shares have been held, if known.
    pub years: Option<f64>,
    /// Whether the returns below are per year (held ≥ 1 year) or over
    /// the holding period.
    pub annualised: bool,
    /// CAPM expected return %, per year or over the holding period.
    pub expected_return: Decimal,
    /// Actual return % since purchase, per year or over the holding period.
    pub actual_return: Decimal,
    /// Jensen's Alpha %: actual − expected. `None` when the holding
    /// period is unknown.
    pub alpha: Option<Decimal>,
    /// Treynor Ratio: (annual actual − Rf) / β. `None` for returns that
    /// aren't annualised or when β ≈ 0.
    pub treynor: Option<Decimal>,
}

/// Aggregate CAPM metrics for the whole portfolio.
#[derive(Clone, Debug, PartialEq)]
pub struct PortfolioCAPM {
    pub inputs: CAPMInputs,
    /// Weighted average beta  Σ wᵢβᵢ.
    pub portfolio_beta: Decimal,
    /// CAPM expected return for the portfolio, per year.
    pub portfolio_expected_return: Decimal,
    /// Value-weighted annual return of the holdings held a year or more;
    /// `None` if there are none.
    pub portfolio_actual_return: Option<Decimal>,
    /// Their value-weighted alpha, per year.
    pub portfolio_alpha: Option<Decimal>,
    /// Value-weighted beta of the same holdings.
    pub annual_beta: Option<Decimal>,
    /// Portfolio Treynor Ratio (same holdings).
    pub treynor_ratio: Option<Decimal>,
    /// Share of the portfolio (0–1) the annual figures cover.
    pub annual_share: Decimal,
    /// Per-position breakdown, sorted by alpha descending.
    pub positions: Vec<PositionCAPM>,
}

/// `total` (e.g. 0.60 for +60%) spread evenly over `years`, per year.
fn per_year(total: f64, years: f64) -> f64 {
    (1.0 + total).max(0.0).powf(1.0 / years) - 1.0
}

/// A yearly rate compounded over `years`.
fn over_years(annual: f64, years: f64) -> f64 {
    (1.0 + annual).max(0.0).powf(years) - 1.0
}

fn pct(x: f64) -> Decimal {
    Decimal::try_from(x * 100.0).unwrap_or_default().round_dp(4)
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
/// * `held_years` — ticker → cost-weighted years held (see the Risk tab)
pub fn compute_capm(
    positions: &[Position],
    total_value: Decimal,
    beta_map: &HashMap<TickerSymbol, Decimal>,
    inputs: &CAPMInputs,
    held_years: &HashMap<TickerSymbol, f64>,
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
    let f = |d: Decimal| d.to_f64().unwrap_or(0.0);

    // ── Per-position metrics ───────────────────────────────────────────────────
    let mut pos_capm: Vec<PositionCAPM> = live
        .iter()
        .map(|p| {
            let weight = p.market_value() / total_value;
            let beta = *beta_map.get(&p.ticker).unwrap_or(&Decimal::ONE);
            let expected_annual = f(rf + beta * premium) / 100.0;
            let since_purchase = f(p.unrealized_pnl_pct()) / 100.0;
            let years = held_years.get(&p.ticker).copied().filter(|y| *y > 0.0);
            let annualised = years.is_some_and(|y| y >= ANNUALISE_AFTER_YEARS);
            let (expected, actual) = match years {
                Some(y) if annualised => (expected_annual, per_year(since_purchase, y)),
                Some(y) => (over_years(expected_annual, y), since_purchase),
                None => (expected_annual, since_purchase),
            };
            let (expected_return, actual_return) = (pct(expected), pct(actual));
            let treynor = (annualised && beta.abs() > dec!(0.0001))
                .then(|| ((actual_return - rf) / beta).round_dp(4));
            PositionCAPM {
                ticker: p.ticker.clone(),
                weight,
                beta,
                years,
                annualised,
                expected_return,
                actual_return,
                alpha: years.map(|_| (actual_return - expected_return).round_dp(4)),
                treynor,
            }
        })
        .collect();

    // Sort by alpha descending (best alpha-generators first).
    pos_capm.sort_by(|a, b| b.alpha.cmp(&a.alpha));

    // ── Portfolio-level aggregates ─────────────────────────────────────────────
    let portfolio_beta: Decimal = pos_capm
        .iter()
        .map(|p| p.weight * p.beta)
        .sum::<Decimal>()
        .round_dp(4);

    let portfolio_expected_return = (rf + portfolio_beta * premium).round_dp(4);

    // Annual figures only from holdings whose returns are annual.
    let annual: Vec<&PositionCAPM> = pos_capm.iter().filter(|p| p.annualised).collect();
    let annual_share: Decimal = annual.iter().map(|p| p.weight).sum();
    let weighted = |pick: fn(&PositionCAPM) -> Decimal| {
        (annual_share > Decimal::ZERO).then(|| {
            (annual.iter().map(|p| p.weight * pick(p)).sum::<Decimal>() / annual_share).round_dp(4)
        })
    };
    let portfolio_actual_return = weighted(|p| p.actual_return);
    let portfolio_alpha = weighted(|p| p.alpha.unwrap_or_default());
    let annual_beta = weighted(|p| p.beta);

    // Treynor  = (rₚ − Rf) / βₚ
    let treynor_ratio = match (portfolio_actual_return, annual_beta) {
        (Some(r), Some(b)) if b.abs() > dec!(0.0001) => Some(((r - rf) / b).round_dp(4)),
        _ => None,
    };

    Some(PortfolioCAPM {
        inputs: inputs.clone(),
        portfolio_beta,
        portfolio_expected_return,
        portfolio_actual_return,
        portfolio_alpha,
        annual_beta,
        treynor_ratio,
        annual_share,
        positions: pos_capm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(ticker: &str, avg_cost: Decimal, price: Decimal) -> Position {
        Position {
            ticker: TickerSymbol::new(ticker).unwrap(),
            shares: dec!(10),
            avg_cost,
            current_price: price,
            daily_change_pct: dec!(0),
        }
    }

    #[test]
    fn long_holdings_are_annualised_and_short_ones_compared_over_their_period() {
        let (old, new) = (TickerSymbol::new("OLD").unwrap(), TickerSymbol::new("NEW").unwrap());
        let positions = vec![position("OLD", dec!(100), dec!(160)), position("NEW", dec!(100), dec!(110))];
        let total = positions.iter().map(Position::market_value).sum();
        let held = HashMap::from([(old.clone(), 5.0), (new.clone(), 0.25)]);
        let inputs = CAPMInputs { rf: dec!(4), rm: dec!(10) };
        let r = compute_capm(&positions, total, &HashMap::new(), &inputs, &held).unwrap();
        let get = |t: &TickerSymbol| r.positions.iter().find(|p| &p.ticker == t).unwrap();

        // +60% over five years is ≈9.86% a year, just below the 10% CAPM
        // asks of a beta-1 stock: a small negative alpha, not +50%.
        let o = get(&old);
        assert!(o.annualised);
        assert!((o.actual_return - dec!(9.8561)).abs() < dec!(0.001), "{}", o.actual_return);
        assert!((o.alpha.unwrap() - dec!(-0.1439)).abs() < dec!(0.001));

        // +10% in three months vs 10%/yr compounded for a quarter (2.41%).
        let n = get(&new);
        assert!(!n.annualised && n.treynor.is_none());
        assert_eq!(n.actual_return, dec!(10));
        assert!((n.expected_return - dec!(2.4114)).abs() < dec!(0.001), "{}", n.expected_return);

        // Portfolio annual figures come from OLD only.
        assert_eq!(r.portfolio_actual_return, Some(o.actual_return));
        assert!((r.annual_share - dec!(160) / dec!(270)).abs() < dec!(0.0001));
    }

    #[test]
    fn unknown_holding_periods_have_no_alpha() {
        let positions = vec![position("AAA", dec!(100), dec!(120))];
        let r = compute_capm(&positions, dec!(1200), &HashMap::new(), &CAPMInputs::default(), &HashMap::new()).unwrap();
        assert_eq!(r.positions[0].alpha, None);
        assert_eq!(r.portfolio_alpha, None);
    }
}
