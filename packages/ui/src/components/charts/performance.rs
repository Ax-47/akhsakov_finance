//! Performance comparison over a period, as broker apps show it.
//!
//! * Every line starts at 0% at the same point: the period start, or the
//!   first trade of the compared holdings if that is later, so everything
//!   is measured over the same window.
//! * Holdings (all, or one portfolio) use a time-weighted return: trades are
//!   cash flows at their actual price, so buying moves the value but not the
//!   return line, while the fill-to-close move on the trade day still counts.
//! * A ticker is simply its price change.

use crate::components::analysis::stats::{civil_from_days, days_from_civil, MONTHS};
use dtos::Transaction;
use rust_decimal::prelude::ToPrimitive;
use std::collections::{BTreeSet, HashMap};
use types::{candle::Candle, ticker_symbol::TickerSymbol, transaction_type::TransactionType};

/// Labels use the same UTC+7 clock as the rest of the app.
const LABEL_OFFSET_SECS: i64 = 7 * 3600;

#[derive(Clone, Copy, PartialEq)]
pub enum LabelStyle {
    /// `14:30`
    Time,
    /// `5 Mar 14:30`
    DayTime,
    /// `5 Mar`
    Date,
}

/// What one line of the chart tracks.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    /// A set of transactions (all holdings, or one portfolio).
    Holdings(Vec<Transaction>),
    Ticker(TickerSymbol),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    /// Cumulative return (%) per label, starting at 0.
    pub values: Vec<f64>,
    /// Money made over the window (holdings only): value change minus
    /// net deposits.
    pub gain: Option<f64>,
}

impl Line {
    pub fn change(&self) -> f64 {
        self.values.last().copied().unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comparison {
    pub labels: Vec<String>,
    /// One per subject, in the order given.
    pub lines: Vec<Line>,
}

/// Builds one line per subject from candles for every ticker involved.
///
/// With `last_session`, only the most recent trading day is shown, measured
/// from the previous close (the first point, labelled "Prev close"), so a
/// 1-day chart matches the usual "today" change. `charts` should then cover
/// a few days so that previous close is included.
pub fn compare(
    charts: &HashMap<TickerSymbol, Vec<Candle>>,
    subjects: &[Subject],
    style: LabelStyle,
    last_session: bool,
) -> Comparison {
    let prices = PriceGrid::new(charts);
    if prices.timeline.is_empty() || subjects.is_empty() {
        return Comparison::default();
    }

    let raw: Vec<Raw> = subjects
        .iter()
        .map(|s| match s {
            Subject::Holdings(txs) => holdings(&prices, txs),
            Subject::Ticker(t) => Raw {
                level: prices.column(t),
                ..Raw::default()
            },
        })
        .collect();

    // Earliest allowed start: the previous close for a single session.
    let floor = if last_session {
        prices.last_session_start().saturating_sub(1)
    } else {
        0
    };

    // Start just before the first trade inside the window — unless some
    // compared holdings already existed when the window opened.
    let holdings_raw = raw.iter().filter(|r| r.is_holdings);
    let start = if holdings_raw.clone().any(|r| r.held_at_start) {
        floor
    } else {
        holdings_raw
            .filter_map(|r| r.first_trade_step)
            .min()
            .map_or(floor, |k| k.saturating_sub(1).max(floor))
    };

    let mut labels: Vec<String> = prices.timeline[start..]
        .iter()
        .map(|ts| label(*ts, style))
        .collect();
    if last_session && start > 0 && start == floor {
        labels[0] = "Prev close".into();
    }
    Comparison {
        labels,
        lines: raw.iter().map(|r| r.line_from(start)).collect(),
    }
}

// ─── Internals ────────────────────────────────────────────────────────────────

/// Latest close for every ticker at every timestamp of the union timeline.
struct PriceGrid<'a> {
    timeline: Vec<i64>,
    index_of: HashMap<&'a TickerSymbol, usize>,
    /// `closes[step][ticker]`
    closes: Vec<Vec<Option<f64>>>,
}

impl<'a> PriceGrid<'a> {
    fn new(charts: &'a HashMap<TickerSymbol, Vec<Candle>>) -> Self {
        let tickers: Vec<&TickerSymbol> = charts.keys().collect();
        let series: Vec<Vec<(i64, f64)>> = tickers
            .iter()
            .map(|t| {
                let mut points: Vec<(i64, f64)> = charts[*t]
                    .iter()
                    .filter_map(|c| Some((c.ts.timestamp(), c.close.to_f64()?)))
                    .filter(|(_, close)| *close > 0.0)
                    .collect();
                points.sort_by_key(|p| p.0);
                points
            })
            .collect();
        let timeline: Vec<i64> = series
            .iter()
            .flat_map(|s| s.iter().map(|p| p.0))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        // Before a ticker's first candle, fall back to that first close.
        let mut cursor = vec![0usize; series.len()];
        let closes = timeline
            .iter()
            .map(|&ts| {
                series
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        while cursor[i] + 1 < s.len() && s[cursor[i] + 1].0 <= ts {
                            cursor[i] += 1;
                        }
                        s.get(cursor[i]).map(|p| p.1)
                    })
                    .collect()
            })
            .collect();

