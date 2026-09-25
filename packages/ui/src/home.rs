use crate::{
    components::{charts::*, tables::*},
    hooks::{use_dashboard, DashboardState},
    Mascot,
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
        div { class: "mocha ak-page min-h-screen text-ctp-text",

            // ── Header ────────────────────────────────────────────────────────
            div {
                class: "ak-glass ak-rise px-6 pt-5 pb-6",
                style: "border-width:0 0 1px 0;",

                div { class: "flex items-center justify-between mb-3",
                    span { class: "ak-kicker", "✿ All Portfolio Holdings" }
                    button { class: "ak-btn", "＋  New Portfolio" }
                }

                div { class: "flex items-baseline gap-3 mb-5",
                    span { class: "text-4xl font-bold tabular-nums ak-gradient-text",
                        "{fmt_usd(total_value, 2)}"
                    }
                    if loaded && !positions.is_empty() {
                        span { class: "ak-live", "Live" }
                    }
                }

                div { class: "flex items-center",
                    StatItem {
                        label: "Cash Holdings",
                        value: "--",
                        sub: "",
                        neutral: true,
                    }
                    div { class: "ak-divider" }
                    StatItem {
                        label: "Day Change",
                        value: fmt_signed(day_change, 2),
                        sub: format!("({:+.2}%)", day_pct),
                        neutral: !loaded,
                    }
                    div { class: "ak-divider" }
                    StatItem {
                        label: "Unrealized Gain/Loss",
                        value: fmt_signed(total_pnl, 2),
                        sub: format!("({:+.2}%)", pnl_pct),
                        neutral: !loaded,
                    }
                    div { class: "ak-divider" }
                    StatItem {
                        label: "Realized Gain/Loss",
                        value: fmt_usd(realized, 2),
                        sub: "(0.00%)",
                        neutral: true,
                    }
                    div { style: "margin-left:auto;",
                        Mascot { message: mascot_line(loaded, day_change) }
                    }
                }
            }

            // ── Chart ─────────────────────────────────────────────────────────
            div { class: "ak-rise", style: "--d:120ms;",
            ChartSection {
                transactions: data().transactions.clone(),
                pnl_pct: total_pnl,
                is_positive: chart_positive,
                height: dec!(220.0),
            }
            }
            div { class: "ak-rise", style: "--d:240ms;",
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
}

/// What the mascot says on the home header, based on today's move.
fn mascot_line(loaded: bool, day_change: Decimal) -> String {
    if !loaded {
        "Fetching prices… nya~ ⏳".to_string()
    } else if day_change > Decimal::ZERO {
        "Green day! Yatta~ 🎉".to_string()
    } else if day_change < Decimal::ZERO {
        "Red day… hold steady, ganbatte! 💪".to_string()
    } else {
        "Quiet market today~ 🍵".to_string()
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
        div { class: "flex flex-col ak-fade",
            div { class: "text-xs text-ctp-subtext0 mb-1", "{label}" }
            div { class: "text-sm font-semibold tabular-nums {color}", "{value}" }
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
