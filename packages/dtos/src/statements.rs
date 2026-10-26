//! The financial statements as tables: which lines each statement shows,
//! and the trailing-year, year-on-year and compound-growth figures beside
//! them. Labels are English; the UI translates them where they're shown.

use crate::fundamentals::{growth, ttm, PeriodFinancials};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statement {
    Income,
    BalanceSheet,
    CashFlow,
}

impl Statement {
    pub const ALL: [Self; 3] = [Self::Income, Self::BalanceSheet, Self::CashFlow];

    pub fn title(self) -> &'static str {
        match self {
            Self::Income => "Income statement",
            Self::BalanceSheet => "Balance sheet",
            Self::CashFlow => "Cash flow",
        }
    }

    /// Income and cash flow add up over time; a balance sheet is a snapshot
    /// at the period's end, so it has no trailing-year total.
    pub fn adds_up_over_time(self) -> bool {
        match self {
            Self::Income | Self::CashFlow => true,
            Self::BalanceSheet => false,
        }
    }

    pub fn lines(self) -> impl Iterator<Item = &'static StatementLine> {
        LINES.iter().filter(move |l| l.statement == self)
    }
}

/// How often the periods in a table come: it decides which period is "a
/// year earlier".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequency {
    Quarterly,
    Annual,
}

impl Frequency {
    fn periods_per_year(self) -> usize {
        match self {
            Self::Quarterly => 4,
            Self::Annual => 1,
        }
    }
}

pub struct StatementLine {
    pub statement: Statement,
    pub label: &'static str,
    pub get: fn(&PeriodFinancials) -> Option<f64>,
}

impl StatementLine {
    /// The last four quarters added up (`quarters` newest first); `None`
    /// for balance-sheet lines.
    pub fn trailing_year(&self, quarters: &[PeriodFinancials]) -> Option<f64> {
        self.statement.adds_up_over_time().then(|| ttm(quarters, self.get)).flatten()
    }

    /// The latest period against the same period a year earlier
    /// (`periods` newest first).
    pub fn year_on_year(&self, periods: &[PeriodFinancials], frequency: Frequency) -> Option<f64> {
        let now = (self.get)(periods.first()?)?;
        let then = (self.get)(periods.get(frequency.periods_per_year())?)?;
        growth(now, then)
    }

    /// Compound yearly growth from the oldest year with a value to the
    /// latest (`years` newest first), with how many years that spans.
    /// Needs two years or more, and both ends above zero.
    pub fn compound_growth(&self, years: &[PeriodFinancials]) -> Option<(f64, usize)> {
        let latest = (self.get)(years.first()?)?;
        let (span, oldest) = years.iter().enumerate().skip(2).filter_map(|(i, y)| Some((i, (self.get)(y)?))).last()?;
        if latest <= 0.0 || oldest <= 0.0 {
            return None;
        }
        Some(((latest / oldest).powf(1.0 / span as f64) - 1.0, span))
    }
}

/// Every line, statement by statement, in the order a statement reads.
pub const LINES: &[StatementLine] = &[
    line(Statement::Income, "Revenue", |p| p.revenue),
    line(Statement::Income, "Gross profit", |p| p.gross_profit),
    line(Statement::Income, "Operating income", |p| p.operating_income),
    line(Statement::Income, "EBITDA", |p| p.ebitda()),
    line(Statement::Income, "Interest expense", |p| p.interest_expense),
    line(Statement::Income, "Income tax", |p| p.income_tax_expense),
    line(Statement::Income, "Net income", |p| p.net_income),
    line(Statement::BalanceSheet, "Cash", |p| p.cash),
    line(Statement::BalanceSheet, "Accounts receivable", |p| p.accounts_receivable),
    line(Statement::BalanceSheet, "Inventory", |p| p.inventory),
    line(Statement::BalanceSheet, "Current assets", |p| p.current_assets),
    line(Statement::BalanceSheet, "Property, plant & equipment", |p| p.property_plant_equipment),
    line(Statement::BalanceSheet, "Goodwill", |p| p.goodwill),
    line(Statement::BalanceSheet, "Total assets", |p| p.total_assets),
    line(Statement::BalanceSheet, "Accounts payable", |p| p.accounts_payable),
    line(Statement::BalanceSheet, "Current liabilities", |p| p.current_liabilities),
    line(Statement::BalanceSheet, "Long-term debt", |p| p.long_term_debt),
    line(Statement::BalanceSheet, "Total liabilities", |p| p.total_liabilities),
    line(Statement::BalanceSheet, "Shareholders' equity", |p| p.total_equity),
    line(Statement::CashFlow, "Operating cash flow", |p| p.operating_cashflow),
    line(Statement::CashFlow, "Capital spending", |p| p.capital_expenditures),
    line(Statement::CashFlow, "Free cash flow", |p| p.free_cash_flow),
    line(Statement::CashFlow, "Depreciation & amortization", |p| p.depreciation_amortization),
];

const fn line(statement: Statement, label: &'static str, get: fn(&PeriodFinancials) -> Option<f64>) -> StatementLine {
    StatementLine { statement, label, get }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn periods(revenue: &[Option<f64>]) -> Vec<PeriodFinancials> {
        revenue
            .iter()
            .map(|r| PeriodFinancials { revenue: *r, cash: Some(10.0), ..Default::default() })
            .collect()
    }

    fn revenue() -> &'static StatementLine {
        Statement::Income.lines().next().unwrap()
    }

    #[test]
    fn every_statement_has_lines_and_labels_are_unique() {
        for s in Statement::ALL {
            assert!(s.lines().count() >= 4, "{s:?}");
        }
        let mut labels: Vec<&str> = LINES.iter().map(|l| l.label).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), LINES.len());
    }

    #[test]
    fn trailing_year_adds_up_flows_only() {
        let q = periods(&[Some(4.0), Some(3.0), Some(2.0), Some(1.0), Some(9.0)]);
        assert_eq!(revenue().trailing_year(&q), Some(10.0));
        let cash = Statement::BalanceSheet.lines().next().unwrap();
        assert_eq!(cash.trailing_year(&q), None);
    }

    #[test]
    fn year_on_year_compares_the_same_period_a_year_earlier() {
        let q = periods(&[Some(120.0), Some(1.0), Some(1.0), Some(1.0), Some(100.0)]);
        assert!((revenue().year_on_year(&q, Frequency::Quarterly).unwrap() - 0.2).abs() < 1e-12);
        let years = periods(&[Some(150.0), Some(100.0)]);
        assert!((revenue().year_on_year(&years, Frequency::Annual).unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn compound_growth_spans_from_the_oldest_year_with_a_value() {
        // 100 → 121 over two years: 10% a year; the missing oldest year is skipped.
        let years = periods(&[Some(121.0), Some(110.0), Some(100.0), None]);
        let (rate, span) = revenue().compound_growth(&years).unwrap();
        assert!((rate - 0.1).abs() < 1e-12);
        assert_eq!(span, 2);
        assert_eq!(revenue().compound_growth(&periods(&[Some(2.0), Some(1.0)])), None, "one year is just year-on-year");
        assert_eq!(revenue().compound_growth(&periods(&[Some(5.0), Some(1.0), Some(-1.0)])), None, "from a loss");
    }
}
