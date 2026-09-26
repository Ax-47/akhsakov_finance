//! History-based risk analysis: how much the portfolio swings, how bad a
//! bad day gets, which holdings drive the risk, and how it compares with
//! the S&P 500. Uses today's weights applied to past daily returns.

use crate::i18n::tr;
use super::stats::{daily_returns, day_label, RiskReport};
use crate::hooks::mpt::MptAnalysis;
use crate::{
    components::{
        analysis::{CAPMCard, MptAnalysisCard},
        card::{Card, MetricTile, Segmented, ToggleButton},
        charts::CorrelationGraph,
        color_schema::CHART_COLOR_CLASSES,
    },
    format::fmt_usd,
};
use api::quote::quote::get_charts;
use dioxus::prelude::*;
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::HashMap;
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

const PORTFOLIO_HEX: &str = "var(--catppuccin-color-mauve)"; // mauve
const MARKET_HEX: &str = "var(--catppuccin-color-overlay2)"; // overlay2
const LOSS_HEX: &str = "var(--catppuccin-color-red)"; // red

#[derive(Clone, Copy, PartialEq)]
enum Grade {
    Low,
    Moderate,
    High,
}

impl Grade {
    fn from_volatility(vol: f64) -> Self {
        match vol {
            v if v < 0.12 => Self::Low,
            v if v < 0.22 => Self::Moderate,
            _ => Self::High,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Low => tr("Low risk"),
            Self::Moderate => tr("Moderate risk"),
            Self::High => tr("High risk"),
        }
    }

    fn pill(self) -> &'static str {
        match self {
            Self::Low => "bg-ctp-green/15 text-ctp-green",
            Self::Moderate => "bg-ctp-yellow/15 text-ctp-yellow",
            Self::High => "bg-ctp-red/15 text-ctp-red",
        }
    }
}

/// The portfolio page's Risk tab: risk overview, performance vs the market,
/// stress test, risk contribution, relations, diversification and CAPM.
/// Price history is fetched once here and shared, so CAPM uses real betas.
/// `allocation` is `(ticker, weight %)`.
#[component]
pub fn RiskTab(
    allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>,
    positions: Vec<Position>,
    total_value: Decimal,
    mpt: Option<MptAnalysis>,
) -> Element {
    let mut range = use_signal(|| Range::Y1);

    // Refetch only when the set of tickers changes, not on price ticks.
    let tickers = use_memo(move || {
        allocation
            .read()
            .iter()
            .map(|(t, _)| t.clone())
            .collect::<Vec<_>>()
    });

    let app_settings = use_context::<crate::app::AppSettings>();
    let history = crate::cache::use_cached(
        move || format!("risk/{:?}/{:?}/{}", tickers(), range(), app_settings.benchmark()),
        move || {
        let tickers = tickers();
        let range = range();
        async move {
            let market = app_settings.benchmark();
            let mut symbols = tickers.clone();
            symbols.push(market.clone());
            let charts = get_charts(symbols, range, Interval::D1, false).await.ok()?;
            let holdings = tickers
                .iter()
                .map(|t| charts.get(t).map(|c| daily_returns(c)).unwrap_or_default())
                .collect::<Vec<_>>();
            Some((holdings, daily_returns(charts.get(&market)?)))
        }
    },
    );

    let report = use_memo(move || {
        let weights: Vec<f64> = allocation
            .read()
            .iter()
            .map(|(_, w)| w.to_f64().unwrap_or(0.0))
            .collect();
        let history = history.read();
        // Outer None: still loading. Inner None: not enough data.
        let loaded = history.as_ref()?;
        Some(
            loaded
                .as_ref()
                .and_then(|(h, m)| RiskReport::compute(h, &weights, m, app_settings.risk_free())),
        )
    });

    let value = total_value.to_f64().unwrap_or(0.0);
    let period = if range() == Range::M3 {
        "3 months"
    } else {
        "year"
    };
    let range_toggle = rsx! {
        Segmented {
            ToggleButton { label: "3M", active: range() == Range::M3, onclick: move |_| range.set(Range::M3) }
            ToggleButton { label: "1Y", active: range() == Range::Y1, onclick: move |_| range.set(Range::Y1) }
        }
    };

    // Historical betas for CAPM (index-aligned with `tickers`).
    let betas: HashMap<TickerSymbol, Decimal> = match report() {
        Some(Some(r)) => tickers()
            .into_iter()
            .zip(&r.holdings)
            .filter_map(|(t, h)| Some((t, Decimal::try_from(h.beta).ok()?.round_dp(2))))
            .collect(),
        _ => HashMap::new(),
    };

    let risk = match report() {
        None => rsx! {
            Card { title: tr("Risk overview"), actions: range_toggle,
                p { class: "py-10 text-center text-sm text-ctp-subtext0", {tr("Crunching price history…")} }
            }
        },
        Some(None) => rsx! {
            Card { title: tr("Risk overview"), actions: range_toggle,
                p { class: "py-10 text-center text-sm text-ctp-subtext0",
                    {tr("Not enough shared price history yet to measure risk.")}
                }
            }
        },
        Some(Some(report)) => rsx! {
            RiskOverview { report: report.clone(), value, period, actions: range_toggle }
            div { class: "grid gap-5 lg:grid-cols-[1.4fr_1fr]",
                PerformanceChart { report: report.clone() }
                StressTest { report: report.clone(), value }
            }
            RiskContribution { report, tickers: tickers() }
        },
    };

    rsx! {
        {risk}
        CorrelationGraph { allocation }
        MptAnalysisCard { mpt, allocation }
        CAPMCard { positions, total_value, beta_map: betas }
    }
}

