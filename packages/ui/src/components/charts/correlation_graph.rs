use crate::i18n::tr;
use super::force_graph::ForceGraph;
use crate::components::{
    analysis::stats::{correlation_matrix, daily_returns, Returns},
    card::{Card, Segmented, ToggleButton},
};
use api::quote::quote::get_charts;
use dioxus::prelude::*;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::HashMap;
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

/// Graph of holdings where correlated stocks sit close together and
/// uncorrelated / inversely correlated ones are pushed apart.
///
/// `allocation` is `(ticker, weight %)` in the same order as the allocation
/// card, so node colours match its legend.
#[component]
pub fn CorrelationGraph(allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>) -> Element {
    let mut range = use_signal(|| Range::Y1);
    let hovered = use_signal(|| None::<usize>);

    // Only refetch when the set of tickers changes, not on every price tick.
    let tickers = use_memo(move || {
        allocation
            .read()
            .iter()
            .map(|(t, _)| t.clone())
            .collect::<Vec<_>>()
    });
    let weights = use_memo(move || {
        allocation
            .read()
            .iter()
            .map(|(_, w)| w.to_f64().unwrap_or(0.0))
            .collect::<Vec<_>>()
    });

    let history = crate::cache::use_cached(move || format!("correlation/{:?}/{:?}", tickers(), range()), move || {
        let tickers = tickers();
        let range = range();
        async move {
            if tickers.len() < 2 {
                return HashMap::new();
            }
            get_charts(tickers, range, Interval::D1, false)
                .await
                .unwrap_or_default()
        }
    });

    let corr = use_memo(move || {
        let history = history.read();
        let history = history.as_ref()?;
        let returns: Vec<Returns> = tickers
            .read()
            .iter()
            .map(|t| history.get(t).map(|c| daily_returns(c)).unwrap_or_default())
            .collect();
        Some(correlation_matrix(&returns))
    });

    let tickers_now = tickers();

    rsx! {
        Card {
            title: tr("Relations"),
            subtitle: tr("Stocks that move together sit close — drag to explore").to_string(),
            actions: rsx! {
                Segmented {
                    ToggleButton { label: "3M", active: range() == Range::M3, onclick: move |_| range.set(Range::M3) }
                    ToggleButton { label: "1Y", active: range() == Range::Y1, onclick: move |_| range.set(Range::Y1) }
                }
            },

            match corr() {
                _ if tickers_now.len() < 2 => rsx! {
                    GraphPlaceholder { text: "Need at least two priced holdings" }
                },
                None => rsx! { GraphPlaceholder { text: "Loading price history…" } },
                Some(corr) => rsx! {
                    div { class: "flex flex-col lg:flex-row gap-4",
                        ForceGraph { tickers: tickers_now.clone(), corr: corr.clone(), weights, hovered }
                        PairList { tickers: tickers_now.clone(), corr, hovered: hovered() }
                    }
                    GraphLegend {}
                },
            }
        }
    }
}

#[component]
fn GraphPlaceholder(text: String) -> Element {
    rsx! {
        div { class: "flex items-center justify-center h-[300px] text-ctp-overlay1 text-xs", "{text}" }
    }
}

/// Strongest and weakest pairs, or the hovered stock's pairs.
#[component]
fn PairList(tickers: Vec<TickerSymbol>, corr: Vec<Vec<f64>>, hovered: Option<usize>) -> Element {
    let mut pairs: Vec<(usize, usize, f64)> = (0..tickers.len())
        .flat_map(|i| ((i + 1)..tickers.len()).map(move |j| (i, j)))
        .filter(|&(i, j)| hovered.is_none_or(|h| h == i || h == j))
        .map(|(i, j)| (i, j, corr[i][j]))
        .collect();
    pairs.sort_by(|a, b| b.2.total_cmp(&a.2));

    let (title, shown): (String, Vec<_>) = match hovered {
        Some(h) => (format!("{} vs others", tickers[h]), pairs),
        None if pairs.len() > 6 => {
            let mut shown = pairs[..3].to_vec();
            shown.extend_from_slice(&pairs[pairs.len() - 3..]);
            ("Most / least related".into(), shown)
        }
        None => ("All pairs".into(), pairs),
    };

    rsx! {
        div { class: "lg:w-52 shrink-0 flex flex-col gap-1.5 text-xs",
            div { class: "text-ctp-subtext0 font-semibold uppercase tracking-wide mb-1", "{title}" }
            for (i, j, c) in shown {
                div { key: "{i}-{j}", class: "flex items-center justify-between gap-2",
                    span { class: "text-ctp-subtext1 truncate", "{tickers[i]} · {tickers[j]}" }
                    div { class: "flex items-center gap-2 shrink-0",
                        div { class: "w-12 h-1 rounded-sm bg-ctp-surface0 overflow-hidden",
                            div {
                                class: if c >= 0.0 { "h-full bg-ctp-green" } else { "h-full bg-ctp-red" },
                                style: "width:{c.abs() * 100.0:.0}%;",
                            }
                        }
                        span {
                            class: if c >= 0.0 { "tabular-nums w-10 text-right text-ctp-green" } else { "tabular-nums w-10 text-right text-ctp-red" },
                            "{c:+.2}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GraphLegend() -> Element {
    rsx! {
        div { class: "flex flex-wrap items-center gap-x-4 gap-y-1 mt-2 text-xs text-ctp-subtext0",
            span { class: "flex items-center gap-1.5",
                span { class: "inline-block w-5 h-[3px] rounded bg-ctp-green" }
                "move together"
            }
            span { class: "flex items-center gap-1.5",
                span { class: "inline-block w-5 border-t-2 border-dashed border-ctp-red" }
                "move opposite"
            }
            span { "closer = more related · circle size = weight · drag stocks around" }
        }
    }
}
