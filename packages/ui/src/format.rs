//! Number formatting shared across pages.

use dioxus::{core::Runtime, prelude::*};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::cell::RefCell;

thread_local! {
    /// (symbol, USD → display rate). Set once settings load.
    static DISPLAY: RefCell<(&'static str, Decimal)> = const { RefCell::new(("$", Decimal::ONE)) };
}

/// Bumped on every currency change. Formatting reads it, so any component
/// (or memo) that shows an amount re-renders when the currency changes.
static CURRENCY_VERSION: GlobalSignal<u32> = Signal::global(|| 0);

/// Shows every amount in another currency: values are multiplied by
/// `rate` (units per USD) and prefixed with `symbol`.
pub fn set_display_currency(symbol: &'static str, rate: Decimal) {
    DISPLAY.with(|d| *d.borrow_mut() = (symbol, rate));
    if Runtime::try_current().is_some() {
        *CURRENCY_VERSION.write() += 1;
    }
}

/// Symbol and units-per-USD of the display currency, for charts drawn in
/// JavaScript.
pub fn display_currency() -> (&'static str, f64) {
    let (symbol, rate) = display();
    (symbol, rate.to_f64().unwrap_or(1.0))
}

fn display() -> (&'static str, Decimal) {
    // Outside an app (tests) there is nothing to subscribe.
    if Runtime::try_current().is_some() {
        CURRENCY_VERSION.read();
    }
    DISPLAY.with(|d| *d.borrow())
}

/// `$1,234.56` style, with a leading `-` for negatives.
pub fn fmt_usd(value: Decimal, decimals: u32) -> String {
    let (symbol, rate) = display();
    let value = value * rate;
    let abs = value.abs().round_dp(decimals);
    let whole = abs.trunc();
    let frac = ((abs - whole) * Decimal::from(10u64.pow(decimals)))
        .round()
        .to_string();

    let digits = whole.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }

    let sign = if value.is_sign_negative() && !abs.is_zero() {
        "-"
    } else {
        ""
    };
    if decimals == 0 {
        format!("{sign}{symbol}{grouped}")
    } else {
        format!(
            "{sign}{symbol}{grouped}.{frac:0>width$}",
            width = decimals as usize
        )
    }
}

/// Like [`fmt_usd`] but always signed: `+$12.00` / `-$3.50`.
pub fn fmt_signed(value: Decimal, decimals: u32) -> String {
    let sign = if value >= Decimal::ZERO { "+" } else { "" };
    format!("{sign}{}", fmt_usd(value, decimals))
}

/// Compact money: `$5.45T`, `$482.30B`, `$12.40M`, `$950.00`.
pub fn fmt_compact(value: f64) -> String {
    let (symbol, rate) = display();
    let value = value * rate.to_f64().unwrap_or(1.0);
    let sign = if value < 0.0 { "-" } else { "" };
    let v = value.abs();
    let (n, unit) = match v {
        v if v >= 1e12 => (v / 1e12, "T"),
        v if v >= 1e9 => (v / 1e9, "B"),
        v if v >= 1e6 => (v / 1e6, "M"),
        v if v >= 1e3 => (v / 1e3, "K"),
        v => (v, ""),
    };
    format!("{sign}{symbol}{n:.2}{unit}")
}

/// A share count people can read: at most 4 decimals, no trailing zeros,
/// thousands grouped — `1.405399` → `1.4054`, `1500` → `1,500`.
pub fn fmt_shares(shares: Decimal) -> String {
    let rounded = shares.round_dp(4).normalize();
    let text = rounded.abs().to_string();
    let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let sign = if rounded.is_sign_negative() && !rounded.is_zero() { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

/// `—` for missing values, otherwise the formatted value.
pub fn or_dash<T>(value: Option<T>, fmt: impl Fn(T) -> String) -> String {
    value.map_or_else(|| "—".to_string(), fmt)
}

/// Tailwind text colour for a gain / loss.
pub fn signed_color(value: Decimal) -> &'static str {
    if value >= Decimal::ZERO {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn formats_usd() {
        assert_eq!(fmt_usd(dec!(1234567.891), 2), "$1,234,567.89");
        assert_eq!(fmt_usd(dec!(-12.5), 2), "-$12.50");
        assert_eq!(fmt_usd(dec!(999), 0), "$999");
        assert_eq!(fmt_usd(dec!(-0.001), 2), "$0.00");
        assert_eq!(fmt_signed(dec!(3), 2), "+$3.00");
        assert_eq!(fmt_compact(5.45e12), "$5.45T");
        assert_eq!(fmt_compact(-12_400_000.0), "-$12.40M");
        assert_eq!(fmt_compact(950.0), "$950.00");
        assert_eq!(or_dash(None::<f64>, |v| v.to_string()), "—");
        assert_eq!(fmt_shares(dec!(1.405399)), "1.4054");
        assert_eq!(fmt_shares(dec!(1500)), "1,500");
        assert_eq!(fmt_shares(dec!(0.5000)), "0.5");
    }

    #[test]
    fn converts_to_display_currency() {
        set_display_currency("฿", dec!(32.5));
        assert_eq!(fmt_usd(dec!(10), 2), "฿325.00");
        assert_eq!(fmt_signed(dec!(-2), 2), "-฿65.00");
        assert_eq!(fmt_compact(2e6), "฿65.00M");
        set_display_currency("$", Decimal::ONE);
        assert_eq!(fmt_usd(dec!(10), 2), "$10.00");
    }
}