        Self {
            timeline,
            index_of: tickers.iter().enumerate().map(|(i, t)| (*t, i)).collect(),
            closes,
        }
    }

    /// Index of the first timestamp on the last (UTC) trading day. US
    /// sessions fall within one UTC day, so this is today's open.
    fn last_session_start(&self) -> usize {
        let Some(last) = self.timeline.last() else {
            return 0;
        };
        let day = last.div_euclid(86_400);
        self.timeline
            .iter()
            .position(|ts| ts.div_euclid(86_400) == day)
            .unwrap_or(0)
    }

    fn column(&self, ticker: &TickerSymbol) -> Vec<f64> {
        let Some(&i) = self.index_of.get(ticker) else {
            return vec![1.0; self.timeline.len()];
        };
        let first = self.closes.iter().find_map(|row| row[i]).unwrap_or(1.0);
        self.closes
            .iter()
            .map(|row| row[i].unwrap_or(first))
            .collect()
    }
}

/// Per-step data for one subject before rebasing to the window start.
#[derive(Default)]
struct Raw {
    /// Growth index (holdings) or price (ticker) per step.
    level: Vec<f64>,
    is_holdings: bool,
    /// Holdings only: market value and cumulative net deposits per step.
    value: Vec<f64>,
    flow: Vec<f64>,
    first_trade_step: Option<usize>,
    held_at_start: bool,
}

impl Raw {
    fn line_from(&self, start: usize) -> Line {
        let Some(&base) = self.level.get(start) else {
            return Line::default();
        };
        let gain = self.is_holdings.then(|| {
            let end = self.value.len() - 1;
            (self.value[end] - self.value[start]) - (self.flow[end] - self.flow[start])
        });
        Line {
            values: self.level[start..]
                .iter()
                .map(|v| {
                    if base > 0.0 {
                        (v / base - 1.0) * 100.0
                    } else {
                        0.0
                    }
                })
                .collect(),
            gain,
        }
    }
}

/// Time-weighted growth of a set of transactions.
fn holdings(prices: &PriceGrid, transactions: &[Transaction]) -> Raw {
    // Trades as (unix time, ticker index, share delta, price), oldest first.
    let mut trades: Vec<(i64, usize, f64, f64)> = transactions
        .iter()
        .filter_map(|tx| {
            let sign = match tx.transaction_type {
                TransactionType::Buy => 1.0,
                TransactionType::Sell => -1.0,
                _ => return None,
            };
            Some((
                date_to_unix(&tx.date)?,
                *prices.index_of.get(&tx.ticker)?,
                sign * tx.shares.to_f64()?,
                tx.price.to_f64()?,
            ))
        })
        .collect();
    trades.sort_by_key(|t| t.0);

    let n = prices.index_of.len();
    let mut shares = vec![0.0; n];
    let value_of = |shares: &[f64], row: &[Option<f64>]| -> f64 {
        (0..n).filter_map(|i| Some(shares[i] * row[i]?)).sum()
    };

    let mut raw = Raw {
        is_holdings: true,
        ..Raw::default()
    };
    let (mut growth, mut net_flow, mut next) = (1.0, 0.0, 0);

    for (k, &ts) in prices.timeline.iter().enumerate() {
        let row = &prices.closes[k];
        let value_before = match k {
            0 => 0.0,
            _ => value_of(&shares, &prices.closes[k - 1]),
        };
        let mut flow = 0.0;
        while next < trades.len() && trades[next].0 <= ts {
            let (_, i, delta, price) = trades[next];
            shares[i] += delta;
            flow += delta * price;
            next += 1;
            if k == 0 {
                raw.held_at_start = true;
            } else {
                raw.first_trade_step.get_or_insert(k);
            }
        }
        net_flow += flow;
        let value = value_of(&shares, row);

        // Trades before the window just set the starting holdings.
        let invested = value_before + flow;
        if k > 0 && invested > 0.0 {
            growth *= value / invested;
        }
        raw.level.push(growth);
        raw.value.push(value);
        raw.flow.push(net_flow);
    }
    raw
}

