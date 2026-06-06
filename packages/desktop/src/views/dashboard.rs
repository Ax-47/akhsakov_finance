use dioxus::prelude::*;
use ui::{PieChart, CHART_COLORS};

#[component]
pub fn Dashboard() -> Element {
    let data = use_context::<Signal<AppData>>();

    let price_res = use_resource(move || {
        let tickers: Vec<String> = data()
            .transactions
            .iter()
            .map(|t| t.ticker.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        async move { yahoo::fetch_prices_bulk(tickers).await }
    });

    let price_data = price_res();
    let loading = price_data.is_none() && !data().transactions.is_empty();
    let prices = price_data.clone().unwrap_or_default();
    let positions = compute_positions(&data(), &prices);
    let (total_value, total_cost, total_pnl, day_change) = portfolio_summary(&positions);

    let pnl_pct = if total_cost > 0.0 {
        total_pnl / total_cost * 100.0
    } else {
        0.0
    };
    let day_pct = if total_value > 0.0 {
        day_change / total_value * 100.0
    } else {
        0.0
    };

    let allocation: Vec<(String, f64)> = {
        let total: f64 = positions
            .iter()
            .map(|p| {
                if p.current_price > 0.0 {
                    p.market_value()
                } else {
                    p.cost_basis()
                }
            })
            .sum();
        let mut v: Vec<(String, f64)> = positions
            .iter()
            .map(|p| {
                let val = if p.current_price > 0.0 {
                    p.market_value()
                } else {
                    p.cost_basis()
                };
                (
                    p.ticker.clone(),
                    if total > 0.0 {
                        val / total * 100.0
                    } else {
                        0.0
                    },
                )
            })
            .collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v
    };

    rsx! {
        div {
            class: "page",
            div {
                class: "page-header",
                h1 { "Portfolio Overview" }
                div {
                    class: "page-header-sub",
                    if loading { "Refreshing prices…" }
                    else if positions.is_empty() { "Add transactions to get started" }
                    else { "Live prices" }
                }
            }

            // Stat cards
            div {
                class: "stats-grid",
                StatCard {
                    label: "Total Value",
                    value: format!("${:.2}", total_value),
                    sub: format!("${:.2} invested", total_cost),
                    color: "var(--blue)",
                }
                StatCard {
                    label: "Unrealized P&L",
                    value: format!("{:+.2}", total_pnl),
                    sub: format!("{:+.2}% all-time", pnl_pct),
                    color: if total_pnl >= 0.0 { "var(--green)" } else { "var(--red)" },
                }
                StatCard {
                    label: "Day Change",
                    value: format!("{:+.2}", day_change),
                    sub: format!("{:+.2}% today", day_pct),
                    color: if day_change >= 0.0 { "var(--green)" } else { "var(--red)" },
                }
                StatCard {
                    label: "Positions",
                    value: positions.len().to_string(),
                    sub: format!("{} transactions", data().transactions.len()),
                    color: "var(--mauve)",
                }
            }

            // Main grid
            div {
                style: "display: grid; grid-template-columns: 1fr 310px; gap: 1.25rem;",

                // Holdings table
                div {
                    class: "card card-flush",
                    if positions.is_empty() {
                        div {
                            class: "empty-state",
                            div { class: "empty-icon", "◈" }
                            div { class: "empty-title", "No holdings yet" }
                            div { class: "empty-desc", "Import a CSV or add transactions" }
                        }
                    } else {
                        table {
                            class: "data-table",
                            thead {
                                tr {
                                    th { "Ticker" } th { "Shares" } th { "Avg Cost" }
                                    th { "Price" }  th { "Value" }  th { "P&L" }
                                    th { "Day %" }
                                }
                            }
                            tbody {
                                for pos in &positions {
                                    tr {
                                        key: "{pos.ticker}",
                                        td { span { class: "ticker-label", "{pos.ticker}" } }
                                        td { class: "num", "{pos.shares:.4}" }
                                        td { class: "num muted", "${pos.avg_cost:.2}" }
                                        td {
                                            class: "num",
                                            if pos.current_price > 0.0 { "${pos.current_price:.2}" } else { "—" }
                                        }
                                        td {
                                            class: "num",
                                            if pos.current_price > 0.0 { "${pos.market_value():.2}" } else { "—" }
                                        }
                                        td {
                                            class: if pos.unrealized_pnl() >= 0.0 { "num pos" } else { "num neg" },
                                            if pos.current_price > 0.0 {
                                                "{pos.unrealized_pnl():+.2} ({pos.unrealized_pnl_pct():+.1}%)"
                                            } else { "—" }
                                        }
                                        td {
                                            class: if pos.daily_change_pct >= 0.0 { "num pos" } else { "num neg" },
                                            if pos.current_price > 0.0 { "{pos.daily_change_pct:+.2}%" } else { "—" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Allocation donut + legend
                div {
                    class: "card",
                    div { style: "font-size:0.82rem;font-weight:700;margin-bottom:1rem;", "Allocation" }
                    if allocation.is_empty() {
                        div { class: "empty-state", "No data" }
                    } else {
                        PieChart { data: allocation.clone(), size: 180.0 }
                        div {
                            style: "margin-top:1rem;display:flex;flex-direction:column;gap:0.35rem;",
                            for (i, (ticker, pct)) in allocation.iter().enumerate() {
                                div {
                                    key: "{ticker}",
                                    style: "display:flex;align-items:center;justify-content:space-between;font-size:0.75rem;",
                                    div {
                                        style: "display:flex;align-items:center;gap:0.4rem;",
                                        div {
                                            style: "width:9px;height:9px;border-radius:50%;background:{CHART_COLORS[i % CHART_COLORS.len()]};",
                                        }
                                        span { "{ticker}" }
                                    }
                                    span { class: "num muted", "{pct:.1}%" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn StatCard(label: String, value: String, sub: String, color: String) -> Element {
    rsx! {
        div {
            class: "card",
            div { class: "stat-label", "{label}" }
            div { class: "stat-value num", style: "color:{color};", "{value}" }
            div { class: "stat-sub", "{sub}" }
        }
    }
}
