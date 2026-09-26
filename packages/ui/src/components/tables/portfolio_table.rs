use crate::i18n::tr;
use crate::{
    app::PortfolioScope,
    components::card::Card,
    editors::{Dialog, Dialogs},
    format::{fmt_signed, fmt_usd, signed_color},
    LiveNumber,
};
use dioxus::prelude::*;
use dtos::{
    asset::get_asset_response::GetAssetResponse, portfolio::GetDashBoardResponse,
    position::realized_pnl,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

/// One row per portfolio: value, today, return, realized and its share of
/// everything you own.
#[component]
pub fn PortfoliosCard(
    data: Signal<GetDashBoardResponse>,
    price_map: HashMap<TickerSymbol, Decimal>,
    change_map: HashMap<TickerSymbol, Decimal>,
    loaded: bool,
) -> Element {
    let rows: Vec<PortfolioRow> = data
        .read()
        .portfolios
        .iter()
        .map(|port| {
            let (count, cost, value, day, pnl) =
                portfolio_stats(&port.assets, &price_map, &change_map);
            PortfolioRow {
                id: port.id.to_string(),
                name: port.name.clone(),
                count,
                value,
                day,
                day_pct: if value > Decimal::ZERO {
                    day / value * dec!(100)
                } else {
                    Decimal::ZERO
                },
                pnl,
                pnl_pct: if cost > Decimal::ZERO {
                    pnl / cost * dec!(100)
                } else {
                    Decimal::ZERO
                },
                realized: realized_pnl(
                    &data
                        .read()
                        .transactions
                        .iter()
                        .filter(|tx| tx.portfolio_id == port.id)
                        .cloned()
                        .collect::<Vec<_>>(),
                ),
            }
        })
        .collect();
    let grand_total: Decimal = rows.iter().map(|r| r.value).sum();

    rsx! {
        Card {
            title: tr("Portfolios"),
            subtitle: format!("{} portfolios", rows.len()),
            flush: true,
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-overlay1",
                            th { class: "pl-6 pr-4 py-2.5 text-left font-medium", {tr("Portfolio")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Value")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Today")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Return")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Realized")} }
                            th { class: "pl-4 pr-6 py-2.5 text-right font-medium", {tr("Share")} }
                        }
                    }
                    tbody {
                        for row in rows {
                            PortfolioRowView {
                                key: "{row.id}",
                                share: if grand_total > Decimal::ZERO { row.value / grand_total * dec!(100) } else { Decimal::ZERO },
                                row,
                                loaded,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Rename / delete, shown when the row is hovered.
#[component]
fn RowActions(id: String, name: String) -> Element {
    let dialogs = use_context::<Dialogs>();
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return rsx! {};
    };
    let rename_name = name.clone();
    rsx! {
        span { class: "ml-auto flex gap-1 opacity-0 transition-opacity group-hover:opacity-100",
            button {
                class: "rounded-full px-2 py-1 text-xs text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                title: tr("Rename"),
                onclick: move |e| {
                    e.stop_propagation();
                    dialogs.open(Dialog::RenamePortfolio(uuid, rename_name.clone()));
                },
                "✎"
            }
            button {
                class: "rounded-full px-2 py-1 text-xs text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                title: tr("Delete"),
                onclick: move |e| {
                    e.stop_propagation();
                    dialogs.open(Dialog::DeletePortfolio(uuid, name.clone()));
                },
                "🗑"
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct PortfolioRow {
    id: String,
    name: String,
    count: usize,
    value: Decimal,
    day: Decimal,
    day_pct: Decimal,
    pnl: Decimal,
    pnl_pct: Decimal,
    realized: Decimal,
}

#[component]
fn PortfolioRowView(row: PortfolioRow, share: Decimal, loaded: bool) -> Element {
    let PortfolioScope(mut scope) = use_context::<PortfolioScope>();
    let id = row.id.clone();
    let open = move |_| {
        scope.set(Some(id.clone()));
        navigator().push("/portfolio");
    };
    let initial = row
        .name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let priced = loaded && row.value > Decimal::ZERO;
    let cell = "px-4 py-3.5 text-right tabular-nums";

    rsx! {
        tr {
            class: "group border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors cursor-pointer",
            title: "Open {row.name}",
            onclick: open,
            td { class: "pl-6 pr-4 py-3.5",
                div { class: "flex items-center gap-3",
                    span { class: "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl \
                                   bg-gradient-to-br from-ctp-mauve/30 to-ctp-sky/30 font-semibold text-ctp-text",
                        "{initial}"
                    }
                    div {
                        div { class: "flex items-center gap-1.5 font-semibold text-ctp-text",
                            "{row.name}"
                            span { class: "text-ctp-overlay0 opacity-0 transition-opacity group-hover:opacity-100", "→" }
                        }
                        div { class: "text-xs text-ctp-overlay1", "{row.count} holdings" }
                    }
                    RowActions { id: row.id.clone(), name: row.name.clone() }
                }
            }
            if priced {
                td { class: "{cell} font-medium text-ctp-text",
                    LiveNumber { value: row.value, text: fmt_usd(row.value, 2) }
                }
                td { class: "{cell} {signed_color(row.day)}",
                    div { LiveNumber { value: row.day, text: fmt_signed(row.day, 2) } }
                    div { class: "text-xs opacity-75", "{row.day_pct:+.2}%" }
                }
                td { class: "{cell} {signed_color(row.pnl)}",
                    div { class: "font-medium", LiveNumber { value: row.pnl, text: fmt_signed(row.pnl, 2) } }
                    div { class: "text-xs opacity-75", "{row.pnl_pct:+.2}%" }
                }
            } else {
                for _ in 0..3 {
                    td { class: "{cell} text-ctp-overlay0", "—" }
                }
            }
            td { class: "{cell} {signed_color(row.realized)}",
                if row.realized.abs() > dec!(0.01) { "{fmt_signed(row.realized, 2)}" } else { span { class: "text-ctp-overlay0", "—" } }
            }
            td { class: "pl-4 pr-6 py-3.5 text-right",
                span { class: "inline-flex items-center justify-end gap-2",
                    span { class: "h-1 w-16 rounded-full bg-ctp-surface0 overflow-hidden",
                        span { class: "block h-full rounded-full bg-gradient-to-r from-ctp-mauve to-ctp-sky", style: "width:{share.min(dec!(100)):.0}%;" }
                    }
                    span { class: "w-11 tabular-nums text-ctp-subtext0", "{share:.1}%" }
                }
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn portfolio_stats(
    assets: &[GetAssetResponse],
    prices: &HashMap<TickerSymbol, Decimal>,
    changes: &HashMap<TickerSymbol, Decimal>,
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
                .get(&a.ticker_symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            a.quantity.value() * price
        })
        .sum();

    let day_change: Decimal = assets
        .iter()
        .map(|a| {
            let price = prices
                .get(&a.ticker_symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            let chg_pct = changes
                .get(&a.ticker_symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            a.quantity.value() * price * chg_pct / dec!(100)
        })
        .sum();

    let total_pnl = total_value - total_cost;
    (pos_count, total_cost, total_value, day_change, total_pnl)
}
