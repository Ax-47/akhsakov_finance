//! Number formatting shared across pages.

use dioxus::{core::Runtime, prelude::*};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::cell::{Cell, RefCell};

thread_local! {
    /// (symbol, USD → display rate). Set once settings load.
    static DISPLAY: RefCell<(&'static str, Decimal)> = const { RefCell::new(("$", Decimal::ONE)) };
    /// Minutes the viewer's clock is ahead of UTC, once the webview says.
    static UTC_OFFSET: Cell<i32> = const { Cell::new(0) };
}

/// Bumped when the viewer's time zone becomes known, so times already on
/// screen are redrawn in it.
static ZONE_VERSION: GlobalSignal<u32> = Signal::global(|| 0);

/// Learns the viewer's time zone after the first render (the server renders
/// in UTC, so hydration still matches).
pub fn use_time_zone_init() {
    use_effect(|| {
        spawn(async {
            if let Ok(minutes) = document::eval("return -new Date().getTimezoneOffset();").join::<i32>().await {
                UTC_OFFSET.with(|o| o.set(minutes));
                *ZONE_VERSION.write() += 1;
            }
        });
    });
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// A stored time (SQLite's `YYYY-MM-DD HH:MM:SS` or RFC 3339, in UTC) in the
/// viewer's time zone: `21 Oct 2026, 13:50`. A bare date stays a date, and
/// text that isn't a time comes back as it was. Uses today's offset, so a
/// time on the other side of a daylight-saving change can be an hour out.
pub fn fmt_when(raw: &str) -> String {
    if Runtime::try_current().is_some() {
        ZONE_VERSION.read();
    }
    when_in(raw, UTC_OFFSET.with(Cell::get))
}

fn when_in(raw: &str, offset_minutes: i32) -> String {
    use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Timelike};
    let text = raw.trim();
    let month = |m0: u32| crate::i18n::tr(MONTHS[m0 as usize]);
    if let Ok(d) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return format!("{} {} {}", d.day(), month(d.month0()), d.year());
    }
    let utc = DateTime::parse_from_rfc3339(text)
        .map(|t| t.naive_utc())
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S%.f"))
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f"));
    let Ok(utc) = utc else {
        return raw.to_string();
    };
    let local = utc + chrono::Duration::minutes(offset_minutes.into());
    format!("{} {} {}, {:02}:{:02}", local.day(), month(local.month0()), local.year(), local.hour(), local.minute())
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
    fmt_money(value * rate, symbol, decimals)
}

/// `฿1,234.56` style: `value` as is (not converted to the display
/// currency), with `symbol` in front.
pub fn fmt_money(value: Decimal, symbol: &str, decimals: u32) -> String {
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
/// thousands grouped — `1.405399` → `1.4054`, `1500` → `1,500`. Amounts
/// under one keep 4 significant digits, up to 8 decimals, so small crypto
/// holdings don't round away: `0.00012345` → `0.0001234`.
pub fn fmt_shares(shares: Decimal) -> String {
    let dp = (4..=8)
        .find(|&dp| shares.abs() * Decimal::from(10u64.pow(dp)) >= Decimal::from(1000))
        .unwrap_or(8);
    let rounded = shares.round_dp(dp).normalize();
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
        assert_eq!(fmt_shares(dec!(0.00012345)), "0.0001234");
        assert_eq!(fmt_shares(dec!(0.000000004)), "0");
        assert_eq!(fmt_shares(dec!(-0.0215678)), "-0.02157");
        assert_eq!(fmt_shares(dec!(12.3456789)), "12.3457");
    }

    #[test]
    fn readable_times_in_the_viewers_zone() {
        // Bangkok is seven hours ahead of UTC.
        assert_eq!(when_in("2026-10-21 18:05:09", 420), "22 Oct 2026, 01:05");
        assert_eq!(when_in("2026-10-21T06:50:21.123Z", 420), "21 Oct 2026, 13:50");
        assert_eq!(when_in("2026-10-21T06:50:21+00:00", -240), "21 Oct 2026, 02:50");
        assert_eq!(when_in("2026-01-05", 420), "5 Jan 2026");
        assert_eq!(when_in("never", 0), "never");
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
