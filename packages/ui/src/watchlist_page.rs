//! Watched stocks with live prices, and every price alert.

use crate::{
    app::DataRefresh,
    components::{
        analysis::stock::open_stock,
        card::{Card, Segmented, ToggleButton},
        charts::{HeatItem, HeatmapLegend, Treemap, DAY_SATURATION},
    },
    editors::{Dialog, Dialogs},
    format::{fmt_compact, fmt_usd, signed_color},
    hooks::use_price_stream,
    page::{GhostButton, Page},
};
use dioxus::prelude::*;
use dtos::watch::Alert;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

#[component]
pub fn WatchlistPage() -> Element {
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let mut adding = use_signal(String::new);

    let watchlist = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_watchlist().await.ok()
    });
    let alerts = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_alerts().await.unwrap_or_default()
    });
    let tickers = use_memo(move || {
        watchlist
            .read()
            .clone()
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .map(|w| w.ticker)
            .collect::<Vec<_>>()
    });
    let (quotes, _) = use_price_stream(tickers, Range::D1, Interval::I2m, false);
    let mut by_cap = use_signal(|| true);
    // Names and market caps; the colour comes from the live price stream.
    let snapshot = use_resource(move || async move {
        let list = tickers();
        if list.is_empty() {
            return vec![];
        }
        api::get_tickers_heatmap(list).await.unwrap_or_default()
    });
    let tiles = use_memo(move || {
        let snapshot = snapshot.read().clone().unwrap_or_default();
        let live = quotes.read();
        // Stocks with an unknown cap get the smallest known one.
        let fallback = snapshot
            .iter()
            .filter_map(|i| i.market_cap)
            .fold(f64::INFINITY, f64::min);
        let fallback = if fallback.is_finite() { fallback } else { 1.0 };
        tickers
            .read()
            .iter()
            .map(|t| {
                let known = snapshot.iter().find(|i| i.ticker == t.as_str());
                let live_change = live
                    .get(t)
                    .filter(|q| !q.previous_close_price.is_zero())
                    .and_then(|q| {
                        ((q.current_price / q.previous_close_price - Decimal::ONE) * Decimal::ONE_HUNDRED)
                            .to_f64()
                    });
                let cap = known.and_then(|i| i.market_cap);
                HeatItem {
                    ticker: t.to_string(),
                    name: known.map_or_else(|| t.to_string(), |i| i.name.clone()),
                    size: if by_cap() { cap.unwrap_or(fallback) } else { 1.0 },
                    change: live_change.or(known.and_then(|i| i.change_pct)),
                    group: None,
                    detail: cap.map_or_else(
                        || "market cap unknown".into(),
                        |c| format!("{} market cap", fmt_compact(c)),
                    ),
                }
            })
            .collect::<Vec<_>>()
    });
    let all_alerts = alerts.read().clone().unwrap_or_default();

    let add = move |_| async move {
        if let Ok(t) = TickerSymbol::new(&adding()) {
            if api::watch_ticker(t).await.is_ok() {
                adding.set(String::new());
                refresh.reload();
            }
        }
    };

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise flex flex-wrap items-end justify-between gap-4",
                div {
                    h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                        "Watchlist"
                    }
                    p { class: "mt-2 text-sm text-ctp-overlay1", "Stocks you follow, and alerts on price moves." }
                }
                GhostButton { label: "🔔 New alert", onclick: move |_| dialogs.open(Dialog::NewAlert(None)) }
            }

            div { class: "mt-10 grid gap-5 motion-safe:animate-rise",
                if !tickers.read().is_empty() {
                    Card {
                        title: "Heatmap",
                        subtitle: "Colour is today's change. Click a stock to open it.".to_string(),
                        actions: rsx! {
                            Segmented {
                                ToggleButton { label: "Market cap", active: by_cap(), onclick: move |_| by_cap.set(true) }
                                ToggleButton { label: "Equal", active: !by_cap(), onclick: move |_| by_cap.set(false) }
                            }
                        },
                        Treemap { items: tiles, saturation: DAY_SATURATION, height: 320 }
                        HeatmapLegend { saturation: DAY_SATURATION }
                    }
                }
                Card {
                    title: "Watching",
                    subtitle: format!("{} stocks", tickers.read().len()),
                    flush: true,
                    actions: rsx! {
                        form {
                            class: "flex gap-2",
                            onsubmit: move |e| e.prevent_default(),
                            input {
                                class: "w-40 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm uppercase text-ctp-text \
                                        placeholder:normal-case placeholder:text-ctp-overlay0 outline-none focus:border-ctp-mauve",
                                placeholder: "Add ticker",
                                value: "{adding}",
                                oninput: move |e| adding.set(e.value()),
                            }
                            GhostButton { label: "Add", onclick: add }
                        }
                    },
                    match watchlist.read().clone() {
                        None => rsx! { p { class: "px-6 pb-8 text-sm text-ctp-overlay1", "Loading…" } },
                        Some(None) => rsx! { p { class: "px-6 pb-8 text-sm text-ctp-red", "Couldn't load your watchlist." } },
                        Some(Some(items)) if items.is_empty() => rsx! {
                            p { class: "px-6 pb-8 text-sm text-ctp-overlay1",
                                "Nothing yet. Add a ticker above, use ☆ Watch on any stock page, or search in the sidebar."
                            }
                        },
                        Some(Some(items)) => rsx! {
                            for item in items {
                                {
                                    let quote = quotes.read().get(&item.ticker).cloned();
                                    let price = quote.as_ref().map(|q| q.current_price);
                                    let day = quote.as_ref().filter(|q| !q.previous_close_price.is_zero()).map(|q| (q.current_price / q.previous_close_price - Decimal::ONE) * Decimal::ONE_HUNDRED);
                                    let count = all_alerts.iter().filter(|a| a.ticker == item.ticker && a.is_active()).count();
                                    rsx! {
                                        WatchRow { key: "{item.ticker}", ticker: item.ticker.clone(), price, day, alerts: count }
                                    }
                                }
                            }
                        },
                    }
                }
                AlertsCard { alerts: all_alerts.clone() }
            }
        }
    }
}

