//! Thai personal income tax view of the portfolio, in baht per calendar
//! (tax) year: dividends and the tax withheld on them, gains on Thai and
//! foreign assets, and SSF / RMF / Thai ESG purchases against their limits.
//!
//! The rules summarised (check the Revenue Department for your case):
//! - Thai dividends are withheld at 10%. You may leave it at that, or
//!   declare them and claim the dividend tax credit (roughly 20/80 of the
//!   dividend for companies paying 20% corporate tax).
//! - Gains on SET shares are tax-free for individuals.
//! - Since 2024, foreign income (dividends, gains) of Thai residents is
//!   taxable when it's brought into Thailand. US dividends are withheld at
//!   15% under the tax treaty (with a W-8BEN).
//! - SSF, RMF and Thai ESG purchases are deductible up to a share of
//!   income and a cap, if the units are held long enough.

use crate::{
    assets::TaxWrapper,
    planning::{days_between, fifo_lots},
    transaction::Transaction,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{BTreeMap, HashMap};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};

/// Whether the ticker trades on the Stock Exchange of Thailand.
pub fn is_thai(ticker: &str) -> bool {
    ticker.ends_with(".BK")
}

/// Usual withholding tax on dividends paid to a Thai resident, percent:
/// 10% in Thailand, 15% in the US (treaty rate with a W-8BEN), none in
/// Hong Kong, Singapore or the UK. `None` where it varies.
pub fn withholding_rate(ticker: &str) -> Option<Decimal> {
    if ticker.starts_with('$') || ticker.starts_with('^') || ticker.contains('=') || ticker.contains('-') {
        return None;
    }
    match ticker.rsplit_once('.').map(|(_, suffix)| suffix) {
        None => Some(dec!(15)),
        Some("BK") => Some(dec!(10)),
        Some("HK" | "SI" | "L") => Some(Decimal::ZERO),
        _ => None,
    }
}

/// Transactions with every amount in baht: `fx_to_usd` becomes the baht
/// rate, so the USD-based maths (FIFO lots, dividends) counts in baht.
/// `thb_per_usd(date)` is the USD/THB rate on a date; transactions it has
/// no rate for are left out (unless already in baht).
pub fn in_baht(transactions: &[Transaction], thb_per_usd: impl Fn(&str) -> Option<Decimal>) -> Vec<Transaction> {
    transactions
        .iter()
        .filter(|t| !t.is_cash())
        .filter_map(|t| {
            let rate = if t.currency == "THB" { Decimal::ONE } else { t.fx_to_usd * thb_per_usd(&t.date)? };
            Some(Transaction { fx_to_usd: rate, ..t.clone() })
        })
        .collect()
}

/// Dates on which a baht rate is needed for [`in_baht`].
pub fn dates_needing_rates(transactions: &[Transaction]) -> Vec<String> {
    let mut dates: Vec<String> = transactions
        .iter()
        .filter(|t| !t.is_cash() && t.currency != "THB")
        .map(|t| t.date.clone())
        .collect();
    dates.sort();
    dates.dedup();
    dates
}

/// One tax year, in baht.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TaxYear {
    pub year: i32,
    /// Thai dividends before tax, and the tax withheld.
    pub thai_dividends: Decimal,
    pub thai_withheld: Decimal,
    /// Foreign dividends before tax, and the foreign tax withheld.
    pub foreign_dividends: Decimal,
    pub foreign_withheld: Decimal,
    /// Realized gains on SET shares (tax-free).
    pub thai_gains: Decimal,
    /// Realized gains on foreign assets (taxable when brought into Thailand).
    pub foreign_gains: Decimal,
    /// Purchases of each kind of tax-saving fund.
    pub fund_buys: BTreeMap<String, Decimal>,
}

impl TaxYear {
    /// Estimated dividend tax credit when Thai dividends are declared,
    /// assuming the companies paid 20% corporate tax.
    pub fn dividend_credit(&self) -> Decimal {
        (self.thai_dividends * dec!(20) / dec!(80)).round_dp(2)
    }

    /// Foreign income that becomes taxable if brought into Thailand.
    pub fn foreign_income(&self) -> Decimal {
        self.foreign_dividends + self.foreign_gains.max(Decimal::ZERO)
    }

