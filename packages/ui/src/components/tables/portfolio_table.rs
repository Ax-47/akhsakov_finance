use dioxus::prelude::*;
use dtos::{asset::get_asset_response::GetAssetResponse, portfolio::GetDashBoardResponse};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::transaction_type::TransactionType;
#[component]
pub fn PortfoliosTable(
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
                                    class: if p_day >= Decimal::ZERO { " py-4 pr-6 text-right tabular-nums text-ctp-green" } else { "  py-4 pr-6 text-right tabular-nums text-ctp-red" },
                                    if loaded {
                                        div { "{fmt_signed(p_day, 2)}" }
                                        div { class: "text-xs", "{p_day_pct:+.2}%" }
                                    } else { "--" }
                                }
                                td {
                                    class: if p_pnl >= Decimal::ZERO { " py-4 pr-6 text-right tabular-nums text-ctp-green" } else { " py-4 pr-6 text-right tabular-nums text-ctp-red" },
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