#[component]
fn WatchRow(
    ticker: TickerSymbol,
    price: Option<Decimal>,
    day: Option<Decimal>,
    alerts: usize,
) -> Element {
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let (open, alert, remove) = (ticker.clone(), ticker.clone(), ticker.clone());
    rsx! {
        div {
            class: "group flex items-center gap-4 border-t border-ctp-surface0/60 px-6 py-3.5 cursor-pointer transition-colors hover:bg-ctp-surface0/30",
            onclick: move |_| open_stock(&open),
            span { class: "w-24 font-semibold text-ctp-text", "{ticker}" }
            span { class: "flex-1 text-right tabular-nums text-ctp-text",
                {price.map(|p| fmt_usd(p, 2)).unwrap_or_else(|| "—".into())}
            }
            span { class: "w-20 text-right text-sm tabular-nums {day.map(signed_color).unwrap_or(\"text-ctp-overlay0\")}",
                {day.map(|d| format!("{d:+.2}%")).unwrap_or_else(|| "—".into())}
            }
            span { class: "w-20 text-right text-xs text-ctp-overlay1",
                if alerts > 0 { "🔔 {alerts}" }
            }
            span { class: "flex gap-1 opacity-0 transition-opacity group-hover:opacity-100",
                button {
                    class: "rounded-full px-2 py-1 text-xs text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                    title: "New alert",
                    onclick: move |e| {
                        e.stop_propagation();
                        dialogs.open(Dialog::NewAlert(Some(alert.clone())));
                    },
                    "🔔"
                }
                button {
                    class: "rounded-full px-2 py-1 text-xs text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                    title: "Stop watching",
                    onclick: move |e| {
                        e.stop_propagation();
                        let t = remove.clone();
                        spawn(async move {
                            if api::unwatch_ticker(t).await.is_ok() {
                                refresh.reload();
                            }
                        });
                    },
                    "×"
                }
            }
        }
    }
}

#[component]
fn AlertsCard(alerts: Vec<Alert>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let active = alerts.iter().filter(|a| a.is_active()).count();
    rsx! {
        Card {
            title: "Alerts",
            subtitle: format!("{active} active · {} fired", alerts.len() - active),
            flush: true,
            if alerts.is_empty() {
                p { class: "px-6 pb-8 text-sm text-ctp-overlay1", "No alerts. Create one with 🔔 New alert." }
            }
            for a in alerts {
                div { key: "{a.id}", class: "group flex items-center gap-4 border-t border-ctp-surface0/60 px-6 py-3",
                    span {
                        class: if a.is_active() {
                            "w-16 shrink-0 rounded-full bg-ctp-green/15 px-2 py-0.5 text-center text-[0.68rem] font-semibold text-ctp-green"
                        } else {
                            "w-16 shrink-0 rounded-full bg-ctp-surface0 px-2 py-0.5 text-center text-[0.68rem] font-semibold text-ctp-overlay1"
                        },
                        if a.is_active() { "Active" } else { "Fired" }
                    }
                    span { class: "flex-1 text-sm text-ctp-text", "{a.describe()}" }
                    if let Some(when) = &a.triggered_at {
                        span { class: "text-xs text-ctp-overlay0", "{when}" }
                    }
                    button {
                        class: "rounded-full px-2 py-1 text-xs text-ctp-overlay1 opacity-0 cursor-pointer transition-opacity group-hover:opacity-100 hover:bg-ctp-surface0 hover:text-ctp-red",
                        title: "Delete alert",
                        onclick: move |_| async move {
                            if api::delete_alert(a.id).await.is_ok() {
                                refresh.reload();
                            }
                        },
                        "×"
                    }
                }
            }
        }
    }
}
