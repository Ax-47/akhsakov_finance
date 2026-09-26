//! Stock screener: filter a market by sector, size, valuation, dividend
//! yield and today's move.

use crate::i18n::tr;
use crate::{
    components::card::{Card, Field, Select, INPUT},
    page::{GhostButton, Page},
    stock_table::StockTable,
};
use dioxus::prelude::*;
use dtos::market::{ScreenFilter, ScreenSort, REGIONS, SCREEN_PAGE_SIZE, SECTORS};

/// Market-cap bands as (label, min, max) in USD.
const SIZES: [(&str, Option<f64>, Option<f64>); 6] = [
    ("Any size", None, None),
    ("$10B and up", Some(10e9), None),
    ("Mega · $200B+", Some(200e9), None),
    ("Large · $10–200B", Some(10e9), Some(200e9)),
    ("Mid · $2–10B", Some(2e9), Some(10e9)),
    ("Small · under $2B", None, Some(2e9)),
];

/// One-click starting points.
fn presets() -> Vec<(&'static str, ScreenFilter)> {
    let base = ScreenFilter::default();
    vec![
        ("Largest", base.clone()),
        (
            "Top gainers today",
            ScreenFilter {
                min_market_cap: Some(2e9),
                sort: ScreenSort::Change,
                ..base.clone()
            },
        ),
        (
            "Top losers today",
            ScreenFilter {
                min_market_cap: Some(2e9),
                sort: ScreenSort::Change,
                descending: false,
                ..base.clone()
            },
        ),
        (
            "Dividend payers",
            ScreenFilter {
                min_dividend_yield: Some(3.0),
                sort: ScreenSort::DividendYield,
                ..base.clone()
            },
        ),
        (
            "Value · P/E under 15",
            ScreenFilter {
                min_pe: Some(0.1),
                max_pe: Some(15.0),
                ..base.clone()
            },
        ),
        (
            "Most traded",
            ScreenFilter {
                sort: ScreenSort::Volume,
                ..base
            },
        ),
    ]
}

