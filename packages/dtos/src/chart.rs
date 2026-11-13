//! A stock's price chart as data: the candles drawn, kept moving by
//! streamed trades between fetches, and what's marked on them.

use serde::{Deserialize, Serialize};
use types::candle::Candle;

/// A gap between candles longer than this ends a session (seconds):
/// longer than a lunch break, shorter than any night.
const SESSION_GAP: i64 = 3 * 3600;

/// The feed's count of the day's volume can't grow by more than this share
/// of itself in one trade: a bigger jump is the count being picked up from
/// another source (or from nothing), not trading.
const MAX_VOLUME_STEP: f64 = 0.5;

/// Candles for a chart in the instrument's own currency.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionChart {
    pub candles: Vec<Candle>,
    pub currency: String,
    /// When the candles include trading outside regular hours: each run of
    /// regular trading, as the starts of its first and last candle. Empty
    /// when every candle is regular trading.
    #[serde(default)]
    pub regular: Vec<(i64, i64)>,
}

/// Runs of candles with no long gap between them, as (first start, last
/// start): the sessions a list of candle times covers.
pub fn sessions(times: &[i64]) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::new();
    for &t in times {
        match out.last_mut() {
            Some((_, end)) if t - *end <= SESSION_GAP => *end = t,
            _ => out.push((t, t)),
        }
    }
    out
}

/// Whether a candle starting at `ts` is inside one of `regular`'s runs.
/// With no runs given, everything is regular.
pub fn is_regular(regular: &[(i64, i64)], ts: i64) -> bool {
    regular.is_empty() || regular.iter().any(|(from, to)| (*from..=*to).contains(&ts))
}

/// The day cut into quarter hours: the grain trading hours are kept at.
const SLOT: i64 = 900;
const SLOTS: usize = (DAY / SLOT) as usize;

/// How a chart's candles follow the clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    /// Seconds a candle covers; a day's worth or more means one candle per
    /// calendar day, week or month rather than a fixed count of seconds.
    pub step: i64,
    /// The quarter hours of the UTC day the fetched candles trade in: a
    /// trade in any other (before the open, after the close, over lunch,
    /// overnight) isn't part of these candles. `None` when unknown, or
    /// when they trade round the clock.
    pub hours: Option<[bool; SLOTS]>,
}

const DAY: i64 = 86_400;

impl Clock {
    /// Reads the trading hours off the candles' own times.
    pub fn of(times: &[i64], step: i64) -> Self {
        let mut slots = [false; SLOTS];
        let slot = |seconds: i64| (seconds.rem_euclid(DAY) / SLOT) as usize;
        let known = if step < DAY {
            for t in times {
                // Every quarter hour the candle runs through.
                let mut at = *t;
                while at < t + step {
                    slots[slot(at)] = true;
                    at += SLOT;
                }
                slots[slot(t + step - 1)] = true;
            }
            // One day of candles may be a session still under way.
            times.first().zip(times.last()).is_some_and(|(a, b)| a.div_euclid(DAY) != b.div_euclid(DAY))
        } else {
            // Daily and longer candles are stamped with the session's
            // open: a trade earlier in the day is before it. Left alone
            // where the stamp falls late in the UTC day, on a market whose
            // session runs through midnight.
            let recent = &times[times.len().saturating_sub(10)..];
            let open = recent.iter().map(|t| t.rem_euclid(DAY)).min().filter(|o| (1..20 * 3600).contains(o));
            for mark in slots.iter_mut().skip(open.map_or(SLOTS, slot)) {
                *mark = true;
            }
            open.is_some()
        };
        // Round the clock says nothing about hours.
        let hours = (known && slots.contains(&false)).then_some(slots);
        Self { step, hours }
    }

    fn trades_at(&self, ts: i64) -> bool {
        self.hours.is_none_or(|slots| slots[(ts.rem_euclid(DAY) / SLOT) as usize])
    }

