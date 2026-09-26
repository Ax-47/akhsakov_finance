//! Economy: market gauges, US macro indicators and the Treasury yield curve.

use crate::i18n::tr;
use crate::{components::card::Card, page::Page};
use dioxus::prelude::*;
use dtos::economy::{value_on_or_before, CurvePoint, EconomySnapshot, Gauge, Indicator};

const REFRESH_MS: u32 = 60_000;
const RETRY_MS: u32 = 5_000;

#[component]
pub fn EconomyPage() -> Element {
    const KEY: &str = "economy";
    let mut data = use_signal(|| crate::cache::get::<EconomySnapshot>(KEY).map(Ok));
    use_future(move || async move {
        loop {
            let result = api::get_economy().await.map_err(|e| match e {
                ServerFnError::ServerError { message, .. } => message,
                e => e.to_string(),
            });
            if let Ok(snapshot) = &result {
                crate::cache::put(KEY.to_string(), snapshot.clone());
            }
            let ok = result.is_ok();
            // Keep the last good snapshot if a refresh fails.
            if ok || !matches!(*data.peek(), Some(Ok(_))) {
                data.set(Some(result));
            }
            crate::notify::poll_delay(if ok { REFRESH_MS } else { RETRY_MS }).await;
        }
    });

    let body = match data() {
        None => rsx! { p { class: "mt-10 text-sm text-ctp-subtext0", {tr("Loading economic data…")} } },
        Some(Err(message)) => rsx! {
            p { class: "mt-10 text-sm text-ctp-red", "Couldn't load economic data: {message}. Retrying…" }
        },
        Some(Ok(snap)) => rsx! {
            div { class: "mt-10 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 motion-safe:animate-rise",
                for g in snap.gauges.clone() {
                    GaugeTile { key: "{g.symbol}", gauge: g }
                }
            }
            if !snap.yield_curve.iter().all(|p| p.now.is_none()) {
                div { class: "mt-5 motion-safe:animate-rise",
                    YieldCurve { points: snap.yield_curve.clone() }
                }
            }
            div { class: "mt-5 grid gap-5 md:grid-cols-2 xl:grid-cols-3 motion-safe:animate-rise",
                for i in snap.indicators.clone() {
                    IndicatorCard { key: "{i.id}", indicator: i }
                }
            }
            p { class: "mt-6 text-xs text-ctp-overlay1",
                {tr("Macro data from FRED (Federal Reserve Bank of St. Louis); market levels from Yahoo Finance.")}
            }
        },
    };

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("Economy")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0", {tr("Rates, inflation, jobs and the markets' mood.")} }
            }
            {body}
        }
    }
}

#[component]
fn GaugeTile(gauge: Gauge) -> Element {
    let value = match gauge.unit.as_str() {
        "%" => format!("{:.2}%", gauge.value),
        "$" => format!("${}", grouped(gauge.value, 2)),
        _ => grouped(gauge.value, 2),
    };
    let (change, tone) = match gauge.change_pct {
        Some(c) if c >= 0.0 => (format!("+{c:.2}%"), "text-ctp-green"),
        Some(c) => (format!("{c:.2}%"), "text-ctp-red"),
        None => (String::new(), ""),
    };
    rsx! {
        div { class: "rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60 px-4 py-3",
            div { class: "truncate text-xs text-ctp-subtext0", "{gauge.label}" }
            div { class: "mt-1 flex items-baseline justify-between gap-2",
                span { class: "text-lg font-semibold tabular-nums text-ctp-text", "{value}" }
                span { class: "text-xs tabular-nums {tone}", "{change}" }
            }
        }
    }
}

/// Treasury yields by maturity, today against a year ago.
#[component]
fn YieldCurve(points: Vec<CurvePoint>) -> Element {
    const W: f64 = 600.0;
    const H: f64 = 180.0;
    const PAD: f64 = 24.0;
    let values: Vec<f64> = points
        .iter()
        .flat_map(|p| [p.now, p.year_ago])
        .flatten()
        .collect();
    let lo = values.iter().cloned().fold(f64::INFINITY, f64::min).floor();
    let hi = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max).ceil().max(lo + 1.0);
    let n = points.len().max(2) as f64 - 1.0;
    let x = |i: usize| PAD + i as f64 / n * (W - 2.0 * PAD);
    let y = |v: f64| H - PAD - (v - lo) / (hi - lo) * (H - 2.0 * PAD);
    let line = |pick: fn(&CurvePoint) -> Option<f64>| {
        points
            .iter()
            .enumerate()
            .filter_map(|(i, p)| pick(p).map(|v| format!("{:.1},{:.1}", x(i), y(v))))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (now, before) = (line(|p| p.now), line(|p| p.year_ago));
    let short = points.first().and_then(|p| p.now);
    let long = points.iter().find(|p| p.label == "10Y").and_then(|p| p.now);
    let summary = match (short, long) {
        (Some(s), Some(l)) if l < s => tr("Inverted: short-term rates are above long-term ones."),
        (Some(_), Some(_)) => tr("Normal: longer loans pay more."),
        _ => "",
    };
    rsx! {
        Card { title: tr("Treasury yield curve"), subtitle: summary.to_string(),
            actions: rsx! {
                div { class: "flex gap-4 text-xs text-ctp-subtext0",
                    span { class: "flex items-center gap-1.5", span { class: "h-0.5 w-4 bg-ctp-mauve" } "Today" }
                    span { class: "flex items-center gap-1.5", span { class: "h-0.5 w-4 border-t border-dashed border-ctp-overlay1" } "A year ago" }
                }
            },
            svg { class: "w-full", view_box: "0 0 {W} {H}",
                for step in 0..=((hi - lo) as i32) {
                    {
                        let v = lo + step as f64;
                        rsx! {
                            g { key: "{step}",
                                line { x1: "{PAD}", x2: "{W - PAD}", y1: "{y(v):.1}", y2: "{y(v):.1}", stroke: "var(--catppuccin-color-surface0)", stroke_width: "1" }
                                text { x: "2", y: "{y(v) + 3.0:.1}", font_size: "11", fill: "var(--catppuccin-color-overlay0)", "{v:.0}%" }
                            }
                        }
                    }
                }
                polyline { points: "{before}", fill: "none", stroke: "var(--catppuccin-color-overlay1)", stroke_width: "1.5", stroke_dasharray: "4 4" }
                polyline { points: "{now}", fill: "none", stroke: "var(--catppuccin-color-mauve)", stroke_width: "2.5", stroke_linejoin: "round" }
                for (i, p) in points.iter().enumerate() {
                    g { key: "{i}",
                        if let Some(v) = p.now {
                            circle { cx: "{x(i):.1}", cy: "{y(v):.1}", r: "3.5", fill: "var(--catppuccin-color-mauve)",
                                title { "{p.label}: {v:.2}%" }
                            }
                        }
                        text { x: "{x(i):.1}", y: "{H - 6.0}", font_size: "11", text_anchor: "middle", fill: "var(--catppuccin-color-overlay1)", "{p.label}" }
                    }
                }
            }
        }
    }
}

