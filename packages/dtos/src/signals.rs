//! What the usual indicators say about a price series right now: each
//! moving average and oscillator read as buy, neutral or sell by its
//! textbook rule, and the tally. A summary of indicators, not advice: in
//! a strong trend every average agrees and every oscillator says
//! "overbought" for months.

use crate::technicals::{ema, macd, rsi, sma, stochastic};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Buy,
    Neutral,
    Sell,
}

/// One indicator's latest value and what its rule makes of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// `SMA 50`, `RSI (14)`, …
    pub name: String,
    pub value: f64,
    pub action: Action,
}

/// The tally of several readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    StrongBuy,
    Buy,
    Neutral,
    Sell,
    StrongSell,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::StrongBuy => "Strong buy",
            Self::Buy => "Buy",
            Self::Neutral => "Neutral",
            Self::Sell => "Sell",
            Self::StrongSell => "Strong sell",
        }
    }
}

/// (buys, neutrals, sells) among `readings`.
pub fn counts(readings: &[Reading]) -> (usize, usize, usize) {
    let of = |a: Action| readings.iter().filter(|r| r.action == a).count();
    (of(Action::Buy), of(Action::Neutral), of(Action::Sell))
}

/// Buys minus sells as a share of all readings: −1 (all sell) to +1 (all
/// buy). `None` without readings.
pub fn score(readings: &[Reading]) -> Option<f64> {
    let (buy, _, sell) = counts(readings);
    (!readings.is_empty()).then(|| (buy as f64 - sell as f64) / readings.len() as f64)
}

/// Past ±0.5 is "strong"; within ±0.1 is neutral.
pub fn verdict(score: f64) -> Verdict {
    match score {
        s if s > 0.5 => Verdict::StrongBuy,
        s if s > 0.1 => Verdict::Buy,
        s if s < -0.5 => Verdict::StrongSell,
        s if s < -0.1 => Verdict::Sell,
        _ => Verdict::Neutral,
    }
}

/// The readings for one stock, from daily candles (oldest first).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TechnicalRating {
    /// Price against its simple and exponential averages: above is a buy.
    pub averages: Vec<Reading>,
    pub oscillators: Vec<Reading>,
}

impl TechnicalRating {
    /// `None` with fewer than 30 candles. Averages that need more history
    /// than there is are left out.
    pub fn compute(high: &[f64], low: &[f64], close: &[f64]) -> Option<Self> {
        let n = close.len().min(high.len()).min(low.len());
        if n < 30 {
            return None;
        }
        let (high, low, close) = (&high[..n], &low[..n], &close[..n]);
        let price = close[n - 1];
        let last = |v: Vec<Option<f64>>| v.last().copied().flatten();

        let mut averages = Vec::new();
        for period in [10, 20, 50, 100, 200] {
            for (kind, value) in [("SMA", last(sma(close, period))), ("EMA", last(ema(close, period)))] {
                if let Some(value) = value {
                    let action = match price {
                        p if p > value => Action::Buy,
                        p if p < value => Action::Sell,
                        _ => Action::Neutral,
                    };
                    averages.push(Reading { name: format!("{kind} {period}"), value, action });
                }
            }
        }

        let mut oscillators = Vec::new();
        let mut read = |name: &str, value: Option<f64>, rule: &dyn Fn(f64) -> Action| {
            if let Some(value) = value.filter(|v| v.is_finite()) {
                oscillators.push(Reading { name: name.to_string(), value, action: rule(value) });
            }
        };
        // Stretched too far one way is expected to come back.
        let band = |low: f64, high: f64| {
            move |v: f64| match v {
                v if v < low => Action::Buy,
                v if v > high => Action::Sell,
                _ => Action::Neutral,
            }
        };
        // Above or below zero, beyond rounding noise.
        let noise = price.abs() * 1e-9;
        let sign = move |v: f64| match v {
            v if v > noise => Action::Buy,
            v if v < -noise => Action::Sell,
            _ => Action::Neutral,
        };
        read("RSI (14)", last(rsi(close, 14)), &band(30.0, 70.0));
        read("Stochastic %K (14, 3)", last(stochastic(high, low, close, 14, 3).0), &band(20.0, 80.0));
        read("CCI (20)", last(cci(high, low, close, 20)), &band(-100.0, 100.0));
        read("Williams %R (14)", last(williams_r(high, low, close, 14)), &band(-80.0, -20.0));
        let m = macd(close, 12, 26, 9);
        read("MACD (12, 26, 9)", last(m.histogram), &sign);
        read("Momentum (10)", last(momentum(close, 10)), &sign);
        // A trend counts only when it's strong enough (ADX over 20); its
        // direction is whichever of +DI and −DI leads.
        let trend = adx(high, low, close, 14);
        if let (Some(strength), Some(up), Some(down)) = (last(trend.adx), last(trend.plus_di), last(trend.minus_di)) {
            let action = match (strength > 20.0, up > down) {
                (false, _) => Action::Neutral,
                (true, true) => Action::Buy,
                (true, false) => Action::Sell,
            };
            oscillators.push(Reading { name: "ADX (14)".into(), value: strength, action });
        }
        Some(Self { averages, oscillators })
    }

