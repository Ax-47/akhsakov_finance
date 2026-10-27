//! Allocation donut: a slice for each of the largest holdings and one grey
//! slice for the rest, with every holding listed below. The list and the
//! donut highlight together on hover.

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
const OTHER_CLASS: &str = "bg-ctp-overlay2";

/// One slice of the donut.
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
}

/// What the pointer is on: one holding (a row, or its own slice), or the
/// grey slice of the smaller holdings.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hover {
    Holding(usize),
    Other,
}

/// How many holdings (largest first) get a colour of their own: all of
/// them when one grey slice would hold a single holding anyway.
fn named_count(holdings: usize) -> usize {
    if holdings <= NAMED_SLICES + 1 {
        holdings
    } else {
        NAMED_SLICES
    }
}

/// The donut: the named holdings, then one "Other" slice for the rest
/// (`weights` largest first).
fn slices(weights: &[(String, f64)]) -> Vec<Slice> {
    let named = named_count(weights.len());
    let (own, rest) = weights.split_at(named);
    let mut out: Vec<Slice> = own
        .iter()
        .enumerate()
        .map(|(i, (label, pct))| Slice { label: label.clone(), pct: *pct, colour: Some(i) })
        .collect();
    if !rest.is_empty() {
        out.push(Slice {
            label: trf("Other ({})", &[&rest.len()]),
            pct: rest.iter().map(|(_, pct)| pct).sum(),
            colour: None,
        });
    }
    out
}

/// The donut slice holding `i` is drawn in.
fn slice_of(holding: usize, named: usize) -> usize {
    holding.min(named)
}

/// `allocation` is `(ticker, weight %)`, largest first.
#[component]
pub fn AllocationCard(allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>) -> Element {
    let hovered = use_signal(|| None::<Hover>);
    let weights: Vec<(String, f64)> = allocation
        .read()
        .iter()
        .map(|(t, w)| (t.to_string(), w.to_f64().unwrap_or(0.0)))
        .collect();
    let named = named_count(weights.len());
    let slices = slices(&weights);
    // Bars compare holdings with each other: the largest one fills its bar.
    let largest = weights.iter().map(|w| w.1).fold(0.0, f64::max);
    let other = slices.get(named).cloned();

    rsx! {
        Card { title: tr("Allocation"),
            if weights.is_empty() {
                div { class: "flex items-center justify-center h-24 text-ctp-overlay1", {tr("No data")} }
            } else {
                Donut { slices, weights: weights.clone(), named, hovered }
                div { class: "mt-5 flex flex-col gap-0.5",
                    for (i, (ticker, pct)) in weights.iter().cloned().enumerate().take(named) {
                        LegendRow { key: "{ticker}", index: i, ticker, pct, class: CHART_COLOR_CLASSES[i], largest, named, hovered }
                    }
                    if let Some(other) = other {
                        OtherDivider { slice: other, hovered }
                        for (i, (ticker, pct)) in weights.iter().cloned().enumerate().skip(named) {
                            LegendRow { key: "{ticker}", index: i, ticker, pct, class: OTHER_CLASS, largest, named, hovered }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Donut(slices: Vec<Slice>, weights: Vec<(String, f64)>, named: usize, mut hovered: Signal<Option<Hover>>) -> Element {
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
    let lit = hovered().map(|h| match h {
        Hover::Holding(i) => slice_of(i, named),
        Hover::Other => named,
    });
    let (big, small) = match hovered() {
        Some(Hover::Holding(i)) => weights
            .get(i)
            .map(|(ticker, pct)| (format!("{pct:.1}%"), ticker.clone()))
            .unwrap_or_default(),
        Some(Hover::Other) => slices
            .get(named)
            .map(|s| (format!("{:.1}%", s.pct), s.label.clone()))
            .unwrap_or_default(),
        None => (weights.len().to_string(), tr("holdings").to_string()),
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
                        stroke_width: if lit == Some(i) { "{STROKE + 6.0}" } else { "{STROKE}" },
                        stroke_dasharray: "{len:.2} {circumference:.2}",
                        stroke_dashoffset: "{-offset:.2}",
                        opacity: if lit.is_some_and(|l| l != i) { "0.35" } else { "1" },
                        style: "transition:stroke-width .2s ease, opacity .2s ease;cursor:pointer;",
                        onmouseenter: move |_| hovered.set(Some(if i < named { Hover::Holding(i) } else { Hover::Other })),
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

/// Heads the holdings in the grey slice, with their combined weight.
#[component]
fn OtherDivider(slice: Slice, mut hovered: Signal<Option<Hover>>) -> Element {
    rsx! {
        div {
            class: "mt-2 flex items-center justify-between gap-3 border-t border-ctp-surface0/60 px-2 pt-2.5 pb-1 text-xs text-ctp-overlay1 cursor-default",
            onmouseenter: move |_| hovered.set(Some(Hover::Other)),
            onmouseleave: move |_| hovered.set(None),
            span { class: "flex items-center gap-2",
                span { class: "h-2.5 w-2.5 rounded-full shrink-0 {OTHER_CLASS}" }
                "{slice.label}"
            }
            span { class: "tabular-nums", "{slice.pct:.1}%" }
        }
    }
}

#[component]
fn LegendRow(
    index: usize,
    ticker: String,
    pct: f64,
    class: &'static str,
    /// The largest holding's %, which fills a bar.
    largest: f64,
    named: usize,
    mut hovered: Signal<Option<Hover>>,
) -> Element {
    let bar = (pct / largest.max(f64::EPSILON) * 100.0).min(100.0);
    let active = match hovered() {
        Some(Hover::Holding(i)) => i == index,
        Some(Hover::Other) => index >= named,
        None => false,
    };
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
            onmouseenter: move |_| hovered.set(Some(Hover::Holding(index))),
            onmouseleave: move |_| hovered.set(None),
            span { class: "flex items-center gap-2",
                span { class: "h-2.5 w-2.5 rounded-full shrink-0 {class}" }
                span { class: "font-medium text-ctp-subtext1", "{ticker}" }
            }
            span { class: "flex items-center gap-2",
                span { class: "h-1 w-16 overflow-hidden rounded-full bg-ctp-surface0",
                    span { class: "block h-full rounded-full {class}", style: "width:{bar:.0}%;" }
                }
                span { class: "w-11 text-right tabular-nums text-ctp-subtext0", "{pct:.1}%" }
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
        assert_eq!(named_count(8), 8);
    }

    #[test]
    fn past_that_the_rest_share_one_grey_slice_and_no_colour_repeats() {
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

    #[test]
    fn every_holding_maps_to_the_slice_it_is_drawn_in() {
        let named = named_count(22);
        assert_eq!(slice_of(0, named), 0);
        assert_eq!(slice_of(6, named), 6);
        // The 15 smaller holdings are all in the grey slice.
        assert_eq!(slice_of(7, named), 7);
        assert_eq!(slice_of(21, named), 7);
    }
}
