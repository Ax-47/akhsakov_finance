//! Diversification card: how spread out the money is (Modern Portfolio
//! Theory concentration metrics), with a score ring and weight bar.

use crate::i18n::tr;
use crate::components::{
    card::{Card, MetricTile},
    color_schema::CHART_COLOR_CLASSES,
};
use crate::format::signed_color;
use crate::hooks::mpt::{ConcentrationRisk, MptAnalysis};
use dioxus::prelude::*;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use types::ticker_symbol::TickerSymbol;

/// `allocation` is only used so segment colours match the allocation card.
#[component]
pub fn MptAnalysisCard(
    mpt: Option<MptAnalysis>,
    allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>,
) -> Element {
    rsx! {
        Card {
            title: tr("Diversification"),
            subtitle: tr("How evenly your money is spread").to_string(),
            if let Some(analysis) = mpt {
                MptBody { analysis, allocation }
            } else {
                p { class: "py-10 text-center text-sm text-ctp-subtext0", {tr("Waiting for live prices…")} }
            }
        }
    }
}

#[component]
fn MptBody(analysis: MptAnalysis, allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>) -> Element {
    let a = &analysis;
    let (top, top_pct) = &a.top_holding;
    let concentration_tone = a.concentration_risk.color();
    let win_tone = if a.win_rate >= dec!(50) {
        "text-ctp-green"
    } else {
        "text-ctp-peach"
    };

    rsx! {
        div { class: "grid gap-6 md:grid-cols-[200px_1fr] md:items-center",
            ScoreRing { score: a.diversification_score }
            div {
                p { class: "text-sm leading-relaxed text-ctp-subtext0 mb-4",
                    "Your {a.live_positions} holdings behave like "
                    span { class: "font-semibold text-ctp-text", "{a.effective_n:.1} equally-sized positions" }
                    ". "
                    if a.concentration_risk == ConcentrationRisk::High {
                        "{top} alone is {top_pct:.0}% of the portfolio."
                    } else {
                        "The largest, {top}, is {top_pct:.0}%."
                    }
                }
                div { class: "grid grid-cols-2 gap-3",
                    MetricTile {
                        label: tr("Effective holdings"),
                        value: format!("{:.1}", a.effective_n),
                        hint: crate::i18n::trf("out of {}", &[&a.live_positions]),
                    }
                    MetricTile {
                        label: tr("Concentration"),
                        value: a.concentration_risk.label().to_string(),
                        hint: crate::i18n::trf("HHI {} · lower is better", &[&format!("{:.3}", a.hhi)]),
                        tone: concentration_tone,
                    }
                    MetricTile {
                        label: tr("Win rate"),
                        value: format!("{:.0}%", a.win_rate),
                        hint: tr("Holdings in profit").to_string(),
                        tone: win_tone,
                    }
                    MetricTile {
                        label: tr("Average return"),
                        value: format!("{:+.2}%", a.weighted_avg_return),
                        hint: format!("Value-weighted · spread ±{:.1}%", a.return_dispersion),
                        tone: signed_color(a.weighted_avg_return),
                    }
                }
            }
        }
        WeightBar { weights: a.weights.clone(), allocation }
    }
}

/// Circular 0–100 gauge.
#[component]
fn ScoreRing(score: Decimal) -> Element {
    const R: f64 = 52.0;
    let score = score.to_f64().unwrap_or(0.0).clamp(0.0, 100.0);
    let circumference = 2.0 * std::f64::consts::PI * R;
    let dash = circumference * score / 100.0;
    let (color, verdict) = match score {
        s if s >= 70.0 => ("var(--catppuccin-color-green)", "Well diversified"),
        s if s >= 40.0 => ("var(--catppuccin-color-yellow)", "Somewhat concentrated"),
        _ => ("var(--catppuccin-color-red)", "Concentrated"),
    };

    rsx! {
        div { class: "flex flex-col items-center",
            div { class: "relative h-40 w-40",
                svg { class: "h-full w-full -rotate-90", view_box: "0 0 128 128",
                    circle { cx: "64", cy: "64", r: "{R}", fill: "none", stroke: "var(--catppuccin-color-surface0)", stroke_width: "10" }
                    circle {
                        cx: "64", cy: "64", r: "{R}",
                        fill: "none",
                        stroke: color,
                        stroke_width: "10",
                        stroke_linecap: "round",
                        stroke_dasharray: "{dash:.1} {circumference:.1}",
                        style: "transition:stroke-dasharray .6s ease;",
                    }
                }
                div { class: "absolute inset-0 flex flex-col items-center justify-center",
                    span { class: "text-4xl font-semibold tabular-nums text-ctp-text", "{score:.0}" }
                    span { class: "text-xs text-ctp-subtext0", "/ 100" }
                }
            }
            span { class: "mt-2 text-sm font-medium", style: "color:{color};", "{verdict}" }
        }
    }
}

/// Stacked bar of portfolio weights with a legend.
#[component]
fn WeightBar(
    weights: Vec<(TickerSymbol, Decimal)>,
    allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>,
) -> Element {
    let mut weights = weights;
    weights.sort_by(|a, b| b.1.cmp(&a.1));
    let color_of = |ticker: &TickerSymbol| {
        allocation
            .read()
            .iter()
            .position(|(t, _)| t == ticker)
            .map(|i| CHART_COLOR_CLASSES[i % CHART_COLOR_CLASSES.len()])
            .unwrap_or("bg-ctp-overlay0")
    };
    let equal = if weights.is_empty() {
        0.0
    } else {
        100.0 / weights.len() as f64
    };

    rsx! {
        div { class: "mt-6",
            div { class: "flex items-center justify-between mb-2 text-xs",
                span { class: "text-ctp-subtext0", {tr("Weights")} }
                span { class: "text-ctp-overlay1", "Equal weight would be {equal:.1}% each" }
            }
            div { class: "flex h-3 gap-0.5 overflow-hidden rounded-full",
                for (ticker, pct) in weights.iter() {
                    div {
                        key: "{ticker}",
                        class: "h-full first:rounded-l-full last:rounded-r-full transition-all {color_of(ticker)}",
                        style: "width:{pct}%;",
                        title: "{ticker}: {pct:.1}%",
                    }
                }
            }
            div { class: "mt-3 flex flex-wrap gap-x-4 gap-y-1.5 text-xs",
                for (ticker, pct) in weights.iter() {
                    span { key: "{ticker}", class: "flex items-center gap-1.5",
                        span { class: "h-2 w-2 rounded-full {color_of(ticker)}" }
                        span { class: "font-medium text-ctp-subtext1", "{ticker}" }
                        span { class: "tabular-nums text-ctp-overlay1", "{pct:.1}%" }
                    }
                }
            }
        }
    }
}
