//! Small SVG charts for fundamentals: grouped bars, lines and sparklines.

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
    /// Fraction shown as a signed change, `0.25` → `+25.0%`.
    Change,
}

impl Unit {
    pub fn format(self, v: f64) -> String {
        match self {
            Self::Money => fmt_compact(v),
            Self::Percent => format!("{:.1}%", v * 100.0),
            Self::Number => format!("{v:.2}"),
            Self::Change if v.abs() < 0.0005 => "0%".into(),
            Self::Change => format!("{:+.1}%", v * 100.0),
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
            p { class: "py-10 text-center text-sm text-ctp-overlay1", {tr("No data reported")} }
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
                    text { x: "{LEFT - 8.0}", y: "{y(t) + 4.0:.1}", text_anchor: "end", font_size: "11", fill: LABEL_HEX, "{format(t)}" }
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
                                    font_size: "11",
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
        return rsx! { span { class: "text-ctp-overlay1", "—" } };
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

/// One line of a [`LineChart`].
#[derive(Clone, Debug, PartialEq)]
pub struct LineSeries {
    pub name: String,
    pub color: &'static str,
    /// One per label; `None` leaves a gap.
    pub values: Vec<Option<f64>>,
}

/// Lines on one shared y-axis; `labels` oldest first. Lines are told apart
/// by colour, by a dash on the second and by the value labelled at each
/// line's end. Hovering a period shows every line's value there.
#[component]
pub fn LineChart(
    labels: Vec<String>,
    series: Vec<LineSeries>,
    unit: Unit,
    /// In the chart's own units; it scales with the width.
    #[props(default = 240.0)]
    height: f64,
) -> Element {
    /// Room at the right for the end labels.
    const END_LABELS: f64 = 64.0;
    /// Smallest gap between two end labels before they're pushed apart.
    const LABEL_GAP: f64 = 14.0;
    let values: Vec<f64> = series
        .iter()
        .flat_map(|s| s.values.iter().flatten().copied())
        .collect();
    if labels.len() < 2 || values.is_empty() {
        return rsx! { p { class: "py-10 text-center text-sm text-ctp-overlay1", {tr("Not enough periods reported")} } };
    }
    let plot_w = W - LEFT - END_LABELS;
    let plot_h = height - TOP - BOTTOM;
    let lo = values.iter().cloned().fold(f64::MAX, f64::min);
    let hi = values.iter().cloned().fold(f64::MIN, f64::max);
    let ticks = round_ticks(lo, hi);
    let (lo, hi) = (ticks[0], ticks[ticks.len() - 1]);
    let x = |i: usize| LEFT + plot_w * i as f64 / (labels.len() - 1) as f64;
    let y = move |v: f64| TOP + (hi - v) / (hi - lo) * plot_h;
    let step = plot_w / (labels.len() - 1) as f64;

    let lines: Vec<(usize, Vec<String>, Vec<(f64, f64)>)> = series
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let points: Vec<Option<(f64, f64)>> = s
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| Some((x(i), y((*v)?))))
                .collect();
            (n, unbroken_runs(&points), points.into_iter().flatten().collect())
        })
        .collect();
    let end_labels = spread_apart(
        series
            .iter()
            .filter_map(|s| {
                let (i, v) = s.values.iter().enumerate().rev().find_map(|(i, v)| Some((i, (*v)?)))?;
                Some((x(i), y(v), unit.format(v)))
            })
            .collect(),
        LABEL_GAP,
    );
    let hover_text = |i: usize| {
        let parts: Vec<String> = series
            .iter()
            .map(|s| {
                let value = s.values.get(i).copied().flatten().map_or("—".to_string(), |v| unit.format(v));
                format!("{}: {value}", s.name)
            })
            .collect();
        format!("{} · {}", labels[i], parts.join(" · "))
    };

