use std::collections::HashMap;

use dioxus::prelude::*;
use dtos::{
    asset::get_asset_response::GetAssetResponse, portfolio::GetDashBoardResponse,
    position::Position,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use types::transaction_type::TransactionType;
#[component]
pub fn HoldingsTable(positions: Vec<Position>, loaded: bool) -> Element {
    rsx! {
        table { class: "w-full text-xs mt-4",
            thead {
                tr { class: "text-ctp-overlay0 border-b border-ctp-surface1",
                    th { class: "py-2 pr-6 text-left  font-semibold uppercase tracking-wider",
                        "Ticker"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Shares"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Avg Cost"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Total Cost"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Value"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Market Price"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "P&L"
                    }
                    th { class: "py-2 text-right      font-semibold uppercase tracking-wider",
                        "Day"
                    }
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
                            if pos.current_price > Decimal::ZERO {
                                "{fmt_usd(pos.current_price, 2)}"
                            } else {
                                "—"
                            }
                        }
                        td { class: "py-3 pr-6 text-right tabular-nums font-medium",
                            if pos.current_price > Decimal::ZERO {
                                "{fmt_usd(pos.market_value(), 2)}"
                            } else {
                                "—"
                            }
                        }
                        td { class: if pos.unrealized_pnl() >= Decimal::ZERO { "py-3 pr-6 text-right tabular-nums text-ctp-green" } else { "py-3 pr-6 text-right tabular-nums text-ctp-red" },
                            if pos.current_price > Decimal::ZERO {
                                "{fmt_signed(pos.unrealized_pnl(), 2)} ({pos.unrealized_pnl_pct():+.1}%)"
                            } else {
                                "—"
                            }
                        }
                        td { class: if pos.daily_change_pct >= Decimal::ZERO { "py-3 text-right tabular-nums text-ctp-green" } else { "py-3 text-right tabular-nums text-ctp-red" },
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
