pub mod growth_chart;
pub mod period_button;
use dtos::Transaction;
pub use growth_chart::*;
pub use period_button::*;

use dioxus::prelude::*;
use rust_decimal::Decimal;

#[component]
pub fn ChartSection(
    transactions: Vec<Transaction>,
    pnl_pct: Decimal,
    is_positive: bool,
    height: Decimal,
) -> Element {
    let active_period = use_signal(|| "6M".to_string());
    let line_color = if is_positive { "#89b4fa" } else { "#f38ba8" };
    let history = use_resource(move || {
        let txs = transactions.clone();
        let period = active_period();
        async move {
            api::get_portfolio_history(txs, period)
                .await
                .unwrap_or_default()
        }
    });
    let (chart_dates, chart_values) = match history() {
        Some(data) if !data.is_empty() => (
            data.iter().map(|(d, _)| d.clone()).collect::<Vec<_>>(),
            data.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        ),
        _ => (vec![], vec![]),
    };

    let loading = history().is_none();
    rsx! {
        div {
            class: "border-b border-ctp-surface0",
            div {
                class: "flex items-center justify-between px-6 pt-4 pb-3",
                div {
                    class: "flex items-center gap-0.5",
                    for p in ["1D", "5D", "1M", "6M", "YTD", "1Y", "All"] {
                        PeriodButton { label: p.to_string(), active_period, }
                    }
                }
            }

            if loading {
                div {
                    class: "flex items-center justify-center text-ctp-subtext0 text-xs",
                    style: "height:{height}px;",
                    "Loading…"
                }
            } else {
                GrowthChart {
                    active_period,
                    chart_dates,
                    series: vec![
                            Series {
                                name:   "Portfolio".into(),
                                color:  line_color.into(),
                                values: chart_values,
                            },
                        ],
                    height,
                }
            }
        }
    }
}

// ─── growth_path (unchanged) ───────────────────────────────────────────────────

fn growth_path(period: &str, pnl_pct: f64) -> (Vec<String>, Vec<f64>) {
    use std::f64::consts::PI;

    let (n, labels): (usize, Vec<String>) = match period {
        "1D" => (24, (0..24).map(|h| format!("{h:02}:00")).collect()),
        "5D" => {
            let n = 40;
            (
                n,
                (0..n)
                    .map(|i| format!("D{} {:02}h", i / 8 + 1, (i % 8) * 2))
                    .collect(),
            )
        }
        "1M" => (30, (1..=30).map(|d| format!("May {d}")).collect()),
        "1Y" => (
            12,
            [
                "Jun '25", "Jul '25", "Aug '25", "Sep '25", "Oct '25", "Nov '25", "Dec '25",
                "Jan '26", "Feb '26", "Mar '26", "Apr '26", "May '26",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        "YTD" => (
            6,
            [
                "Jan '26", "Feb '26", "Mar '26", "Apr '26", "May '26", "Jun '26",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        "All" => {
            let n = 24;
            (
                n,
                (0..n)
                    .map(|i| format!("{}-{:02}", 2024 + i / 12, i % 12 + 1))
                    .collect(),
            )
        }
        _ => {
            // "6M"
            let n = 26;
            (
                n,
                (0..n)
                    .map(|i| {
                        let months = ["Dec", "Jan", "Feb", "Mar", "Apr", "May", "Jun"];
                        format!("{} W{}", months[(i / 4).min(6)], i % 4 + 1)
                    })
                    .collect(),
            )
        }
    };

    let mut raw: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1).max(1) as f64;
            t + (t * PI * 3.5).sin() * 0.18 + (t * PI * 7.1).sin() * 0.07
        })
        .collect();

    if let Some(&last) = raw.last() {
        if last.abs() > 1e-9 {
            let scale = pnl_pct / last;
            for v in &mut raw {
                *v *= scale;
            }
        }
    }
    if let Some(last) = raw.last_mut() {
        *last = pnl_pct;
    }

    (labels, raw)
}
