//! CAPM card: is each holding earning enough for the market risk (beta) it
//! takes? Plots holdings against the Security Market Line; anything above
//! the line beat what its beta implied.

use crate::components::card::{Card, MetricTile, Stepper};
use crate::format::signed_color;
use crate::hooks::capm::{compute_capm, CAPMInputs, PortfolioCAPM, PositionCAPM};
use dioxus::prelude::*;
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::ticker_symbol::TickerSymbol;

const DEFAULT_RM: &str = "10.00";

/// `beta_map` holds historical betas; missing tickers default to 1.0 and
/// the user can override any beta inline.
#[component]
pub fn CAPMCard(
    positions: Vec<Position>,
    total_value: Decimal,
    #[props(default)] beta_map: HashMap<TickerSymbol, Decimal>,
) -> Element {
    let app_settings = use_context::<crate::app::AppSettings>();
    let default_rf = app_settings.0.read().risk_free.normalize().to_string();
    let mut rf_str = use_signal(move || default_rf);
    let mut rm_str = use_signal(|| DEFAULT_RM.to_string());
    let mut overrides: Signal<HashMap<TickerSymbol, String>> = use_signal(HashMap::new);

    let parse = |s: &str, fallback: Decimal| s.trim().parse::<Decimal>().unwrap_or(fallback);
    let inputs = CAPMInputs {
        rf: parse(&rf_str.read(), dec!(4)),
        rm: parse(&rm_str.read(), dec!(10)),
    };

    // History betas, then user overrides on top.
    let mut betas = beta_map.clone();
    for (ticker, s) in overrides.read().iter() {
        if let Ok(b) = s.trim().parse::<Decimal>() {
            betas.insert(ticker.clone(), b);
        }
    }
    let capm = compute_capm(&positions, total_value, &betas, &inputs);
    let beta_note = if beta_map.is_empty() {
        "Betas default to 1.00 until price history loads · edit any to override"
    } else {
        "Betas from price history vs the S&P 500 · edit any to override"
    };

    rsx! {
        Card {
            title: "CAPM",
            subtitle: "Is each holding earning enough for the market risk it takes?".to_string(),
            actions: rsx! {
                div { class: "flex flex-wrap gap-2",
                    Stepper {
                        label: "Risk-free",
                        suffix: "%",
                        aria_label: "Risk-free rate",
                        value: rf_str(),
                        step: 0.25,
                        on_change: move |v| rf_str.set(v),
                    }
                    Stepper {
                        label: "Market",
                        suffix: "%",
                        aria_label: "Expected market return",
                        value: rm_str(),
                        step: 0.25,
                        on_change: move |v| rm_str.set(v),
                    }
                }
            },
            if let Some(result) = capm {
                Summary { result: result.clone() }
                SecurityMarketLine { result: result.clone() }
                div { class: "-mx-6 mt-6",
                    PositionTable {
                        positions: result.positions.clone(),
                        history_betas: beta_map.clone(),
                        overrides: overrides(),
                        on_override: move |(ticker, value): (TickerSymbol, String)| {
                            overrides.write().insert(ticker, value);
                        },
                    }
                }
                p { class: "mt-3 text-[0.7rem] text-ctp-overlay0", "{beta_note}. Actual return is since purchase." }
            } else {
                p { class: "py-10 text-center text-sm text-ctp-overlay1", "Waiting for live prices…" }
            }
        }
    }
}

// ─── Summary ──────────────────────────────────────────────────────────────────

#[component]
fn Summary(result: PortfolioCAPM) -> Element {
    let r = &result;
    let treynor = r
        .treynor_ratio
        .map(|t| format!("Treynor {t:.2}"))
        .unwrap_or_default();
    let alpha_hint = if treynor.is_empty() {
        "Actual − expected".to_string()
    } else {
        format!("Actual − expected · {treynor}")
    };
    rsx! {
        div { class: "grid grid-cols-2 lg:grid-cols-4 gap-3",
            MetricTile {
                label: "Portfolio beta",
                value: format!("{:.2}", r.portfolio_beta),
                hint: beta_label(r.portfolio_beta).to_string(),
            }
            MetricTile {
                label: "Expected return",
                value: format!("{:+.2}%", r.portfolio_expected_return),
                hint: format!("Rf + β × {:.2}% premium", r.inputs.market_premium()),
            }
            MetricTile {
                label: "Actual return",
                value: format!("{:+.2}%", r.portfolio_actual_return),
                hint: "Value-weighted, since purchase".to_string(),
                tone: signed_color(r.portfolio_actual_return),
            }
            MetricTile {
                label: "Alpha",
                value: format!("{:+.2}%", r.portfolio_alpha),
                hint: alpha_hint,
                tone: signed_color(r.portfolio_alpha),
            }
        }
    }
}

// ─── Security Market Line ─────────────────────────────────────────────────────

const W: f64 = 600.0;
const H: f64 = 280.0;
const LEFT: f64 = 44.0;
const RIGHT: f64 = 16.0;
const TOP: f64 = 14.0;
const BOTTOM: f64 = 30.0;