#[component]
fn IndicatorCard(indicator: Indicator) -> Element {
    let Some((date, latest)) = indicator.latest().cloned() else {
        return rsx! {};
    };
    let year_ago = year_before(&date).and_then(|d| value_on_or_before(&indicator.history, &d));
    let change = year_ago.map(|y| latest - y);
    let values: Vec<f64> = indicator.history.iter().map(|(_, v)| *v).collect();
    rsx! {
        Card { title: indicator.label.clone(), subtitle: indicator.description.clone(),
            div { class: "flex items-end justify-between gap-4",
                div {
                    div { class: "text-3xl font-semibold tabular-nums text-ctp-text", "{latest:.2}{indicator.unit}" }
                    div { class: "mt-1 text-xs text-ctp-subtext0",
                        {month_year(&date)}
                        if let Some(c) = change {
                            " · "
                            span { class: if c >= 0.0 { "text-ctp-peach" } else { "text-ctp-teal" }, "{c:+.2} pts" }
                            " vs a year earlier"
                        }
                    }
                }
            }
            Sparkline { values, zero_line: values_cross_zero(&indicator.history) }
            div { class: "mt-1 flex justify-between text-xs text-ctp-overlay1",
                span { {indicator.history.first().map(|(d, _)| month_year(d)).unwrap_or_default()} }
                span { {month_year(&date)} }
            }
        }
    }
}

/// A small line chart of `values`, scaled to fit.
#[component]
fn Sparkline(values: Vec<f64>, zero_line: bool) -> Element {
    const W: f64 = 300.0;
    const H: f64 = 60.0;
    if values.len() < 2 {
        return rsx! {};
    }
    let lo = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1e-9);
    let n = values.len() as f64 - 1.0;
    let y = |v: f64| H - 4.0 - (v - lo) / span * (H - 8.0);
    let points = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{:.1},{:.1}", i as f64 / n * W, y(*v)))
        .collect::<Vec<_>>()
        .join(" ");
    rsx! {
        svg { class: "mt-4 h-16 w-full", view_box: "0 0 {W} {H}", preserve_aspect_ratio: "none",
            if zero_line {
                line { x1: "0", x2: "{W}", y1: "{y(0.0):.1}", y2: "{y(0.0):.1}", stroke: "var(--catppuccin-color-surface1)", stroke_dasharray: "3 3", vector_effect: "non-scaling-stroke" }
            }
            polyline { points, fill: "none", stroke: "var(--catppuccin-color-sky)", stroke_width: "2", vector_effect: "non-scaling-stroke", stroke_linejoin: "round" }
        }
    }
}

fn values_cross_zero(history: &[(String, f64)]) -> bool {
    history.iter().any(|(_, v)| *v < 0.0) && history.iter().any(|(_, v)| *v > 0.0)
}

/// `2026-08-01` → `2025-08-01`.
fn year_before(date: &str) -> Option<String> {
    let year: i32 = date.get(..4)?.parse().ok()?;
    Some(format!("{}{}", year - 1, date.get(4..)?))
}

/// `2026-08-01` → `Aug 2026`.
fn month_year(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month: usize = date.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(0);
    match (MONTHS.get(month.wrapping_sub(1)), date.get(..4)) {
        (Some(m), Some(y)) => format!("{m} {y}"),
        _ => date.to_string(),
    }
}

/// `1234567.891` → `1,234,567.89`.
fn grouped(value: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, value.abs());
    let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut out = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let sign = if value < 0.0 { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{out}")
    } else {
        format!("{sign}{out}.{frac}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_levels_and_dates() {
        assert_eq!(grouped(51828.624, 2), "51,828.62");
        assert_eq!(grouped(-0.5, 2), "-0.50");
        assert_eq!(month_year("2026-08-01"), "Aug 2026");
        assert_eq!(year_before("2026-02-28").as_deref(), Some("2025-02-28"));
    }
}
