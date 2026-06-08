use crate::pie_chart::{PieChart, CHART_COLORS};
use dioxus::prelude::*;
use dtos::{
    portfolio::GetDashBoardResponse,
    position::{compute_positions, portfolio_summary},
    transaction::Transaction,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::transaction_type::TransactionType;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

// ─── Dashboard ────────────────────────────────────────────────────────────────

#[component]
pub fn Dashboard() -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();

    let mut price_res = use_resource(move || {
        let tickers: Vec<String> = data()
            .transactions
            .iter()
            .map(|tx| tx.ticker.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        async move { api::get_live_prices(tickers).await }
    });

    // ── Derived state ─────────────────────────────────────────────────────────
    let prices: HashMap<String, (Decimal, Decimal)> =
        price_res().and_then(|r| r.ok()).unwrap_or_default();

    let loaded = !prices.is_empty();
    let positions = compute_positions(&data(), &prices);

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

    // Allocation (% of portfolio per ticker)
    let allocation: Vec<(String, Decimal)> = {
        let total = positions
            .iter()
            .map(|p| {
                if p.current_price > Decimal::ZERO {
                    p.market_value()
                } else {
                    p.cost_basis()
                }
            })
            .sum::<Decimal>()
            .max(dec!(1));

        let mut v: Vec<(String, Decimal)> = positions
            .iter()
            .map(|p| {
                let val = if p.current_price > Decimal::ZERO {
                    p.market_value()
                } else {
                    p.cost_basis()
                };
                (p.ticker.clone(), val / total * dec!(100))
            })
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    };

    let recent_txns: Vec<Transaction> = {
        let mut txs = data().transactions.clone();
        txs.sort_by(|a, b| b.date.cmp(&a.date));
        txs.into_iter().take(5).collect()
    };

    let empty = positions.is_empty() && data().transactions.is_empty();

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }

        div { class: "mocha min-h-screen bg-ctp-base text-ctp-text p-6",

            // ── Header ─────────────────────────────────────────────────────────
            div { class: "flex items-start justify-between mb-6 gap-4",
                div {
                    div { class: "flex items-center gap-3 mb-1",
                        div { style: "width:4px;height:28px;border-radius:2px;\
                                    background:linear-gradient(180deg,var(--blue),var(--mauve));flex-shrink:0;" }
                        h1 { class: "text-2xl font-bold text-ctp-text", "Portfolio Overview" }
                    }
                    div { class: "text-xs text-ctp-subtext0 flex items-center gap-2 pl-4",
                        if empty {
                            "Add transactions to get started"
                        } else if !loaded {
                            "Fetching live prices…"
                        } else {
                            "Live prices"
                        }
                        if loaded && !positions.is_empty() {
                            span { style: "background:color-mix(in srgb,var(--green) 18%,transparent);\
                                        color:var(--green);\
                                        border:1px solid color-mix(in srgb,var(--green) 35%,transparent);\
                                        padding:.15rem .5rem;border-radius:.3rem;\
                                        font-size:.68rem;font-weight:700;letter-spacing:.04em;",
                                "● Live"
                            }
                        }
                    }
                }
                button {
                    class: "flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold \
                            border border-ctp-surface2 text-ctp-subtext1 \
                            hover:bg-ctp-surface1 hover:text-ctp-text transition-colors cursor-pointer mt-1",
                    style: "background:transparent;",
                    onclick: move |_| price_res.restart(),
                    "↻  Refresh"
                }
            }

            // ── Stat cards ─────────────────────────────────────────────────────
            div { class: "grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6",
                StatCard {
                    label: "Total Value",
                    value: fmt_usd(total_value, 2),
                    sub: format!("{} invested", fmt_usd(total_cost, 2)),
                    color: "var(--blue)",
                    icon: "💰",
                }
                StatCard {
                    label: "Unrealized P&L",
                    value: fmt_signed(total_pnl, 2),
                    sub: format!("{:+.2}% all-time", pnl_pct),
                    color: if total_pnl >= Decimal::ZERO { "var(--green)" } else { "var(--red)" },
                    icon: if total_pnl >= Decimal::ZERO { "📈" } else { "📉" },
                }
                StatCard {
                    label: "Day Change",
                    value: fmt_signed(day_change, 2),
                    sub: format!("{:+.2}% today", day_pct),
                    color: if day_change >= Decimal::ZERO { "var(--green)" } else { "var(--red)" },
                    icon: if day_change >= Decimal::ZERO { "▲" } else { "▼" },
                }
                StatCard {
                    label: "Positions",
                    value: positions.len().to_string(),
                    sub: format!("{} transactions", data().transactions.len()),
                    color: "var(--mauve)",
                    icon: "◈",
                }
            }

            if empty {
                div { class: "rounded-xl bg-ctp-surface0 border border-ctp-surface1 p-12 \
                              flex flex-col items-center gap-3 text-ctp-overlay0",
                    span { class: "text-4xl opacity-30", "◈" }
                    span { class: "font-semibold", "No portfolio data yet" }
                    span { class: "text-xs", "Add transactions to get started" }
                }
            } else {

                // ── Main grid: holdings + donut ───────────────────────────────
                div {
                    class: "grid gap-5 mb-5",
                    style: "grid-template-columns:1fr 280px;",

                    // Holdings table
                    div { class: "rounded-xl bg-ctp-surface0 border border-ctp-surface1 overflow-hidden",
                        div { class: "flex justify-between items-center px-4 py-3 border-b border-ctp-surface1",
                            div { class: "flex items-center gap-2",
                                span { style: "display:inline-block;width:3px;height:14px;border-radius:2px;background:var(--blue);flex-shrink:0;" }
                                span { class: "text-xs font-bold uppercase tracking-wide",
                                    "Holdings"
                                }
                            }
                            span { class: "text-xs text-ctp-subtext0", "by market value" }
                        }
                        table { class: "w-full text-xs",
                            thead {
                                tr { class: "border-b border-ctp-surface1 text-ctp-overlay0",
                                    th { class: "px-4 py-2.5 text-left  font-semibold uppercase tracking-wider",
                                        "Ticker"
                                    }
                                    th { class: "px-4 py-2.5 text-right font-semibold uppercase tracking-wider",
                                        "Shares"
                                    }
                                    th { class: "px-4 py-2.5 text-right font-semibold uppercase tracking-wider",
                                        "Avg Cost"
                                    }
                                    th { class: "px-4 py-2.5 text-right font-semibold uppercase tracking-wider",
                                        "Total Cost"
                                    }
                                    th { class: "px-4 py-2.5 text-right font-semibold uppercase tracking-wider",
                                        "Market Value"
                                    }
                                    th { class: "px-4 py-2.5 text-right font-semibold uppercase tracking-wider",
                                        "P&L"
                                    }
                                    th { class: "px-4 py-2.5 text-right font-semibold uppercase tracking-wider",
                                        "Day"
                                    }
                                }
                            }
                            tbody {
                                for pos in &positions {
                                    tr {
                                        key: "{pos.ticker}",
                                        class: "border-b border-ctp-surface1 hover:bg-ctp-surface1 transition-colors",
                                        td { class: "px-4 py-2.5",
                                            span { class: "font-bold text-ctp-blue tracking-wide",
                                                "{pos.ticker}"
                                            }
                                        }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0",
                                            "{pos.shares:.4}"
                                        }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0",
                                            "{fmt_usd(pos.avg_cost, 2)}"
                                        }
                                        td { class: "px-4 py-2.5 text-right tabular-nums",
                                            if pos.current_price > Decimal::ZERO {
                                                "{fmt_usd(pos.cost_basis(), 2)}"
                                            } else {
                                                "—"
                                            }
                                        }
                                        td { class: "px-4 py-2.5 text-right tabular-nums font-medium",
                                            if pos.current_price > Decimal::ZERO {
                                                "{fmt_usd(pos.market_value(), 2)}"
                                            } else {
                                                "—"
                                            }
                                        }
                                        td { class: if pos.unrealized_pnl() >= Decimal::ZERO { "px-4 py-2.5 text-right tabular-nums text-ctp-green" } else { "px-4 py-2.5 text-right tabular-nums text-ctp-red" },
                                            if pos.current_price > Decimal::ZERO {
                                                "{fmt_signed(pos.unrealized_pnl(), 2)} ({pos.unrealized_pnl_pct():+.1}%)"
                                            } else {
                                                "—"
                                            }
                                        }
                                        td { class: if pos.daily_change_pct >= Decimal::ZERO { "px-4 py-2.5 text-right tabular-nums text-ctp-green" } else { "px-4 py-2.5 text-right tabular-nums text-ctp-red" },
                                            if pos.current_price > Decimal::ZERO {
                                                if pos.daily_change_pct >= Decimal::ZERO {
                                                    "▲ {pos.daily_change_pct:.2}%"
                                                } else {
                                                    "▼ {pos.daily_change_pct.abs():.2}%"
                                                }
                                            } else {
                                                "—"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Allocation donut + legend
                    div { class: "rounded-xl bg-ctp-surface0 border border-ctp-surface1 p-4",
                        div { class: "flex items-center gap-2 mb-4",
                            span { style: "display:inline-block;width:3px;height:14px;border-radius:2px;background:var(--mauve);flex-shrink:0;" }
                            span { class: "text-xs font-bold uppercase tracking-wide",
                                "Allocation"
                            }
                        }
                        if allocation.is_empty() {
                            div { class: "flex items-center justify-center h-24 text-ctp-overlay0",
                                "No data"
                            }
                        } else {
                            // PieChart รับ Vec<(String, f64)> — แปลงตอน render เท่านั้น
                            PieChart {
                                data: allocation.iter()
                                                                    .map(|(t, p)| (t.clone(), *p))
                                                                    .collect::<Vec<_>>(),
                                size: dec!(160.0),
                            }
                            div { class: "mt-4 flex flex-col gap-2",
                                for (i, (ticker, pct)) in allocation.iter().enumerate() {
                                    div {
                                        key: "{ticker}",
                                        class: "flex items-center justify-between text-xs",
                                        div { class: "flex items-center gap-1.5",
                                            div {
                                                class: "w-2 h-2 rounded-full flex-shrink-0",
                                                style: "background:{CHART_COLORS[i % CHART_COLORS.len()]};",
                                            }
                                            span { class: "text-ctp-subtext1", "{ticker}" }
                                        }
                                        div { class: "flex items-center gap-2",
                                            div { style: "height:4px;width:{(pct * dec!(0.6)).round():.0}px;\
                                                        min-width:4px;max-width:60px;border-radius:2px;\
                                                        background:{CHART_COLORS[i % CHART_COLORS.len()]};opacity:.6;" }
                                            span { class: "tabular-nums text-ctp-subtext0 w-10 text-right",
                                                "{pct:.1}%"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Recent transactions ───────────────────────────────────────
                if !recent_txns.is_empty() {
                    div { class: "rounded-xl bg-ctp-surface0 border border-ctp-surface1 overflow-hidden",
                        div { class: "flex justify-between items-center px-4 py-3 border-b border-ctp-surface1",
                            div { class: "flex items-center gap-2",
                                span { style: "display:inline-block;width:3px;height:14px;border-radius:2px;background:var(--peach);flex-shrink:0;" }
                                span { class: "text-xs font-bold uppercase tracking-wide",
                                    "Recent Transactions"
                                }
                            }
                            span { class: "text-xs text-ctp-subtext0", "last 5" }
                        }
                        for tx in &recent_txns {
                            div {
                                key: "{tx.id}",
                                class: "flex items-center gap-3 px-4 py-3 border-b border-ctp-surface1",
                                span {
                                    style: match tx.transaction_type {
                                        TransactionType::Buy => {
                                            "background:color-mix(in srgb,var(--green) 15%,transparent);color:var(--green);border:1px solid color-mix(in srgb,var(--green) 30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;white-space:nowrap;letter-spacing:.04em;"
                                        }
                                        TransactionType::Sell => {
                                            "background:color-mix(in srgb,var(--red)   15%,transparent);color:var(--red);  border:1px solid color-mix(in srgb,var(--red)   30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;white-space:nowrap;letter-spacing:.04em;"
                                        }
                                        _ => {
                                            "background:color-mix(in srgb,var(--blue)  15%,transparent);color:var(--blue); border:1px solid color-mix(in srgb,var(--blue)  30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;white-space:nowrap;letter-spacing:.04em;"
                                        }
                                    },
                                    "{tx.transaction_type}"
                                }
                                div { class: "flex-1 min-w-0",
                                    span { class: "font-bold text-ctp-blue text-xs",
                                        "{tx.ticker}"
                                    }
                                    div { class: "text-xs text-ctp-subtext0 mt-0.5",
                                        if tx.shares > Decimal::ZERO {
                                            "{tx.shares:.4} shares @ {fmt_usd(tx.price, 2)}"
                                        } else {
                                            "{fmt_usd(tx.price, 2)} received"
                                        }
                                    }
                                }
                                div { class: "text-right flex-shrink-0",
                                    div { class: "text-xs text-ctp-subtext0", "{tx.date}" }
                                    div { class: "text-xs tabular-nums font-semibold text-ctp-text",
                                        "{fmt_usd(tx.shares * tx.price, 2)}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── StatCard ──────────────────────────────────────────────────────────────────

#[component]
fn StatCard(label: String, value: String, sub: String, color: String, icon: String) -> Element {
    rsx! {
        div {
            class: "rounded-xl bg-ctp-surface0 border border-ctp-surface1 p-4 relative overflow-hidden",
            style: "border-left: 3px solid {color};",
            span { style: "position:absolute;top:-24px;right:-24px;width:90px;height:90px;\
                        border-radius:50%;background:color-mix(in srgb,{color} 8%,transparent);\
                        pointer-events:none;display:block;" }
            div { class: "flex items-start justify-between mb-2",
                div { class: "text-xs font-semibold uppercase tracking-widest text-ctp-subtext0",
                    "{label}"
                }
                span { style: "font-size:1.05rem;line-height:1;opacity:.75;flex-shrink:0;",
                    "{icon}"
                }
            }
            div {
                class: "text-2xl font-bold tabular-nums mb-1",
                style: "color:{color};",
                "{value}"
            }
            div { class: "text-xs text-ctp-overlay1", "{sub}" }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

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

fn fmt_signed(value: Decimal, decimals: u32) -> String {
    let sign = if value >= Decimal::ZERO { "+" } else { "" };
    format!("{sign}{}", fmt_usd(value, decimals))
}
