//! Allocation donut: a slice for each of the largest holdings and one for
//! the rest, with the legend and the donut highlighting together on hover.

use crate::i18n::{tr, trf};
use crate::components::{
    card::Card,
    color_schema::{CHART_COLORS_HEX, CHART_COLOR_CLASSES, CHART_NEUTRAL_HEX},
};
use dioxus::prelude::*;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::ticker_symbol::TickerSymbol;

const SIZE: f64 = 200.0;
const RADIUS: f64 = 78.0;
const STROKE: f64 = 22.0;
/// Gap between slices, in the same units as the circumference.
const GAP: f64 = 2.5;
/// Holdings that get a colour of their own; the rest share one grey
/// "Other" slice. More than this many hues stop being told apart, and the
/// palette would start repeating (two holdings in the same colour).
const NAMED_SLICES: usize = 7;

/// One slice of the donut and its legend row.
#[derive(Clone, Debug, PartialEq)]
struct Slice {
    label: String,
    pct: f64,
    /// `None` for "Other", drawn in a neutral grey.
    colour: Option<usize>,
}

impl Slice {
    fn hex(&self) -> &'static str {
        self.colour.map_or(CHART_NEUTRAL_HEX, |i| CHART_COLORS_HEX[i])
    }

    fn class(&self) -> &'static str {
        self.colour.map_or("bg-ctp-overlay2", |i| CHART_COLOR_CLASSES[i])
    }
}

/// The largest `NAMED_SLICES` holdings each in their own colour, the rest
/// folded into one "Other" slice (`weights` largest first).
fn slices(weights: &[(String, f64)]) -> Vec<Slice> {
    if weights.len() <= NAMED_SLICES + 1 {
        return weights
            .iter()
            .enumerate()
            .map(|(i, (label, pct))| Slice { label: label.clone(), pct: *pct, colour: Some(i) })
            .collect();
    }
    let (named, rest) = weights.split_at(NAMED_SLICES);
    let mut out: Vec<Slice> = named
        .iter()
        .enumerate()
        .map(|(i, (label, pct))| Slice { label: label.clone(), pct: *pct, colour: Some(i) })
        .collect();
    out.push(Slice {
        label: trf("Other ({})", &[&rest.len()]),
        pct: rest.iter().map(|(_, pct)| pct).sum(),
        colour: None,
    });
    out
}

/// `allocation` is `(ticker, weight %)`, largest first.
#[component]
pub fn AllocationCard(allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>) -> Element {
    let hovered = use_signal(|| None::<usize>);
    let weights: Vec<(String, f64)> = allocation
        .read()
        .iter()
        .map(|(t, w)| (t.to_string(), w.to_f64().unwrap_or(0.0)))
        .collect();
    let holdings = weights.len();
    let slices = slices(&weights);
    // Bars compare holdings with each other: the largest holding fills its
    // bar. "Other" isn't a holding, so it has no bar.
    let largest = slices.iter().filter(|s| s.colour.is_some()).map(|s| s.pct).fold(0.0, f64::max);

    rsx! {
        Card { title: tr("Allocation"),
            if slices.is_empty() {
                div { class: "flex items-center justify-center h-24 text-ctp-overlay1", {tr("No data")} }
            } else {
                Donut { slices: slices.clone(), holdings, hovered }
                div { class: "mt-5 flex flex-col gap-0.5",
                    for (i, slice) in slices.into_iter().enumerate() {
                        LegendRow { key: "{slice.label}", index: i, slice, largest, hovered }
                    }
                }
            }
        }
    }
}

