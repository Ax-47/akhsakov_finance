use crate::hooks::capm::{compute_capm, CAPMInputs, PortfolioCAPM};
use dioxus::prelude::*;
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use std::collections::HashMap;

// ─── Public component ─────────────────────────────────────────────────────────

/// Full CAPM card.  Drop this anywhere in the dashboard after `MptAnalysisCard`.
///
/// ```rsx
/// CAPMCard {
///     positions: positions.clone(),
///     total_value,
/// }
/// ```
///
/// Beta source priority:
///   1. `beta_map` passed by the caller (populated from price-stream if available).
///   2. User-entered overrides inside this component.
///   3. Default: 1.0 (market beta).
#[component]
pub fn CAPMCard(
    positions: Vec<Position>,
    total_value: Decimal,
    /// Optional betas from the price-stream / backend.
    #[props(default)]
    beta_map: HashMap<String, Decimal>,
) -> Element {
    // ── Local state ────────────────────────────────────────────────────────────
    let mut rf_str = use_signal(|| "5.25".to_string());
    let mut rm_str = use_signal(|| "10.00".to_string());
    // Per-ticker beta override strings (text box content)
    let mut beta_overrides: Signal<HashMap<String, String>> = use_signal(HashMap::new);

    // ── Derive merged beta map and parsed inputs ───────────────────────────────
    let inputs = {
        let rf = rf_str.read().parse::<f64>().unwrap_or(5.25);
        let rm = rm_str.read().parse::<f64>().unwrap_or(10.0);
        CAPMInputs {
            rf: Decimal::try_from(rf).unwrap_or(dec!(5.25)),
            rm: Decimal::try_from(rm).unwrap_or(dec!(10.0)),
        }
    };

    // Merge: backend beta_map → user overrides (user wins)
    let merged_betas: HashMap<String, Decimal> = {
        let mut m = beta_map.clone();
        for (ticker, s) in beta_overrides.read().iter() {
            if let Ok(v) = s.parse::<f64>() {
                if let Ok(d) = Decimal::try_from(v) {
                    m.insert(ticker.clone(), d);
                }
            }
        }
        m
    };

    let capm = compute_capm(&positions, total_value, &merged_betas, &inputs);

    // Unique tickers with live prices for the beta input table
    let live_tickers: Vec<String> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| p.ticker.clone())
        .collect();

    rsx! {
        div { class: "rounded-xl bg-ctp-base border border-ctp-surface0 overflow-hidden",

            // ── Header ────────────────────────────────────────────────────────
            div { class: "flex items-center gap-2 px-4 py-3 border-b border-ctp-surface1",
                span { class: "inline-block w-[3px] h-[14px] rounded-[2px] bg-ctp-sapphire shrink-0" }
                span { class: "text-xs font-bold uppercase tracking-wide", "CAPM Analysis" }
                span { class: "ml-1 text-[10px] text-ctp-overlay0 font-normal",
                    "Capital Asset Pricing Model"
                }
            }

            // ── Parameter row ─────────────────────────────────────────────────
            div { class: "grid grid-cols-2 sm:grid-cols-4 gap-4 px-4 py-3
                          bg-ctp-mantle border-b border-ctp-surface1",
                ParamInput {
                    label: "Risk-Free Rate (Rf)",
                    hint:  "10-yr treasury yield %",
                    value: rf_str.read().clone(),
                    on_input: move |v: String| rf_str.set(v),
                }
                ParamInput {
                    label: "Market Return (Rm)",
                    hint:  "expected annual market return %",
                    value: rm_str.read().clone(),
                    on_input: move |v: String| rm_str.set(v),
                }
                // Derived read-only fields
                div {
                    span { class: "block text-[11px] text-ctp-subtext0 mb-1", "Market Premium" }
                    span { class: "text-sm font-bold text-ctp-peach",
                        "{ inputs.market_premium() :+.2}%",                     }
                    span { class: "block text-[10px] text-ctp-overlay0 mt-0.5", "Rm − Rf" }
                }
                div {
                    span { class: "block text-[11px] text-ctp-subtext0 mb-1", "Beta Inputs" }
                    span { class: "text-sm font-bold text-ctp-lavender",
                        "{live_tickers.len()} tickers"
                    }
                    span { class: "block text-[10px] text-ctp-overlay0 mt-0.5",
                        "edit below · default 1.00"
                    }
                }
            }

            if let Some(ref result) = capm {
                // ── Portfolio summary row ─────────────────────────────────────
                PortfolioSummaryRow { result: result.clone() }

                // ── Beta editor + per-position table side by side ─────────────
                div {
                    class: "grid gap-0",
                    style: "grid-template-columns: 180px 1fr;",

                    // Beta editor panel
                    div { class: "border-r border-ctp-surface1 p-3",
                        div { class: "text-[11px] text-ctp-subtext0 font-semibold uppercase \
                                      tracking-wider mb-2",
                            "Beta (β) Overrides"
                        }
                        for ticker in live_tickers.iter() {
                            BetaInput {
                                key: "{ticker}",
                                ticker: ticker.clone(),
                                stream_beta: beta_map.get(ticker).copied(),
                                value: beta_overrides.read()
                                    .get(ticker)
                                    .cloned()
                                    .unwrap_or_default(),
                                on_input: {
                                    let ticker = ticker.clone();
                                    move |v: String| {
                                        beta_overrides.write().insert(ticker.clone(), v);
                                    }
                                },
                            }
                        }
                    }

                    // Per-position CAPM table
                    PositionTable { positions: result.positions.clone() }
                }

                // ── Alpha bar chart ───────────────────────────────────────────
                AlphaChart { positions: result.positions.clone() }

            } else {
                div { class: "p-6 flex items-center justify-center text-ctp-overlay0 text-xs gap-2",
                    span { class: "animate-pulse", "⏳" }
                    span { "Waiting for live prices…" }
                }
            }
        }
    }
}

