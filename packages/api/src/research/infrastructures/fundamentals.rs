//! Fundamentals from Yahoo: profile, statistics, statements, valuation
//! history and analyst views.

use dtos::fundamentals::{
    valuation_history, Analysts, EpsSurprise, KeyStats, PeriodFinancials, Profile,
    StockFundamentals,
};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::BTreeMap;
use yfinance_rs::{
    BalanceSheetRow, CashflowRow, IncomeStatementRow, Interval, Money, Price, Range, Ticker,
    YfClient,
};

pub async fn fetch(client: &YfClient, symbol: &str) -> Result<StockFundamentals, String> {
    let t = Ticker::new(client, symbol);
    let (info, q_inc, q_bal, q_cf, a_inc, a_bal, a_cf, earnings, trend, history) = tokio::join!(
        t.info(),
        t.quarterly_income_stmt(None),
        t.quarterly_balance_sheet(None),
        t.quarterly_cashflow(None),
        t.income_stmt(None),
        t.balance_sheet(None),
        t.cashflow(None),
        t.earnings(None),
        t.earnings_trend(None),
        t.history(Some(Range::Y2), Some(Interval::D1), false),
    );
    let info = info.map_err(|e| format!("{symbol}: {e}"))?;

    let quarterly = merge(
        q_inc.unwrap_or_default(),
        q_bal.unwrap_or_default(),
        q_cf.unwrap_or_default(),
    );
    let annual = merge(
        a_inc.unwrap_or_default(),
        a_bal.unwrap_or_default(),
        a_cf.unwrap_or_default(),
    );
    let closes: Vec<(String, f64)> = history
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| {
            Some((
                c.ts.date_naive().to_string(),
                c.ohlc.close.into_inner().to_f64()?,
            ))
        })
        .collect();
    let price = closes.last().map(|c| c.1);

    let ks = &info.key_statistics;
    let forward_eps = trend.ok().and_then(|rows| {
        ["+1y", "0y"].iter().find_map(|code| {
            rows.iter()
                .find(|r| r.period.to_string() == *code)
                .and_then(|r| price_f64(r.earnings_estimate.avg.as_ref()))
        })
    });

    let profile = match &info.profile {
        Some(yfinance_rs::Profile::Company(c)) => Profile {
            name: Some(c.name.clone()),
            sector: c.sector.clone(),
            industry: c.industry.clone(),
            website: c.website.clone(),
            summary: c.summary.clone(),
        },
        Some(yfinance_rs::Profile::Fund(f)) => Profile {
            name: Some(f.name.clone()),
            ..Profile::default()
        },
        _ => Profile::default(),
    };

    let analysts = info.recommendation_summary.as_ref().map(|r| {
        let target = info.price_target.as_ref();
        Analysts {
            strong_buy: r.strong_buy.unwrap_or(0),
            buy: r.buy.unwrap_or(0),
            hold: r.hold.unwrap_or(0),
            sell: r.sell.unwrap_or(0),
            strong_sell: r.strong_sell.unwrap_or(0),
            mean: dec(r.mean),
            rating: r.mean_rating_text.clone(),
            target_low: target.and_then(|t| price_f64(t.low.as_ref())),
            target_mean: target.and_then(|t| price_f64(t.mean.as_ref())),
            target_high: target.and_then(|t| price_f64(t.high.as_ref())),
            count: target.and_then(|t| t.number_of_analysts),
        }
    });

    let eps_surprises = earnings
        .map(|e| {
            e.quarterly_eps
                .iter()
                .map(|q| EpsSurprise {
                    period: q.period.to_string(),
                    actual: price_f64(q.actual.as_ref()),
                    estimate: price_f64(q.estimate.as_ref()),
                })
                .collect()
        })
        .unwrap_or_default();

    let stats = KeyStats {
        price,
        market_cap: money(ks.market_cap.as_ref()),
        shares_outstanding: ks.shares_outstanding.map(|s| s as f64),
        eps_ttm: price_f64(ks.eps_trailing_twelve_months.as_ref()),
        pe_ttm: dec(ks.pe_trailing_twelve_months),
        forward_pe: price
            .zip(forward_eps)
            .filter(|(_, e)| *e > 0.0)
            .map(|(p, e)| p / e),
        forward_eps,
        dividend_yield: dec(ks.dividend_yield_trailing),
        ex_dividend_date: ks.ex_dividend_date.map(|d| d.to_string()),
        next_earnings: info
            .calendar
            .as_ref()
            .and_then(|c| c.earnings_dates.first())
            .map(|d| d.date_naive().to_string()),
        high_52w: price_f64(ks.fifty_two_week_high.as_ref()),
        low_52w: price_f64(ks.fifty_two_week_low.as_ref()),
        sma50: price_f64(info.moving_averages.fifty_day.as_ref()),
        sma200: price_f64(info.moving_averages.two_hundred_day.as_ref()),
    };

    Ok(StockFundamentals {
        profile,
        stats,
        valuation: valuation_history(&quarterly, &closes),
        quarterly,
        annual,
        eps_surprises,
        analysts,
    })
}