    pub fn bought(&self, wrapper: TaxWrapper) -> Decimal {
        self.fund_buys.get(wrapper.key()).copied().unwrap_or_default()
    }
}

/// Every year with dividends, sales or fund purchases, newest first.
/// `baht` comes from [`in_baht`]; `wrappers` says which tickers are
/// tax-saving funds.
pub fn tax_years(baht: &[Transaction], wrappers: &HashMap<TickerSymbol, TaxWrapper>) -> Vec<TaxYear> {
    let mut years: BTreeMap<i32, TaxYear> = BTreeMap::new();
    let year_of = |date: &str| date.get(..4).and_then(|y| y.parse::<i32>().ok());
    fn entry(years: &mut BTreeMap<i32, TaxYear>, y: i32) -> &mut TaxYear {
        years.entry(y).or_insert_with(|| TaxYear { year: y, ..Default::default() })
    }
    for tx in baht {
        let Some(y) = year_of(&tx.date) else { continue };
        match tx.transaction_type {
            TransactionType::Dividend => {
                let e = entry(&mut years, y);
                if is_thai(&tx.ticker) {
                    e.thai_dividends += tx.usd_price();
                    e.thai_withheld += tx.usd_fee();
                } else {
                    e.foreign_dividends += tx.usd_price();
                    e.foreign_withheld += tx.usd_fee();
                }
            }
            TransactionType::Buy => {
                if let Some(w) = wrappers.get(&tx.ticker) {
                    *entry(&mut years, y).fund_buys.entry(w.key().to_string()).or_default() +=
                        tx.shares * tx.usd_price() + tx.usd_fee();
                }
            }
            _ => {}
        }
    }
    let (_, realized) = fifo_lots(baht);
    for r in realized {
        let Some(y) = year_of(&r.sold) else { continue };
        let e = entry(&mut years, y);
        if is_thai(&r.ticker) || wrappers.contains_key(&r.ticker) {
            // Thai funds' gains are tax-free for individuals too.
            e.thai_gains += r.gain();
        } else {
            e.foreign_gains += r.gain();
        }
    }
    years.into_values().rev().collect()
}

/// A tax-saving fund's deduction for one year.
#[derive(Debug, Clone, PartialEq)]
pub struct FundRoom {
    pub wrapper: TaxWrapper,
    /// Most you can deduct this year.
    pub limit: Decimal,
    pub bought: Decimal,
    /// Still deductible this year (never negative).
    pub room: Decimal,
}

/// SSF: 30% of income up to ฿200,000. RMF: 30% up to ฿500,000. Thai ESG:
/// 30% up to ฿300,000, on its own. SSF and RMF together (with provident
/// funds, not tracked here) stay within ฿500,000.
pub fn fund_room(income: Decimal, year: &TaxYear) -> Vec<FundRoom> {
    let share = income.max(Decimal::ZERO) * dec!(0.30);
    let cap = |c: Decimal| share.min(c);
    let (ssf, rmf) = (year.bought(TaxWrapper::Ssf), year.bought(TaxWrapper::Rmf));
    let combined_left = (dec!(500000) - ssf - rmf).max(Decimal::ZERO);
    TaxWrapper::ALL
        .into_iter()
        .map(|w| {
            let (limit, bought) = match w {
                TaxWrapper::Ssf => (cap(dec!(200000)), ssf),
                TaxWrapper::Rmf => (cap(dec!(500000)), rmf),
                TaxWrapper::ThaiEsg => (cap(dec!(300000)), year.bought(w)),
            };
            let mut room = (limit - bought).max(Decimal::ZERO);
            if w != TaxWrapper::ThaiEsg {
                room = room.min(combined_left);
            }
            FundRoom { wrapper: w, limit, bought, room }
        })
        .collect()
}

/// Years a tax-saving fund must be held from each purchase.
pub fn holding_years(wrapper: TaxWrapper) -> i64 {
    match wrapper {
        TaxWrapper::Ssf => 10,
        TaxWrapper::Rmf | TaxWrapper::ThaiEsg => 5,
    }
}

