//! Monthly report: how the portfolio did over one calendar month — value
//! at the start and end, money added, market gain and return, dividends,
//! fees, trades, and the biggest movers. Amounts in USD, like the rest of
//! the stored data; the app shows them in the display currency.

use crate::{
    planning::add_days,
    position::compute_positions,
    portfolio::GetDashBoardResponse,
    transaction::Transaction,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Mover {
    pub ticker: String,
    /// Gain over the month, USD (price change on the shares held).
    pub gain: Decimal,
    /// Price change over the month, percent.
    pub pct: Decimal,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MonthlyReport {
    /// `YYYY-MM`.
    pub month: String,
    /// Holdings' value at the end of the previous month.
    pub start_value: Decimal,
    pub end_value: Decimal,
    /// Bought minus sold during the month (money put in, net).
    pub net_invested: Decimal,
    /// End − start − net invested: what the market added.
    pub market_gain: Decimal,
    /// Market gain over start value plus half the money added, percent
    /// (a simple money-weighted return for the month).
    pub return_pct: Decimal,
    /// Dividends before tax, and the tax withheld.
    pub dividends: Decimal,
    pub dividend_tax: Decimal,
    pub fees: Decimal,
    pub buys: usize,
    pub sells: usize,
    /// Best first, at most three each way.
    pub best: Vec<Mover>,
    pub worst: Vec<Mover>,
    /// Holdings that had no price for a date and were valued at zero.
    pub unpriced: Vec<String>,
}

/// Last day of the month before `month` (`YYYY-MM`), and of `month`.
pub fn month_bounds(month: &str) -> Option<(String, String)> {
    let first = format!("{month}-01");
    let before = add_days(&first, -1)?;
    // The 28th plus 4 days is always next month; step back to its 1st.
    let next = add_days(&format!("{month}-28"), 4)?;
    let end = add_days(&format!("{}-01", next.get(..7)?), -1)?;
    Some((before, end))
}

/// `YYYY-MM` before `month`.
pub fn previous_month(month: &str) -> Option<String> {
    Some(add_days(&format!("{month}-01"), -1)?.get(..7)?.to_string())
}

/// Builds the report for `month` from every transaction and
/// `price_on(ticker, date)`: the USD close on or before a date.
pub fn monthly_report(
    transactions: &[Transaction],
    month: &str,
    price_on: impl Fn(&TickerSymbol, &str) -> Option<Decimal>,
) -> Option<MonthlyReport> {
    let (start, end) = month_bounds(month)?;
    let upto = |date: &str| GetDashBoardResponse {
        portfolios: vec![],
        transactions: transactions.iter().filter(|t| t.date.as_str() <= date).cloned().collect(),
    };
    let mut unpriced = Vec::new();
    let mut value_on = |date: &str| -> (Decimal, HashMap<TickerSymbol, Decimal>) {
        let positions = compute_positions(&upto(date), &HashMap::new());
        let mut shares = HashMap::new();
        let mut total = Decimal::ZERO;
        for p in positions {
            match price_on(&p.ticker, date) {
                Some(price) => total += p.shares * price,
                None => {
                    if !unpriced.contains(&p.ticker.to_string()) {
                        unpriced.push(p.ticker.to_string());
                    }
                }
            }
            shares.insert(p.ticker, p.shares);
        }
        (total, shares)
    };
    let (start_value, start_shares) = value_on(&start);
    let (end_value, end_shares) = value_on(&end);

    let mut r = MonthlyReport { month: month.to_string(), start_value, end_value, ..Default::default() };
    for t in transactions.iter().filter(|t| !t.is_cash() && t.date.get(..7) == Some(month)) {
        let gross = t.shares * t.usd_price();
        match t.transaction_type {
            TransactionType::Buy => {
                r.buys += 1;
                r.net_invested += gross + t.usd_fee();
                r.fees += t.usd_fee();
            }
            TransactionType::Sell => {
                r.sells += 1;
                r.net_invested -= gross - t.usd_fee();
                r.fees += t.usd_fee();
            }
            TransactionType::Dividend => {
                r.dividends += t.usd_price();
                r.dividend_tax += t.usd_fee();
            }
            _ => {}
        }
    }
    r.market_gain = end_value - start_value - r.net_invested + r.dividends - r.dividend_tax;
    let base = start_value + r.net_invested / Decimal::TWO;
    if base > Decimal::ZERO {
        r.return_pct = (r.market_gain / base * Decimal::ONE_HUNDRED).round_dp(2);
    }

    // Movers: price change over the month on the shares held at its end
    // (or start, if sold).
    let mut movers: Vec<Mover> = end_shares
        .keys()
        .chain(start_shares.keys())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter_map(|t| {
            let (p0, p1) = (price_on(t, &start)?, price_on(t, &end)?);
            if p0 <= Decimal::ZERO {
                return None;
            }
            let shares = end_shares.get(t).or(start_shares.get(t)).copied().unwrap_or_default();
            Some(Mover {
                ticker: t.to_string(),
                gain: ((p1 - p0) * shares).round_dp(2),
                pct: ((p1 / p0 - Decimal::ONE) * Decimal::ONE_HUNDRED).round_dp(2),
            })
        })
        .collect();
    movers.sort_by(|a, b| b.pct.cmp(&a.pct).then(a.ticker.cmp(&b.ticker)));
    r.best = movers.iter().filter(|m| m.pct > Decimal::ZERO).take(3).cloned().collect();
    r.worst = movers.iter().rev().filter(|m| m.pct < Decimal::ZERO).take(3).cloned().collect();
    unpriced.sort();
    r.unpriced = unpriced;
    Some(r)
}

/// The report as plain text, e.g. for a chat message. `money` formats an
/// amount (in the reader's currency).
pub fn report_text(r: &MonthlyReport, money: impl Fn(Decimal) -> String) -> String {
    let signed = |v: Decimal| if v >= Decimal::ZERO { format!("+{}", money(v)) } else { money(v) };
    let mut out = format!(
        "Value: {} → {}\nMarket gain: {} ({:+}%)\nNet invested: {}\n",
        money(r.start_value),
        money(r.end_value),
        signed(r.market_gain),
        r.return_pct,
        signed(r.net_invested),
    );
    if r.dividends > Decimal::ZERO {
        out.push_str(&format!("Dividends: {} (tax {})\n", money(r.dividends), money(r.dividend_tax)));
    }
    out.push_str(&format!("Trades: {} buys, {} sells · fees {}\n", r.buys, r.sells, money(r.fees)));
    let list = |movers: &[Mover]| movers.iter().map(|m| format!("{} {:+}%", m.ticker, m.pct)).collect::<Vec<_>>().join(", ");
    if !r.best.is_empty() {
        out.push_str(&format!("Best: {}\n", list(&r.best)));
    }
    if !r.worst.is_empty() {
        out.push_str(&format!("Worst: {}\n", list(&r.worst)));
    }
    if !r.unpriced.is_empty() {
        out.push_str(&format!("No price for: {}\n", r.unpriced.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn tx(ticker: &str, kind: TransactionType, date: &str, shares: Decimal, price: Decimal, fee: Decimal) -> Transaction {
        Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new(ticker).unwrap(),
            transaction_type: kind,
            shares,
            price,
            date: date.into(),
            fee,
            currency: "USD".into(),
            fx_to_usd: Decimal::ONE,
        }
    }

    #[test]
    fn bounds() {
        assert_eq!(month_bounds("2026-03"), Some(("2026-02-28".into(), "2026-03-31".into())));
        assert_eq!(month_bounds("2024-02"), Some(("2024-01-31".into(), "2024-02-29".into())));
        assert_eq!(month_bounds("2026-01"), Some(("2025-12-31".into(), "2026-01-31".into())));
        assert_eq!(previous_month("2026-01").as_deref(), Some("2025-12"));
    }

    #[test]
    fn a_month() {
        use TransactionType::*;
        let txs = vec![
            tx("AAA", Buy, "2026-01-10", dec!(10), dec!(100), dec!(0)),
            tx("BBB", Buy, "2026-01-10", dec!(5), dec!(50), dec!(0)),
            tx("AAA", Buy, "2026-02-10", dec!(5), dec!(110), dec!(1)),
            tx("BBB", Dividend, "2026-02-15", dec!(0), dec!(10), dec!(1)),
            tx("BBB", Sell, "2026-02-20", dec!(5), dec!(40), dec!(1)),
        ];
        // AAA 100 → 120, BBB 50 → 40 over February.
        let price = |t: &TickerSymbol, d: &str| {
            let end = d >= "2026-02-28";
            Some(match (t.as_str(), end) {
                ("AAA", false) => dec!(100),
                ("AAA", true) => dec!(120),
                (_, false) => dec!(50),
                _ => dec!(40),
            })
        };
        let r = monthly_report(&txs, "2026-02", price).unwrap();
        assert_eq!(r.start_value, dec!(1250));
        assert_eq!(r.end_value, dec!(1800));
        // Bought 551, sold 199.
        assert_eq!(r.net_invested, dec!(352));
        assert_eq!((r.buys, r.sells, r.fees), (1, 1, dec!(2)));
        assert_eq!((r.dividends, r.dividend_tax), (dec!(10), dec!(1)));
        // 1800 − 1250 − 352 + 9
        assert_eq!(r.market_gain, dec!(207));
        assert_eq!(r.return_pct, dec!(14.52));
        assert_eq!(r.best[0].ticker, "AAA");
        assert_eq!((r.worst[0].ticker.as_str(), r.worst[0].pct), ("BBB", dec!(-20)));
        let text = report_text(&r, |v| format!("${v}"));
        assert!(text.contains("Market gain: +$207 (+14.52%)"), "{text}");
        assert!(text.contains("Best: AAA +20"), "{text}");
    }
}