/// Joins the three statements by period, newest first.
fn merge(
    income: Vec<IncomeStatementRow>,
    balance: Vec<BalanceSheetRow>,
    cashflow: Vec<CashflowRow>,
) -> Vec<PeriodFinancials> {
    let mut by_period: BTreeMap<String, PeriodFinancials> = BTreeMap::new();
    for r in income {
        let p = slot(&mut by_period, r.period.to_string());
        p.revenue = money(r.total_revenue.as_ref());
        p.gross_profit = money(r.gross_profit.as_ref());
        p.operating_income = money(r.operating_income.as_ref());
        p.net_income = money(r.net_income.as_ref());
    }
    for r in balance {
        let p = slot(&mut by_period, r.period.to_string());
        p.total_assets = money(r.total_assets.as_ref());
        p.total_liabilities = money(r.total_liabilities.as_ref());
        p.total_equity = money(r.total_equity.as_ref());
        p.cash = money(r.cash.as_ref());
        p.long_term_debt = money(r.long_term_debt.as_ref());
        p.current_assets = money(r.current_assets.as_ref());
        p.current_liabilities = money(r.current_liabilities.as_ref());
        p.shares = r.shares_outstanding.map(|s| s as f64);
    }
    for r in cashflow {
        let p = slot(&mut by_period, r.period.to_string());
        p.operating_cashflow = money(r.operating_cashflow.as_ref());
        p.free_cash_flow = money(r.free_cash_flow.as_ref());
    }
    let mut rows: Vec<PeriodFinancials> = by_period.into_values().collect();
    rows.sort_by(|a, b| {
        b.end_date
            .cmp(&a.end_date)
            .then_with(|| b.period.cmp(&a.period))
    });
    rows
}

fn slot(map: &mut BTreeMap<String, PeriodFinancials>, period: String) -> &mut PeriodFinancials {
    map.entry(period.clone())
        .or_insert_with(|| PeriodFinancials {
            end_date: end_date(&period),
            period,
            ..PeriodFinancials::default()
        })
}

/// Period end date for `2026-07-31`, `2026Q2` or `2025`.
fn end_date(period: &str) -> Option<String> {
    if period.len() == 10 && period.as_bytes()[4] == b'-' {
        return Some(period.to_string());
    }
    if let Some((year, q)) = period.split_once('Q') {
        let end = match q {
            "1" => "03-31",
            "2" => "06-30",
            "3" => "09-30",
            "4" => "12-31",
            _ => return None,
        };
        return Some(format!("{year}-{end}"));
    }
    (period.len() == 4 && period.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("{period}-12-31"))
}

fn money(m: Option<&Money>) -> Option<f64> {
    m.and_then(|m| m.amount().to_f64())
}

fn price_f64(p: Option<&Price>) -> Option<f64> {
    p.and_then(|p| p.amount().to_f64())
}

fn dec(d: Option<Decimal>) -> Option<f64> {
    d.and_then(|d| d.to_f64())
}
