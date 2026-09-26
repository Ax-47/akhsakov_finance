//! Small SVG charts for fundamentals: grouped bars and sparklines.

use crate::i18n::tr;
use crate::format::fmt_compact;
use dioxus::prelude::*;

/// How chart values are labelled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    /// Compact money, `$4.2B`.
    Money,
    /// Fraction shown as percent, `0.42` → `42%`.
    Percent,
    /// Plain number, `1.23`.
    Number,
}

impl Unit {
    pub fn format(self, v: f64) -> String {
        match self {
            Self::Money => fmt_compact(v),
            Self::Percent => format!("{:.1}%", v * 100.0),
            Self::Number => format!("{v:.2}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarSeries {
    pub name: String,
    pub color: &'static str,
    /// One per label; `None` leaves a gap.
    pub values: Vec<Option<f64>>,
}

const W: f64 = 600.0;
const H: f64 = 220.0;
const LEFT: f64 = 60.0;
const RIGHT: f64 = 8.0;
const TOP: f64 = 10.0;
const BOTTOM: f64 = 26.0;
const GRID_HEX: &str = "var(--catppuccin-color-surface0)";
const LABEL_HEX: &str = "var(--catppuccin-color-overlay1)";

/// Grouped bar chart; `labels` oldest first. Handles negative values.
#[component]
pub fn BarChart(labels: Vec<String>, series: Vec<BarSeries>, unit: Unit) -> Element {
    let format = move |v: f64| unit.format(v);
    let values: Vec<f64> = series
        .iter()
        .flat_map(|s| s.values.iter().flatten().copied())
        .collect();
    if labels.is_empty() || values.is_empty() {
        return rsx! {
            p { class: "py-10 text-center text-sm text-ctp-overlay0", {tr("No data reported")} }
        };
    }
    let hi = values.iter().cloned().fold(0.0_f64, f64::max);
    let lo = values.iter().cloned().fold(0.0_f64, f64::min);
    let span = (hi - lo).max(f64::EPSILON);
    let (hi, lo) = (
        hi + span * 0.08,
        if lo < 0.0 { lo - span * 0.08 } else { 0.0 },
    );
    let plot_w = W - LEFT - RIGHT;
    let plot_h = H - TOP - BOTTOM;
    let y = move |v: f64| TOP + (hi - v) / (hi - lo) * plot_h;
    let zero = y(0.0);

    let group = plot_w / labels.len() as f64;
    let bar = (group * 0.72 / series.len() as f64).min(28.0);
    let ticks: Vec<f64> = (0..=3).map(|i| lo + (hi - lo) * i as f64 / 3.0).collect();

    rsx! {
        div {
            div { class: "mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-ctp-subtext0",
                for s in series.iter() {
                    span { class: "flex items-center gap-1.5",
                        span { class: "h-2 w-2 rounded-sm", style: "background:{s.color};" }
                        "{s.name}"
                    }
                }
            }
            svg { class: "w-full", view_box: "0 0 {W} {H}",
                for t in ticks {
                    line { x1: "{LEFT}", x2: "{W - RIGHT}", y1: "{y(t):.1}", y2: "{y(t):.1}", stroke: GRID_HEX, stroke_dasharray: "3 5" }
                    text { x: "{LEFT - 8.0}", y: "{y(t) + 4.0:.1}", text_anchor: "end", font_size: "10", fill: LABEL_HEX, "{format(t)}" }
                }
                line { x1: "{LEFT}", x2: "{W - RIGHT}", y1: "{zero:.1}", y2: "{zero:.1}", stroke: "var(--catppuccin-color-surface2)" }
                for (i, label) in labels.iter().enumerate() {
                    {
                        let x0 = LEFT + group * i as f64 + (group - bar * series.len() as f64) / 2.0;
                        rsx! {
                            g { key: "{label}",
                                for (j, s) in series.iter().enumerate() {
                                    if let Some(v) = s.values.get(i).copied().flatten() {
                                        rect {
                                            x: "{x0 + bar * j as f64:.1}",
                                            y: "{y(v.max(0.0)):.1}",
                                            width: "{(bar - 2.0).max(1.0):.1}",
                                            height: "{(y(v.min(0.0)) - y(v.max(0.0))).max(1.0):.1}",
                                            rx: "3",
                                            fill: s.color,
                                            fill_opacity: if v < 0.0 { "0.55" } else { "0.9" },
                                            title { "{label} · {s.name}: {format(v)}" }
                                        }
                                    }
                                }
                                text {
                                    x: "{LEFT + group * (i as f64 + 0.5):.1}",
                                    y: "{H - 8.0}",
                                    text_anchor: "middle",
                                    font_size: "10",
                                    fill: LABEL_HEX,
                                    "{label}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Tiny trend line; `values` oldest first.
#[component]
pub fn Sparkline(
    values: Vec<Option<f64>>,
    #[props(default = "var(--catppuccin-color-mauve)")] color: &'static str,
) -> Element {
    const SW: f64 = 80.0;
    const SH: f64 = 22.0;
    let points: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| Some((i, (*v)?)))
        .collect();
    if points.len() < 2 {
        return rsx! { span { class: "text-ctp-overlay0", "—" } };
    }
    let lo = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let hi = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let span = (hi - lo).max(f64::EPSILON);
    let n = (values.len() - 1).max(1) as f64;
    let xy = |(i, v): (usize, f64)| {
        (
            i as f64 / n * (SW - 4.0) + 2.0,
            SH - 3.0 - (v - lo) / span * (SH - 6.0),
        )
    };
    let path = points
        .iter()
        .map(|p| {
            let (x, y) = xy(*p);
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let (lx, ly) = xy(*points.last().unwrap_or(&(0, 0.0)));
    rsx! {
        svg { class: "inline-block", width: "{SW}", height: "{SH}", view_box: "0 0 {SW} {SH}",
            polyline { points: "{path}", fill: "none", stroke: color, stroke_width: "1.5", stroke_linejoin: "round" }
            circle { cx: "{lx:.1}", cy: "{ly:.1}", r: "2.2", fill: color }
        }
    }
}

/// One line of a [`DualLineChart`].
#[derive(Clone, Debug, PartialEq)]
pub struct AxisLine {
    pub name: String,
    pub color: &'static str,
    pub unit: Unit,
    /// One per label; `None` leaves a gap.
    pub values: Vec<Option<f64>>,
}

/// Two lines with their own y-axes (left for `left`, right for `right`),
/// for comparing measures in different units. `labels` oldest first.
#[component]
pub fn DualLineChart(labels: Vec<String>, left: AxisLine, right: AxisLine) -> Element {
    const DW: f64 = 600.0;
    const DH: f64 = 240.0;
    const PAD_X: f64 = 64.0;
    if labels.len() < 2 {
        return rsx! { p { class: "py-10 text-center text-sm text-ctp-overlay0", {tr("Not enough periods reported")} } };
    }
    let plot_w = DW - 2.0 * PAD_X;
    let plot_h = DH - TOP - BOTTOM;
    let x = |i: usize| PAD_X + plot_w * i as f64 / (labels.len() - 1) as f64;

    // Per-axis scale: (min, max) with padding.
    let scale = |line: &AxisLine| {
        let vals: Vec<f64> = line.values.iter().flatten().copied().collect();
        let lo = vals.iter().cloned().fold(f64::MAX, f64::min);
        let hi = vals.iter().cloned().fold(f64::MIN, f64::max);
        let pad = ((hi - lo) * 0.1).max(hi.abs() * 0.02).max(f64::EPSILON);
        (lo - pad, hi + pad)
    };
    let geometry = |line: &AxisLine| {
        let (lo, hi) = scale(line);
        let y = move |v: f64| TOP + (hi - v) / (hi - lo) * plot_h;
        let points: Vec<(f64, f64)> = line
            .values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| Some((x(i), y((*v)?))))
            .collect();
        let path = points
            .iter()
            .map(|(px, py)| format!("{px:.1},{py:.1}"))
            .collect::<Vec<_>>()
            .join(" ");
        let ticks: Vec<(f64, String)> = (0..=3)
            .map(|k| {
                let v = lo + (hi - lo) * k as f64 / 3.0;
                (y(v), line.unit.format(v))
            })
            .collect();
        (path, points, ticks)
    };
    let (lpath, lpoints, lticks) = geometry(&left);
    let (rpath, rpoints, rticks) = geometry(&right);

    rsx! {
        div {
            div { class: "mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-ctp-subtext0",
                span { class: "flex items-center gap-1.5",
                    span { class: "h-0.5 w-4 rounded", style: "background:{left.color};" }
                    "{left.name} (left axis)"
                }
                span { class: "flex items-center gap-1.5",
                    span { class: "h-0.5 w-4 rounded", style: "background:{right.color};" }
                    "{right.name} (right axis)"
                }
            }
            svg { class: "w-full", view_box: "0 0 {DW} {DH}",
                for (y, label) in lticks {
                    line { x1: "{PAD_X}", x2: "{DW - PAD_X}", y1: "{y:.1}", y2: "{y:.1}", stroke: GRID_HEX, stroke_dasharray: "3 5" }
                    text { x: "{PAD_X - 8.0}", y: "{y + 4.0:.1}", text_anchor: "end", font_size: "10", fill: left.color, "{label}" }
                }
                for (y, label) in rticks {
                    text { x: "{DW - PAD_X + 8.0}", y: "{y + 4.0:.1}", font_size: "10", fill: right.color, "{label}" }
                }
                polyline { points: "{lpath}", fill: "none", stroke: left.color, stroke_width: "2.2", stroke_linejoin: "round" }
                polyline { points: "{rpath}", fill: "none", stroke: right.color, stroke_width: "2.2", stroke_linejoin: "round", stroke_dasharray: "6 4" }
                for (px, py) in lpoints {
                    circle { cx: "{px:.1}", cy: "{py:.1}", r: "3.5", fill: left.color }
                }
                for (px, py) in rpoints {
                    circle { cx: "{px:.1}", cy: "{py:.1}", r: "3.5", fill: "var(--catppuccin-color-base)", stroke: right.color, stroke_width: "2" }
                }
                for (i, label) in labels.iter().enumerate() {
                    text { x: "{x(i):.1}", y: "{DH - 8.0}", text_anchor: "middle", font_size: "10", fill: LABEL_HEX, "{label}" }
                }
            }
        }
    }
}
