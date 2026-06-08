use crate::{
    components::{charts::AllocationCard, section_header::SectionHeader},
    hooks::{use_portfolio, PortfolioState},
};
use dioxus::prelude::*;
use dtos::{portfolio::GetDashBoardResponse, transaction::Transaction, Position};
use rust_decimal::Decimal;
use types::transaction_type::TransactionType;
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

// ─── Dashboard ────────────────────────────────────────────────────────────────

#[component]
pub fn Dashboard() -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let PortfolioState {
        loaded,
        positions,
        total_value,
        total_cost,
        total_pnl,
        day_change,
        pnl_pct,
        day_pct,
        allocation,
        ..
    } = use_portfolio();
    let pos = positions.clone();

    let recent_txns: Vec<Transaction> = {
        let mut txs = data().transactions.clone();
        txs.sort_by(|a, b| b.date.cmp(&a.date));
        txs.into_iter().take(5).collect()
    };

    let empty = positions.is_empty() && data().transactions.is_empty();
    let subtitle = if empty {
        "Add transactions to get started"
    } else if !loaded {
        "Fetching live prices…"
    } else {
        "Live prices"
    };

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        div { class: "mocha min-h-screen bg-ctp-mantle text-ctp-text p-6",
            SectionHeader {
                title: "Portfolio Overview",
                subtitle,
                live: loaded && !positions.is_empty(),
                on_refresh: move |_| {},
            }
            div { class: "grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6",
                StatCard {
                    label: "Total Value",
                    value: fmt_usd(total_value, 2),
                    sub:   format!("{} invested", fmt_usd(total_cost, 2)),
                    color: "text-ctp-blue",
                    icon:  "💰",
                }
                StatCard {
                    label: "Unrealized P&L",
                    value: fmt_signed(total_pnl, 2),
                    sub:   format!("{:+.2}% all-time", pnl_pct),
                    color: if total_pnl  >= Decimal::ZERO { "text-ctp-green" } else { "text-ctp-red" },
                    icon:  if total_pnl  >= Decimal::ZERO { "📈" } else { "📉" },
                }
                StatCard {
                    label: "Day Change",
                    value: fmt_signed(day_change, 2),
                    sub:   format!("{:+.2}% today", day_pct),
                    color: if day_change >= Decimal::ZERO { "text-ctp-green" } else { "text-ctp-red" },
                    icon:  if day_change >= Decimal::ZERO { "▲" } else { "▼" },
                }
                StatCard {
                    label: "Positions",
                    value: positions.len().to_string(),
                    sub:   format!("{} transactions", data().transactions.len()),
                    color: "text-ctp-mauve",
                    icon:  "◈",
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
                div {
                    class: "grid gap-5 mb-5",
                    style: "grid-template-columns:1fr 280px;",
                    HoldingsTable { positions: positions.clone() }
                    AllocationCard { allocation: allocation.clone() }

                }
                if !recent_txns.is_empty() {
                    RecentTransactions { transactions: recent_txns }
                }
            }
        }
    }
}
#[component]
fn HoldingsTable(positions: Vec<Position>) -> Element {
    rsx! {
        div { class: "rounded-xl bg-ctp-base border border-ctp-surface0 overflow-hidden",
            div { class: "flex justify-between items-center px-4 py-3 border-b border-ctp-surface1",
                div { class: "flex items-center gap-2",
                    span { class: "inline-block w-[3px] h-[14px] rounded-[2px] bg-ctp-blue shrink-0" }
                    span { class: "text-xs font-bold uppercase tracking-wide", "Holdings" }
                }
                span { class: "text-xs text-ctp-subtext0", "by market value" }
            }
            table { class: "w-full text-xs",
                thead {
                    tr { class: "border-b border-ctp-surface1 text-ctp-overlay1",
                        for h in ["Ticker", "Shares", "Avg Cost", "Total Cost", "Market Value", "P&L", "Day"] {
                            th { class: "px-4 py-2.5 text-right first:text-left font-semibold uppercase tracking-wider", "{h}" }
                        }
                    }
                }
                tbody {
                    for pos in &positions {
                        tr {
                            key: "{pos.ticker}",
                            class: "border-b border-ctp-surface1 hover:bg-ctp-surface1 transition-colors",
                            td { class: "px-4 py-2.5",
                                span { class: "font-bold text-ctp-blue tracking-wide", "{pos.ticker}" }
                            }
                            td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", "{pos.shares:.4}" }
                            td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", "{fmt_usd(pos.avg_cost, 2)}" }
                            td { class: "px-4 py-2.5 text-right tabular-nums",
                                if pos.current_price > Decimal::ZERO { "{fmt_usd(pos.cost_basis(), 2)}" } else { "—" }
                            }
                            td { class: "px-4 py-2.5 text-right tabular-nums font-medium",
                                if pos.current_price > Decimal::ZERO { "{fmt_usd(pos.market_value(), 2)}" } else { "—" }
                            }
                            td {
                                class: if pos.unrealized_pnl() >= Decimal::ZERO { "px-4 py-2.5 text-right tabular-nums text-ctp-green" } else { "px-4 py-2.5 text-right tabular-nums text-ctp-red" },
                                if pos.current_price > Decimal::ZERO {
                                    "{fmt_signed(pos.unrealized_pnl(), 2)} ({pos.unrealized_pnl_pct():+.1}%)"
                                } else { "—" }
                            }
                            td {
                                class: if pos.daily_change_pct >= Decimal::ZERO { "px-4 py-2.5 text-right tabular-nums text-ctp-green" } else { "px-4 py-2.5 text-right tabular-nums text-ctp-red" },
                                if pos.current_price > Decimal::ZERO {
                                    if pos.daily_change_pct >= Decimal::ZERO { "▲ {pos.daily_change_pct:.2}%" } else { "▼ {pos.daily_change_pct.abs():.2}%" }
                                } else { "—" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RecentTransactions(transactions: Vec<Transaction>) -> Element {
    rsx! {
        div { class: "rounded-xl bg-ctp-surface0 border border-ctp-surface1 overflow-hidden",
            div { class: "flex justify-between items-center px-4 py-3 border-b border-ctp-surface1",
                div { class: "flex items-center gap-2",
                    span { style: "display:inline-block;width:3px;height:14px;border-radius:2px;background:var(--peach);flex-shrink:0;" }
                    span { class: "text-xs font-bold uppercase tracking-wide", "Recent Transactions" }
                }
                span { class: "text-xs text-ctp-subtext0", "last 5" }
            }
            for tx in &transactions {
                div {
                    key: "{tx.id}",
                    class: "flex items-center gap-3 px-4 py-3 border-b border-ctp-surface1",
                    span {
                        style: match tx.transaction_type {
                            TransactionType::Buy  => "background:color-mix(in srgb,var(--green) 15%,transparent);color:var(--green);border:1px solid color-mix(in srgb,var(--green) 30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;white-space:nowrap;letter-spacing:.04em;",
                            TransactionType::Sell => "background:color-mix(in srgb,var(--red) 15%,transparent);color:var(--red);border:1px solid color-mix(in srgb,var(--red) 30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;white-space:nowrap;letter-spacing:.04em;",
                            _                     => "background:color-mix(in srgb,var(--blue) 15%,transparent);color:var(--blue);border:1px solid color-mix(in srgb,var(--blue) 30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;white-space:nowrap;letter-spacing:.04em;",
                        },
                        "{tx.transaction_type}"
                    }
                    div { class: "flex-1 min-w-0",
                        span { class: "font-bold text-ctp-blue text-xs", "{tx.ticker}" }
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
                        div { class: "text-xs tabular-nums font-semibold", "{fmt_usd(tx.shares * tx.price, 2)}" }
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
            class: "
                    rounded-xl
                    border border-ctp-surface0
                    bg-ctp-base
                    p-4
                ",

            div {
                class: "flex items-center justify-between",

                span {
                    class: "text-sm text-ctp-subtext1",
                    "{label}"
                }

                span {
                    class: "{color} text-xl",
                    "{icon}"
                }
            }

            div {
                class: "mt-2 text-2xl font-bold text-ctp-text",
                "{value}"
            }

            div {
                class: "mt-1 text-xs text-ctp-subtext0",
                "{sub}"
            }
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
