use crate::hooks::mpt::{ConcentrationRisk, MptAnalysis};
use dioxus::prelude::*;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;

// ─── Palette for the weight bar chart ─────────────────────────────────────────

const BAR_COLORS: &[&str] = &[
    "bg-ctp-blue",
    "bg-ctp-mauve",
    "bg-ctp-green",
    "bg-ctp-peach",
    "bg-ctp-teal",
    "bg-ctp-sapphire",
    "bg-ctp-yellow",
    "bg-ctp-lavender",
    "bg-ctp-pink",
    "bg-ctp-red",
];

// ─── Public component ─────────────────────────────────────────────────────────

/// Renders a full-width MPT analysis card.
/// Pass the `Option<MptAnalysis>` from `PortfolioState`; the card renders
/// a loading / unavailable state automatically when it is `None`.
#[component]
pub fn MptAnalysisCard(mpt: Option<MptAnalysis>) -> Element {
    rsx! {
        div { class: "rounded-xl bg-ctp-base border border-ctp-surface0 overflow-hidden",

            // ── Header ────────────────────────────────────────────────────────
            div {
                class: "flex items-center gap-2 px-4 py-3 border-b border-ctp-surface1",
                span { class: "inline-block w-[3px] h-[14px] rounded-[2px] bg-ctp-mauve shrink-0" }
                span { class: "text-xs font-bold uppercase tracking-wide", "Portfolio Analysis" }
                span { class: "ml-1 text-[10px] text-ctp-overlay0 font-normal normal-case tracking-normal",
                    "Modern Portfolio Theory"
                }
                span { class: "ml-auto text-[10px] text-ctp-overlay0",
                    if let Some(ref a) = mpt {
                        "{a.live_positions} positions analysed"
                    } else {
                        "awaiting live prices…"
                    }
                }
            }

            if let Some(analysis) = mpt {
                MptBody { analysis }
            } else {
                // Skeleton / placeholder while prices load
                div {
                    class: "p-6 flex items-center justify-center text-ctp-overlay0 text-xs gap-2",
                    span { class: "animate-pulse", "⏳" }
                    span { "Waiting for live prices to compute MPT metrics…" }
                }
            }
        }
    }
}

// ─── Body (only shown when analysis is ready) ─────────────────────────────────

#[component]
fn MptBody(analysis: MptAnalysis) -> Element {
    let score = analysis.diversification_score;
    let score_color = score_to_color(score);

    rsx! {
        // ── Metric grid ───────────────────────────────────────────────────────
        div { class: "grid grid-cols-2 sm:grid-cols-4 gap-px bg-ctp-surface0",

            MptMetric {
                label: "Diversification Score",
                value: format!("{score:.0} / 100"),
                sub: "vs equal-weight ideal",
                color: score_color,
                icon: "⬡",
            }
            MptMetric {
                label: "Effective Positions",
                value: "{analysis.effective_n:.1}",
                sub: "HHI {analysis.hhi:.4}",
                color: "text-ctp-sapphire",
                icon: "◈",
            }
            MptMetric {
                label: "Concentration Risk",
                value: analysis.concentration_risk.label(),
                sub: "top: {analysis.top_holding.0} @ {analysis.top_holding.1:.1}%",
                color: analysis.concentration_risk.color(),
                icon: "⚡",
            }
            MptMetric {
                label: "Win Rate",
                value: "{analysis.win_rate:.1}%",
                sub: "positions in profit",
                color: win_rate_color(analysis.win_rate),
                icon: "✦",
            }
        }

        div { class: "grid grid-cols-2 sm:grid-cols-3 gap-px bg-ctp-surface0",
            MptMetric {
                label: "Wtd. Avg. Return",
                value: "{analysis.weighted_avg_return:+.2}%",
                sub: "unrealized, mkt-value weighted",
                color: signed_color(analysis.weighted_avg_return),
                icon: if analysis.weighted_avg_return >= Decimal::ZERO { "▲" } else { "▼" },
            }
            MptMetric {
                label: "Return Dispersion",
                value: "{analysis.return_dispersion:.2}%",
                sub: "cross-sectional σ of returns",
                color: "text-ctp-lavender",
                icon: "σ",
            }
            div { class: "col-span-2 sm:col-span-1 bg-ctp-base p-4",
                ConcentrationGauge {
                    risk: analysis.concentration_risk.clone(),
                    hhi: analysis.hhi,
                }
            }
        }

        // ── Weight distribution bar chart ─────────────────────────────────────
        WeightChart { weights: analysis.weights.clone() }
    }
}

// ─── Sub-components ───────────────────────────────────────────────────────────