    /// Averages and oscillators together.
    pub fn all(&self) -> Vec<Reading> {
        self.averages.iter().chain(&self.oscillators).cloned().collect()
    }
}

/// Commodity channel index: how far the typical price is from its
/// `n`-candle average, in units of its typical deviation (±100 is far).
pub fn cci(high: &[f64], low: &[f64], close: &[f64], n: usize) -> Vec<Option<f64>> {
    let len = close.len().min(high.len()).min(low.len());
    let typical: Vec<f64> = (0..len).map(|i| (high[i] + low[i] + close[i]) / 3.0).collect();
    let mut out = vec![None; close.len()];
    for (i, mean) in sma(&typical, n).into_iter().enumerate() {
        let Some(mean) = mean else { continue };
        let deviation = typical[i + 1 - n..=i].iter().map(|t| (t - mean).abs()).sum::<f64>() / n as f64;
        out[i] = Some(if deviation > 0.0 { (typical[i] - mean) / (0.015 * deviation) } else { 0.0 });
    }
    out
}

/// Williams %R: where the close sits in the last `n` candles' range, from
/// 0 (at the high) to −100 (at the low).
pub fn williams_r(high: &[f64], low: &[f64], close: &[f64], n: usize) -> Vec<Option<f64>> {
    let len = close.len().min(high.len()).min(low.len());
    let mut out = vec![None; close.len()];
    if n == 0 {
        return out;
    }
    for i in n - 1..len {
        let hi = high[i + 1 - n..=i].iter().cloned().fold(f64::MIN, f64::max);
        let lo = low[i + 1 - n..=i].iter().cloned().fold(f64::MAX, f64::min);
        out[i] = Some(if hi > lo { (hi - close[i]) / (hi - lo) * -100.0 } else { -50.0 });
    }
    out
}

/// Change in price over `n` candles.
pub fn momentum(close: &[f64], n: usize) -> Vec<Option<f64>> {
    (0..close.len()).map(|i| (i >= n).then(|| close[i] - close[i - n])).collect()
}

/// Average directional index and the two directional lines it's built
/// from.
pub struct Adx {
    /// Trend strength, 0–100, whichever way it points.
    pub adx: Vec<Option<f64>>,
    pub plus_di: Vec<Option<f64>>,
    pub minus_di: Vec<Option<f64>>,
}