/// A sale of fund units bought too recently to keep the tax deduction.
#[derive(Debug, Clone, PartialEq)]
pub struct EarlySale {
    pub ticker: TickerSymbol,
    pub wrapper: TaxWrapper,
    pub bought: String,
    pub sold: String,
    pub shares: Decimal,
}

/// Sales that break a fund's holding rule. RMF units must also be held
/// until age 55, checked when `birth_year` is known.
pub fn early_sales(
    transactions: &[Transaction],
    wrappers: &HashMap<TickerSymbol, TaxWrapper>,
    birth_year: Option<i32>,
) -> Vec<EarlySale> {
    let (_, realized) = fifo_lots(transactions);
    realized
        .into_iter()
        .filter_map(|r| {
            let wrapper = *wrappers.get(&r.ticker)?;
            let held = days_between(&r.bought, &r.sold)?;
            let too_soon = held < holding_years(wrapper) * 365 + holding_years(wrapper) / 4;
            let too_young = wrapper == TaxWrapper::Rmf
                && birth_year.is_some_and(|b| r.sold.get(..4).and_then(|y| y.parse::<i32>().ok()).is_some_and(|y| y - b < 55));
            (too_soon || too_young).then(|| EarlySale {
                ticker: r.ticker,
                wrapper,
                bought: r.bought,
                sold: r.sold,
                shares: r.shares,
            })
        })
        .collect()
}

