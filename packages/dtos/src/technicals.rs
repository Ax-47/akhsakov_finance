//! Technical indicators over a price series (oldest first). Each returns
//! one value per input, `None` until there's enough history.

/// Simple moving average over `n` values.
pub fn sma(values: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if n == 0 {
        return out;
    }
    let mut sum = 0.0;
    for (i, v) in values.iter().enumerate() {
        sum += v;
        if i >= n {
            sum -= values[i - n];
        }
        if i + 1 >= n {
            out[i] = Some(sum / n as f64);
        }
    }
    out
}

/// Exponential moving average, seeded with the SMA of the first `n`.
pub fn ema(values: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if n == 0 || values.len() < n {
        return out;
    }
    let k = 2.0 / (n as f64 + 1.0);
    let mut prev = values[..n].iter().sum::<f64>() / n as f64;
    out[n - 1] = Some(prev);
    for i in n..values.len() {
        prev = values[i] * k + prev * (1.0 - k);
        out[i] = Some(prev);
    }
    out
}

/// Bollinger bands: (middle, upper, lower) at `n` periods and `k` standard
/// deviations (population).
pub fn bollinger(values: &[f64], n: usize, k: f64) -> Vec<Option<(f64, f64, f64)>> {
    sma(values, n)
        .into_iter()
        .enumerate()
        .map(|(i, mid)| {
            let mid = mid?;
            let window = &values[i + 1 - n..=i];
            let var = window.iter().map(|v| (v - mid).powi(2)).sum::<f64>() / n as f64;
            let sd = var.sqrt();
            Some((mid, mid + k * sd, mid - k * sd))
        })
        .collect()
}

/// Relative strength index with Wilder's smoothing, 0–100.
pub fn rsi(values: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if n == 0 || values.len() <= n {
        return out;
    }
    let (mut gain, mut loss) = (0.0, 0.0);
    for i in 1..=n {
        let d = values[i] - values[i - 1];
        if d > 0.0 {
            gain += d;
        } else {
            loss -= d;
        }
    }
    gain /= n as f64;
    loss /= n as f64;
    let value = |gain: f64, loss: f64| {
        if loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + gain / loss)
        }
    };
    out[n] = Some(value(gain, loss));
    for i in n + 1..values.len() {
        let d = values[i] - values[i - 1];
        gain = (gain * (n as f64 - 1.0) + d.max(0.0)) / n as f64;
        loss = (loss * (n as f64 - 1.0) + (-d).max(0.0)) / n as f64;
        out[i] = Some(value(gain, loss));
    }
    out
}

/// MACD line (fast EMA − slow EMA), its signal EMA, and the histogram.
pub struct Macd {
    pub macd: Vec<Option<f64>>,
    pub signal: Vec<Option<f64>>,
    pub histogram: Vec<Option<f64>>,
}

pub fn macd(values: &[f64], fast: usize, slow: usize, signal: usize) -> Macd {
    let (f, s) = (ema(values, fast), ema(values, slow));
    let line: Vec<Option<f64>> = f
        .iter()
        .zip(&s)
        .map(|(a, b)| Some((*a)? - (*b)?))
        .collect();
    // Signal is an EMA of the defined part of the MACD line.
    let start = line.iter().position(Option::is_some).unwrap_or(line.len());
    let defined: Vec<f64> = line[start..].iter().map(|v| v.unwrap_or(0.0)).collect();
    let mut sig = vec![None; start];
    sig.extend(ema(&defined, signal));
    let histogram = line
        .iter()
        .zip(&sig)
        .map(|(m, s)| Some((*m)? - (*s)?))
        .collect();
    Macd {
        macd: line,
        signal: sig,
        histogram,
    }
}

/// Average true range with Wilder's smoothing: how far price typically
/// moves in one candle, gaps included.
pub fn atr(high: &[f64], low: &[f64], close: &[f64], n: usize) -> Vec<Option<f64>> {
    let len = close.len().min(high.len()).min(low.len());
    let mut out = vec![None; close.len()];
    if n == 0 || len <= n {
        return out;
    }
    let tr = |i: usize| {
        let range = high[i] - low[i];
        if i == 0 {
            return range;
        }
        range.max((high[i] - close[i - 1]).abs()).max((low[i] - close[i - 1]).abs())
    };
    let mut prev = (1..=n).map(tr).sum::<f64>() / n as f64;
    out[n] = Some(prev);
    for (i, slot) in out.iter_mut().enumerate().take(len).skip(n + 1) {
        prev = (prev * (n as f64 - 1.0) + tr(i)) / n as f64;
        *slot = Some(prev);
    }
    out
}