/// `YYYY-MM-DD` (UTC midnight) to unix seconds.
fn date_to_unix(date: &str) -> Option<i64> {
    let mut parts = date.trim().splitn(3, '-');
    let y = parts.next()?.parse().ok()?;
    let m = parts.next()?.parse().ok()?;
    let d = parts.next()?.parse().ok()?;
    Some(days_from_civil(y, m, d) * 86_400)
}

fn label(ts: i64, style: LabelStyle) -> String {
    let local = ts + LABEL_OFFSET_SECS;
    let (_, m, d) = civil_from_days(local.div_euclid(86_400));
    let secs = local.rem_euclid(86_400);
    let (hh, mm) = (secs / 3600, secs % 3600 / 60);
    let month = MONTHS[(m - 1) as usize];
    match style {
        LabelStyle::Time => format!("{hh:02}:{mm:02}"),
        LabelStyle::DayTime => format!("{d} {month} {hh:02}:{mm:02}"),
        LabelStyle::Date => format!("{d} {month}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    const DAY: i64 = 86_400;
    const START: i64 = 20_000;

    fn candles(closes: &[f64]) -> Vec<Candle> {
        closes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let close = Decimal::try_from(*c).unwrap();
                Candle {
                    ts: Utc.timestamp_opt((START + i as i64) * DAY, 0).unwrap(),
                    open: close,
                    high: close,
                    low: close,
                    close,
                    volume: None,
                }
            })
            .collect()
    }

    fn buy(ticker: &TickerSymbol, day: i64, shares: i64, price: f64) -> Transaction {
        let (y, m, d) = civil_from_days(day);
        Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: ticker.clone(),
            transaction_type: TransactionType::Buy,
            shares: Decimal::from(shares),
            price: Decimal::try_from(price).unwrap(),
            date: format!("{y:04}-{m:02}-{d:02}"),
            fee: Decimal::ZERO,
        }
    }

    fn sym(s: &str) -> TickerSymbol {
        TickerSymbol::new(s).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn buys_are_cash_flows_not_gains() {
        let (spx, aaa) = (sym("^GSPC"), sym("AAA"));
        let charts = HashMap::from([
            (spx.clone(), candles(&[100.0, 110.0, 110.0, 121.0])),
            (aaa.clone(), candles(&[10.0, 10.0, 20.0, 20.0])),
        ]);
        // 1 share held from before the window; 9 more bought at 20 on day 3.
        let txs = vec![
            buy(&aaa, START - 10, 1, 10.0),
            buy(&aaa, START + 3, 9, 20.0),
        ];

        let cmp = compare(
            &charts,
            &[Subject::Ticker(spx), Subject::Holdings(txs)],
            LabelStyle::Date,
            false,
        );
        assert_eq!(cmp.labels.len(), 4);
        let (market, mine) = (&cmp.lines[0], &cmp.lines[1]);
        assert_eq!((market.values[0], mine.values[0]), (0.0, 0.0));
        assert!(close(market.change(), 21.0));
        assert_eq!(market.gain, None);
        // Price doubled while holding; the later buy adds value, not return.
        assert!(close(mine.change(), 100.0));
        assert!(close(mine.gain.unwrap(), 10.0));
    }

    #[test]
    fn window_starts_at_first_trade_and_counts_fill_price() {
        let (spx, aaa) = (sym("^GSPC"), sym("AAA"));
        let charts = HashMap::from([
            (spx.clone(), candles(&[100.0, 100.0, 105.0, 105.0, 110.25])),
            (aaa.clone(), candles(&[10.0, 10.0, 10.0, 10.0, 11.0])),
        ]);
        // Bought on day 3 at 8, closed at 10; then rose to 11.
        let txs = vec![buy(&aaa, START + 3, 2, 8.0)];

        let cmp = compare(
            &charts,
            &[Subject::Ticker(spx), Subject::Holdings(txs)],
            LabelStyle::Date,
            false,
        );
        // Starts the day before the trade (day 2), both at 0%.
        assert_eq!(cmp.labels.len(), 3);
        // S&P 105 → 110.25 over the same window, not 100 → 110.25.
        assert!(close(cmp.lines[0].change(), 5.0));
        // 8 → 11 = +37.5%, including the fill-to-close move on day 3.
        assert!(close(cmp.lines[1].change(), 37.5));
        assert!(close(cmp.lines[1].gain.unwrap(), 6.0));
    }

    #[test]
    fn compares_portfolios_and_single_stocks() {
        let (aaa, bbb) = (sym("AAA"), sym("BBB"));
        let charts = HashMap::from([
            (aaa.clone(), candles(&[10.0, 10.0, 20.0])),
            (bbb.clone(), candles(&[10.0, 10.0, 5.0])),
        ]);
        let one = buy(&aaa, START + 1, 1, 10.0);
        let two = buy(&bbb, START + 1, 1, 10.0);

        let cmp = compare(
            &charts,
            &[
                Subject::Holdings(vec![one.clone(), two.clone()]),
                Subject::Holdings(vec![one]),
                Subject::Ticker(bbb),
            ],
            LabelStyle::Date,
            false,
        );
        // All holdings: 20 → 25 = +25%; portfolio of AAA only: +100%.
        assert!(close(cmp.lines[0].change(), 25.0));
        assert!(close(cmp.lines[1].change(), 100.0));
        // BBB on its own, over the same window: 10 → 5.
        assert!(close(cmp.lines[2].change(), -50.0));
    }

    /// Intraday candles: `days` of `per_day` closes, 5 minutes apart from
    /// 13:30 UTC (the US open).
    fn intraday(closes_by_day: &[&[f64]]) -> Vec<Candle> {
        closes_by_day
            .iter()
            .enumerate()
            .flat_map(|(d, closes)| {
                closes.iter().enumerate().map(move |(i, c)| {
                    let close = Decimal::try_from(*c).unwrap();
                    let ts = (START + d as i64) * DAY + 13 * 3600 + 1800 + i as i64 * 300;
                    Candle {
                        ts: Utc.timestamp_opt(ts, 0).unwrap(),
                        open: close,
                        high: close,
                        low: close,
                        close,
                        volume: None,
                    }
                })
            })
            .collect()
    }

    #[test]
    fn one_day_is_measured_from_previous_close() {
        let (spx, aaa) = (sym("^GSPC"), sym("AAA"));
        // Yesterday closed at 100; today gapped up to 102 and drifted to 101.
        let charts = HashMap::from([
            (
                spx.clone(),
                intraday(&[&[99.0, 100.0], &[102.0, 101.5, 101.0]]),
            ),
            (aaa.clone(), intraday(&[&[49.0, 50.0], &[51.0, 50.8, 50.5]])),
        ]);
        let txs = vec![buy(&aaa, START - 30, 2, 40.0)];

        let cmp = compare(
            &charts,
            &[Subject::Ticker(spx), Subject::Holdings(txs)],
            LabelStyle::Time,
            true,
        );
        // Previous close + today's three points.
        assert_eq!(cmp.labels.len(), 4);
        assert_eq!(cmp.labels[0], "Prev close");
        // +1% from yesterday's close, not −0.98% from today's open.
        assert!(close(cmp.lines[0].change(), 1.0));
        // Holdings 50 → 50.5 = +1%, and $1 on 2 shares.
        assert!(close(cmp.lines[1].change(), 1.0));
        assert!(close(cmp.lines[1].gain.unwrap(), 1.0));
    }

    #[test]
    fn labels_use_local_clock() {
        // 2026-01-01 00:00 UTC is 07:00 in UTC+7.
        assert_eq!(label(20_454 * DAY, LabelStyle::Time), "07:00");
        assert_eq!(label(20_454 * DAY, LabelStyle::Date), "1 Jan");
    }
}