/// Wilder's ADX over `n` candles.
pub fn adx(high: &[f64], low: &[f64], close: &[f64], n: usize) -> Adx {
    let len = close.len().min(high.len()).min(low.len());
    let empty = || vec![None; close.len()];
    let mut out = Adx { adx: empty(), plus_di: empty(), minus_di: empty() };
    if n == 0 || len <= 2 * n {
        return out;
    }
    // Per candle from the second: true range, upward and downward movement.
    let step = |i: usize| {
        let (up, down) = (high[i] - high[i - 1], low[i - 1] - low[i]);
        let range = (high[i] - low[i]).max((high[i] - close[i - 1]).abs()).max((low[i] - close[i - 1]).abs());
        (range, if up > down && up > 0.0 { up } else { 0.0 }, if down > up && down > 0.0 { down } else { 0.0 })
    };
    let (mut range, mut plus, mut minus) = (1..=n).map(step).fold((0.0, 0.0, 0.0), |a, s| (a.0 + s.0, a.1 + s.1, a.2 + s.2));
    let mut dx = Vec::with_capacity(len);
    for i in n..len {
        if i > n {
            let (r, p, m) = step(i);
            range = range - range / n as f64 + r;
            plus = plus - plus / n as f64 + p;
            minus = minus - minus / n as f64 + m;
        }
        let (up, down) = if range > 0.0 { (plus / range * 100.0, minus / range * 100.0) } else { (0.0, 0.0) };
        out.plus_di[i] = Some(up);
        out.minus_di[i] = Some(down);
        dx.push(if up + down > 0.0 { (up - down).abs() / (up + down) * 100.0 } else { 0.0 });
    }
    // ADX: Wilder's average of DX, seeded with the mean of its first `n`.
    let mut average = dx[..n].iter().sum::<f64>() / n as f64;
    out.adx[2 * n - 1] = Some(average);
    for (k, value) in dx.iter().enumerate().skip(n) {
        average = (average * (n as f64 - 1.0) + value) / n as f64;
        out.adx[n + k] = Some(average);
    }
    out
}

/// Classic floor-trader pivots from one session's high, low and close:
/// the levels day traders watch the next session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pivots {
    pub pivot: f64,
    /// Resistance 1 to 3, rising.
    pub resistance: [f64; 3],
    /// Support 1 to 3, falling.
    pub support: [f64; 3],
}

