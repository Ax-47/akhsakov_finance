//! Dividend income (received and forecast) and what drove your return.

use crate::i18n::tr;
use crate::{
    components::{
        analysis::stock::open_stock,
        card::{Card, MetricTile, Segmented, ToggleButton},
        charts::bars::{BarChart, BarSeries, Unit},
    },
    format::{fmt_signed, fmt_usd},
};
use dioxus::prelude::*;
use dtos::{
    insights::{dividends_by_month, gains_by_ticker, income_forecast, TickerGain},
    Position, Transaction,
};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::HashMap;
use types::ticker_symbol::TickerSymbol;

/// Months of dividend history in the chart.
const HISTORY_MONTHS: usize = 24;

fn money(v: f64) -> String {
    fmt_usd(Decimal::try_from(v).unwrap_or_default(), 2)
}

fn pct(v: Option<f64>) -> String {
    v.map_or("—".into(), |v| format!("{:.2}%", v * 100.0))
}

#[component]
pub fn IncomeTab(positions: Vec<Position>, transactions: Vec<Transaction>, total_value: Decimal) -> Element {
    let tickers: Vec<TickerSymbol> = positions.iter().map(|p| p.ticker.clone()).collect();
    let tickers = use_memo(use_reactive!(|tickers| tickers));
    let infos = crate::cache::use_cached(move || format!("dividends/{:?}", tickers()), move || async move {
        let tickers = tickers();
        if tickers.is_empty() {
            return Ok(vec![]);
        }
        api::get_dividends(tickers).await.map_err(|e| e.to_string())
    });

    let received = dividends_by_month(&transactions);
    let last_year: f64 = received.iter().rev().take(12).map(|(_, v)| v).sum();
    let all_time: f64 = received.iter().map(|(_, v)| v).sum();
    let shown = &received[received.len().saturating_sub(HISTORY_MONTHS)..];

    let forecast = match &*infos.read() {
        Some(Ok(infos)) => Some(income_forecast(&positions, infos)),
        _ => None,
    };
    let annual: f64 = forecast.iter().flatten().map(|r| r.annual_income).sum();
    let cost: f64 = positions.iter().filter_map(|p| p.cost_basis().to_f64()).sum();
    let value = total_value.to_f64().unwrap_or(0.0);

    rsx! {
        div { class: "grid grid-cols-2 gap-3 lg:grid-cols-4",
            MetricTile {
                label: tr("Expected next 12 months"),
                value: if forecast.is_some() { money(annual) } else { "…".to_string() },
                hint: format!("≈ {} a month", money(annual / 12.0)),
                tone: "text-ctp-green",
            }
            MetricTile {
                label: tr("Portfolio yield"),
                value: pct((value > 0.0).then(|| annual / value)),
                hint: tr("Expected income ÷ value today"),
            }
            MetricTile {
                label: tr("Yield on cost"),
                value: pct((cost > 0.0).then(|| annual / cost)),
                hint: tr("Expected income ÷ what you paid"),
            }
            MetricTile {
                label: tr("Received, last 12 months"),
                value: money(last_year),
                hint: format!("{} since you started", money(all_time)),
            }
        }

        Card { title: tr("Dividends received"), subtitle: tr("Net of tax withheld, per month").to_string(),
            if shown.is_empty() {
                p { class: "text-sm text-ctp-subtext0",
                    {tr("No dividends recorded yet. Add them with ＋ Transaction → Dividend, or import them from your broker's CSV.")}
                }
            } else {
                BarChart {
                    labels: shown.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>(),
                    series: vec![BarSeries { name: "Received".into(), color: "var(--catppuccin-color-green)", values: shown.iter().map(|(_, v)| Some(*v)).collect() }],
                    unit: Unit::Money,
                }
            }
        }

        Card { title: tr("Expected income by holding"), subtitle: tr("From each company's current dividend rate").to_string(), flush: true,
            match (&forecast, &*infos.read()) {
                (_, Some(Err(e))) => rsx! { p { class: "px-6 pb-6 text-sm text-ctp-red", "Couldn't load dividend rates: {e}" } },
                (None, _) => rsx! { p { class: "px-6 pb-6 text-sm text-ctp-subtext0", {tr("Loading dividend rates…")} } },
                (Some(rows), _) if rows.is_empty() => rsx! { p { class: "px-6 pb-6 text-sm text-ctp-subtext0", {tr("None of your holdings pay a dividend.")} } },
                (Some(rows), _) => rsx! {
                    div { class: "overflow-x-auto",
                        table { class: "w-full min-w-[36rem] text-sm",
                            thead {
                                tr { class: "text-left text-xs text-ctp-subtext0",
                                    th { class: "px-6 py-2 font-normal", {tr("Stock")} }
                                    th { class: "px-3 py-2 text-right font-normal", {tr("Shares")} }
                                    th { class: "px-3 py-2 text-right font-normal", {tr("A year")} }
                                    th { class: "px-3 py-2 text-right font-normal", {tr("Share of income")} }
                                    th { class: "px-3 py-2 text-right font-normal", {tr("Yield on cost")} }
                                    th { class: "px-3 py-2 text-right font-normal", {tr("Yield now")} }
                                    th { class: "px-6 py-2 text-right font-normal", {tr("Next payment")} }
                                }
                            }
                            tbody {
                                for r in rows.clone() {
                                    {
                                        let t = TickerSymbol::new(&r.ticker).ok();
                                        let share = if annual > 0.0 { r.annual_income / annual * 100.0 } else { 0.0 };
                                        rsx! {
                                            tr {
                                                key: "{r.ticker}",
                                                class: "cursor-pointer border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30",
                                                onclick: move |_| if let Some(t) = &t { open_stock(t) },
                                                td { class: "px-6 py-2.5 font-semibold text-ctp-text", "{r.ticker}" }
                                                td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1", {crate::format::fmt_shares(Decimal::try_from(r.shares).unwrap_or_default())} }
                                                td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-green", {money(r.annual_income)} }
                                                td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1", "{share:.1}%" }
                                                td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1", {pct(r.yield_on_cost)} }
                                                td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1", {pct(r.current_yield)} }
                                                td { class: "px-6 py-2.5 text-right tabular-nums text-ctp-subtext0", {r.next_payment.clone().unwrap_or("—".into())} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "h-3" }
                },
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Period {
    AllTime,
    Today,
}

/// Each stock's contribution to your gain, all time or today.
#[component]
pub fn ReturnDrivers(
    transactions: Vec<Transaction>,
    positions: Vec<Position>,
    prices: HashMap<TickerSymbol, Decimal>,
) -> Element {
    let mut period = use_signal(|| Period::AllTime);
    let price_map: HashMap<String, f64> = prices
        .iter()
        .filter_map(|(t, p)| Some((t.to_string(), p.to_f64()?)))
        .collect();
    // (ticker, amount, breakdown)
    let rows: Vec<(String, f64, String)> = match period() {
        Period::AllTime => gains_by_ticker(&transactions, &price_map)
            .into_iter()
            .filter(|g| g.total().abs() > 0.005)
            .map(|g: TickerGain| {
                let detail = [
                    ("price", g.unrealized),
                    ("sales", g.realized),
                    ("dividends", g.dividends),
                ]
                .iter()
                .filter(|(_, v)| v.abs() > 0.005)
                .map(|(what, v)| format!("{} {what}", fmt_signed(Decimal::try_from(*v).unwrap_or_default(), 2)))
                .collect::<Vec<_>>()
                .join(" · ");
                (g.ticker.clone(), g.total(), detail)
            })
            .collect(),
        Period::Today => {
            let mut rows: Vec<(String, f64, String)> = positions
                .iter()
                .filter(|p| p.current_price > Decimal::ZERO)
                .filter_map(|p| {
                    let prev = p.current_price / (Decimal::ONE + p.daily_change_pct / Decimal::ONE_HUNDRED);
                    let change = (p.shares * (p.current_price - prev)).to_f64()?;
                    Some((p.ticker.to_string(), change, format!("{:+.2}% today", p.daily_change_pct)))
                })
                .collect();
            rows.sort_by(|a, b| b.1.total_cmp(&a.1));
            rows
        }
    };
    let total: f64 = rows.iter().map(|r| r.1).sum();
    let scale = rows.iter().map(|r| r.1.abs()).fold(0.0, f64::max).max(1e-9);

    rsx! {
        Card {
            title: tr("What drove your return"),
            subtitle: format!("{} in total", fmt_signed(Decimal::try_from(total).unwrap_or_default(), 2)),
            actions: rsx! {
                Segmented {
                    ToggleButton { label: tr("All time"), active: period() == Period::AllTime, onclick: move |_| period.set(Period::AllTime) }
                    ToggleButton { label: tr("Today"), active: period() == Period::Today, onclick: move |_| period.set(Period::Today) }
                }
            },
            if rows.is_empty() {
                p { class: "text-sm text-ctp-subtext0", {tr("Nothing yet.")} }
            }
            div { class: "grid gap-1.5",
                for (ticker, amount, detail) in rows {
                    {
                        let width = amount.abs() / scale * 50.0;
                        let (bar, text) = if amount >= 0.0 { ("bg-ctp-green/70", "text-ctp-green") } else { ("bg-ctp-red/70", "text-ctp-red") };
                        // Gains grow right from the middle, losses left.
                        let bar_style = if amount >= 0.0 { format!("left:50%;width:{width:.1}%;") } else { format!("right:50%;width:{width:.1}%;") };
                        let t = TickerSymbol::new(&ticker).ok();
                        rsx! {
                            button {
                                key: "{ticker}",
                                class: "grid grid-cols-[4.5rem_1fr_7rem] items-center gap-3 rounded-lg px-1 py-1 text-left cursor-pointer hover:bg-ctp-surface0/40",
                                title: "{detail}",
                                onclick: move |_| if let Some(t) = &t { open_stock(t) },
                                span { class: "text-sm font-semibold text-ctp-text", "{ticker}" }
                                span { class: "relative h-3",
                                    span { class: "absolute inset-y-0 left-1/2 w-px bg-ctp-surface1" }
                                    span { class: "absolute inset-y-0 rounded-sm {bar}", style: bar_style }
                                }
                                span { class: "text-right text-sm tabular-nums {text}", {fmt_signed(Decimal::try_from(amount).unwrap_or_default(), 2)} }
                            }
                        }
                    }
                }
            }
        }
    }
}
