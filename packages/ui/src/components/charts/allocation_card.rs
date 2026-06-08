use crate::components::color_schema::CHART_COLOR_CLASSES;

use super::pie_chart::PieChart;
use dioxus::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
#[component]
pub fn AllocationCard(allocation: Vec<(String, Decimal)>) -> Element {
    let allocation_data = allocation;
    rsx! {
        div {
            class: "rounded-xl bg-ctp-base border border-ctp-surface0 p-4",

            div {
                class: "flex items-center gap-2 mb-4",

                span {
                    class: "inline-block w-[3px] h-[14px] rounded-[2px] bg-ctp-mauve shrink-0"
                }

                span {
                    class: "text-xs font-bold uppercase tracking-wide",
                    "Allocation"
                }
            }

            if allocation_data.is_empty() {
                div {
                    class: "flex items-center justify-center h-24 text-ctp-overlay0",
                    "No data"
                }
            } else {
                PieChart  {
                    data: allocation_data
                        .iter()
                        .map(|(ticker, pct)| (ticker.clone(), *pct))
                        .collect::<Vec<_>>(),
                    size: dec!(160.0),
                }

                div {
                    class: "mt-4 flex flex-col gap-2",

                    for (i, (ticker, pct)) in allocation_data.iter().enumerate() {
                        AllocationLegendItem {
                            key: "{ticker}",
                            ticker: ticker.clone(),
                            percentage: *pct,
                            color_class: CHART_COLOR_CLASSES[i % CHART_COLOR_CLASSES.len()],
                        }
                    }
                }
            }
        }
    }
}
#[component]
fn AllocationLegendItem(ticker: String, percentage: Decimal, color_class: String) -> Element {
    rsx! {
        div {
            class: "flex items-center justify-between text-xs",

            div {
                class: "flex items-center gap-1.5",

                div {
                    class: "w-2 h-2 rounded-full shrink-0 {color_class}"
                }

                span {
                    class: "text-ctp-subtext1",
                    "{ticker}"
                }
            }

            div {
                class: "flex items-center gap-2",

                div {
                    class: "{color_class} h-1 min-w-1 max-w-[60px] rounded-[2px]",
                    style: "width:{(percentage * dec!(0.6)).round():.0}px;",
                }

                span {
                    class: "tabular-nums text-ctp-subtext0 w-10 text-right",
                    "{percentage:.1}%"
                }
            }
        }
    }
}
