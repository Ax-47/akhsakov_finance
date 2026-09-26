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
    /// Reported vs estimated EPS, oldest first.
    pub eps_surprises: Vec<EpsSurprise>,
    pub analysts: Option<Analysts>,
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
}

/// One reporting period's income statement, balance sheet and cash flow.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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
    let (now, then) = (field(quarters.first()?)?, field(quarters.get(4)?)?);
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
    let enterprise_value = market_cap.map(|m| {
        m + latest.and_then(|q| q.long_term_debt).unwrap_or(0.0)
            - latest.and_then(|q| q.cash).unwrap_or(0.0)
    });
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
    }
}

fn ratio(num: Option<f64>, den: Option<f64>) -> Option<f64> {
    let (n, d) = (num?, den?);
    (d != 0.0).then(|| n / d)
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
        let window = quarters.get(i..i + 4)?;
        window.iter().map(field).sum::<Option<f64>>()
    };

    quarters
        .iter()
        .enumerate()
        .filter_map(|(i, q)| {
            let date = q.end_date.clone()?;
            let price = close_on(&date)?;
            let market_cap = q.shares.map(|s| s * price);
            let revenue_ttm = ttm(i, |p| p.revenue);
            let enterprise_value =
                market_cap.map(|m| m + q.long_term_debt.unwrap_or(0.0) - q.cash.unwrap_or(0.0));
            Some(ValuationPoint {
                date,
                price,
                market_cap,
                enterprise_value,
                pe_ttm: ratio(market_cap, ttm(i, |p| p.net_income)).filter(|pe| *pe > 0.0),
                price_to_sales: ratio(market_cap, revenue_ttm),
                price_to_book: ratio(market_cap, q.total_equity).filter(|pb| *pb > 0.0),
                ev_to_revenue: ratio(enterprise_value, revenue_ttm),
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