#[component]
fn MptMetric(label: String, value: String, sub: String, color: String, icon: String) -> Element {
    rsx! {
        div { class: "bg-ctp-base p-4",
            div { class: "flex items-center justify-between mb-2",
                span { class: "text-[11px] text-ctp-subtext0 font-medium", "{label}" }
                span { class: "{color} text-base leading-none", "{icon}" }
            }
            div { class: "text-xl font-bold text-ctp-text", "{value}" }
            div { class: "text-[11px] text-ctp-overlay0 mt-0.5 truncate", "{sub}" }
        }
    }
}

/// Visual gauge showing where the portfolio sits on the Low→High concentration
/// spectrum, using a segmented progress bar.
#[component]
fn ConcentrationGauge(risk: ConcentrationRisk, hhi: Decimal) -> Element {
    // Map HHI (0.05 … 1.0) → 0-100% for the needle position.
    // Clamp to [0.05, 1.0] so the needle stays within the bar.
    let hhi_f = hhi.to_f64().unwrap_or(0.0).clamp(0.05, 1.0);
    let needle_pct = ((hhi_f - 0.05) / 0.95 * 100.0) as u32;
    let risk_color = risk.color();
    let risk_label = risk.label();

    rsx! {
        div {
            div { class: "flex items-center justify-between mb-1",
                span { class: "text-[11px] text-ctp-subtext0 font-medium", "Concentration Gauge" }
                span { class: "{risk_color} text-[11px] font-semibold", "{risk_label}" }
            }
            // Segmented bar: green / yellow / red
            div { class: "relative h-2.5 rounded-full overflow-hidden flex",
                div { class: "h-full bg-ctp-green", style: "width:33%" }
                div { class: "h-full bg-ctp-yellow", style: "width:34%" }
                div { class: "h-full bg-ctp-red",   style: "width:33%" }
                // Needle
                div {
                    class: "absolute top-0 h-full w-0.5 bg-ctp-text rounded",
                    style: format!("left:{}%", needle_pct),
                }
            }
            div { class: "flex justify-between text-[10px] text-ctp-overlay0 mt-1",
                span { "Low (<0.15)" }
                span { "Mod" }
                span { "High (>0.25)" }
            }
        }
    }
}

/// Stacked horizontal bar showing each position's share of the portfolio.
#[component]
fn WeightChart(weights: Vec<(String, Decimal)>) -> Element {
    rsx! {
        div { class: "p-4 border-t border-ctp-surface1",
            div { class: "flex items-center justify-between mb-2",
                span { class: "text-[11px] text-ctp-subtext0 font-medium uppercase tracking-wide",
                    "Weight Distribution"
                }
                span { class: "text-[10px] text-ctp-overlay0", "% of portfolio value" }
            }

            // Stacked bar
            div { class: "flex h-4 rounded-full overflow-hidden gap-px mb-3",
                for (i, (ticker, pct)) in weights.iter().enumerate() {
                    {
                        let bar_cls = BAR_COLORS.get(i).copied().unwrap_or("bg-ctp-overlay0");
                        rsx! {
                            div {
                                key: "{ticker}",
                                class: "{bar_cls} h-full transition-all",
                                style: format!("width:{pct}%"),
                                title: format!("{ticker}: {pct:.1}%"),
                            }
                        }
                    }
                }
            }

            // Legend
            div { class: "flex flex-wrap gap-x-4 gap-y-1",
                for (i, (ticker, pct)) in weights.iter().enumerate() {
                    {
                        let dot_cls = BAR_COLORS.get(i).copied().unwrap_or("bg-ctp-overlay0");
                        rsx! {
                            div { class: "flex items-center gap-1.5",
                                key: "{ticker}",
                                span { class: "inline-block w-2 h-2 rounded-sm {dot_cls}" }
                                span { class: "text-[11px] text-ctp-subtext1 font-medium", "{ticker}" }
                                span { class: "text-[11px] text-ctp-overlay0", "{pct:.1}%" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Colour helpers ───────────────────────────────────────────────────────────

fn score_to_color(score: Decimal) -> &'static str {
    if score >= dec!(70) {
        "text-ctp-green"
    } else if score >= dec!(40) {
        "text-ctp-yellow"
    } else {
        "text-ctp-red"
    }
}

fn win_rate_color(rate: Decimal) -> &'static str {
    if rate >= dec!(66) {
        "text-ctp-green"
    } else if rate >= dec!(40) {
        "text-ctp-yellow"
    } else {
        "text-ctp-red"
    }
}

fn signed_color(v: Decimal) -> &'static str {
    if v >= Decimal::ZERO {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    }
}
