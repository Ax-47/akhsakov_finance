//! Company fundamentals for the stock page: profile, key statistics,
//! financial statements, valuation history and analyst views.
//!
//! Plain `f64` / `String` so it serialises simply; `None` means Yahoo had
//! no value.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StockFundamentals {
    pub profile: Profile,
    pub stats: KeyStats,
    /// Newest first.
    pub quarterly: Vec<PeriodFinancials>,
    /// Newest first.
    pub annual: Vec<PeriodFinancials>,
    /// Valuation measures at each quarter end, newest first.
    pub valuation: Vec<ValuationPoint>,
    /// Valuation measures at each fiscal year end, newest first: a longer
    /// history than the quarters give, for the P/E's usual range.
    #[serde(default)]
    pub yearly_valuation: Vec<ValuationPoint>,
    /// Reported vs estimated EPS, oldest first.
    pub eps_surprises: Vec<EpsSurprise>,
    pub analysts: Option<Analysts>,
    /// Currency of prices, market cap, EPS and targets. The API converts
    /// everything to USD, so clients see "USD".
    #[serde(default = "usd")]
    pub currency: String,
    /// Currency the statements are reported in (can differ, e.g. ADRs).
    #[serde(default = "usd")]
    pub financial_currency: String,
}

fn usd() -> String {
    "USD".into()
}

