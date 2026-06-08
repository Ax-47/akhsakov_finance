pub mod growth_chart;
pub use growth_chart::*;
pub mod period_button;
pub use period_button::*;
pub mod allocation_card;
pub use allocation_card::*;
pub mod pie_chart;

use dioxus::prelude::*;
use dtos::Transaction;
use rust_decimal::Decimal;

use rust_decimal_macros::dec;

#[component]
pub fn ChartSection(
    transactions: ReadSignal<Vec<Transaction>>,
    pnl_pct: Decimal,
    is_positive: bool,
    height: Decimal,
) -> Element {
    let active_period = use_signal(|| "6M".to_string());
    let line_color = if is_positive { "#89b4fa" } else { "#f38ba8" };
    let history = use_resource(move || {
        let txs = transactions.read().clone();
        let period = active_period();
        async move {
            api::get_portfolio_history(txs, period)
                .await
                .unwrap_or_default()
        }
    });

    let history_data = history();
    let loading = history_data.is_none();
    let has_data = history_data.as_ref().map_or(false, |d| !d.is_empty());
    println!("{:?}", history_data.clone());
    let (chart_dates, chart_values) = match history_data {
        Some(data) if !data.is_empty() => (
            data.iter().map(|(d, _)| d.clone()).collect::<Vec<_>>(),
            data.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        ),
        _ => (vec![], vec![]),
    };
    let period_key = active_period.read().clone();

    rsx! {
        div { class: "border-b border-ctp-surface0",
            div { class: "flex items-center justify-between px-6 pt-4 pb-3",
                div { class: "flex items-center gap-0.5",
                    for p in ["1D", "5D", "1M", "6M", "YTD", "1Y", "All"] {
                        PeriodButton { label: p.to_string(), active_period }
                    }
                }
            }

            if loading {
                div {
                    class: "flex items-center justify-center text-ctp-subtext0 text-xs",
                    style: "height:{height}px;",
                    "Loading…"
                }
            } else if !has_data {
                div {
                    class: "flex items-center justify-center text-ctp-subtext0 text-xs",
                    style: "height:{height}px;",
                    "No data"
                }
            } else {
                GrowthChart {
                    key: "{period_key}",
                    active_period,
                    chart_dates,
                    series: vec![
                        Series {
                            name: "Portfolio".into(),
                            color: line_color.into(),
                            values: chart_values,
                        },
                    ],
                    height,
                }
            }
        }
    }
}
