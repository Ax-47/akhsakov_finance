//! Piotroski F-score: nine yes-or-no checks of a company's financial
//! strength, comparing its latest fiscal year with the year before. Eight
//! or nine passes is strong, two or fewer weak.
//!
//! Returns on assets use each year's own closing assets; Piotroski used the
//! year's opening assets, which Yahoo's few years of statements can't
//! always give.

use crate::fundamentals::PeriodFinancials;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    PositiveReturnOnAssets,
    PositiveOperatingCashFlow,
    ImprovingReturnOnAssets,
    CashFlowAboveProfit,
    DebtNotRising,
    ImprovingCurrentRatio,
    NoNewShares,
    ImprovingGrossMargin,
    ImprovingAssetTurnover,
}

/// The three things the checks look at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckGroup {
    Profitability,
    BalanceSheet,
    Efficiency,
}

impl CheckGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Profitability => "Profitability",
            Self::BalanceSheet => "Debt, liquidity and shares",
            Self::Efficiency => "Efficiency",
        }
    }
}

impl Check {
    pub const ALL: [Self; 9] = [
        Self::PositiveReturnOnAssets,
        Self::PositiveOperatingCashFlow,
        Self::ImprovingReturnOnAssets,
        Self::CashFlowAboveProfit,
        Self::DebtNotRising,
        Self::ImprovingCurrentRatio,
        Self::NoNewShares,
        Self::ImprovingGrossMargin,
        Self::ImprovingAssetTurnover,
    ];

    /// What passing means, in plain words.
    pub fn label(self) -> &'static str {
        match self {
            Self::PositiveReturnOnAssets => "Made a profit (return on assets above zero)",
            Self::PositiveOperatingCashFlow => "Brought in cash from operations",
            Self::ImprovingReturnOnAssets => "Return on assets rose",
            Self::CashFlowAboveProfit => "Cash from operations beat profit",
            Self::DebtNotRising => "Long-term debt didn't rise against assets",
            Self::ImprovingCurrentRatio => "Current ratio rose",
            Self::NoNewShares => "Didn't issue new shares",
            Self::ImprovingGrossMargin => "Gross margin rose",
            Self::ImprovingAssetTurnover => "Sales rose against assets",
        }
    }

    pub fn group(self) -> CheckGroup {
        match self {
            Self::PositiveReturnOnAssets
            | Self::PositiveOperatingCashFlow
            | Self::ImprovingReturnOnAssets
            | Self::CashFlowAboveProfit => CheckGroup::Profitability,
            Self::DebtNotRising | Self::ImprovingCurrentRatio | Self::NoNewShares => CheckGroup::BalanceSheet,
            Self::ImprovingGrossMargin | Self::ImprovingAssetTurnover => CheckGroup::Efficiency,
        }
    }

    /// `this` year against `last`; `None` when a figure is missing.
    fn passes(self, this: &PeriodFinancials, last: &PeriodFinancials) -> Option<bool> {
        let return_on_assets = |p: &PeriodFinancials| per_asset(p.net_income, p);
        Some(match self {
            Self::PositiveReturnOnAssets => return_on_assets(this)? > 0.0,
            Self::PositiveOperatingCashFlow => this.operating_cashflow? > 0.0,
            Self::ImprovingReturnOnAssets => return_on_assets(this)? > return_on_assets(last)?,
            Self::CashFlowAboveProfit => this.operating_cashflow? > this.net_income?,
            // No debt in either year counts as not rising.
            Self::DebtNotRising => {
                per_asset(Some(this.long_term_debt.unwrap_or(0.0)), this)?
                    <= per_asset(Some(last.long_term_debt.unwrap_or(0.0)), last)?
            }
            Self::ImprovingCurrentRatio => this.current_ratio()? > last.current_ratio()?,
            Self::NoNewShares => this.shares? <= last.shares?,
            Self::ImprovingGrossMargin => this.gross_margin()? > last.gross_margin()?,
            Self::ImprovingAssetTurnover => per_asset(this.revenue, this)? > per_asset(last.revenue, last)?,
        })
    }
}

/// `value` over the period's total assets.
fn per_asset(value: Option<f64>, p: &PeriodFinancials) -> Option<f64> {
    Some(value? / p.total_assets.filter(|a| *a > 0.0)?)
}

#[derive(Debug, Clone, PartialEq)]
pub struct FScore {
    /// Every check in [`Check::ALL`] order; `None` where it couldn't be run.
    pub results: Vec<(Check, Option<bool>)>,
}

impl FScore {
    /// `years` newest first; needs the latest two.
    pub fn compute(years: &[PeriodFinancials]) -> Option<Self> {
        let (this, last) = (years.first()?, years.get(1)?);
        Some(Self { results: Check::ALL.iter().map(|c| (*c, c.passes(this, last))).collect() })
    }

    pub fn passed(&self) -> usize {
        self.results.iter().filter(|(_, r)| *r == Some(true)).count()
    }

    /// How many checks had the figures to run.
    pub fn checked(&self) -> usize {
        self.results.iter().filter(|(_, r)| r.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn year(net: f64, cash_flow: f64, debt: Option<f64>, shares: f64, gross: f64, revenue: f64) -> PeriodFinancials {
        PeriodFinancials {
            net_income: Some(net),
            operating_cashflow: Some(cash_flow),
            long_term_debt: debt,
            shares: Some(shares),
            gross_profit: Some(gross),
            revenue: Some(revenue),
            total_assets: Some(1000.0),
            current_assets: Some(300.0),
            current_liabilities: Some(200.0),
            ..Default::default()
        }
    }

    fn result(score: &FScore, check: Check) -> Option<bool> {
        score.results.iter().find(|(c, _)| *c == check).unwrap().1
    }

    #[test]
    fn a_company_improving_on_every_count_scores_nine() {
        let this = PeriodFinancials { current_assets: Some(350.0), ..year(100.0, 150.0, Some(100.0), 10.0, 500.0, 900.0) };
        let last = year(50.0, 60.0, Some(200.0), 11.0, 400.0, 800.0);
        let score = FScore::compute(&[this, last]).unwrap();
        assert_eq!((score.passed(), score.checked()), (9, 9));
    }

    #[test]
    fn a_declining_company_scores_low() {
        let this = year(-20.0, -5.0, Some(300.0), 12.0, 300.0, 700.0);
        let last = year(50.0, 60.0, Some(200.0), 11.0, 400.0, 800.0);
        let score = FScore::compute(&[this, last]).unwrap();
        // Only "cash beat profit" passes: −5 > −20.
        assert_eq!(score.passed(), 1);
        assert_eq!(result(&score, Check::CashFlowAboveProfit), Some(true));
    }

    #[test]
    fn no_debt_counts_as_not_rising_and_missing_figures_are_unknown() {
        let mut this = year(10.0, 10.0, None, 10.0, 50.0, 100.0);
        let last = year(10.0, 10.0, None, 10.0, 50.0, 100.0);
        this.shares = None;
        let score = FScore::compute(&[this, last.clone()]).unwrap();
        assert_eq!(result(&score, Check::DebtNotRising), Some(true));
        assert_eq!(result(&score, Check::NoNewShares), None);
        assert_eq!(score.checked(), 8);
        assert_eq!(FScore::compute(&[last]), None, "needs two years");
    }

    #[test]
    fn every_check_has_a_group_and_a_distinct_label() {
        let mut labels: Vec<&str> = Check::ALL.iter().map(|c| c.label()).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), 9);
        assert_eq!(Check::ALL.iter().filter(|c| c.group() == CheckGroup::Profitability).count(), 4);
    }
}