// ─── Overview ─────────────────────────────────────────────────────────────────

#[component]
fn RiskOverview(report: RiskReport, value: f64, period: &'static str, actions: Element) -> Element {
    let grade = Grade::from_volatility(report.volatility);
    let r = &report;
    let pct = |x: f64| x * 100.0;
    let usd = |x: f64| fmt_usd(Decimal::try_from(x).unwrap_or_default(), 0);

    let vs_market = if r.volatility > r.market_volatility * 1.1 {
        "more volatile than"
    } else if r.volatility < r.market_volatility * 0.9 {
        "calmer than"
    } else {
        "about as volatile as"
    };

    rsx! {
        Card {
            title: tr("Risk overview"),
            subtitle: crate::i18n::trf("{} trading days · {} – {}", &[&r.days, &day_label(r.first_day), &day_label(r.last_day)]),
            actions,
            div { class: "flex flex-col sm:flex-row sm:items-start gap-4 mb-6",
                span { class: "self-start shrink-0 rounded-full px-3 py-1 text-sm font-semibold {grade.pill()}",
                    "{grade.label()}"
                }
                p { class: "text-sm leading-relaxed text-ctp-subtext0",
                    "Over the last {period} your portfolio returned "
                    Strong { "{pct(r.annual_return):+.1}% a year" }
                    " with "
                    Strong { "{pct(r.volatility):.0}% volatility" }
                    ", {vs_market} the S&P 500 ({pct(r.market_return):+.1}% / {pct(r.market_volatility):.0}%). "
                    {tr("On a bad day — 1 in 20 — expect to lose about ")}
                    Strong { "{usd(r.var_95 * value)}" }
                    " or more."
                }
            }
            div { class: "grid grid-cols-2 lg:grid-cols-4 gap-3",
                MetricTile {
                    label: tr("Volatility"),
                    value: format!("{:.1}%", pct(r.volatility)),
                    hint: crate::i18n::trf("Yearly swing · S&P {}%", &[&format!("{:.1}", pct(r.market_volatility))]),
                    tone: tone(r.volatility <= r.market_volatility),
                }
                MetricTile {
                    label: tr("Max drawdown"),
                    value: format!("−{:.1}%", pct(r.max_drawdown)),
                    hint: crate::i18n::trf("Worst peak-to-trough · S&P −{}%", &[&format!("{:.1}", pct(r.market_max_drawdown))]),
                    tone: tone(r.max_drawdown <= r.market_max_drawdown),
                }
                MetricTile {
                    label: tr("Value at risk (95%)"),
                    value: format!("−{:.2}%", pct(r.var_95)),
                    hint: format!("1 in 20 days: ≥ {} loss", usd(r.var_95 * value)),
                }
                MetricTile {
                    label: tr("Expected shortfall"),
                    value: format!("−{:.2}%", pct(r.cvar_95)),
                    hint: crate::i18n::trf("Average bad day: {}", &[&usd(r.cvar_95 * value)]),
                }
                MetricTile {
                    label: tr("Sharpe ratio"),
                    value: format!("{:.2}", r.sharpe),
                    hint: crate::i18n::trf("Return per risk · Sortino {}", &[&format!("{:.2}", r.sortino)]),
                    tone: tone(r.sharpe >= 1.0),
                }
                MetricTile {
                    label: tr("Beta"),
                    value: format!("{:.2}", r.beta),
                    hint: format!("Moves {:.1}× the S&P · corr {:.2}", r.beta, r.market_correlation),
                }
                MetricTile {
                    label: tr("Alpha"),
                    value: format!("{:+.1}%", pct(r.alpha)),
                    hint: tr("Yearly return beyond beta").to_string(),
                    tone: tone(r.alpha >= 0.0),
                }
                MetricTile {
                    label: tr("Diversification"),
                    value: format!("{:.2}×", r.diversification_ratio),
                    hint: tr("1.0× = no benefit · higher is better").to_string(),
                    tone: tone(r.diversification_ratio >= 1.2),
                }
            }
            p { class: "mt-4 text-xs text-ctp-overlay1",
                "Based on today's weights applied to past daily returns. Risk-free rate {pct(r.risk_free):.1}% (change it in Settings)."
            }
        }
    }
}