    /// Where the candle holding `ts` starts, given the last candle starts
    /// at `last`: `None` while `ts` still belongs to that candle.
    fn next_start(&self, last: i64, ts: i64) -> Option<i64> {
        if self.step < DAY {
            return (ts >= last + self.step).then(|| last + (ts - last) / self.step * self.step);
        }
        let (was, now) = (last.div_euclid(DAY), ts.div_euclid(DAY));
        let new = match self.step {
            // Weeks run Monday to Sunday (day 0 of the epoch was a Thursday).
            s if s >= 28 * DAY => month_of(now) != month_of(was),
            s if s >= 7 * DAY => (now + 3).div_euclid(7) != (was + 3).div_euclid(7),
            _ => now != was,
        };
        new.then(|| last + (now - was) * DAY)
    }
}

/// Months since 1970 of a day counted from the epoch (civil calendar).
fn month_of(day: i64) -> i64 {
    // Howard Hinnant's days-to-civil, kept to the year and month.
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    year * 12 + month
}

/// What a streamed trade did to the candles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// The last candle moved.
    Moved,
    /// A new candle started.
    Added,
}

/// The candles of a chart, oldest first, as traded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bars {
    /// When each candle starts, Unix seconds.
    pub times: Vec<i64>,
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
    pub volume: Vec<f64>,
    /// The day's volume when the last trade was applied: the next trade's
    /// volume is the difference.
    day_volume: Option<f64>,
}

impl Bars {
    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    pub fn push(&mut self, ts: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) {
        self.times.push(ts);
        self.open.push(open);
        self.high.push(high);
        self.low.push(low);
        self.close.push(close);
        self.volume.push(volume);
    }

    /// Puts a candle still forming on its step. The provider stamps it
    /// with the time of its last trade (17:03:46 on one-minute candles)
    /// rather than when it started, and the candles that follow would be
    /// counted from there. When it falls in the candle before, the two are
    /// one candle. Candles of a day or longer are stamped as they are.
    pub fn align_last(&mut self, step: i64) {
        let n = self.len();
        if n < 2 || step <= 0 || step >= DAY {
            return;
        }
        let (prev, last) = (self.times[n - 2], self.times[n - 1]);
        let start = prev + (last - prev).div_euclid(step) * step;
        if start > prev {
            self.times[n - 1] = start;
            return;
        }
        self.high[n - 2] = self.high[n - 2].max(self.high[n - 1]);
        self.low[n - 2] = self.low[n - 2].min(self.low[n - 1]);
        self.close[n - 2] = self.close[n - 1];
        self.volume[n - 2] = self.volume[n - 2].max(self.volume[n - 1]);
        for column in [&mut self.open, &mut self.high, &mut self.low, &mut self.close, &mut self.volume] {
            column.pop();
        }
        self.times.pop();
    }

    /// Moves the chart with a trade at `price`, at `ts`: the last candle's
    /// close (and its high or low), or a new candle once that one's time
    /// is up. `day_volume`, the instrument's volume so far today, adds
    /// what traded since the last trade to the candle. Nothing happens for
    /// a trade older than the last candle, outside the hours these candles
    /// trade in, or without a price.
    pub fn apply(&mut self, price: f64, ts: i64, day_volume: Option<f64>, clock: &Clock) -> Option<Applied> {
        let last = *self.times.last()?;
        if !(price > 0.0 && price.is_finite()) || ts < last || !clock.trades_at(ts) {
            return None;
        }
        // What traded since the trade before, when the count is known both
        // times. The feed sends zero when it has no count, and at times a
        // lower count than it sent before; neither is trading, and the
        // count kept is the highest seen (a new day's count starts with
        // the next fetch of the candles).
        let now = day_volume.filter(|v| *v > 0.0 && v.is_finite());
        let traded = match (self.day_volume, now) {
            (Some(before), Some(now)) if now > before && now - before <= now * MAX_VOLUME_STEP => now - before,
            _ => 0.0,
        };
        if let Some(now) = now {
            self.day_volume = Some(self.day_volume.map_or(now, |before| before.max(now)));
        }
        if let Some(start) = clock.next_start(last, ts) {
            self.push(start, price, price, price, price, traded);
            return Some(Applied::Added);
        }
        let i = self.len() - 1;
        if self.close[i] == price && traded == 0.0 {
            return None;
        }
        self.close[i] = price;
        self.high[i] = self.high[i].max(price);
        self.low[i] = self.low[i].min(price);
        self.volume[i] += traded;
        Some(Applied::Moved)
    }
}

