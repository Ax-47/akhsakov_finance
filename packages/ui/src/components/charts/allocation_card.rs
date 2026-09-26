//! Allocation donut: one slice per holding, with the legend and the donut
//! highlighting together on hover.

use crate::i18n::tr;
use crate::components::{
    card::Card,
    color_schema::{CHART_COLORS_HEX, CHART_COLOR_CLASSES},
};
use dioxus::prelude::*;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::ticker_symbol::TickerSymbol;

const SIZE: f64 = 200.0;
const RADIUS: f64 = 78.0;
const STROKE: f64 = 22.0;
/// Gap between slices, in the same units as the circumference.
const GAP: f64 = 2.5;

/// `allocation` is `(ticker, weight %)`, largest first.
#[component]
pub fn AllocationCard(allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>) -> Element {
    let hovered = use_signal(|| None::<usize>);
    let slices: Vec<(String, f64)> = allocation
        .read()
        .iter()
        .map(|(t, w)| (t.to_string(), w.to_f64().unwrap_or(0.0)))
        .collect();

    rsx! {
        Card { title: tr("Allocation"),
            if slices.is_empty() {
                div { class: "flex items-center justify-center h-24 text-ctp-overlay0", {tr("No data")} }
            } else {
                Donut { slices: slices.clone(), hovered }
                div { class: "mt-5 flex flex-col gap-0.5",
                    for (i, (ticker, pct)) in slices.into_iter().enumerate() {
                        LegendRow { key: "{ticker}", index: i, ticker, pct, hovered }
                    }
                }
            }
        }
    }
}

#[component]
fn Donut(slices: Vec<(String, f64)>, mut hovered: Signal<Option<usize>>) -> Element {
    let circumference = 2.0 * std::f64::consts::PI * RADIUS;
    let total: f64 = slices.iter().map(|s| s.1).sum::<f64>().max(f64::EPSILON);
    let gap = if slices.len() > 1 { GAP } else { 0.0 };
    let center = SIZE / 2.0;

    let mut start = 0.0;
    let arcs: Vec<(usize, f64, f64)> = slices
        .iter()
        .enumerate()
        .map(|(i, (_, pct))| {
            let len = pct / total * circumference;
            let arc = (i, (len - gap).max(0.5), start);
            start += len;
            arc
        })
        .collect();

    let (big, small) = match hovered().and_then(|i| slices.get(i)) {
        Some((ticker, pct)) => (format!("{pct:.1}%"), ticker.clone()),
        None => (slices.len().to_string(), "holdings".to_string()),
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
                        stroke: CHART_COLORS_HEX[i % CHART_COLORS_HEX.len()],
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
                span { class: "text-xs text-ctp-overlay1", "{small}" }
            }
        }
    }
}

#[component]
fn LegendRow(
    index: usize,
    ticker: String,
    pct: f64,
    mut hovered: Signal<Option<usize>>,
) -> Element {
    let color = CHART_COLOR_CLASSES[index % CHART_COLOR_CLASSES.len()];
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
                span { class: "font-medium text-ctp-subtext1", "{ticker}" }
            }
            span { class: "flex items-center gap-2",
                span { class: "h-1 w-16 overflow-hidden rounded-full bg-ctp-surface0",
                    span { class: "block h-full rounded-full {color}", style: "width:{pct.min(100.0):.0}%;" }
                }
                span { class: "w-11 text-right tabular-nums text-ctp-subtext0", "{pct:.1}%" }
            }
        }
    }
}