#[component]
fn Strong(children: Element) -> Element {
    rsx! { span { class: "font-semibold text-ctp-text", {children} } }
}

fn tone(good: bool) -> &'static str {
    if good {
        "text-ctp-green"
    } else {
        "text-ctp-peach"
    }
}

// ─── Performance chart ────────────────────────────────────────────────────────

const CHART_W: f64 = 600.0;
const CHART_H: f64 = 200.0;
const DD_H: f64 = 60.0;

/// Growth of $100 for the portfolio vs the S&P 500, with a drawdown strip.
#[component]
fn PerformanceChart(report: RiskReport) -> Element {
    let g = &report.growth;
    let (lo, hi) = g.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, p, m)| {
        (lo.min(*p).min(*m), hi.max(*p).max(*m))
    });
    let pad = ((hi - lo) * 0.08).max(0.01);
    let (lo, hi) = (lo - pad, hi + pad);
    let x = |i: usize| i as f64 / (g.len().max(2) - 1) as f64 * CHART_W;
    let y = |v: f64| CHART_H - (v - lo) / (hi - lo) * CHART_H;

    let path = |pick: fn(&(i64, f64, f64)) -> f64| {
        g.iter()
            .enumerate()
            .map(|(i, p)| format!("{:.1},{:.1}", x(i), y(pick(p))))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let port_line = path(|p| p.1);
    let market_line = path(|p| p.2);
    let port_area = format!("0,{CHART_H} {port_line} {CHART_W},{CHART_H}");

    let mut peak = 1.0_f64;
    let dd: Vec<f64> = g
        .iter()
        .map(|(_, p, _)| {
            peak = peak.max(*p);
            p / peak - 1.0
        })
        .collect();
    let dd_floor = dd.iter().cloned().fold(0.0_f64, f64::min).min(-0.01);
    let dd_area = format!(
        "0,0 {} {CHART_W},0",
        dd.iter()
            .enumerate()
            .map(|(i, d)| format!("{:.1},{:.1}", x(i), d / dd_floor * DD_H))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let (port_end, market_end) = g.last().map(|(_, p, m)| (*p, *m)).unwrap_or((1.0, 1.0));
    let base_y = y(1.0);

    rsx! {
        Card { title: tr("Performance"), subtitle: tr("Growth of $100 vs the S&P 500").to_string(),
            div { class: "flex flex-wrap gap-x-5 gap-y-1 mb-3 text-xs",
                LegendDot { color: PORTFOLIO_HEX, label: format!("Portfolio ${:.0}", port_end * 100.0) }
                LegendDot { color: MARKET_HEX, label: format!("S&P 500 ${:.0}", market_end * 100.0) }
            }
            svg { class: "w-full", view_box: "0 0 {CHART_W} {CHART_H}", preserve_aspect_ratio: "none",
                defs {
                    linearGradient { id: "perf-fill", x1: "0", y1: "0", x2: "0", y2: "1",
                        stop { offset: "0%", stop_color: PORTFOLIO_HEX, stop_opacity: "0.25" }
                        stop { offset: "100%", stop_color: PORTFOLIO_HEX, stop_opacity: "0" }
                    }
                }
                line {
                    x1: "0", x2: "{CHART_W}", y1: "{base_y:.1}", y2: "{base_y:.1}",
                    stroke: "var(--catppuccin-color-surface1)", stroke_dasharray: "4 4", vector_effect: "non-scaling-stroke",
                }
                polygon { points: "{port_area}", fill: "url(#perf-fill)" }
                polyline {
                    points: "{market_line}", fill: "none", stroke: MARKET_HEX,
                    stroke_width: "1.5", vector_effect: "non-scaling-stroke",
                }
                polyline {
                    points: "{port_line}", fill: "none", stroke: PORTFOLIO_HEX,
                    stroke_width: "2", stroke_linejoin: "round", vector_effect: "non-scaling-stroke",
                }
            }
            div { class: "mt-4 mb-1 flex justify-between text-xs text-ctp-subtext0",
                span { {tr("Drawdown")} }
                span { class: "text-ctp-red", "worst −{-dd_floor * 100.0:.1}%" }
            }
            svg { class: "w-full h-12", view_box: "0 0 {CHART_W} {DD_H}", preserve_aspect_ratio: "none",
                polygon { points: "{dd_area}", fill: LOSS_HEX, fill_opacity: "0.25" }
            }
            div { class: "mt-2 flex justify-between text-xs text-ctp-overlay1",
                span { "{day_label(report.first_day)}" }
                span { "{day_label(report.last_day)}" }
            }
        }
    }
}

#[component]
fn LegendDot(color: &'static str, label: String) -> Element {
    rsx! {
        span { class: "flex items-center gap-1.5 text-ctp-subtext0",
            span { class: "h-2 w-2 rounded-full", style: "background:{color};" }
            "{label}"
        }
    }
}

// ─── Stress test ──────────────────────────────────────────────────────────────

#[component]
fn StressTest(report: RiskReport, value: f64) -> Element {
    let shocks = [0.10, 0.20, 0.30].map(|drop| {
        (
            format!("S&P 500 falls {:.0}%", drop * 100.0),
            "Estimated from beta".to_string(),
            -drop * report.beta,
        )
    });
    let history = [
        (
            "Worst day".to_string(),
            day_label(report.worst_day.0),
            report.worst_day.1,
        ),
        (
            "Worst week".to_string(),
            format!("Ending {}", day_label(report.worst_week.0)),
            report.worst_week.1,
        ),
        (
            "Max drawdown again".to_string(),
            "Peak-to-trough repeat".to_string(),
            -report.max_drawdown,
        ),
    ];

    rsx! {
        Card { title: tr("Stress test"), subtitle: tr("What a bad stretch would cost today").to_string(),
            div { class: "flex flex-col",
                for (label, note, change) in shocks.into_iter().chain(history) {
                    StressRow { label, note, change, value }
                }
            }
        }
    }
}

#[component]
fn StressRow(label: String, note: String, change: f64, value: f64) -> Element {
    let loss = Decimal::try_from(change * value).unwrap_or_default();
    rsx! {
        div { class: "flex items-center justify-between gap-3 py-2.5 border-t border-ctp-surface0/60 first:border-t-0",
            div { class: "min-w-0",
                div { class: "text-sm text-ctp-text", "{label}" }
                div { class: "text-xs text-ctp-overlay1 truncate", "{note}" }
            }
            div { class: "text-right shrink-0 tabular-nums",
                div { class: "text-sm font-semibold text-ctp-red", "{fmt_usd(loss, 0)}" }
                div { class: "text-xs text-ctp-subtext0", "{change * 100.0:+.1}%" }
            }
        }
    }
}

// ─── Risk contribution ────────────────────────────────────────────────────────

#[component]
fn RiskContribution(report: RiskReport, tickers: Vec<TickerSymbol>) -> Element {
    let mut rows: Vec<(usize, &super::stats::HoldingRisk)> =
        report.holdings.iter().enumerate().collect();
    rows.sort_by(|a, b| b.1.risk_share.total_cmp(&a.1.risk_share));

    rsx! {
        Card {
            title: tr("Where your risk comes from"),
            subtitle: tr("Share of portfolio swings each holding causes, next to its weight").to_string(),
            flush: true,
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            th { class: "pl-6 pr-4 py-2.5 text-left font-medium", {tr("Asset")} }
                            th { class: "px-4 py-2.5 text-left font-medium", {tr("Weight → risk share")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Volatility")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Beta")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Max drawdown")} }
                            th { class: "pl-4 pr-6 py-2.5 text-right font-medium", "" }
                        }
                    }
                    tbody {
                        for (i, h) in rows {
                            ContributionRow {
                                key: "{i}",
                                ticker: tickers.get(i).map(|t| t.to_string()).unwrap_or_default(),
                                color: CHART_COLOR_CLASSES[i % CHART_COLOR_CLASSES.len()],
                                holding: h.clone(),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ContributionRow(
    ticker: String,
    color: &'static str,
    holding: super::stats::HoldingRisk,
) -> Element {
    let h = &holding;
    let (weight, share) = (h.weight * 100.0, h.risk_share * 100.0);
    let flag = if h.risk_share > h.weight * 1.3 && h.risk_share > 0.1 {
        Some(("Outsized", "bg-ctp-peach/15 text-ctp-peach"))
    } else if h.risk_share < h.weight * 0.7 {
        Some(("Stabiliser", "bg-ctp-green/15 text-ctp-green"))
    } else {
        None
    };
    let cell = "px-4 py-3.5 text-right tabular-nums text-ctp-subtext0";

    rsx! {
        tr { class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
            td { class: "pl-6 pr-4 py-3.5",
                span { class: "flex items-center gap-2.5",
                    span { class: "h-2.5 w-2.5 rounded-full shrink-0 {color}" }
                    span { class: "font-semibold text-ctp-text", "{ticker}" }
                }
            }
            td { class: "px-4 py-3.5",
                div { class: "flex items-center gap-3",
                    div { class: "flex w-32 flex-col gap-1",
                        span { class: "h-1 rounded-full bg-ctp-surface2", style: "width:{weight.clamp(2.0, 100.0):.0}%;" }
                        span { class: "h-1 rounded-full {color}", style: "width:{share.clamp(2.0, 100.0):.0}%;" }
                    }
                    span { class: "text-xs tabular-nums text-ctp-subtext0",
                        "{weight:.0}% → "
                        span { class: "font-semibold text-ctp-text", "{share:.0}%" }
                    }
                }
            }
            td { class: cell, "{h.volatility * 100.0:.1}%" }
            td { class: cell, "{h.beta:.2}" }
            td { class: cell, "−{h.max_drawdown * 100.0:.1}%" }
            td { class: "pl-4 pr-6 py-3.5 text-right",
                if let Some((label, style)) = flag {
                    span { class: "rounded-full px-2.5 py-0.5 text-xs font-semibold {style}", "{label}" }
                }
            }
        }
    }
}