/// Stochastic oscillator: %K, where the close sits in the last `k`
/// candles' range (0–100), and %D, its `d`-candle average.
pub fn stochastic(high: &[f64], low: &[f64], close: &[f64], k: usize, d: usize) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
    let len = close.len().min(high.len()).min(low.len());
    let mut pct_k = vec![None; close.len()];
    if k == 0 {
        return (pct_k.clone(), pct_k);
    }
    for i in k.saturating_sub(1)..len {
        let hi = high[i + 1 - k..=i].iter().cloned().fold(f64::MIN, f64::max);
        let lo = low[i + 1 - k..=i].iter().cloned().fold(f64::MAX, f64::min);
        pct_k[i] = Some(if hi > lo { (close[i] - lo) / (hi - lo) * 100.0 } else { 50.0 });
    }
    let start = pct_k.iter().position(Option::is_some).unwrap_or(pct_k.len());
    let defined: Vec<f64> = pct_k[start..].iter().map(|v| v.unwrap_or(0.0)).collect();
    let mut pct_d = vec![None; start];
    pct_d.extend(sma(&defined, d));
    (pct_k, pct_d)
}

/// On-balance volume: volume added on up candles and taken away on down
/// ones, starting from zero.
pub fn obv(close: &[f64], volume: &[f64]) -> Vec<Option<f64>> {
    let mut total = 0.0;
    close
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i > 0 {
                let v = volume.get(i).copied().unwrap_or(0.0);
                if *c > close[i - 1] {
                    total += v;
                } else if *c < close[i - 1] {
                    total -= v;
                }
            }
            Some(total)
        })
        .collect()
}

/// Volume-weighted average price of each session so far: the typical
/// price ((high + low + close) / 3) weighted by volume, starting again
/// whenever `session` changes. `None` until a session has traded volume.
pub fn vwap(high: &[f64], low: &[f64], close: &[f64], volume: &[f64], session: &[i64]) -> Vec<Option<f64>> {
    let (mut value, mut traded, mut current) = (0.0, 0.0, None);
    (0..close.len())
        .map(|i| {
            let s = session.get(i).copied();
            if s != current {
                (value, traded, current) = (0.0, 0.0, s);
            }
            let v = volume.get(i).copied().unwrap_or(0.0);
            let typical = (high.get(i)? + low.get(i)? + close[i]) / 3.0;
            value += typical * v;
            traded += v;
            (traded > 0.0).then(|| value / traded)
        })
        .collect()
}

/// Donchian channel: the highest high and lowest low of the last `n`
/// candles, as (upper, lower).
pub fn donchian(high: &[f64], low: &[f64], n: usize) -> Vec<Option<(f64, f64)>> {
    let len = high.len().min(low.len());
    let mut out = vec![None; high.len()];
    if n == 0 {
        return out;
    }
    for (i, slot) in out.iter_mut().enumerate().take(len).skip(n - 1) {
        let top = high[i + 1 - n..=i].iter().cloned().fold(f64::MIN, f64::max);
        let bottom = low[i + 1 - n..=i].iter().cloned().fold(f64::MAX, f64::min);
        *slot = Some((top, bottom));
    }
    out
}

/// Supertrend: a stop trailing the price `multiplier` ATRs (over `n`
/// candles) from the middle of each candle's range. It sits under the price
/// while the trend is up and over it while the trend is down, and only
/// moves the trend's way until a close crosses it. Each value comes with
/// whether the trend is up.
pub fn supertrend(high: &[f64], low: &[f64], close: &[f64], n: usize, multiplier: f64) -> Vec<Option<(f64, bool)>> {
    let len = close.len().min(high.len()).min(low.len());
    let ranges = atr(high, low, close, n);
    let mut out = vec![None; close.len()];
    // The bands as they stood on the last candle, and the trend then.
    let mut last: Option<(f64, f64, bool)> = None;
    for i in 0..len {
        let Some(range) = ranges[i] else {
            continue;
        };
        let middle = (high[i] + low[i]) / 2.0;
        let (mut upper, mut lower) = (middle + multiplier * range, middle - multiplier * range);
        let up = match last {
            None => close[i] >= middle,
            Some((last_upper, last_lower, was_up)) => {
                // A band only tightens, unless the last close broke it.
                if upper > last_upper && close[i - 1] <= last_upper {
                    upper = last_upper;
                }
                if lower < last_lower && close[i - 1] >= last_lower {
                    lower = last_lower;
                }
                if was_up {
                    close[i] >= lower
                } else {
                    close[i] > upper
                }
            }
        };
        last = Some((upper, lower, up));
        out[i] = Some((if up { lower } else { upper }, up));
    }
    out
}