pub fn pivots(high: f64, low: f64, close: f64) -> Pivots {
    let pivot = (high + low + close) / 3.0;
    let range = high - low;
    Pivots {
        pivot,
        resistance: [2.0 * pivot - low, pivot + range, high + 2.0 * (pivot - low)],
        support: [2.0 * pivot - high, pivot - range, low - 2.0 * (high - pivot)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` candles climbing (or falling) by `step` a day, each a point wide.
    fn trend(n: usize, step: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let close: Vec<f64> = (0..n).map(|i| 500.0 + step * i as f64).collect();
        (close.iter().map(|c| c + 0.5).collect(), close.iter().map(|c| c - 0.5).collect(), close)
    }

    #[test]
    fn a_steady_climb_is_a_buy_on_trend_and_overbought_on_oscillators() {
        let (high, low, close) = trend(260, 1.0);
        let rating = TechnicalRating::compute(&high, &low, &close).unwrap();
        assert_eq!(rating.averages.len(), 10, "five simple, five exponential");
        assert!(rating.averages.iter().all(|r| r.action == Action::Buy), "price is above every average");
        assert_eq!(verdict(score(&rating.averages).unwrap()), Verdict::StrongBuy);

        let action = |name: &str| rating.oscillators.iter().find(|r| r.name.starts_with(name)).map(|r| r.action);
        assert_eq!(action("RSI"), Some(Action::Sell), "RSI is pinned at 100");
        assert_eq!(action("Stochastic"), Some(Action::Sell));
        assert_eq!(action("Williams"), Some(Action::Sell));
        assert_eq!(action("Momentum"), Some(Action::Buy));
        assert_eq!(action("ADX"), Some(Action::Buy), "a strong trend, pointing up");
        assert_eq!(rating.oscillators.len(), 7);
        assert_eq!(rating.all().len(), 17);
    }

    #[test]
    fn a_steady_fall_is_the_mirror_image() {
        let (high, low, close) = trend(260, -1.0);
        let rating = TechnicalRating::compute(&high, &low, &close).unwrap();
        assert!(rating.averages.iter().all(|r| r.action == Action::Sell));
        assert_eq!(verdict(score(&rating.averages).unwrap()), Verdict::StrongSell);
        let action = |name: &str| rating.oscillators.iter().find(|r| r.name.starts_with(name)).map(|r| r.action);
        assert_eq!(action("RSI"), Some(Action::Buy), "oversold");
        assert_eq!(action("ADX"), Some(Action::Sell), "a strong trend, pointing down");
        assert_eq!(action("MACD"), Some(Action::Neutral), "a straight line: MACD equals its signal");
    }

    #[test]
    fn short_histories_leave_out_long_averages() {
        let (high, low, close) = trend(60, 1.0);
        let rating = TechnicalRating::compute(&high, &low, &close).unwrap();
        let names: Vec<&str> = rating.averages.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["SMA 10", "EMA 10", "SMA 20", "EMA 20", "SMA 50", "EMA 50"]);
        assert!(TechnicalRating::compute(&high[..29], &low[..29], &close[..29]).is_none());
    }

    #[test]
    fn tallies_and_verdicts() {
        let reading = |action| Reading { name: String::new(), value: 0.0, action };
        let mixed = [reading(Action::Buy), reading(Action::Buy), reading(Action::Neutral), reading(Action::Sell)];
        assert_eq!(counts(&mixed), (2, 1, 1));
        assert_eq!(score(&mixed), Some(0.25));
        assert_eq!(score(&[]), None);
        assert_eq!(
            [0.6, 0.25, 0.05, -0.1, -0.3, -0.9].map(verdict),
            [Verdict::StrongBuy, Verdict::Buy, Verdict::Neutral, Verdict::Neutral, Verdict::Sell, Verdict::StrongSell]
        );
    }

    #[test]
    fn williams_r_and_cci_read_the_range() {
        let high = [10.0, 11.0, 12.0, 13.0];
        let low = [8.0, 9.0, 10.0, 11.0];
        let close = [9.0, 10.0, 11.0, 13.0];
        assert_eq!(williams_r(&high, &low, &close, 3), [None, None, Some(-25.0), Some(0.0)], "at the high: 0");
        // Four quiet days, then a jump: (20 − 12) / (0.015 × 3.2).
        let jump = [10.0, 10.0, 10.0, 10.0, 20.0];
        let c = cci(&jump, &jump, &jump, 5);
        assert_eq!(&c[..4], [None, None, None, None]);
        assert!((c[4].unwrap() - 166.666_666_7).abs() < 1e-6, "{c:?}");
        assert_eq!(cci(&[5.0; 4], &[5.0; 4], &[5.0; 4], 3)[3], Some(0.0), "a flat price has no deviation");
    }

    #[test]
    fn momentum_is_the_change_over_n_candles() {
        assert_eq!(momentum(&[1.0, 2.0, 4.0, 8.0], 2), [None, None, Some(3.0), Some(6.0)]);
    }

    #[test]
    fn adx_measures_strength_not_direction() {
        let (high, low, close) = trend(80, 1.0);
        let up = adx(&high, &low, &close, 14);
        let (h2, l2, c2) = trend(80, -1.0);
        let down = adx(&h2, &l2, &c2, 14);
        let last = |v: &[Option<f64>]| v.last().copied().flatten().unwrap();
        assert!(last(&up.adx) > 90.0 && last(&down.adx) > 90.0, "both are strong trends");
        assert!(last(&up.plus_di) > last(&up.minus_di));
        assert!(last(&down.minus_di) > last(&down.plus_di));
        assert_eq!(up.adx[26], None, "needs two full periods");
        assert!(up.adx[27].is_some());
        assert!(adx(&high[..28], &low[..28], &close[..28], 14).adx.iter().all(Option::is_none));
    }

    #[test]
    fn pivots_come_from_one_session() {
        let p = pivots(110.0, 100.0, 105.0);
        assert_eq!(p.pivot, 105.0);
        assert_eq!(p.resistance, [110.0, 115.0, 120.0]);
        assert_eq!(p.support, [100.0, 95.0, 90.0]);
    }
}