// ─── Portfolio summary metrics ────────────────────────────────────────────────

#[component]
fn PortfolioSummaryRow(result: PortfolioCAPM) -> Element {
    let alpha_color = if result.portfolio_alpha >= Decimal::ZERO {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    };

    let beta_color = if result.portfolio_beta < dec!(0.8) {
        "text-ctp-green"
    } else if result.portfolio_beta < dec!(1.2) {
        "text-ctp-blue"
    } else {
        "text-ctp-red"
    };

    rsx! {
        div { class: "grid grid-cols-3 sm:grid-cols-6 gap-px bg-ctp-surface0 \
                      border-b border-ctp-surface1",
            SummaryCell {
                label: "Portfolio Beta",
                value: format!("{:.4}", result.portfolio_beta),
                sub: beta_label(result.portfolio_beta),
                color: beta_color,
            }
            SummaryCell {
                label: "Expected Return",
                value: format!("{:+.2}%", result.portfolio_expected_return),
                sub: "CAPM-implied",
                color: "text-ctp-sapphire",
            }
            SummaryCell {
                label: "Actual Return",
                value: format!("{:+.2}%", result.portfolio_actual_return),
                sub: "unrealized, wtd-avg",
                color: signed_color(result.portfolio_actual_return),
            }
            SummaryCell {
                label: "Jensen's Alpha",
                value: format!("{:+.2}%", result.portfolio_alpha),
                sub: "actual − CAPM",
                color: alpha_color,
            }
            SummaryCell {
                label: "Sharpe Ratio",
                value: result.sharpe_ratio
                    .map(|r| format!("{:.3}", r))
                    .unwrap_or_else(|| "—".to_string()),
                sub: "(r − Rf) / σ",
                color: result.sharpe_ratio
                    .map(|r| signed_color(r))
                    .unwrap_or("text-ctp-overlay0".into()),
            }
            SummaryCell {
                label: "Treynor Ratio",
                value: result.treynor_ratio
                    .map(|r| format!("{:.3}", r))
                    .unwrap_or_else(|| "—".to_string()),
                sub: "(r − Rf) / β",
                color: result.treynor_ratio
                    .map(|r| signed_color(r))
                    .unwrap_or("text-ctp-overlay0".into()),
            }
        }
    }
}