/// The average price paid for the shares still held, by the average-cost
/// method, from `trades` in date order as (bought?, shares, price);
/// `None` when nothing is held.
pub fn average_cost(trades: impl IntoIterator<Item = (bool, f64, f64)>) -> Option<f64> {
    let (mut shares, mut cost) = (0.0_f64, 0.0_f64);
    for (bought, count, price) in trades {
        if bought {
            shares += count;
            cost += count * price;
        } else if shares > 0.0 {
            let sold = count.min(shares);
            cost -= cost / shares * sold;
            shares -= sold;
        }
    }
    (shares > 1e-9).then(|| cost / shares)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 14:30 UTC on a Thursday (2023-11-16).
    const OPEN: i64 = 1_700_145_000;

    fn minute_bars(n: i64) -> Bars {
        let mut bars = Bars::default();
        for i in 0..n {
            bars.push(OPEN + i * 60, 100.0, 101.0, 99.0, 100.0, 500.0);
        }
        bars
    }

    #[test]
    fn candles_fall_into_sessions() {
        let day = |d: i64| (0..3).map(move |i| OPEN + d * DAY + i * 60);
        let times: Vec<i64> = day(0).chain(day(1)).collect();
        assert_eq!(sessions(&times), [(OPEN, OPEN + 120), (OPEN + DAY, OPEN + DAY + 120)]);
        assert_eq!(sessions(&[]), []);
        let regular = sessions(&times);
        assert!(is_regular(&regular, OPEN + 60) && !is_regular(&regular, OPEN - 3600));
        assert!(is_regular(&[], 5), "no runs given: all of it is regular");
    }

    #[test]
    fn a_trade_moves_the_last_candle_or_starts_the_next() {
        let mut bars = minute_bars(3);
        let clock = Clock::of(&bars.times, 60);
        let last = OPEN + 120;
        // Inside the last minute: its close and high move.
        assert_eq!(bars.apply(102.0, last + 30, None, &clock), Some(Applied::Moved));
        assert_eq!((bars.close[2], bars.high[2], bars.low[2], bars.len()), (102.0, 102.0, 99.0, 3));
        assert_eq!(bars.apply(102.0, last + 40, None, &clock), None, "the same price again");
        // Two minutes on: a candle for the minute it traded in.
        assert_eq!(bars.apply(98.0, last + 150, None, &clock), Some(Applied::Added));
        assert_eq!((bars.times[3], bars.open[3], bars.close[3]), (last + 120, 98.0, 98.0));
        // Old, priceless and absurd trades change nothing.
        assert_eq!(bars.apply(50.0, OPEN, None, &clock), None);
        assert_eq!(bars.apply(0.0, last + 200, None, &clock), None);
        assert_eq!(bars.apply(f64::NAN, last + 200, None, &clock), None);
        assert_eq!(Bars::default().apply(1.0, 1, None, &clock), None, "no candles to move");
    }

    #[test]
    fn a_trade_outside_the_candles_hours_is_left_out() {
        // Two sessions of 14:30–21:00 UTC, one-minute candles.
        let mut bars = Bars::default();
        for day in 0..2 {
            for i in 0..390 {
                bars.push(OPEN + day * DAY + i * 60, 100.0, 100.0, 100.0, 100.0, 1.0);
            }
        }
        let clock = Clock::of(&bars.times, 60);
        let at = |h: i64, m: i64| clock.trades_at(OPEN - OPEN.rem_euclid(DAY) + h * 3600 + m * 60);
        assert!(at(14, 30) && at(17, 0) && at(20, 59));
        assert!(!at(14, 29) && !at(21, 0) && !at(4, 58) && !at(0, 30));
        let n = bars.len();
        // 12:00 UTC the next day is before the open: not a candle.
        assert_eq!(bars.apply(110.0, OPEN + 2 * DAY - 9000, None, &clock), None);
        assert_eq!(bars.len(), n);
        // 14:31 the next day is.
        assert_eq!(bars.apply(110.0, OPEN + 2 * DAY + 60, None, &clock), Some(Applied::Added));
        // After the close that evening: left out again.
        assert_eq!(bars.apply(120.0, OPEN + 2 * DAY + 7 * 3600, None, &clock), None);
        // Candles round the clock take any trade, and so does a first day
        // of trading, whose close isn't known yet.
        let crypto: Vec<i64> = (0..48).map(|i| OPEN + i * 3600).collect();
        assert_eq!(Clock::of(&crypto, 3600).hours, None);
        assert_eq!(Clock::of(&bars.times[..200], 60).hours, None);
    }

    #[test]
    fn hours_through_midnight_and_a_lunch_break_are_kept() {
        let midnight = OPEN - OPEN.rem_euclid(DAY);
        // Trading outside regular hours: 09:00 UTC through midnight to 01:00.
        let long_day = |d: i64| (0..192).map(move |i| midnight + d * DAY + 9 * 3600 + i * 300);
        let times: Vec<i64> = (0..2).flat_map(long_day).collect();
        let clock = Clock::of(&times, 300);
        let at = |h: i64, m: i64| clock.trades_at(midnight + h * 3600 + m * 60);
        assert!(at(9, 0) && at(23, 59) && at(0, 30), "the evening runs past midnight");
        // An overnight trade at 04:58 is nobody's candle.
        assert!(!at(4, 58) && !at(1, 0) && !at(8, 59));
        // A market shut for lunch: 03:00–05:30 and 07:00–09:30 UTC.
        let halves = |d: i64| (0..30).chain(48..78).map(move |i| midnight + d * DAY + 3 * 3600 + i * 300);
        let lunch = Clock::of(&(0..2).flat_map(halves).collect::<Vec<_>>(), 300);
        let at = |h: i64, m: i64| lunch.trades_at(midnight + h * 3600 + m * 60);
        assert!(at(3, 0) && at(5, 29) && at(7, 0) && at(9, 29));
        assert!(!at(6, 0) && !at(9, 30) && !at(2, 59));
    }

    #[test]
    fn daily_candles_follow_the_calendar() {
        let mut bars = Bars::default();
        // Wednesday and Thursday, stamped at the 14:30 open.
        bars.push(OPEN - DAY, 10.0, 11.0, 9.0, 10.0, 1e6);
        bars.push(OPEN, 10.0, 11.0, 9.0, 10.5, 1e6);
        let clock = Clock::of(&bars.times, DAY);
        assert!(clock.hours.is_some_and(|slots| !slots[57] && slots[58]), "from the 14:30 open");
        // A market stamped late in the UTC day (its session runs through
        // midnight) is left alone.
        assert_eq!(Clock::of(&[OPEN + 8 * 3600 + 1800, OPEN + DAY + 8 * 3600 + 1800], DAY).hours, None);
        assert_eq!(bars.apply(11.5, OPEN + 3600, None, &clock), Some(Applied::Moved));
        assert_eq!((bars.close[1], bars.high[1]), (11.5, 11.5));
        // Friday before the open: not Friday's candle yet.
        assert_eq!(bars.apply(12.0, OPEN + DAY - 3600, None, &clock), None);
        // Friday after it: a new candle, stamped like the others.
        assert_eq!(bars.apply(12.0, OPEN + DAY + 60, None, &clock), Some(Applied::Added));
        assert_eq!(bars.times[2], OPEN + DAY);

        // Weekly candles: Friday stays in Thursday's week, Monday starts one.
        let week = Clock { step: 7 * DAY, hours: None };
        assert_eq!(week.next_start(OPEN, OPEN + DAY), None);
        assert_eq!(week.next_start(OPEN, OPEN + 4 * DAY), Some(OPEN + 4 * DAY));
        // Monthly: the 30th of November stays, the 1st of December doesn't.
        let month = Clock { step: 30 * DAY, hours: None };
        assert_eq!(month.next_start(OPEN, OPEN + 14 * DAY), None);
        assert_eq!(month.next_start(OPEN, OPEN + 15 * DAY), Some(OPEN + 15 * DAY));
        assert_eq!(month_of(0), 1970 * 12 + 1);
        assert_eq!(month_of(19_677), 2023 * 12 + 11, "2023-11-16");
    }

    #[test]
    fn the_days_volume_becomes_each_candles() {
        let mut bars = minute_bars(1);
        let clock = Clock { step: 60, hours: None };
        // The first trade only sets the count to start from.
        bars.apply(100.5, OPEN + 10, Some(10_000.0), &clock);
        assert_eq!(bars.volume[0], 500.0);
        bars.apply(100.6, OPEN + 20, Some(10_300.0), &clock);
        assert_eq!(bars.volume[0], 800.0);
        // Volume alone moves the candle too.
        assert_eq!(bars.apply(100.6, OPEN + 30, Some(10_400.0), &clock), Some(Applied::Moved));
        // A new candle starts with what traded since.
        bars.apply(100.7, OPEN + 70, Some(10_900.0), &clock);
        assert_eq!((bars.len(), bars.volume[1]), (2, 500.0));
        // No count (the feed sends zero) and a lower count than before
        // add nothing, and don't become what the next trade is counted from.
        bars.apply(100.8, OPEN + 80, Some(0.0), &clock);
        bars.apply(100.9, OPEN + 85, Some(50.0), &clock);
        assert_eq!(bars.volume[1], 500.0);
        bars.apply(101.0, OPEN + 90, Some(11_000.0), &clock);
        assert_eq!(bars.volume[1], 600.0);
        // The whole day's volume arriving at once isn't a minute's trading.
        let mut fresh = minute_bars(1);
        fresh.apply(100.5, OPEN + 10, Some(400.0), &clock);
        fresh.apply(100.6, OPEN + 20, Some(495_000_000.0), &clock);
        assert_eq!(fresh.volume[0], 500.0);
        // ...and what trades after it is counted from there.
        fresh.apply(100.7, OPEN + 30, Some(495_000_900.0), &clock);
        assert_eq!(fresh.volume[0], 1400.0);
    }

    #[test]
    fn a_candle_still_forming_is_put_on_its_step() {
        // The provider's last one-minute candle, stamped mid-minute.
        let mut bars = minute_bars(3);
        bars.push(OPEN + 180 + 46, 100.0, 100.5, 99.5, 100.2, 40.0);
        bars.align_last(60);
        assert_eq!(bars.times[3], OPEN + 180);
        // The next trade then starts its candle on the minute.
        let clock = Clock { step: 60, hours: None };
        assert_eq!(bars.apply(100.3, OPEN + 250, None, &clock), Some(Applied::Added));
        assert_eq!(bars.times[4], OPEN + 240);

        // Stamped inside the candle before: one candle, not two.
        let mut twice = minute_bars(3);
        twice.push(OPEN + 120 + 30, 100.0, 103.0, 98.0, 102.0, 700.0);
        twice.align_last(60);
        assert_eq!(twice.len(), 3);
        assert_eq!((twice.high[2], twice.low[2], twice.close[2], twice.volume[2]), (103.0, 98.0, 102.0, 700.0));
        assert_eq!(twice.open[2], 100.0);

        // Already on its step, a day or longer, or alone: left as it is.
        let mut tidy = minute_bars(3);
        tidy.align_last(60);
        assert_eq!(tidy, minute_bars(3));
        let mut daily = minute_bars(3);
        daily.align_last(DAY);
        assert_eq!(daily, minute_bars(3));
        let mut one = minute_bars(1);
        one.align_last(60);
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn average_cost_follows_buys_and_sales() {
        // 10 at 100 and 10 at 120 average 110; selling 5 leaves it there.
        let held = average_cost([(true, 10.0, 100.0), (true, 10.0, 120.0), (false, 5.0, 150.0)]);
        assert!((held.unwrap() - 110.0).abs() < 1e-9);
        assert_eq!(average_cost([(true, 10.0, 100.0), (false, 10.0, 90.0)]), None, "sold out");
        // Bought back after selling out: only the new shares count.
        assert_eq!(average_cost([(true, 1.0, 50.0), (false, 1.0, 60.0), (true, 2.0, 80.0)]), Some(80.0));
        assert_eq!(average_cost([]), None);
    }
}