const GAIN_HEX: &str = "#a6e3a1";
const LOSS_HEX: &str = "#f38ba8";
const LINE_HEX: &str = "#cba6f7";
const GRID_HEX: &str = "#313244";
const LABEL_HEX: &str = "#7f849c";

#[component]
fn SecurityMarketLine(result: PortfolioCAPM) -> Element {
    let f = |d: Decimal| d.to_f64().unwrap_or(0.0);
    let rf = f(result.inputs.rf);
    let premium = f(result.inputs.market_premium());
    let sml = |beta: f64| rf + beta * premium;

    let betas: Vec<f64> = result.positions.iter().map(|p| f(p.beta)).collect();
    let x_max = betas
        .iter()
        .cloned()
        .chain([f(result.portfolio_beta), 1.5])
        .fold(0.0_f64, f64::max)
        * 1.15;

    let ys: Vec<f64> = result
        .positions
        .iter()
        .map(|p| f(p.actual_return))
        .chain([f(result.portfolio_actual_return), rf, sml(x_max), 0.0])
        .collect();
    let (lo, hi) = ys
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), y| (lo.min(*y), hi.max(*y)));
    let pad = ((hi - lo) * 0.1).max(1.0);
    let (y_lo, y_hi) = (lo - pad, hi + pad);

    let px = move |beta: f64| LEFT + beta / x_max * (W - LEFT - RIGHT);
    let py = move |ret: f64| TOP + (y_hi - ret) / (y_hi - y_lo) * (H - TOP - BOTTOM);

    let (x0, y0, x1, y1) = (px(0.0), py(sml(0.0)), px(x_max), py(sml(x_max)));
    let (plot_top, plot_bottom) = (TOP, H - BOTTOM);
    let above = format!("{x0:.1},{y0:.1} {x1:.1},{y1:.1} {x1:.1},{plot_top} {x0:.1},{plot_top}");
    let below =
        format!("{x0:.1},{y0:.1} {x1:.1},{y1:.1} {x1:.1},{plot_bottom} {x0:.1},{plot_bottom}");

    let y_ticks: Vec<f64> = (0..=4)
        .map(|i| y_lo + (y_hi - y_lo) * i as f64 / 4.0)
        .collect();
    let x_ticks: Vec<f64> = (0..)
        .map(|i| i as f64 * 0.5)
        .take_while(|b| *b <= x_max)
        .collect();

    let dots: Vec<(String, f64, f64, f64, f64, f64)> = result
        .positions
        .iter()
        .map(|p| {
            let beta = f(p.beta);
            (
                p.ticker.to_string(),
                beta,
                px(beta),
                py(f(p.actual_return)),
                py(sml(beta)),
                f(p.weight) * 100.0,
            )
        })
        .collect();
    let (port_x, port_y) = (
        px(f(result.portfolio_beta)),
        py(f(result.portfolio_actual_return)),
    );
    let (mkt_x, mkt_y) = (px(1.0), py(sml(1.0)));

    rsx! {
        div { class: "mt-6",
            div { class: "flex flex-wrap items-center justify-between gap-2 mb-2 text-xs",
                span { class: "text-ctp-overlay1", "Return vs beta" }
                span { class: "flex gap-4 text-ctp-overlay0",
                    span { class: "flex items-center gap-1.5",
                        span { class: "h-2 w-2 rounded-full", style: "background:{GAIN_HEX};" }
                        "beat the line"
                    }
                    span { class: "flex items-center gap-1.5",
                        span { class: "h-2 w-2 rounded-full", style: "background:{LOSS_HEX};" }
                        "fell short"
                    }
                }
            }
            svg { class: "w-full", view_box: "0 0 {W} {H}",
                // Zones either side of the line.
                polygon { points: "{above}", fill: GAIN_HEX, fill_opacity: "0.05" }
                polygon { points: "{below}", fill: LOSS_HEX, fill_opacity: "0.05" }

                for y in y_ticks {
                    line { x1: "{LEFT}", x2: "{W - RIGHT}", y1: "{py(y):.1}", y2: "{py(y):.1}", stroke: GRID_HEX, stroke_dasharray: "3 5" }
                    text { x: "{LEFT - 8.0}", y: "{py(y) + 4.0:.1}", text_anchor: "end", font_size: "10", fill: LABEL_HEX, "{y:.0}%" }
                }
                for b in x_ticks {
                    text { x: "{px(b):.1}", y: "{H - 10.0}", text_anchor: "middle", font_size: "10", fill: LABEL_HEX, "β {b:.1}" }
                }

                // Security Market Line.
                line { x1: "{x0:.1}", y1: "{y0:.1}", x2: "{x1:.1}", y2: "{y1:.1}", stroke: LINE_HEX, stroke_width: "2", stroke_linecap: "round" }
                text { x: "{x1 - 4.0:.1}", y: "{y1 - 8.0:.1}", text_anchor: "end", font_size: "10", fill: LINE_HEX, "Security market line" }

                // Market reference point.
                circle { cx: "{mkt_x:.1}", cy: "{mkt_y:.1}", r: "4", fill: "#1e1e2e", stroke: LINE_HEX, stroke_width: "2" }
                text { x: "{mkt_x:.1}", y: "{mkt_y + 16.0:.1}", text_anchor: "middle", font_size: "10", fill: LABEL_HEX, "Market" }

                // Holdings, with a stem to the line showing alpha.
                for (ticker, beta, x, y, y_line, weight) in dots {
                    g { key: "{ticker}",
                        line {
                            x1: "{x:.1}", x2: "{x:.1}", y1: "{y:.1}", y2: "{y_line:.1}",
                            stroke: if y <= y_line { GAIN_HEX } else { LOSS_HEX },
                            stroke_opacity: "0.5", stroke_dasharray: "2 3",
                        }
                        circle {
                            cx: "{x:.1}", cy: "{y:.1}", r: "{5.0 + weight.max(0.0).sqrt() * 1.2:.1}",
                            fill: if y <= y_line { GAIN_HEX } else { LOSS_HEX },
                            fill_opacity: "0.3",
                            stroke: if y <= y_line { GAIN_HEX } else { LOSS_HEX },
                            stroke_width: "1.5",
                        }
                        text { x: "{x + 12.0:.1}", y: "{y + 4.0:.1}", font_size: "11", font_weight: "600", fill: "#cdd6f4", "{ticker}" }
                        title { "{ticker}: β {beta:.2}, weight {weight:.1}%" }
                    }
                }

                // Whole portfolio.
                circle { cx: "{port_x:.1}", cy: "{port_y:.1}", r: "9", fill: "none", stroke: LINE_HEX, stroke_width: "2.5" }
                circle { cx: "{port_x:.1}", cy: "{port_y:.1}", r: "3", fill: LINE_HEX }
                text { x: "{port_x - 14.0:.1}", y: "{port_y + 4.0:.1}", text_anchor: "end", font_size: "11", font_weight: "700", fill: LINE_HEX, "Portfolio" }
            }
        }
    }
}