    rsx! {
        div {
            if series.len() > 1 {
                div { class: "mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-ctp-subtext0",
                    for (n, s) in series.iter().enumerate() {
                        span { class: "flex items-center gap-1.5",
                            svg { width: "18", height: "4", view_box: "0 0 18 4",
                                line { x1: "0", x2: "18", y1: "2", y2: "2", stroke: s.color, stroke_width: "2", stroke_dasharray: if n == 1 { "5 3" } else { "none" } }
                            }
                            "{s.name}"
                        }
                    }
                }
            }
            svg { class: "w-full", view_box: "0 0 {W} {height}",
                for t in ticks {
                    line { x1: "{LEFT}", x2: "{LEFT + plot_w}", y1: "{y(t):.1}", y2: "{y(t):.1}", stroke: GRID_HEX, stroke_width: "1" }
                    text { x: "{LEFT - 8.0}", y: "{y(t) + 4.0:.1}", text_anchor: "end", font_size: "11", fill: LABEL_HEX, "{unit.format(t)}" }
                }
                if lo < 0.0 && hi > 0.0 {
                    line { x1: "{LEFT}", x2: "{LEFT + plot_w}", y1: "{y(0.0):.1}", y2: "{y(0.0):.1}", stroke: "var(--catppuccin-color-surface2)", stroke_width: "1.5" }
                }
                for i in 0..labels.len() {
                    g { key: "{i}", class: "group",
                        line { class: "opacity-0 group-hover:opacity-100", x1: "{x(i):.1}", x2: "{x(i):.1}", y1: "{TOP}", y2: "{TOP + plot_h}", stroke: "var(--catppuccin-color-overlay0)", stroke_width: "1" }
                        rect { x: "{x(i) - step / 2.0:.1}", y: "{TOP}", width: "{step:.1}", height: "{plot_h:.1}", fill: "transparent",
                            title { "{hover_text(i)}" }
                        }
                    }
                }
                for (n, runs, points) in lines {
                    for run in runs {
                        polyline { points: "{run}", fill: "none", stroke: series[n].color, stroke_width: "2", stroke_linejoin: "round", stroke_linecap: "round", stroke_dasharray: if n == 1 { "6 4" } else { "none" }, pointer_events: "none" }
                    }
                    for (px, py) in points {
                        circle { cx: "{px:.1}", cy: "{py:.1}", r: "4", fill: series[n].color, stroke: "var(--catppuccin-color-base)", stroke_width: "2", pointer_events: "none" }
                    }
                }
                for (lx, ly, text) in end_labels {
                    text { x: "{lx + 9.0:.1}", y: "{ly + 4.0:.1}", font_size: "11", font_weight: "600", fill: "var(--catppuccin-color-subtext1)", "{text}" }
                }
                for (i, label) in labels.iter().enumerate() {
                    text { x: "{x(i):.1}", y: "{height - 8.0}", text_anchor: "middle", font_size: "11", fill: LABEL_HEX, "{label}" }
                }
            }
        }
    }
}

/// Round tick values from at or below `lo` to at or above `hi`, about four
/// steps of 1, 2, 2.5 or 5 times a power of ten apart, so an axis reads
/// 0%, 20%, 40% rather than 3.7%, 21.2%. Zero is a tick whenever it's in range.
fn round_ticks(lo: f64, hi: f64) -> Vec<f64> {
    let rough_step = (hi - lo).max(hi.abs() * 0.01).max(f64::EPSILON) / 4.0;
    let magnitude = 10f64.powf(rough_step.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= rough_step)
        .unwrap_or(10.0 * magnitude);
    let (mut first, mut last) = ((lo / step).floor() as i64, (hi / step).ceil() as i64);
    if first == last {
        // A flat line: one step either side, so it sits mid-chart.
        (first, last) = (first - 1, last + 1);
    }
    // `+ 0.0` turns -0.0 into 0.0, so zero isn't labelled "-0".
    (first..=last).map(|k| k as f64 * step + 0.0).collect()
}

/// `points` as SVG polyline paths, one per stretch without a gap.
fn unbroken_runs(points: &[Option<(f64, f64)>]) -> Vec<String> {
    points
        .split(Option::is_none)
        .filter(|run| !run.is_empty())
        .map(|run| run.iter().flatten().map(|(x, y)| format!("{x:.1},{y:.1}")).collect::<Vec<_>>().join(" "))
        .collect()
}

/// End labels `(x, y, text)` moved apart vertically so none sit closer than
/// `gap`, keeping their order.
fn spread_apart(mut labels: Vec<(f64, f64, String)>, gap: f64) -> Vec<(f64, f64, String)> {
    labels.sort_by(|a, b| a.1.total_cmp(&b.1));
    for i in 1..labels.len() {
        let floor = labels[i - 1].1 + gap;
        if labels[i].1 < floor {
            labels[i].1 = floor;
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_break_at_gaps() {
        let points = [Some((1.0, 2.0)), Some((3.0, 4.0)), None, Some((5.0, 6.0)), None];
        assert_eq!(unbroken_runs(&points), ["1.0,2.0 3.0,4.0", "5.0,6.0"]);
        assert!(unbroken_runs(&[None, None]).is_empty());
    }

    #[test]
    fn end_labels_keep_their_distance() {
        let spread = spread_apart(vec![(0.0, 50.0, "b".into()), (0.0, 45.0, "a".into())], 14.0);
        assert_eq!(spread.iter().map(|l| (l.1, l.2.as_str())).collect::<Vec<_>>(), [(45.0, "a"), (59.0, "b")]);
    }

    #[test]
    fn ticks_are_round_and_include_zero() {
        let close = |got: Vec<f64>, want: &[f64]| {
            assert_eq!(got.len(), want.len(), "{got:?}");
            assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-9 * w.abs().max(1.0)), "{got:?}");
        };
        close(round_ticks(-0.05, 0.52), &[-0.2, 0.0, 0.2, 0.4, 0.6]);
        close(round_ticks(76e9, 122e9), &[60e9, 80e9, 100e9, 120e9, 140e9]);
        close(round_ticks(17.0, 25.5), &[15.0, 17.5, 20.0, 22.5, 25.0, 27.5]);
        close(round_ticks(5.0, 5.0), &[4.98, 5.0, 5.02]);
        assert_eq!(round_ticks(-0.05, 0.52)[1].to_string(), "0", "zero, not -0");
    }

    #[test]
    fn change_is_signed() {
        assert_eq!(Unit::Change.format(0.25), "+25.0%");
        assert_eq!(Unit::Change.format(-0.031), "-3.1%");
        assert_eq!(Unit::Change.format(0.0), "0%");
    }
}