/// Parabolic SAR (Wilder): a stop that starts at the last extreme and
/// closes in on the price by `step` of the gap each candle, faster (up to
/// `max`) every time the trend makes a new extreme; it flips to the other
/// side when the price touches it. Each value comes with whether the trend
/// is up (the stop is under the price).
pub fn parabolic_sar(high: &[f64], low: &[f64], step: f64, max: f64) -> Vec<Option<(f64, bool)>> {
    let len = high.len().min(low.len());
    let mut out = vec![None; high.len()];
    if len < 2 || step <= 0.0 {
        return out;
    }
    let mut up = high[1] + low[1] >= high[0] + low[0];
    let mut sar = if up { low[0] } else { high[0] };
    let mut extreme = if up { high[0].max(high[1]) } else { low[0].min(low[1]) };
    let mut speed = step;
    for (i, slot) in out.iter_mut().enumerate().take(len).skip(1) {
        sar += speed * (extreme - sar);
        if up {
            // Never above the last two lows.
            sar = sar.min(low[i - 1]).min(low[i.saturating_sub(2)]);
            if low[i] < sar {
                (up, sar, extreme, speed) = (false, extreme, low[i], step);
            } else if high[i] > extreme {
                (extreme, speed) = (high[i], (speed + step).min(max));
            }
        } else {
            sar = sar.max(high[i - 1]).max(high[i.saturating_sub(2)]);
            if high[i] > sar {
                (up, sar, extreme, speed) = (true, extreme, high[i], step);
            } else if low[i] < extreme {
                (extreme, speed) = (low[i], (speed + step).min(max));
            }
        }
        *slot = Some((sar, up));
    }
    out
}

/// Money flow index: RSI's idea with volume in it. Each candle's typical
/// price times its volume counts as money flowing in when the typical
/// price rose and out when it fell; the index is the share flowing in over
/// the last `n` candles, 0–100.
pub fn mfi(high: &[f64], low: &[f64], close: &[f64], volume: &[f64], n: usize) -> Vec<Option<f64>> {
    let len = close.len().min(high.len()).min(low.len()).min(volume.len());
    let mut out = vec![None; close.len()];
    if n == 0 || len <= n {
        return out;
    }
    let typical = |i: usize| (high[i] + low[i] + close[i]) / 3.0;
    // Signed flow per candle from the second.
    let flow = |i: usize| {
        let (now, before) = (typical(i), typical(i - 1));
        let money = now * volume[i];
        if now > before {
            (money, 0.0)
        } else if now < before {
            (0.0, money)
        } else {
            (0.0, 0.0)
        }
    };
    let (mut inflow, mut outflow) = (0.0, 0.0);
    for (i, slot) in out.iter_mut().enumerate().take(len).skip(1) {
        let (p, m) = flow(i);
        inflow += p;
        outflow += m;
        if i > n {
            let (p, m) = flow(i - n);
            inflow -= p;
            outflow -= m;
        }
        if i >= n {
            let total = inflow + outflow;
            *slot = Some(if total > 0.0 { (inflow / total * 100.0).clamp(0.0, 100.0) } else { 50.0 });
        }
    }
    out
}

