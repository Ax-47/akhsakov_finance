use crate::components::{charts::*, tables::*};
use dioxus::prelude::*;
use dtos::{
    portfolio::GetDashBoardResponse,
    position::{compute_positions, portfolio_summary},
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::transaction_type::TransactionType;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

// ─── Home ─────────────────────────────────────────────────────────────────────

#[component]
pub fn Home() -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();

    let price_res = use_resource(move || {
        let tickers: Vec<String> = data()
            .transactions
            .iter()
            .map(|tx| tx.ticker.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        async move { api::get_live_prices(tickers).await }
    });

    // ── Derived state ──────────────────────────────────────────────────────────
    let prices: HashMap<String, (Decimal, Decimal)> =
        price_res().and_then(|r| r.ok()).unwrap_or_default();

    let loaded = !prices.is_empty();
    let positions = compute_positions(&data(), &prices);
    let realized = compute_realized_pnl(&data());

    let (total_value, total_cost, total_pnl, day_change) = portfolio_summary(&positions);

    let pnl_pct = if total_cost > Decimal::ZERO {
        total_pnl / total_cost * dec!(100)
    } else {
        Decimal::ZERO
    };
    let day_pct = if total_value > Decimal::ZERO {
        day_change / total_value * dec!(100)
    } else {
        Decimal::ZERO
    };

    // (price, daily_change_pct) per ticker — for portfolio table
    let price_map: HashMap<String, Decimal> =
        prices.iter().map(|(k, (p, _))| (k.clone(), *p)).collect();
    let change_map: HashMap<String, Decimal> =
        prices.iter().map(|(k, (_, c))| (k.clone(), *c)).collect();

    let chart_positive = total_pnl >= Decimal::ZERO;

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
                        span {
                            style: "background:color-mix(in srgb,var(--green) 15%,transparent);\
                                    color:var(--green);\
                                    border:1px solid color-mix(in srgb,var(--green) 30%,transparent);\
                                    padding:.15rem .45rem;border-radius:.3rem;\
                                    font-size:.68rem;font-weight:700;letter-spacing:.04em;",
                            "● Live"
                        }
                    }
                }

                div { class: "flex items-start",
                    StatItem { label: "Cash Holdings",        value: "--",                          sub: "",                               neutral: true    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem { label: "Day Change",           value: fmt_signed(day_change, 2),            sub: format!("({:+.2}%)", day_pct),           neutral: !loaded }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem { label: "Unrealized Gain/Loss", value: fmt_signed(total_pnl, 2),            sub: format!("({:+.2}%)", pnl_pct),           neutral: !loaded }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem { label: "Realized Gain/Loss",   value: fmt_usd(realized, 2),                sub: "(0.00%)",                        neutral: true    }
                }
            }

            // ── Chart ─────────────────────────────────────────────────────────
            ChartSection {
                transactions: data().transactions.clone(),
                pnl_pct:total_pnl,
                is_positive: chart_positive,
                height: dec!(220.0),
            }
            DashboardTable {data, price_map, change_map, positions,loaded ,  }
        }
    }
}

// ── Sub-components ────────────────────────────────────────────────────────────

#[component]
fn StatItem(label: String, value: String, sub: String, neutral: bool) -> Element {
    let positive = value.starts_with('+');
    let color = if neutral || value == "--" {
        "color:var(--subtext1);"
    } else if positive {
        "color:var(--green);"
    } else {
        "color:var(--red);"
    };
    rsx! {
        div { class: "flex flex-col",
            div { class: "text-xs text-ctp-subtext0 mb-1", "{label}" }
            div { class: "text-sm font-semibold tabular-nums", style: "{color}", "{value}" }
            if !sub.is_empty() {
                div { class: "text-xs tabular-nums", style: "{color}", "{sub}" }
            }
        }
    }
}

fn compute_realized_pnl(data: &GetDashBoardResponse) -> Decimal {
    let mut book: HashMap<String, (Decimal, Decimal)> = HashMap::new(); // (cost_basis, shares)
    let mut realized = Decimal::ZERO;

    for tx in &data.transactions {
        match tx.transaction_type {
            TransactionType::Buy => {
                let e = book.entry(tx.ticker.clone()).or_default();
                e.0 += tx.shares * tx.price;
                e.1 += tx.shares;
            }
            TransactionType::Sell => {
                if let Some((cost, shares)) = book.get_mut(&tx.ticker) {
                    if *shares > Decimal::ZERO {
                        let avg = *cost / *shares;
                        realized += tx.shares * (tx.price - avg);
                        *cost -= tx.shares * avg;
                        *shares -= tx.shares;
                    }
                }
            }
            TransactionType::Dividend => {
                realized += tx.price;
            }
            _ => {}
        }
    }
    realized
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
