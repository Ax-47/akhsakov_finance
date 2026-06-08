use std::collections::HashMap;

use dioxus::prelude::*;
use dtos::{portfolio::GetDashBoardResponse, position::Position};
use rust_decimal::Decimal;

use crate::components::tables::{HoldingsTable, PortfoliosTable};
#[component]
pub fn DashboardTable(
    data: Signal<GetDashBoardResponse>,
    price_map: HashMap<String, Decimal>,
    change_map: HashMap<String, Decimal>,
    positions: Vec<Position>,
    loaded: bool,
) -> Element {
    let active_tab = use_signal(|| "My Portfolios".to_string());
    rsx! {
        div { class: "px-6 pb-10",
            div { class: "flex border-b border-ctp-surface1",
                TabButton { label: "My Portfolios".to_string(), active_tab }
                TabButton { label: "My Holdings".to_string(), active_tab }
            }

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

#[component]
fn TabButton(label: String, mut active_tab: Signal<String>) -> Element {
    let label_for_memo = label.clone();
    let is_active = use_memo(move || active_tab() == label_for_memo);
    rsx! {
        button {
            class: if is_active() { "px-5 py-3 text-sm font-medium cursor-pointer border-b-2 border-ctp-blue text-white" } else { "px-5 py-3 text-sm font-medium cursor-pointer border-b-2 border-transparent text-subtext0 hover:text-white" },
            onclick: move |_| active_tab.set(label.clone()),
            "{label}"
        }
    }
}