impl StockFundamentals {
    /// Converts every amount to USD: price-like figures at `trading` USD per
    /// unit, statement figures at `financial`. Ratios are left alone.
    pub fn to_usd(&mut self, trading: f64, financial: f64) {
        let scale = |v: &mut Option<f64>, rate: f64| {
            if let Some(x) = v {
                *x *= rate;
            }
        };
        let s = &mut self.stats;
        for v in [
            &mut s.price,
            &mut s.market_cap,
            &mut s.eps_ttm,
            &mut s.forward_eps,
            &mut s.high_52w,
            &mut s.low_52w,
            &mut s.sma50,
            &mut s.sma200,
        ] {
            scale(v, trading);
        }
        for e in &mut self.eps_surprises {
            scale(&mut e.actual, trading);
            scale(&mut e.estimate, trading);
        }
        if let Some(a) = &mut self.analysts {
            scale(&mut a.target_low, trading);
            scale(&mut a.target_mean, trading);
            scale(&mut a.target_high, trading);
        }
        for p in self.quarterly.iter_mut().chain(self.annual.iter_mut()) {
            for v in [
                &mut p.revenue,
                &mut p.gross_profit,
                &mut p.operating_income,
                &mut p.net_income,
                &mut p.operating_cashflow,
                &mut p.free_cash_flow,
                &mut p.total_assets,
                &mut p.total_liabilities,
                &mut p.total_equity,
                &mut p.cash,
                &mut p.long_term_debt,
                &mut p.current_assets,
                &mut p.current_liabilities,
                &mut p.interest_expense,
                &mut p.income_tax_expense,
                &mut p.depreciation_amortization,
                &mut p.capital_expenditures,
                &mut p.inventory,
                &mut p.accounts_receivable,
                &mut p.accounts_payable,
                &mut p.property_plant_equipment,
                &mut p.goodwill,
            ] {
                scale(v, financial);
            }
        }
        for v in &mut self.valuation {
            v.price *= trading;
            scale(&mut v.market_cap, trading);
            scale(&mut v.enterprise_value, trading);
        }
        self.currency = "USD".into();
        self.financial_currency = "USD".into();
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Profile {
    pub name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub website: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyStats {
    pub price: Option<f64>,
    pub market_cap: Option<f64>,
    pub shares_outstanding: Option<f64>,
    pub eps_ttm: Option<f64>,
    pub pe_ttm: Option<f64>,
    /// Price / next fiscal year's consensus EPS.
    pub forward_pe: Option<f64>,
    pub forward_eps: Option<f64>,
    /// Fraction, e.g. 0.012 for 1.2%.
    pub dividend_yield: Option<f64>,
    pub ex_dividend_date: Option<String>,
    pub next_earnings: Option<String>,
    pub high_52w: Option<f64>,
    pub low_52w: Option<f64>,
    pub sma50: Option<f64>,
    pub sma200: Option<f64>,
    /// Analysts' expected EPS growth a year over the next five years, as a
    /// fraction (0.12 for 12%).
    pub eps_growth_5y: Option<f64>,
}

/// One reporting period's income statement, balance sheet and cash flow.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PeriodFinancials {
    /// `2026Q2`, `2025` or `2026-07-31`.
    pub period: String,
    /// Period end as `YYYY-MM-DD`, when known.
    pub end_date: Option<String>,
    pub revenue: Option<f64>,
    pub gross_profit: Option<f64>,
    pub operating_income: Option<f64>,
    pub net_income: Option<f64>,
    pub operating_cashflow: Option<f64>,
    pub free_cash_flow: Option<f64>,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_equity: Option<f64>,
    pub cash: Option<f64>,
    pub long_term_debt: Option<f64>,
    pub current_assets: Option<f64>,
    pub current_liabilities: Option<f64>,
    pub shares: Option<f64>,
    /// As a cost: positive.
    pub interest_expense: Option<f64>,
    pub income_tax_expense: Option<f64>,
    pub depreciation_amortization: Option<f64>,
    /// As reported: negative when the company spends.
    pub capital_expenditures: Option<f64>,
    pub inventory: Option<f64>,
    pub accounts_receivable: Option<f64>,
    pub accounts_payable: Option<f64>,
    /// Net of depreciation.
    pub property_plant_equipment: Option<f64>,
    pub goodwill: Option<f64>,
}

impl PeriodFinancials {
    pub fn gross_margin(&self) -> Option<f64> {
        ratio(self.gross_profit, self.revenue)
    }
    pub fn operating_margin(&self) -> Option<f64> {
        ratio(self.operating_income, self.revenue)
    }
    pub fn net_margin(&self) -> Option<f64> {
        ratio(self.net_income, self.revenue)
    }
    pub fn current_ratio(&self) -> Option<f64> {
        ratio(self.current_assets, self.current_liabilities)
    }
    pub fn debt_to_equity(&self) -> Option<f64> {
        ratio(self.long_term_debt, self.total_equity)
    }
    /// Operating income before depreciation and amortization.
    pub fn ebitda(&self) -> Option<f64> {
        Some(self.operating_income? + self.depreciation_amortization?)
    }
    /// Current assets less inventory, over current liabilities: what can
    /// pay near-term bills without selling stock. No inventory reported
    /// counts as none.
    pub fn quick_ratio(&self) -> Option<f64> {
        let liquid = self.current_assets? - self.inventory.unwrap_or(0.0);
        ratio(Some(liquid), self.current_liabilities)
    }
    /// Long-term debt less cash. Yahoo's statements don't split out
    /// short-term debt, so it isn't counted.
    pub fn net_debt(&self) -> Option<f64> {
        if self.long_term_debt.is_none() && self.cash.is_none() {
            return None;
        }
        Some(self.long_term_debt.unwrap_or(0.0) - self.cash.unwrap_or(0.0))
    }
}

/// Valuation measures at a quarter end, like Yahoo's "Valuation Measures".
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValuationPoint {
    /// Quarter end, `YYYY-MM-DD`.
    pub date: String,
    pub price: f64,
    pub market_cap: Option<f64>,
    pub enterprise_value: Option<f64>,
    pub pe_ttm: Option<f64>,
    pub price_to_sales: Option<f64>,
    pub price_to_book: Option<f64>,
    pub ev_to_revenue: Option<f64>,
    #[serde(default)]
    pub ev_to_ebitda: Option<f64>,
    #[serde(default)]
    pub price_to_fcf: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EpsSurprise {
    pub period: String,
    pub actual: Option<f64>,
    pub estimate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Analysts {
    pub strong_buy: u32,
    pub buy: u32,
    pub hold: u32,
    pub sell: u32,
    pub strong_sell: u32,
    /// 1 = strong buy … 5 = strong sell.
    pub mean: Option<f64>,
    pub rating: Option<String>,
    pub target_low: Option<f64>,
    pub target_mean: Option<f64>,
    pub target_high: Option<f64>,
    pub count: Option<u32>,
}

/// Sum of the four most recent quarters (`quarters` newest first).
pub fn ttm(
    quarters: &[PeriodFinancials],
    field: fn(&PeriodFinancials) -> Option<f64>,
) -> Option<f64> {
    quarters.get(..4)?.iter().map(field).sum()
}

/// Year-over-year growth of the latest quarter vs the same quarter a year
/// earlier (`quarters` newest first).
pub fn yoy_growth(
    quarters: &[PeriodFinancials],
    field: fn(&PeriodFinancials) -> Option<f64>,
) -> Option<f64> {
    growth(field(quarters.first()?)?, field(quarters.get(4)?)?)
}

/// Change from `then` to `now` as a fraction (0.2 = up 20%). From a
/// negative start, rising counts as growth: −10 → −5 is +50%.
pub fn growth(now: f64, then: f64) -> Option<f64> {
    (then != 0.0).then(|| now / then.abs() - then.signum())
}

/// Valuation measures today: live market cap with the latest statements.
pub fn current_valuation(stats: &KeyStats, quarters: &[PeriodFinancials]) -> ValuationPoint {
    let latest = quarters.first();
    let price = stats.price.unwrap_or_default();
    let market_cap = stats
        .market_cap
        .or_else(|| Some(latest?.shares? * stats.price?));
    let revenue_ttm = ttm(quarters, |p| p.revenue);
    let enterprise_value = market_cap.map(|m| m + latest.and_then(|q| q.net_debt()).unwrap_or(0.0));
    ValuationPoint {
        date: "Current".into(),
        price,
        market_cap,
        enterprise_value,
        pe_ttm: stats
            .pe_ttm
            .or_else(|| ratio(market_cap, ttm(quarters, |p| p.net_income)))
            .filter(|pe| *pe > 0.0),
        price_to_sales: ratio(market_cap, revenue_ttm),
        price_to_book: ratio(market_cap, latest.and_then(|q| q.total_equity))
            .filter(|pb| *pb > 0.0),
        ev_to_revenue: ratio(enterprise_value, revenue_ttm),
        ev_to_ebitda: positive_ratio(enterprise_value, ttm(quarters, |p| p.ebitda())),
        price_to_fcf: positive_ratio(market_cap, ttm(quarters, |p| p.free_cash_flow)),
    }
}

fn ratio(num: Option<f64>, den: Option<f64>) -> Option<f64> {
    let (n, d) = (num?, den?);
    (d != 0.0).then(|| n / d)
}

/// [`ratio`] for multiples like EV / EBITDA, which mean nothing when the
/// earnings underneath are zero or negative.
fn positive_ratio(num: Option<f64>, den: Option<f64>) -> Option<f64> {
    ratio(num, den.filter(|d| *d > 0.0))
}

/// Return, safety and payout measures from the trailing four quarters and
/// the latest balance sheet. Each is `None` when an input is missing or the
/// number would mislead (a multiple of negative earnings, say).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KeyRatios {
    /// Trailing operating income + depreciation & amortization.
    pub ebitda: Option<f64>,
    /// After-tax operating income over the money invested in the business
    /// (equity + long-term debt − cash).
    pub return_on_invested_capital: Option<f64>,
    /// How many times operating income covers interest.
    pub interest_coverage: Option<f64>,
    pub net_debt: Option<f64>,
    /// Years of EBITDA it would take to pay off net debt.
    pub net_debt_to_ebitda: Option<f64>,
    pub quick_ratio: Option<f64>,
    /// Trailing free cash flow over market cap.
    pub free_cash_flow_yield: Option<f64>,
    /// Share of earnings paid out as dividends.
    pub payout_ratio: Option<f64>,
    /// P/E over analysts' expected yearly EPS growth (in %) for 5 years.
    pub peg: Option<f64>,
}

impl KeyRatios {
    /// `quarters` newest first.
    pub fn compute(stats: &KeyStats, quarters: &[PeriodFinancials]) -> Self {
        let latest = quarters.first();
        let now = current_valuation(stats, quarters);
        let ebitda = ttm(quarters, |p| p.ebitda());
        let net_debt = latest.and_then(|q| q.net_debt());
        Self {
            ebitda,
            return_on_invested_capital: return_on_invested_capital(quarters),
            interest_coverage: ratio(
                ttm(quarters, |p| p.operating_income),
                ttm(quarters, |p| p.interest_expense.map(f64::abs)).filter(|i| *i > 0.0),
            ),
            net_debt,
            net_debt_to_ebitda: positive_ratio(net_debt, ebitda),
            quick_ratio: latest.and_then(|q| q.quick_ratio()),
            free_cash_flow_yield: ratio(ttm(quarters, |p| p.free_cash_flow), now.market_cap),
            payout_ratio: stats.dividend_yield.zip(now.pe_ttm).map(|(y, pe)| y * pe),
            peg: positive_ratio(now.pe_ttm, stats.eps_growth_5y.map(|g| g * 100.0)),
        }
    }
}

/// NOPAT over invested capital. The tax rate is what the company actually
/// paid on its pre-tax profit, and none in a loss-making year.
fn return_on_invested_capital(quarters: &[PeriodFinancials]) -> Option<f64> {
    let latest = quarters.first()?;
    let operating_income = ttm(quarters, |p| p.operating_income)?;
    let tax = ttm(quarters, |p| p.income_tax_expense)?;
    let pre_tax = ttm(quarters, |p| p.net_income)? + tax;
    let tax_rate = if pre_tax > 0.0 { (tax / pre_tax).clamp(0.0, 1.0) } else { 0.0 };
    let invested = latest.total_equity? + latest.net_debt().unwrap_or(0.0);
    positive_ratio(Some(operating_income * (1.0 - tax_rate)), Some(invested))
}

/// Valuation measures at each quarter end, computed from statements and the
/// closing price on (or just before) that date.
///
/// `quarters` newest first; `closes` as `(YYYY-MM-DD, close)` in any order.
/// Trailing figures (P/E, P/S) need the four quarters ending at that date.
pub fn valuation_history(
    quarters: &[PeriodFinancials],
    closes: &[(String, f64)],
) -> Vec<ValuationPoint> {
    valuations(quarters, closes, 4)
}

/// [`valuation_history`] at each fiscal year end (`years` newest first):
/// each year is its own trailing twelve months.
pub fn yearly_valuation(years: &[PeriodFinancials], closes: &[(String, f64)]) -> Vec<ValuationPoint> {
    valuations(years, closes, 1)
}

/// `periods_per_year` periods (ending at each one) make a trailing year.
fn valuations(
    periods: &[PeriodFinancials],
    closes: &[(String, f64)],
    periods_per_year: usize,
) -> Vec<ValuationPoint> {
    let mut closes: Vec<&(String, f64)> = closes.iter().collect();
    closes.sort_by(|a, b| a.0.cmp(&b.0));
    let close_on = |date: &str| {
        closes
            .iter()
            .rev()
            .find(|(d, _)| d.as_str() <= date)
            .map(|(_, c)| *c)
    };
    let ttm = |i: usize, field: fn(&PeriodFinancials) -> Option<f64>| -> Option<f64> {
        let window = periods.get(i..i + periods_per_year)?;
        window.iter().map(field).sum::<Option<f64>>()
    };

    periods
        .iter()
        .enumerate()
        .filter_map(|(i, q)| {
            let date = q.end_date.clone()?;
            let price = close_on(&date)?;
            let market_cap = q.shares.map(|s| s * price);
            let revenue_ttm = ttm(i, |p| p.revenue);
            let enterprise_value = market_cap.map(|m| m + q.net_debt().unwrap_or(0.0));
            Some(ValuationPoint {
                date,
                price,
                market_cap,
                enterprise_value,
                pe_ttm: ratio(market_cap, ttm(i, |p| p.net_income)).filter(|pe| *pe > 0.0),
                price_to_sales: ratio(market_cap, revenue_ttm),
                price_to_book: ratio(market_cap, q.total_equity).filter(|pb| *pb > 0.0),
                ev_to_revenue: ratio(enterprise_value, revenue_ttm),
                ev_to_ebitda: positive_ratio(enterprise_value, ttm(i, |p| p.ebitda())),
                price_to_fcf: positive_ratio(market_cap, ttm(i, |p| p.free_cash_flow)),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter(end: &str, revenue: f64, net: f64) -> PeriodFinancials {
        PeriodFinancials {
            period: end.into(),
            end_date: Some(end.into()),
            revenue: Some(revenue),
            net_income: Some(net),
            total_equity: Some(500.0),
            cash: Some(100.0),
            long_term_debt: Some(50.0),
            shares: Some(10.0),
            ..Default::default()
        }
    }

    #[test]
    fn valuation_uses_trailing_four_quarters_and_quarter_end_price() {
        let quarters = vec![
            quarter("2026-06-30", 100.0, 20.0),
            quarter("2026-03-31", 100.0, 20.0),
            quarter("2025-12-31", 100.0, 20.0),
            quarter("2025-09-30", 100.0, 20.0),
            quarter("2025-06-30", 100.0, 20.0),
        ];
        let closes = vec![
            ("2026-06-29".to_string(), 80.0),
            ("2026-06-30".to_string(), 100.0),
            ("2026-07-01".to_string(), 999.0),
        ];
        let points = valuation_history(&quarters, &closes);
        // Only the newest quarter has a price on/before its date here.
        let p = &points[0];
        assert_eq!(p.date, "2026-06-30");
        assert_eq!(p.price, 100.0);
        assert_eq!(p.market_cap, Some(1000.0));
        assert_eq!(p.enterprise_value, Some(950.0));
        assert_eq!(p.pe_ttm, Some(12.5)); // 1000 / 80
        assert_eq!(p.price_to_sales, Some(2.5)); // 1000 / 400
        assert_eq!(p.price_to_book, Some(2.0));
        // Oldest quarter lacks 4 quarters of history: no trailing ratios.
        let oldest = valuation_history(&quarters[4..], &[("2025-06-30".into(), 50.0)]);
        assert_eq!(oldest[0].pe_ttm, None);
        assert_eq!(oldest[0].market_cap, Some(500.0));
    }

    #[test]
    fn yearly_valuation_uses_each_year_on_its_own() {
        let years = vec![quarter("2025-12-31", 400.0, 80.0), quarter("2024-12-31", 360.0, 50.0)];
        let closes = vec![("2025-12-31".to_string(), 100.0), ("2024-12-30".to_string(), 60.0)];
        let points = yearly_valuation(&years, &closes);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].pe_ttm, Some(12.5)); // 1000 / 80
        assert_eq!(points[1].pe_ttm, Some(12.0)); // 600 / 50, priced on the last trading day
    }

    #[test]
    fn trailing_totals_growth_and_current_valuation() {
        let quarters: Vec<PeriodFinancials> = [120.0, 110.0, 105.0, 100.0, 100.0]
            .iter()
            .enumerate()
            .map(|(i, rev)| PeriodFinancials {
                revenue: Some(*rev),
                net_income: Some(10.0),
                total_equity: Some(200.0),
                shares: Some(10.0),
                cash: Some(30.0),
                long_term_debt: Some(10.0),
                ..quarter(&format!("2026-0{}-30", 6 - i), *rev, 10.0)
            })
            .collect();
        assert_eq!(ttm(&quarters, |p| p.revenue), Some(435.0));
        assert_eq!(ttm(&quarters[2..], |p| p.revenue), None);
        // 120 vs 100 a year earlier: +20%.
        assert!((yoy_growth(&quarters, |p| p.revenue).unwrap() - 0.2).abs() < 1e-9);

        let stats = KeyStats {
            price: Some(40.0),
            ..Default::default()
        };
        let now = current_valuation(&stats, &quarters);
        assert_eq!(now.market_cap, Some(400.0)); // 10 shares × 40
        assert_eq!(now.enterprise_value, Some(380.0));
        assert_eq!(now.pe_ttm, Some(10.0)); // 400 / 40
        assert_eq!(now.price_to_book, Some(2.0));
    }

    /// Four identical quarters, so trailing figures are 4× each one.
    fn steady_quarters() -> Vec<PeriodFinancials> {
        let q = PeriodFinancials {
            revenue: Some(250.0),
            operating_income: Some(50.0),
            net_income: Some(30.0),
            income_tax_expense: Some(10.0),
            interest_expense: Some(-5.0),
            depreciation_amortization: Some(25.0),
            free_cash_flow: Some(20.0),
            total_equity: Some(600.0),
            long_term_debt: Some(400.0),
            cash: Some(100.0),
            current_assets: Some(300.0),
            inventory: Some(60.0),
            current_liabilities: Some(200.0),
            shares: Some(10.0),
            ..Default::default()
        };
        vec![q; 4]
    }

    #[test]
    fn key_ratios_from_trailing_quarters() {
        let stats = KeyStats {
            price: Some(80.0), // market cap 800
            dividend_yield: Some(0.02),
            eps_growth_5y: Some(0.10),
            ..Default::default()
        };
        let r = KeyRatios::compute(&stats, &steady_quarters());
        assert_eq!(r.ebitda, Some(300.0));
        // 200 operating income, taxed at 40 / 160 = 25%, over 600 + 300.
        assert!((r.return_on_invested_capital.unwrap() - 150.0 / 900.0).abs() < 1e-12);
        // Interest is reported negative here; coverage uses its size.
        assert_eq!(r.interest_coverage, Some(10.0));
        assert_eq!(r.net_debt, Some(300.0));
        assert_eq!(r.net_debt_to_ebitda, Some(1.0));
        assert_eq!(r.quick_ratio, Some(1.2));
        assert_eq!(r.free_cash_flow_yield, Some(0.1));
        // P/E 800 / 120.
        let pe = 800.0 / 120.0;
        assert!((r.payout_ratio.unwrap() - 0.02 * pe).abs() < 1e-12);
        assert!((r.peg.unwrap() - pe / 10.0).abs() < 1e-12);

        let now = current_valuation(&stats, &steady_quarters());
        assert_eq!(now.enterprise_value, Some(1100.0));
        assert_eq!(now.ev_to_ebitda, Some(1100.0 / 300.0));
        assert_eq!(now.price_to_fcf, Some(10.0));
    }

    #[test]
    fn multiples_of_losses_are_left_out() {
        let losing: Vec<PeriodFinancials> = steady_quarters()
            .into_iter()
            .map(|q| PeriodFinancials {
                operating_income: Some(-40.0),
                net_income: Some(-45.0),
                free_cash_flow: Some(-5.0),
                ..q
            })
            .collect();
        let stats = KeyStats { price: Some(80.0), ..Default::default() };
        let r = KeyRatios::compute(&stats, &losing);
        assert_eq!(r.net_debt_to_ebitda, None); // EBITDA −60
        // A return can be negative, unlike a multiple: −160 untaxed over 900.
        assert!((r.return_on_invested_capital.unwrap() + 160.0 / 900.0).abs() < 1e-12);
        assert_eq!(r.peg, None);
        let now = current_valuation(&stats, &losing);
        assert_eq!((now.ev_to_ebitda, now.price_to_fcf), (None, None));
    }

    #[test]
    fn statements_from_older_servers_still_read() {
        let old = r#"{"period":"2026Q1","end_date":null,"revenue":1.0}"#;
        let p: PeriodFinancials = serde_json::from_str(old).unwrap();
        assert_eq!((p.revenue, p.inventory), (Some(1.0), None));
    }

    #[test]
    fn margins() {
        let q = PeriodFinancials {
            revenue: Some(200.0),
            gross_profit: Some(120.0),
            net_income: Some(50.0),
            ..Default::default()
        };
        assert_eq!(q.gross_margin(), Some(0.6));
        assert_eq!(q.net_margin(), Some(0.25));
        assert_eq!(q.operating_margin(), None);
    }
}