#[component]
pub fn ScreenerPage() -> Element {
    let mut filter = use_signal(ScreenFilter::default);
    let result = crate::cache::use_cached(
        move || format!("screen/{}", serde_json::to_string(&filter()).unwrap_or_default()),
        move || async move {
        api::screen_stocks(filter()).await.map_err(|e| match e {
            ServerFnError::ServerError { message, .. } => message,
            e => e.to_string(),
        })
    },
    );

    let f = filter();
    let size = SIZES
        .iter()
        .position(|(_, lo, hi)| *lo == f.min_market_cap && *hi == f.max_market_cap);
    let size_value = size.map_or("custom".to_string(), |i| i.to_string());
    let mut size_options: Vec<(String, String)> = SIZES
        .iter()
        .enumerate()
        .map(|(i, (label, ..))| (i.to_string(), label.to_string()))
        .collect();
    if size.is_none() {
        size_options.push(("custom".into(), "Custom".into()));
    }
    let active_preset = presets().into_iter().position(|(_, p)| p == ScreenFilter { page: 0, ..f.clone() });

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("Screener")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0", {tr("Find stocks by size, valuation, dividends and today's move. Amounts in USD.")} }
            }

            div { class: "mt-8 flex flex-wrap gap-2 motion-safe:animate-rise",
                for (i, (label, preset)) in presets().into_iter().enumerate() {
                    button {
                        key: "{label}",
                        class: if active_preset == Some(i) {
                            "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3.5 py-1.5 text-sm font-medium text-ctp-text cursor-pointer"
                        } else {
                            "rounded-full border border-ctp-surface0 px-3.5 py-1.5 text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
                        },
                        onclick: move |_| {
                            let region = filter.peek().region.clone();
                            filter.set(ScreenFilter { region, ..preset.clone() });
                        },
                        "{label}"
                    }
                }
            }

            div { class: "mt-5 motion-safe:animate-rise",
                Card { title: tr("Filters"),
                    div { class: "grid gap-4 sm:grid-cols-2 lg:grid-cols-4",
                        Field { label: tr("Market"),
                            Select {
                                options: REGIONS.iter().map(|(c, n)| (c.to_string(), n.to_string())).collect::<Vec<_>>(),
                                value: f.region.clone(),
                                onchange: move |v| filter.with_mut(|f| { f.region = v; f.page = 0; }),
                            }
                        }
                        Field { label: tr("Sector"),
                            Select {
                                options: std::iter::once(("".to_string(), "Any sector".to_string()))
                                    .chain(SECTORS.iter().map(|s| (s.to_string(), s.to_string())))
                                    .collect::<Vec<_>>(),
                                value: f.sector.clone().unwrap_or_default(),
                                onchange: move |v: String| filter.with_mut(|f| { f.sector = (!v.is_empty()).then_some(v); f.page = 0; }),
                            }
                        }
                        Field { label: tr("Size"),
                            Select {
                                options: size_options,
                                value: size_value,
                                onchange: move |v: String| {
                                    if let Some((_, lo, hi)) = v.parse::<usize>().ok().and_then(|i| SIZES.get(i)) {
                                        filter.with_mut(|f| { f.min_market_cap = *lo; f.max_market_cap = *hi; f.page = 0; });
                                    }
                                },
                            }
                        }
                        Field { label: tr("Sort by"),
                            div { class: "flex gap-2",
                                div { class: "flex-1",
                                    Select {
                                        options: ScreenSort::ALL.iter().map(|s| (format!("{s:?}"), tr(s.label()).to_string())).collect::<Vec<_>>(),
                                        value: format!("{:?}", f.sort),
                                        onchange: move |v: String| {
                                            if let Some(s) = ScreenSort::ALL.into_iter().find(|s| format!("{s:?}") == v) {
                                                filter.with_mut(|f| { f.sort = s; f.page = 0; });
                                            }
                                        },
                                    }
                                }
                                button {
                                    class: "rounded-xl border border-ctp-surface0 px-3 text-sm text-ctp-subtext1 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text",
                                    title: tr("Reverse the order"),
                                    onclick: move |_| filter.with_mut(|f| { f.descending = !f.descending; f.page = 0; }),
                                    if f.descending { "↓" } else { "↑" }
                                }
                            }
                        }
                        RangeField {
                            label: tr("P/E"),
                            min: f.min_pe,
                            max: f.max_pe,
                            onchange: move |(lo, hi)| filter.with_mut(|f| { f.min_pe = lo; f.max_pe = hi; f.page = 0; }),
                        }
                        RangeField {
                            label: tr("Today's change, %"),
                            min: f.min_change,
                            max: f.max_change,
                            onchange: move |(lo, hi)| filter.with_mut(|f| { f.min_change = lo; f.max_change = hi; f.page = 0; }),
                        }
                        Field { label: tr("Dividend yield at least, %"),
                            NumberInput {
                                value: f.min_dividend_yield,
                                placeholder: tr("Any"),
                                onchange: move |v| filter.with_mut(|f| { f.min_dividend_yield = v; f.page = 0; }),
                            }
                        }
                        div { class: "flex items-end",
                            GhostButton { label: tr("Reset"), onclick: move |_| filter.set(ScreenFilter::default()) }
                        }
                    }
                }
            }

            div { class: "mt-5 motion-safe:animate-rise",
                match &*result.read() {
                    None => rsx! {
                        Card { title: tr("Results"), p { class: "py-10 text-center text-sm text-ctp-subtext0", {tr("Screening…")} } }
                    },
                    Some(Err(message)) => rsx! {
                        Card { title: tr("Results"), p { class: "text-sm text-ctp-red", "{message}" } }
                    },
                    Some(Ok(r)) => {
                        let from = f.page * SCREEN_PAGE_SIZE + 1;
                        let to = (from - 1 + r.rows.len() as u32).max(from.min(r.total));
                        let pages = r.total.div_ceil(SCREEN_PAGE_SIZE);
                        rsx! {
                            Card {
                                title: tr("Results"),
                                subtitle: if r.total == 0 { "No stocks match".to_string() } else { format!("{from}–{to} of {}", r.total) },
                                flush: true,
                                actions: rsx! {
                                    if pages > 1 {
                                        div { class: "flex items-center gap-2 text-xs text-ctp-subtext0",
                                            GhostButton { label: tr("← Prev"), onclick: move |_| filter.with_mut(|f| f.page = f.page.saturating_sub(1)) }
                                            "Page {f.page + 1} of {pages}"
                                            GhostButton { label: tr("Next →"), onclick: move |_| filter.with_mut(|f| f.page = (f.page + 1).min(pages - 1)) }
                                        }
                                    }
                                },
                                if r.rows.is_empty() {
                                    p { class: "px-6 pb-8 text-sm text-ctp-subtext0", {tr("Try loosening a filter.")} }
                                } else {
                                    StockTable { rows: r.rows.clone() }
                                    div { class: "h-3" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Min / max pair of optional numbers.
#[component]
fn RangeField(
    label: String,
    min: Option<f64>,
    max: Option<f64>,
    onchange: EventHandler<(Option<f64>, Option<f64>)>,
) -> Element {
    rsx! {
        Field { label,
            div { class: "flex items-center gap-2",
                NumberInput { value: min, placeholder: tr("Min"), onchange: move |v| onchange.call((v, max)) }
                span { class: "text-ctp-overlay1", "–" }
                NumberInput { value: max, placeholder: tr("Max"), onchange: move |v| onchange.call((min, v)) }
            }
        }
    }
}

/// Text box for an optional number; applies when you leave it or press ↵.
#[component]
fn NumberInput(value: Option<f64>, placeholder: String, onchange: EventHandler<Option<f64>>) -> Element {
    let shown = value.map(|v| format!("{v}")).unwrap_or_default();
    rsx! {
        input {
            class: "{INPUT} tabular-nums",
            inputmode: "decimal",
            placeholder,
            value: "{shown}",
            onchange: move |e| onchange.call(e.value().trim().replace(',', "").parse::<f64>().ok()),
        }
    }
}