#[component]
fn SummaryCell(label: String, value: String, sub: String, color: String) -> Element {
    rsx! {
        div { class: "bg-ctp-base p-3",
            div { class: "text-[11px] text-ctp-subtext0 mb-1", "{label}" }
            div { class: "text-lg font-bold {color}", "{value}" }
            div { class: "text-[10px] text-ctp-overlay0", "{sub}" }
        }
    }
}

// ─── Per-position table ───────────────────────────────────────────────────────

#[component]
fn PositionTable(positions: Vec<crate::hooks::capm::PositionCAPM>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "w-full text-xs",
                thead {
                    tr { class: "border-b border-ctp-surface1 text-ctp-overlay1",
                        for h in ["Ticker", "Weight", "Beta β", "E(r) CAPM", "Actual r", "α Jensen", "Treynor"] {
                            th {
                                class: "px-3 py-2.5 text-right first:text-left
                                        font-semibold uppercase tracking-wider",
                                "{h}"
                            }
                        }
                    }
                }
                tbody {
                    for pos in &positions {
                        tr {
                            key: "{pos.ticker}",
                            class: "border-b border-ctp-surface1 hover:bg-ctp-surface1
                                    transition-colors",
                            td { class: "px-3 py-2 font-bold text-ctp-blue tracking-wide",
                                "{pos.ticker}"
                            }
                            td { class: "px-3 py-2 text-right tabular-nums text-ctp-subtext0",
                                "{pos.weight * dec!(100):.1}%"
                            }
                            td {
                                class: "px-3 py-2 text-right tabular-nums {beta_color(pos.beta)}" ,
                                "{pos.beta:.2}"
                            }
                            td { class: "px-3 py-2 text-right tabular-nums text-ctp-sapphire",
                                "{pos.expected_return:+.2}%"
                            }
                            td {
                                class: "px-3 py-2 text-right tabular-nums {signed_color(pos.actual_return)}",
                                "{pos.actual_return:+.2}%"
                            }
                            td {
                                class: "px-3 py-2 text-right tabular-nums font-semibold {alpha_color(pos.alpha)}",
                                "{pos.alpha:+.2}%"
                            }
                            td { class: "px-3 py-2 text-right tabular-nums text-ctp-subtext0",
                                if let Some(t) = pos.treynor {
                                    "{t:.3}"
                                } else { "—" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Alpha divergence bar chart ───────────────────────────────────────────────

/// Horizontal diverging bar chart centred at α = 0.
#[component]
fn AlphaChart(positions: Vec<crate::hooks::capm::PositionCAPM>) -> Element {
    // Scale: find max absolute alpha to normalise bar widths.
    let max_abs = positions
        .iter()
        .map(|p| p.alpha.abs())
        .max()
        .unwrap_or(dec!(1));
    let max_f = max_abs.to_f64().unwrap_or(1.0).max(0.01);

    rsx! {
        div { class: "p-4 border-t border-ctp-surface1",
            div { class: "flex items-center justify-between mb-3",
                span { class: "text-[11px] text-ctp-subtext0 font-semibold uppercase tracking-wide",
                    "Jensen's Alpha per Position"
                }
                span { class: "text-[10px] text-ctp-overlay0",
                    "positive = outperforming CAPM expectation"
                }
            }
            div { class: "flex flex-col gap-1.5",
                for pos in &positions {
                    AlphaBar {
                        key: "{pos.ticker}",
                        ticker: pos.ticker.clone(),
                        alpha: pos.alpha,
                        max_abs: max_f,
                    }
                }
            }
        }
    }
}

#[component]
fn AlphaBar(ticker: String, alpha: Decimal, max_abs: f64) -> Element {
    let alpha_f = alpha.to_f64().unwrap_or(0.0);
    let bar_pct = ((alpha_f.abs() / max_abs) * 50.0).min(50.0) as u32;
    let is_pos = alpha_f >= 0.0;
    let bar_color = if is_pos { "bg-ctp-green" } else { "bg-ctp-red" };

    rsx! {
        div { class: "flex items-center gap-2 text-xs",
            // Ticker label
            span { class: "w-12 text-right text-ctp-blue font-bold shrink-0", "{ticker}" }
            // Negative half
            div { class: "flex-1 flex justify-end",
                if !is_pos {
                    div {
                        class: "{bar_color} h-4 rounded-l transition-all",
                        style: format!("width:{bar_pct}%"),
                    }
                }
            }
            // Centre line
            div { class: "w-px h-5 bg-ctp-overlay0 shrink-0" }
            // Positive half
            div { class: "flex-1",
                if is_pos {
                    div {
                        class: "{bar_color} h-4 rounded-r transition-all",
                        style: format!("width:{bar_pct}%"),
                    }
                }
            }
            // Value label
            span { class: "w-14 text-ctp-subtext0 tabular-nums shrink-0",
                "{alpha:+.2}%"
            }
        }
    }
}

// ─── Input helpers ────────────────────────────────────────────────────────────

#[component]
fn ParamInput(
    label: String,
    hint: String,
    value: String,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            span { class: "block text-[11px] text-ctp-subtext0 mb-1", "{label}" }
            input {
                r#type: "number",
                step: "0.01",
                value: "{value}",
                class: "w-full bg-ctp-surface0 border border-ctp-surface1 rounded px-2 py-1 \
                        text-sm text-ctp-text tabular-nums outline-none \
                        focus:border-ctp-sapphire transition-colors",
                oninput: move |e| on_input.call(e.value()),
            }
            span { class: "block text-[10px] text-ctp-overlay0 mt-0.5", "{hint}" }
        }
    }
}

/// Compact beta input for the side-panel, shows stream value as placeholder.
#[component]
fn BetaInput(
    ticker: String,
    stream_beta: Option<Decimal>,
    value: String,
    on_input: EventHandler<String>,
) -> Element {
    let placeholder = stream_beta
        .map(|b| format!("{b:.2}"))
        .unwrap_or_else(|| "1.00".to_string());
    rsx! {
        div { class: "flex items-center gap-2 mb-1.5",
            span { class: "w-10 text-xs font-bold text-ctp-blue shrink-0", "{ticker}" }
            input {
                r#type: "number",
                step: "0.01",
                placeholder: "{placeholder}",
                value: "{value}",
                class: "flex-1 min-w-0 bg-ctp-surface0 border border-ctp-surface1 rounded \
                        px-1.5 py-0.5 text-xs text-ctp-text tabular-nums outline-none \
                        focus:border-ctp-sapphire transition-colors",
                oninput: move |e| on_input.call(e.value()),
            }
        }
    }
}

// ─── Colour / label helpers ───────────────────────────────────────────────────

fn signed_color(v: Decimal) -> String {
    if v >= Decimal::ZERO {
        "text-ctp-green".into()
    } else {
        "text-ctp-red".into()
    }
}

fn alpha_color(v: Decimal) -> String {
    if v >= Decimal::ZERO {
        "text-ctp-green".into()
    } else {
        "text-ctp-red".into()
    }
}

fn beta_color(b: Decimal) -> String {
    if b < dec!(0.8) {
        "text-ctp-teal".into() // defensive
    } else if b < dec!(1.2) {
        "text-ctp-blue".into() // market-neutral
    } else {
        "text-ctp-peach".into() // aggressive
    }
}

fn beta_label(b: Decimal) -> String {
    if b < dec!(0.5) {
        "defensive".into()
    } else if b < dec!(0.8) {
        "below market".into()
    } else if b < dec!(1.2) {
        "market-neutral".into()
    } else if b < dec!(1.5) {
        "above market".into()
    } else {
        "aggressive".into()
    }
}
