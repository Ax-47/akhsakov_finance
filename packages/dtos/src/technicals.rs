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

#[cfg(test)]
mod tests {
    use super::*;

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
}
