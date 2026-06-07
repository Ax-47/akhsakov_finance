use crate::components::charts::*;
use dioxus::prelude::*;
use dtos::{
    asset::get_asset_response::GetAssetResponse,
    portfolio::GetDashBoardResponse,
    position::{compute_positions, portfolio_summary, Position},
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

    let active_tab = use_signal(|| "My Portfolios".to_string());

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

            // ── Tables ────────────────────────────────────────────────────────
            div { class: "px-6 pb-10",
                div { class: "flex border-b border-ctp-surface1",
                    TabButton { label: "My Portfolios".to_string(), active_tab }
                    TabButton { label: "My Holdings".to_string(),   active_tab }
                }

                if active_tab() == "My Portfolios" {
                    PortfoliosTable { data, price_map, change_map, loaded }
                } else {
                    HoldingsTable { positions, loaded }
                }
            }
        }
    }
}

// ── Portfolios table ──────────────────────────────────────────────────────────

#[component]
fn PortfoliosTable(
    data: Signal<GetDashBoardResponse>,
    price_map: HashMap<String, Decimal>,
    change_map: HashMap<String, Decimal>,
    loaded: bool,
) -> Element {
    rsx! {
        table { class: "w-full text-xs mt-4",
            thead {
                tr { class: "text-ctp-overlay0 border-b border-ctp-surface1",
                    th { class: "py-2 w-6" }
                    th { class: "py-2 pr-6 text-left  font-semibold uppercase tracking-wider", "Portfolio Name" }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Symbols" }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        div { "Cost Basis" }
                        div { class: "font-normal normal-case tracking-normal text-ctp-overlay0", "Includes cash" }
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        div { "Market Value" }
                        div { class: "font-normal normal-case tracking-normal text-ctp-overlay0", "Includes cash" }
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Day Change" }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        div { "Unrealized" }
                        div { "Gain/Loss" }
                    }
                    th { class: "py-2 text-right font-semibold uppercase tracking-wider",
                        div { "Realized" }
                        div { "Gain/Loss" }
                    }
                }
            }
            tbody {
                for port in &data().portfolios {
                    {
                        let (p_count, p_cost, p_value, p_day, p_pnl) =
                            portfolio_stats(&port.assets, &price_map, &change_map);

                        let p_pnl_pct  = if p_cost  > Decimal::ZERO { p_pnl / p_cost  * dec!(100) } else { Decimal::ZERO };
                        let p_day_pct  = if p_value > Decimal::ZERO { p_day / p_value * dec!(100) } else { Decimal::ZERO };
                        let p_realized = compute_realized_pnl_for_tickers(
                            &data(),
                            &port.assets.iter().map(|a| a.ticker_symbol.to_string()).collect::<Vec<_>>(),
                        );

                        rsx! {
                            tr { class: "border-b border-ctp-surface1 hover:bg-ctp-surface0 transition-colors",
                                td { class: "py-4 pr-2 text-ctp-overlay0 select-none", "⠿" }
                                td { class: "py-4 pr-6",
                                    div { class: "flex items-center gap-2",
                                        span { class: "font-medium", "{port.name}" }
                                    }
                                }
                                td { class: "py-4 pr-6 text-right tabular-nums text-ctp-subtext1",
                                    "{p_count}"
                                }
                                td { class: "py-4 pr-6 text-right tabular-nums",
                                    "{fmt_usd(p_cost, 2)}"
                                }
                                td { class: "py-4 pr-6 text-right tabular-nums font-medium",
                                    if loaded && p_value > Decimal::ZERO { "{fmt_usd(p_value, 2)}" } else { "--" }
                                }
                                td {
                                    class: "py-4 pr-6 text-right tabular-nums",
                                    style: if p_day >= Decimal::ZERO { "color:var(--green)" } else { "color:var(--red)" },
                                    if loaded {
                                        div { "{fmt_signed(p_day, 2)}" }
                                        div { class: "text-xs", "{p_day_pct:+.2}%" }
                                    } else { "--" }
                                }
                                td {
                                    class: "py-4 pr-6 text-right tabular-nums",
                                    style: if p_pnl >= Decimal::ZERO { "color:var(--green)" } else { "color:var(--red)" },
                                    div { "{fmt_signed(p_pnl, 2)}" }
                                    div { class: "text-xs", "{p_pnl_pct:+.2}%" }
                                }
                                td {
                                    class: "py-4 text-right tabular-nums",
                                    style: if p_realized >= Decimal::ZERO { "color:var(--green)" } else { "color:var(--red)" },
                                    if p_realized.abs() > dec!(0.01) { "{fmt_signed(p_realized, 2)}" } else { "--" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Holdings table ────────────────────────────────────────────────────────────

#[component]
fn HoldingsTable(positions: Vec<Position>, loaded: bool) -> Element {
    rsx! {
        table { class: "w-full text-xs mt-4",
            thead {
                tr { class: "text-ctp-overlay0 border-b border-ctp-surface1",
                    th { class: "py-2 pr-6 text-left  font-semibold uppercase tracking-wider", "Ticker"   }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Shares"   }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Avg Cost" }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Total Cost" }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Value"    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Market Price"    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "P&L"      }
                    th { class: "py-2 text-right      font-semibold uppercase tracking-wider", "Day"      }
                }
            }
            tbody {
                for pos in &positions {
                    tr {
                        key: "{pos.ticker}",
                        class: "border-b border-ctp-surface1 hover:bg-ctp-surface0 transition-colors",

                        td { class: "py-3 pr-6",
                            span { class: "font-bold text-ctp-blue tracking-wide", "{pos.ticker}" }
                        }
                        td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0",
                            "{pos.shares:.4}"
                        }
                        td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0",
                            "{fmt_usd(pos.avg_cost, 2)}"
                        }
                        td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0",
                            "{fmt_usd(pos.cost_basis(),2)}"
                        }
                        td { class: "py-3 pr-6 text-right tabular-nums",
                            if pos.current_price > Decimal::ZERO { "{fmt_usd(pos.current_price, 2)}" } else { "—" }
                        }
                        td { class: "py-3 pr-6 text-right tabular-nums font-medium",
                            if pos.current_price > Decimal::ZERO { "{fmt_usd(pos.market_value(), 2)}" } else { "—" }
                        }
                        td {
                            class: if pos.unrealized_pnl() >= Decimal::ZERO {
                                "py-3 pr-6 text-right tabular-nums text-ctp-green"
                            } else {
                                "py-3 pr-6 text-right tabular-nums text-ctp-red"
                            },
                            if pos.current_price > Decimal::ZERO {
                                "{fmt_signed(pos.unrealized_pnl(), 2)} ({pos.unrealized_pnl_pct():+.1}%)"
                            } else { "—" }
                        }
                        td {
                            class: if pos.daily_change_pct >= Decimal::ZERO {
                                "py-3 text-right tabular-nums text-ctp-green"
                            } else {
                                "py-3 text-right tabular-nums text-ctp-red"
                            },
                            if pos.current_price > Decimal::ZERO {
                                if pos.daily_change_pct >= Decimal::ZERO {
                                    "▲ {pos.daily_change_pct:.2}%"
                                } else {
                                    "▼ {pos.daily_change_pct.abs():.2}%"
                                }
                            } else { "—" }
                        }
                    }
                }
            }
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

#[component]
fn TabButton(label: String, mut active_tab: Signal<String>) -> Element {
    let is_active = active_tab() == label;
    rsx! {
        button {
            class: "px-5 py-3 text-sm font-medium transition-colors cursor-pointer",
            style: if is_active {
                "border-bottom:2px solid var(--blue);color:var(--text);background:transparent;"
            } else {
                "border-bottom:2px solid transparent;color:var(--subtext0);background:transparent;"
            },
            onclick: move |_| active_tab.set(label.clone()),
            "{label}"
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn portfolio_stats(
    assets: &[GetAssetResponse],
    prices: &HashMap<String, Decimal>,
    changes: &HashMap<String, Decimal>,
) -> (usize, Decimal, Decimal, Decimal, Decimal) {
    let pos_count = assets.len();

    let total_cost: Decimal = assets
        .iter()
        .map(|a| a.quantity.value() * a.cost.amount())
        .sum();

    let total_value: Decimal = assets
        .iter()
        .map(|a| {
            let price = prices
                .get(a.ticker_symbol.as_str())
                .copied()
                .unwrap_or(Decimal::ZERO);
            a.quantity.value() * price
        })
        .sum();

    let day_change: Decimal = assets
        .iter()
        .map(|a| {
            let price = prices
                .get(a.ticker_symbol.as_str())
                .copied()
                .unwrap_or(Decimal::ZERO);
            let chg_pct = changes
                .get(a.ticker_symbol.as_str())
                .copied()
                .unwrap_or(Decimal::ZERO);
            a.quantity.value() * price * chg_pct / dec!(100)
        })
        .sum();

    let total_pnl = total_value - total_cost;
    (pos_count, total_cost, total_value, day_change, total_pnl)
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

fn compute_realized_pnl_for_tickers(data: &GetDashBoardResponse, tickers: &[String]) -> Decimal {
    let set: std::collections::HashSet<_> = tickers.iter().collect();
    let filtered = GetDashBoardResponse {
        transactions: data
            .transactions
            .iter()
            .filter(|tx| set.contains(&tx.ticker))
            .cloned()
            .collect(),
        portfolios: vec![],
    };
    compute_realized_pnl(&filtered)
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