// ─── Table ────────────────────────────────────────────────────────────────────

#[component]
fn PositionTable(
    positions: Vec<PositionCAPM>,
    history_betas: HashMap<TickerSymbol, Decimal>,
    overrides: HashMap<TickerSymbol, String>,
    on_override: EventHandler<(TickerSymbol, String)>,
) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "w-full text-sm whitespace-nowrap",
                thead {
                    tr { class: "text-xs text-ctp-overlay1",
                        th { class: "pl-6 pr-4 py-2.5 text-left font-medium", "Asset" }
                        th { class: "px-4 py-2.5 text-right font-medium", "Weight" }
                        th { class: "px-4 py-2.5 text-right font-medium", "Beta" }
                        th { class: "px-4 py-2.5 text-right font-medium", "Expected" }
                        th { class: "px-4 py-2.5 text-right font-medium", "Actual" }
                        th { class: "pl-4 pr-6 py-2.5 text-right font-medium", "Alpha" }
                    }
                }
                tbody {
                    for p in positions {
                        tr {
                            key: "{p.ticker}",
                            class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
                            td { class: "pl-6 pr-4 py-3 font-semibold text-ctp-text", "{p.ticker}" }
                            td { class: "px-4 py-3 text-right tabular-nums text-ctp-subtext0", "{p.weight * dec!(100):.1}%" }
                            td { class: "px-4 py-2 text-right",
                                Stepper {
                                    aria_label: "Beta for {p.ticker}",
                                    placeholder: format!("{:.2}", history_betas.get(&p.ticker).copied().unwrap_or(Decimal::ONE)),
                                    value: overrides.get(&p.ticker).cloned().unwrap_or_default(),
                                    step: 0.05,
                                    on_change: {
                                        let ticker = p.ticker.clone();
                                        move |v: String| on_override.call((ticker.clone(), v))
                                    },
                                }
                            }
                            td { class: "px-4 py-3 text-right tabular-nums text-ctp-subtext0", "{p.expected_return:+.2}%" }
                            td { class: "px-4 py-3 text-right tabular-nums {signed_color(p.actual_return)}", "{p.actual_return:+.2}%" }
                            td { class: "pl-4 pr-6 py-3 text-right",
                                span {
                                    class: if p.alpha >= Decimal::ZERO {
                                        "rounded-full px-2.5 py-0.5 text-xs font-semibold tabular-nums bg-ctp-green/15 text-ctp-green"
                                    } else {
                                        "rounded-full px-2.5 py-0.5 text-xs font-semibold tabular-nums bg-ctp-red/15 text-ctp-red"
                                    },
                                    "{p.alpha:+.2}%"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn beta_label(b: Decimal) -> &'static str {
    match b {
        b if b < dec!(0.5) => "Defensive",
        b if b < dec!(0.8) => "Below market",
        b if b < dec!(1.2) => "Moves with the market",
        b if b < dec!(1.5) => "Above market",
        _ => "Aggressive",
    }
}