/// The report as text (for copying or saving), in baht.
pub fn report_text(year: &TaxYear, income: Option<Decimal>) -> String {
    let b = |v: Decimal| format!("฿{:.2}", v.round_dp(2));
    let mut out = format!("Thai tax summary {}\n\n", year.year);
    out.push_str(&format!(
        "Thai dividends (before tax): {}\nWithheld at source (10%): {}\nDividend tax credit if declared (est.): {}\n\n",
        b(year.thai_dividends),
        b(year.thai_withheld),
        b(year.dividend_credit())
    ));
    out.push_str(&format!(
        "Foreign dividends (before tax): {}\nForeign tax withheld: {}\nForeign realized gains: {}\nTaxable if brought into Thailand: {}\n\n",
        b(year.foreign_dividends),
        b(year.foreign_withheld),
        b(year.foreign_gains),
        b(year.foreign_income())
    ));
    out.push_str(&format!("Gains on Thai shares and funds (tax-free): {}\n", b(year.thai_gains)));
    for w in TaxWrapper::ALL {
        let bought = year.bought(w);
        if bought > Decimal::ZERO {
            out.push_str(&format!("{w} bought: {}\n", b(bought)));
        }
    }
    if let Some(income) = income {
        out.push('\n');
        for r in fund_room(income, year) {
            out.push_str(&format!("{} deduction room left: {} (limit {})\n", r.wrapper, b(r.room), b(r.limit)));
        }
    }
    out.push_str("\nFor information only — check with the Revenue Department or a tax adviser.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tx(ticker: &str, kind: TransactionType, date: &str, shares: Decimal, price: Decimal, fee: Decimal, currency: &str) -> Transaction {
        Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new(ticker).unwrap(),
            transaction_type: kind,
            shares,
            price,
            date: date.into(),
            fee,
            currency: currency.into(),
            fx_to_usd: if currency == "THB" { dec!(0.03) } else { Decimal::ONE },
        }
    }

    #[test]
    fn withholding_rates() {
        assert_eq!(withholding_rate("PTT.BK"), Some(dec!(10)));
        assert_eq!(withholding_rate("AAPL"), Some(dec!(15)));
        assert_eq!(withholding_rate("0700.HK"), Some(dec!(0)));
        assert_eq!(withholding_rate("BTC-USD"), None);
        assert_eq!(withholding_rate("7203.T"), None);
    }

    #[test]
    fn a_year_in_baht() {
        use TransactionType::*;
        let txs = vec![
            tx("PTT.BK", Buy, "2025-01-10", dec!(100), dec!(30), dec!(0), "THB"),
            tx("PTT.BK", Sell, "2026-03-01", dec!(100), dec!(35), dec!(0), "THB"),
            tx("PTT.BK", Dividend, "2026-04-01", dec!(0), dec!(200), dec!(20), "THB"),
            tx("AAPL", Buy, "2026-01-05", dec!(1), dec!(100), dec!(0), "USD"),
            tx("AAPL", Sell, "2026-06-01", dec!(1), dec!(120), dec!(0), "USD"),
            tx("AAPL", Dividend, "2026-05-01", dec!(0), dec!(10), dec!(1.5), "USD"),
            tx("KFSSF", Buy, "2026-07-01", dec!(1000), dec!(10), dec!(0), "THB"),
        ];
        // 35 baht per dollar, except 36 in June.
        let rate = |d: &str| Some(if d.starts_with("2026-06") { dec!(36) } else { dec!(35) });
        assert_eq!(dates_needing_rates(&txs), vec!["2026-01-05", "2026-05-01", "2026-06-01"]);
        let baht = in_baht(&txs, rate);
        let wrappers = HashMap::from([(TickerSymbol::new("KFSSF").unwrap(), TaxWrapper::Ssf)]);
        let years = tax_years(&baht, &wrappers);
        let y = &years[0];
        assert_eq!(y.year, 2026);
        assert_eq!((y.thai_dividends, y.thai_withheld, y.dividend_credit()), (dec!(200), dec!(20), dec!(50)));
        assert_eq!((y.foreign_dividends, y.foreign_withheld), (dec!(350), dec!(52.5)));
        assert_eq!(y.thai_gains, dec!(500));
        // Bought at 100 × 35, sold at 120 × 36.
        assert_eq!(y.foreign_gains, dec!(820));
        assert_eq!(y.foreign_income(), dec!(1170));
        assert_eq!(y.bought(TaxWrapper::Ssf), dec!(10000));
        assert_eq!(years.len(), 1, "2025 had only an ordinary purchase");
        assert!(report_text(y, Some(dec!(1000000))).contains("SSF deduction room left: ฿190000.00"));
    }

    #[test]
    fn fund_limits() {
        let mut y = TaxYear::default();
        y.fund_buys.insert("ssf".into(), dec!(150000));
        y.fund_buys.insert("rmf".into(), dec!(300000));
        let room = fund_room(dec!(1000000), &y);
        // 30% of 1M = 300k: SSF capped at 200k, but SSF + RMF only have 50k left.
        assert_eq!((room[0].limit, room[0].room), (dec!(200000), dec!(50000)));
        assert_eq!((room[1].limit, room[1].room), (dec!(300000), dec!(0)));
        assert_eq!((room[2].limit, room[2].room), (dec!(300000), dec!(300000)));
    }

    #[test]
    fn early_fund_sales() {
        use TransactionType::*;
        let txs = vec![
            tx("KFSSF", Buy, "2020-01-01", dec!(10), dec!(10), dec!(0), "THB"),
            tx("KFSSF", Buy, "2024-01-01", dec!(10), dec!(10), dec!(0), "THB"),
            tx("KFSSF", Sell, "2030-06-01", dec!(15), dec!(12), dec!(0), "THB"),
            tx("KFRMF", Buy, "2020-01-01", dec!(10), dec!(10), dec!(0), "THB"),
            tx("KFRMF", Sell, "2026-01-01", dec!(10), dec!(10), dec!(0), "THB"),
        ];
        let wrappers = HashMap::from([
            (TickerSymbol::new("KFSSF").unwrap(), TaxWrapper::Ssf),
            (TickerSymbol::new("KFRMF").unwrap(), TaxWrapper::Rmf),
        ]);
        let early = early_sales(&txs, &wrappers, None);
        // The 2020 SSF lot is 10 years old; 5 units of the 2024 lot aren't.
        assert_eq!(early.len(), 1);
        assert_eq!((early[0].bought.as_str(), early[0].shares), ("2024-01-01", dec!(5)));
        // The RMF was held 6 years, but its owner is only 36.
        let early = early_sales(&txs, &wrappers, Some(1990));
        assert_eq!(early.len(), 2);
        assert!(early.iter().any(|e| e.wrapper == TaxWrapper::Rmf && e.sold == "2026-01-01"));
    }
}
