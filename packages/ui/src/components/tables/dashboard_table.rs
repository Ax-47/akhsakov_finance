use std::collections::HashMap;

use dioxus::prelude::*;
use dtos::{portfolio::GetDashBoardResponse, position::Position};
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;

use crate::components::tables::{HoldingsTable, PortfoliosTable};
#[component]
pub fn DashboardTable(
    data: Signal<GetDashBoardResponse>,
    price_map: HashMap<TickerSymbol, Decimal>,
    change_map: HashMap<TickerSymbol, Decimal>,
    positions: Vec<Position>,
    loaded: bool,
) -> Element {
    let active_tab = use_signal(|| "My Portfolios".to_string());
    rsx! {
        div { class: "px-6 pb-10",
            div { class: "ak-tabs ak-glass",
                TabButton { label: "My Portfolios".to_string(), active_tab }
                TabButton { label: "My Holdings".to_string(), active_tab }
            }

            div { class: "ak-glass ak-card px-4 pb-4 ak-fade",
            if active_tab() == "My Portfolios" {
                PortfoliosTable {
                    data,
                    price_map,
                    change_map,
                    loaded,
                }
            } else {
                HoldingsTable { positions, loaded }
            }
            }
        }
    }
}

#[component]
fn TabButton(label: String, mut active_tab: Signal<String>) -> Element {
    let label_for_memo = label.clone();
    let is_active = use_memo(move || active_tab() == label_for_memo);
    rsx! {
        button {
            class: if is_active() { "ak-tab active" } else { "ak-tab" },
            onclick: move |_| active_tab.set(label.clone()),
            "{label}"
        }
    }
}