#[component]
fn Donut(slices: Vec<Slice>, holdings: usize, mut hovered: Signal<Option<usize>>) -> Element {
    let circumference = 2.0 * std::f64::consts::PI * RADIUS;
    let total: f64 = slices.iter().map(|s| s.pct).sum::<f64>().max(f64::EPSILON);
    let gap = if slices.len() > 1 { GAP } else { 0.0 };
    let center = SIZE / 2.0;

    let mut start = 0.0;
    let arcs: Vec<(usize, f64, f64)> = slices
        .iter()
        .enumerate()
        .map(|(i, slice)| {
            let len = slice.pct / total * circumference;
            let arc = (i, (len - gap).max(0.5), start);
            start += len;
            arc
        })
        .collect();

    let (big, small) = match hovered().and_then(|i| slices.get(i)) {
        Some(slice) => (format!("{:.1}%", slice.pct), slice.label.clone()),
        None => (holdings.to_string(), tr("holdings").to_string()),
    };

    rsx! {
        div { class: "relative mx-auto", style: "width:{SIZE}px;height:{SIZE}px;",
            svg {
                class: "h-full w-full -rotate-90",
                view_box: "0 0 {SIZE} {SIZE}",
                onmouseleave: move |_| hovered.set(None),
                circle { cx: "{center}", cy: "{center}", r: "{RADIUS}", fill: "none", stroke: "var(--catppuccin-color-surface0)", stroke_opacity: "0.4", stroke_width: "{STROKE}" }
                for (i, len, offset) in arcs {
                    circle {
                        key: "{i}",
                        cx: "{center}", cy: "{center}", r: "{RADIUS}",
                        fill: "none",
                        stroke: slices[i].hex(),
                        stroke_width: if hovered() == Some(i) { "{STROKE + 6.0}" } else { "{STROKE}" },
                        stroke_dasharray: "{len:.2} {circumference:.2}",
                        stroke_dashoffset: "{-offset:.2}",
                        opacity: if hovered().is_some_and(|h| h != i) { "0.35" } else { "1" },
                        style: "transition:stroke-width .2s ease, opacity .2s ease;cursor:pointer;",
                        onmouseenter: move |_| hovered.set(Some(i)),
                    }
                }
            }
            div { class: "pointer-events-none absolute inset-0 flex flex-col items-center justify-center",
                span { class: "text-3xl font-semibold tabular-nums text-ctp-text", "{big}" }
                span { class: "text-xs text-ctp-subtext0", "{small}" }
            }
        }
    }
}

#[component]
fn LegendRow(
    index: usize,
    slice: Slice,
    /// The largest holding's %, which fills a bar.
    largest: f64,
    mut hovered: Signal<Option<usize>>,
) -> Element {
    let color = slice.class();
    let bar = (slice.pct / largest.max(f64::EPSILON) * 100.0).min(100.0);
    let active = hovered() == Some(index);
    let dimmed = hovered().is_some() && !active;
    rsx! {
        div {
            class: if active {
                "flex items-center justify-between gap-3 rounded-xl bg-ctp-surface0/60 px-2 py-1.5 text-xs cursor-default transition-colors"
            } else if dimmed {
                "flex items-center justify-between gap-3 rounded-xl px-2 py-1.5 text-xs opacity-50 cursor-default transition-colors"
            } else {
                "flex items-center justify-between gap-3 rounded-xl px-2 py-1.5 text-xs cursor-default transition-colors"
            },
            onmouseenter: move |_| hovered.set(Some(index)),
            onmouseleave: move |_| hovered.set(None),
            span { class: "flex items-center gap-2",
                span { class: "h-2.5 w-2.5 rounded-full shrink-0 {color}" }
                span { class: "font-medium text-ctp-subtext1", "{slice.label}" }
            }
            span { class: "flex items-center gap-2",
                if slice.colour.is_some() {
                    span { class: "h-1 w-16 overflow-hidden rounded-full bg-ctp-surface0",
                        span { class: "block h-full rounded-full {color}", style: "width:{bar:.0}%;" }
                    }
                } else {
                    span { class: "w-16" }
                }
                span { class: "w-11 text-right tabular-nums text-ctp-subtext0", "{slice.pct:.1}%" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weights(n: usize) -> Vec<(String, f64)> {
        (0..n).map(|i| (format!("T{i}"), (n - i) as f64)).collect()
    }

    #[test]
    fn up_to_eight_holdings_each_get_a_colour() {
        let out = slices(&weights(8));
        assert_eq!(out.len(), 8);
        assert!(out.iter().all(|s| s.colour.is_some()));
    }

    #[test]
    fn past_that_the_rest_fold_into_other_and_no_colour_repeats() {
        let all = weights(22);
        let out = slices(&all);
        assert_eq!(out.len(), NAMED_SLICES + 1);
        let other = out.last().unwrap();
        assert_eq!(other.colour, None);
        assert_eq!(other.label, "Other (15)");
        let rest: f64 = all[NAMED_SLICES..].iter().map(|w| w.1).sum();
        assert_eq!(other.pct, rest);
        let mut colours: Vec<usize> = out.iter().filter_map(|s| s.colour).collect();
        colours.dedup();
        assert_eq!(colours.len(), NAMED_SLICES);
    }
}