/// Heikin-Ashi candles, as (open, high, low, close): each close is the
/// average of the real candle's four prices and each open the middle of
/// the candle before, which smooths a trend into runs of one colour.
pub fn heikin_ashi(open: &[f64], high: &[f64], low: &[f64], close: &[f64]) -> Vec<(f64, f64, f64, f64)> {
    let len = open.len().min(high.len()).min(low.len()).min(close.len());
    let mut out: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(len);
    for i in 0..len {
        let c = (open[i] + high[i] + low[i] + close[i]) / 4.0;
        let o = match out.last() {
            Some(prev) => (prev.0 + prev.3) / 2.0,
            None => (open[i] + close[i]) / 2.0,
        };
        out.push((o, high[i].max(o).max(c), low[i].min(o).min(c), c));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atr_counts_gaps() {
        // Ranges of 2, then a gap up from 10 to a 13–14 candle: its true
        // range is 4, not 1.
        let (h, l, c) = ([11.0, 11.0, 11.0, 14.0], [9.0, 9.0, 9.0, 13.0], [10.0, 10.0, 10.0, 13.5]);
        let a = atr(&h, &l, &c, 2);
        assert_eq!(a[..2], [None, None]);
        assert_eq!(a[2], Some(2.0));
        assert_eq!(a[3], Some(3.0));
    }

    #[test]
    fn stochastic_places_the_close_in_its_range() {
        let h = [10.0, 12.0, 14.0, 14.0];
        let l = [8.0, 8.0, 9.0, 10.0];
        let c = [9.0, 11.0, 14.0, 11.0];
        let (k, d) = stochastic(&h, &l, &c, 3, 2);
        assert_eq!(k[..2], [None, None]);
        assert_eq!(k[2], Some(100.0));
        // Range 8–14, close 11: halfway.
        assert_eq!(k[3], Some(50.0));
        assert_eq!(d[2], None);
        assert_eq!(d[3], Some(75.0));
    }

    #[test]
    fn obv_and_vwap_follow_volume() {
        assert_eq!(obv(&[10.0, 11.0, 10.5, 10.5], &[5.0, 3.0, 2.0, 9.0]), vec![Some(0.0), Some(3.0), Some(1.0), Some(1.0)]);
        // Two sessions: the second starts over.
        let v = vwap(&[11.0, 13.0, 21.0], &[9.0, 11.0, 19.0], &[10.0, 12.0, 20.0], &[1.0, 3.0, 2.0], &[1, 1, 2]);
        assert_eq!(v[0], Some(10.0));
        assert_eq!(v[1], Some(11.5));
        assert_eq!(v[2], Some(20.0));
        assert_eq!(vwap(&[1.0], &[1.0], &[1.0], &[0.0], &[1]), vec![None]);
    }

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-6)
    }

    #[test]
    fn moving_averages() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(sma(&v, 3), vec![None, None, Some(2.0), Some(3.0), Some(4.0)]);
        let e = ema(&v, 3);
        assert!(close(e[2], 2.0));
        assert!(close(e[3], 3.0)); // 4·0.5 + 2·0.5
        assert!(close(e[4], 4.0));
        assert_eq!(ema(&v, 9), vec![None; 5]);
    }

    #[test]
    fn bollinger_bands_widen_with_volatility() {
        let flat = bollinger(&[5.0; 4], 2, 2.0);
        assert_eq!(flat[3], Some((5.0, 5.0, 5.0)));
        let b = bollinger(&[1.0, 3.0], 2, 2.0)[1].unwrap();
        assert_eq!(b, (2.0, 4.0, 0.0)); // sd 1
    }

    #[test]
    fn rsi_is_bounded_and_extreme_for_one_way_moves() {
        let up: Vec<f64> = (0..20).map(f64::from).collect();
        assert!(close(rsi(&up, 14)[19], 100.0));
        let down: Vec<f64> = (0..20).map(|i| 100.0 - i as f64).collect();
        assert!(close(rsi(&down, 14)[19], 0.0));
        let zigzag: Vec<f64> = (0..40).map(|i| if i % 2 == 0 { 10.0 } else { 11.0 }).collect();
        let r = rsi(&zigzag, 14)[39].unwrap();
        assert!((40.0..60.0).contains(&r), "{r}");
        assert_eq!(rsi(&up, 14)[13], None);
    }

    #[test]
    fn macd_histogram_is_line_minus_signal() {
        let v: Vec<f64> = (0..60).map(|i| (i as f64 / 5.0).sin() * 10.0 + 50.0).collect();
        let m = macd(&v, 12, 26, 9);
        assert_eq!(m.macd.iter().position(Option::is_some), Some(25));
        assert_eq!(m.signal.iter().position(Option::is_some), Some(33));
        for i in 33..60 {
            assert!(close(m.histogram[i], m.macd[i].unwrap() - m.signal[i].unwrap()));
        }
    }

    #[test]
    fn donchian_tracks_the_last_highs_and_lows() {
        let (h, l) = ([5.0, 7.0, 6.0, 4.0], [3.0, 5.0, 4.0, 2.0]);
        assert_eq!(donchian(&h, &l, 2), [None, Some((7.0, 3.0)), Some((7.0, 4.0)), Some((6.0, 2.0))]);
        assert!(donchian(&h, &l, 0).iter().all(Option::is_none));
    }

    #[test]
    fn supertrend_trails_under_a_rise_and_flips_on_a_break() {
        // Ten candles climbing a point a day, range 2, then a collapse.
        let mut c: Vec<f64> = (0..10).map(|i| 100.0 + i as f64).collect();
        c.extend([95.0, 90.0]);
        let h: Vec<f64> = c.iter().map(|v| v + 1.0).collect();
        let l: Vec<f64> = c.iter().map(|v| v - 1.0).collect();
        let st = supertrend(&h, &l, &c, 3, 2.0);
        assert!(st[..3].iter().all(Option::is_none), "needs an ATR first");
        let rising: Vec<(f64, bool)> = st[3..10].iter().map(|v| v.unwrap()).collect();
        assert!(rising.iter().all(|(_, up)| *up));
        // Under the price, and it never steps down while the trend holds.
        assert!(rising.iter().zip(&c[3..10]).all(|((stop, _), close)| stop < close));
        assert!(rising.windows(2).all(|w| w[1].0 >= w[0].0));
        // The collapse closes under the stop: it jumps over the price.
        let (stop, up) = st[11].unwrap();
        assert!(!up && stop > c[11]);
    }

    #[test]
    fn parabolic_sar_closes_in_and_flips_when_touched() {
        let h: Vec<f64> = (0..8).map(|i| 11.0 + i as f64).chain([12.0]).collect();
        let l: Vec<f64> = (0..8).map(|i| 9.0 + i as f64).chain([8.0]).collect();
        let sar = parabolic_sar(&h, &l, 0.02, 0.2);
        assert_eq!(sar[0], None);
        let rising: Vec<(f64, bool)> = sar[1..8].iter().map(|v| v.unwrap()).collect();
        assert!(rising.iter().all(|(_, up)| *up));
        assert!(rising.windows(2).all(|w| w[1].0 >= w[0].0), "it only moves the trend's way");
        assert!(rising[6].0 > rising[0].0 + 1.0, "and closes in: {rising:?}");
        assert!(rising.iter().zip(&l[1..8]).all(|((stop, _), low)| stop <= low));
        // The last candle falls through it: the stop goes to the high made.
        assert_eq!(sar[8], Some((18.0, false)));
        assert!(parabolic_sar(&h[..1], &l[..1], 0.02, 0.2).iter().all(Option::is_none));
    }

    #[test]
    fn mfi_is_the_share_of_money_flowing_in() {
        let v = [100.0; 5];
        // Typical price rises every candle: all of it flows in.
        let up = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(mfi(&up, &up, &up, &v, 3)[2..], [None, Some(100.0), Some(100.0)]);
        let down = [5.0, 4.0, 3.0, 2.0, 1.0];
        assert_eq!(mfi(&down, &down, &down, &v, 3)[4], Some(0.0));
        // Up 10 then down 10 at the same volume, from the same price: an
        // even split, weighted a little to the higher price.
        let mixed = [10.0, 20.0, 10.0];
        let m = mfi(&mixed, &mixed, &mixed, &[1.0; 3], 2)[2].unwrap();
        assert!((m - 2000.0 / 30.0).abs() < 1e-9, "{m}");
        // No volume at all says nothing either way.
        assert_eq!(mfi(&up, &up, &up, &[0.0; 5], 3)[4], Some(50.0));
    }

    #[test]
    fn heikin_ashi_averages_each_candle_and_opens_mid_way() {
        let ha = heikin_ashi(&[10.0, 12.0], &[14.0, 13.0], &[9.0, 11.0], &[13.0, 12.0]);
        // (10 + 14 + 9 + 13) / 4 = 11.5, opening half-way between 10 and 13.
        assert_eq!(ha[0], (11.5, 14.0, 9.0, 11.5));
        // Opens at the middle of the candle before: (11.5 + 11.5) / 2.
        assert_eq!(ha[1], (11.5, 13.0, 11.0, 12.0));
    }
}
