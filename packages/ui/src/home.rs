use crate::{
    components::{charts::*, tables::*},
    hooks::{use_dashboard, DashboardState},
};
use dioxus::prelude::*;
use dtos::portfolio::GetDashBoardResponse;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

// ─── Home ─────────────────────────────────────────────────────────────────────

#[component]
pub fn Home() -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();

    let DashboardState {
        prices,
        ticker_price_map,
        change_map,
        loaded,
        positions,
        realized,
        total_value,
        total_cost,
        total_pnl,
        day_change,
        pnl_pct,
        day_pct,
        chart_positive,
    } = use_dashboard();

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        document::Script { src: asset!("/assets/js/growth_chart.js") }
        div { class: "mocha min-h-screen bg-ctp-base text-ctp-text",

            // ── Header ────────────────────────────────────────────────────────
            div { class: "bg-ctp-mantle px-6 pt-5 pb-6 border-b border-ctp-surface0",

                div { class: "flex items-center justify-between mb-3",
                    span { class: "text-xs text-ctp-subtext0 font-medium tracking-wide",
                        "All Portfolio Holdings"
                    }
                    button {
                        class: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg \
                                text-xs font-semibold text-ctp-text \
                                border border-ctp-surface2 hover:bg-ctp-surface0 \
                                transition-colors cursor-pointer",
                        style: "background:transparent;",
                        "＋  New Portfolio"
                    }
                }

                div { class: "flex items-baseline gap-3 mb-5",
                    span { class: "text-4xl font-bold text-ctp-text tabular-nums",
                        "{fmt_usd(total_value, 2)}"
                    }
                    if loaded && !positions.is_empty() {
                        if loaded && !positions.is_empty() {
                            span {
                                class: " bg-ctp-green/15 text-ctp-green border border-ctp-green/30 px-[0.45rem] py-[0.15rem] rounded-[0.3rem] text-[0.68rem] font-[700] tracking-[0.04em] ",
                                "● Live"
                            }
                        }
                    }
                }

                div { class: "flex items-start",
                    StatItem {
                        label: "Cash Holdings",
                        value: "--",
                        sub: "",
                        neutral: true,
                    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem {
                        label: "Day Change",
                        value: fmt_signed(day_change, 2),
                        sub: format!("({:+.2}%)", day_pct),
                        neutral: !loaded,
                    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem {
                        label: "Unrealized Gain/Loss",
                        value: fmt_signed(total_pnl, 2),
                        sub: format!("({:+.2}%)", pnl_pct),
                        neutral: !loaded,
                    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem {
                        label: "Realized Gain/Loss",
                        value: fmt_usd(realized, 2),
                        sub: "(0.00%)",
                        neutral: true,
                    }
                }
            }

            // ── Chart ─────────────────────────────────────────────────────────
            ChartSection {
                transactions: data().transactions.clone(),
                pnl_pct: total_pnl,
                is_positive: chart_positive,
                height: dec!(220.0),
            }
            DashboardTable {
                data,
                price_map:ticker_price_map,
                change_map,
                positions,
                loaded,
            }
        }
    }
}

// ── Sub-components ────────────────────────────────────────────────────────────

#[component]
fn StatItem(label: String, value: String, sub: String, neutral: bool) -> Element {
    let positive = value.starts_with('+');
    let color = if neutral || value == "--" {
        "text-ctp-subtext1"
    } else if positive {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    };
    rsx! {
        div { class: "flex flex-col",
            div { class: "text-xs text-ctp-subtext0 mb-1", "{label}" }
            div { class: "text-sm font-semibold tabular-nums", style: "{color}", "{value}" }
            if !sub.is_empty() {
                div { class: "text-xs tabular-nums {color}", "{sub}" }
            }
        }
    }
}

/// "$1,234.56"  (ไม่มี sign)
fn fmt_usd(value: Decimal, decimals: u32) -> String {
    let neg = value.is_sign_negative();
    let abs = value.abs().round_dp(decimals);
    let whole = abs.trunc();
    let frac = ((abs - whole) * Decimal::from(10u64.pow(decimals)))
        .round()
        .to_string();

    let whole_str = whole.to_string();
    let mut out = String::new();
    for (i, c) in whole_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let whole_fmt: String = out.chars().rev().collect();

    let sign = if neg { "-" } else { "" };
    if decimals == 0 {
        format!("{sign}${whole_fmt}")
    } else {
        format!(
            "{sign}${whole_fmt}.{frac:0>width$}",
            width = decimals as usize
        )
    }
}

/// "+$1,234.56" / "-$1,234.56"
fn fmt_signed(value: Decimal, decimals: u32) -> String {
    let sign = if value >= Decimal::ZERO { "+" } else { "" };
    format!("{sign}{}", fmt_usd(value, decimals))
}
