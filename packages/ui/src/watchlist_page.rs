//! Watchlists (several, named) with live prices, tags from your notes,
//! and every price alert.

use crate::i18n::tr;
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
use dtos::watch::{Alert, Note, Watchlist};
use uuid::Uuid;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::HashMap;
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

/// Column the list is sorted by.
#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Added,
    Ticker,
    Price,
    Day,
    SinceAdded,
}

/// Change (%) from the first close on or after `added` (`YYYY-MM-DD…`) to
/// `price`. Closes are adjusted for dividends and splits, so this is the
/// total return since the stock was added.
fn since_added(candles: &[types::candle::Candle], added: &str, price: Decimal) -> Option<Decimal> {
    let day = added.get(..10)?;
    let base = candles
        .iter()
        .find(|c| c.ts.date_naive().format("%Y-%m-%d").to_string().as_str() >= day)?
        .close;
    (!base.is_zero()).then(|| (price / base - Decimal::ONE) * Decimal::ONE_HUNDRED)
}

#[component]
pub fn WatchlistPage() -> Element {
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let mut adding = use_signal(String::new);
    // The list shown; the first one until you pick another.
    let selected = use_signal(|| None::<Uuid>);
    let mut tag_filter = use_signal(|| None::<String>);

    let lists = crate::cache::use_cached(|| "watchlists".into(), move || async move {
        let _reload = refresh.0();
        api::get_watchlists().await.ok()
    });
    let notes = crate::cache::use_cached(|| "notes".into(), move || async move {
        let _reload = refresh.0();
        api::get_notes().await.unwrap_or_default()
    });
    let current = use_memo(move || {
        let lists = lists.read().clone().flatten().unwrap_or_default();
        let id = selected();
        lists
            .iter()
            .find(|l| Some(l.id) == id)
            .or_else(|| lists.first())
            .cloned()
    });
    let tags_of = move |ticker: &str| -> Vec<String> {
        notes
            .read()
            .as_ref()
            .and_then(|n: &Vec<Note>| n.iter().find(|n| n.ticker == ticker).map(|n| n.tags.clone()))
            .unwrap_or_default()
    };
    // Items of the current list, narrowed to the chosen tag.
    let watchlist = use_memo(move || {
        let items = current().map(|l| l.items).unwrap_or_default();
        match tag_filter() {
            Some(tag) => items.into_iter().filter(|i| tags_of(i.ticker.as_str()).contains(&tag)).collect(),
            None => items,
        }
    });
    let alerts = crate::cache::use_cached(|| "alerts".into(), move || async move {
        let _reload = refresh.0();
        api::get_alerts().await.unwrap_or_default()
    });
    let tickers = use_memo(move || watchlist().into_iter().map(|w| w.ticker).collect::<Vec<_>>());
    let quotes = use_price_stream(tickers);
    let mut sort = use_signal(|| (SortKey::Added, false));
    // Daily closes back to the oldest item, for "since added". Items older
    // than a year use weekly closes over five years.
    let oldest = use_memo(move || watchlist().iter().map(|w| w.added_at.clone()).min().unwrap_or_default());
    let history = crate::cache::use_cached(move || format!("watch-since/{:?}/{}", tickers(), oldest()), move || async move {
        let list = tickers();
        if list.is_empty() {
            return HashMap::new();
        }
        let today = chrono::Utc::now().date_naive();
        let old = chrono::NaiveDate::parse_from_str(oldest().get(..10).unwrap_or_default(), "%Y-%m-%d").unwrap_or(today);
        let (range, interval) = if (today - old).num_days() < 360 { (Range::Y1, Interval::D1) } else { (Range::Y5, Interval::W1) };
        crate::offline::charts(list, range, interval, false).await.unwrap_or_default()
    });
    let mut by_cap = use_signal(|| true);
    // Names and market caps; the colour comes from the live price stream.
    let snapshot = crate::cache::use_cached(move || format!("tickers-heatmap/{:?}", tickers()), move || async move {
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
                        |c| crate::i18n::trf("{} market cap", &[&fmt_compact(c)]),
                    ),
                }
            })
            .collect::<Vec<_>>()
    });
    let all_alerts = alerts.read().clone().unwrap_or_default();

    let list_id = current().map(|l| l.id);
    let list_tags: Vec<String> = {
        let mut tags: Vec<String> = current()
            .map(|l| l.items)
            .unwrap_or_default()
            .iter()
            .flat_map(|i| tags_of(i.ticker.as_str()))
            .collect();
        tags.sort();
        tags.dedup();
        tags
    };
    let add = move |_| async move {
        let (Ok(t), Some(list)) = (TickerSymbol::new(&adding()), list_id) else {
            return;
        };
        {
            if api::watch_in(list, t).await.is_ok() {
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
                        {tr("Watchlist")}
                    }
                    p { class: "mt-2 text-sm text-ctp-subtext0", {tr("Stocks you follow, and alerts on price moves.")} }
                }
                GhostButton { label: tr("🔔 New alert"), onclick: move |_| dialogs.open(Dialog::NewAlert(None)) }
            }

            div { class: "mt-8 motion-safe:animate-rise",
                ListBar { lists: lists.read().clone().flatten().unwrap_or_default(), current: list_id, selected }
            }
            if !list_tags.is_empty() {
                div { class: "mt-3 flex flex-wrap items-center gap-2 text-xs",
                    span { class: "text-ctp-subtext0", {tr("Tags")} }
                    for tag in list_tags {
                        {
                            let on = tag_filter().as_deref() == Some(tag.as_str());
                            let t = tag.clone();
                            rsx! {
                                button {
                                    key: "{tag}",
                                    class: if on { "rounded-full bg-ctp-mauve/20 px-2.5 py-1 text-ctp-mauve cursor-pointer" } else { "rounded-full bg-ctp-surface0 px-2.5 py-1 text-ctp-subtext0 cursor-pointer hover:text-ctp-text" },
                                    onclick: move |_| tag_filter.set(if on { None } else { Some(t.clone()) }),
                                    "#{tag}"
                                }
                            }
                        }
                    }
                }
            }

            div { class: "mt-5 grid gap-5 motion-safe:animate-rise",
                if !tickers.read().is_empty() {
                    Card {
                        title: tr("Heatmap"),
                        subtitle: tr("Colour is today's change. Click a stock to open it.").to_string(),
                        actions: rsx! {
                            Segmented {
                                ToggleButton { label: tr("Market cap"), active: by_cap(), onclick: move |_| by_cap.set(true) }
                                ToggleButton { label: tr("Equal"), active: !by_cap(), onclick: move |_| by_cap.set(false) }
                            }
                        },
                        Treemap { items: tiles, saturation: DAY_SATURATION, height: 320 }
                        HeatmapLegend { saturation: DAY_SATURATION }
                    }
                }
                Card {
                    title: current().map_or(tr("Watching").to_string(), |l| l.name),
                    subtitle: crate::i18n::trf("{} stocks", &[&tickers.read().len()]),
                    flush: true,
                    actions: rsx! {
                        form {
                            class: "flex gap-2",
                            onsubmit: move |e| e.prevent_default(),
                            input {
                                class: "w-40 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm uppercase text-ctp-text \
                                        placeholder:normal-case placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                                placeholder: tr("Add ticker"),
                                value: "{adding}",
                                oninput: move |e| adding.set(e.value()),
                            }
                            GhostButton { label: tr("Add"), onclick: add }
                        }
                    },
                    match (lists.read().clone(), watchlist()) {
                        (None, _) => rsx! { p { class: "px-6 pb-8 text-sm text-ctp-subtext0", {tr("Loading…")} } },
                        (Some(None), _) => rsx! { p { class: "px-6 pb-8 text-sm text-ctp-red", {tr("Couldn't load your watchlists.")} } },
                        (_, items) if items.is_empty() => rsx! {
                            p { class: "px-6 pb-8 text-sm text-ctp-subtext0",
                                if tag_filter().is_some() { {tr("No stocks here have that tag.")} } else { {tr("Nothing yet. Add a ticker above, use ☆ Watch on any stock page, or search in the sidebar.")} }
                            }
                        },
                        (_, items) => {
                            let history = history.read().clone().unwrap_or_default();
                            let live = quotes.read();
                            let mut rows: Vec<Row> = items
                                .into_iter()
                                .map(|item| {
                                    let quote = live.get(&item.ticker);
                                    let price = quote.map(|q| q.current_price);
                                    let day = quote.filter(|q| !q.previous_close_price.is_zero()).map(|q| (q.current_price / q.previous_close_price - Decimal::ONE) * Decimal::ONE_HUNDRED);
                                    let since = price.and_then(|p| since_added(history.get(&item.ticker)?, &item.added_at, p));
                                    Row { price, day, since, item }
                                })
                                .collect();
                            let (key, desc) = sort();
                            rows.sort_by(|a, b| {
                                // Unknown values go last whichever way round.
                                let num = |x: Option<Decimal>, y: Option<Decimal>| match (x, y) {
                                    (Some(x), Some(y)) => if desc { y.cmp(&x) } else { x.cmp(&y) },
                                    (Some(_), None) => std::cmp::Ordering::Less,
                                    (None, Some(_)) => std::cmp::Ordering::Greater,
                                    (None, None) => std::cmp::Ordering::Equal,
                                };
                                let by = |o: std::cmp::Ordering| if desc { o.reverse() } else { o };
                                match key {
                                    SortKey::Added => by(a.item.added_at.cmp(&b.item.added_at)),
                                    SortKey::Ticker => by(a.item.ticker.as_str().cmp(b.item.ticker.as_str())),
                                    SortKey::Price => num(a.price, b.price),
                                    SortKey::Day => num(a.day, b.day),
                                    SortKey::SinceAdded => num(a.since, b.since),
                                }
                            });
                            let head = move |k: SortKey, label: &'static str, class: &'static str| {
                                let (on, down) = (sort().0 == k, sort().1);
                                let arrow = if on { if down { " ↓" } else { " ↑" } } else { "" };
                                let active = if on { "text-ctp-text" } else { "" };
                                rsx! {
                                    button {
                                        class: "{class} cursor-pointer select-none hover:text-ctp-text {active}",
                                        onclick: move |_| sort.set(if on { (k, !down) } else { (k, k != SortKey::Ticker && k != SortKey::Added) }),
                                        "{tr(label)}{arrow}"
                                    }
                                }
                            };
                            rsx! {
                                div { class: "flex items-center gap-3 border-t border-ctp-surface0/60 px-4 py-2 text-xs text-ctp-subtext0 sm:gap-4 sm:px-6",
                                    {head(SortKey::Ticker, "Ticker", "w-20 text-left sm:w-24")}
                                    span { class: "hidden flex-1 sm:block" }
                                    {head(SortKey::Price, "Price", "flex-1 text-right")}
                                    {head(SortKey::Day, "Today", "w-16 text-right sm:w-20")}
                                    {head(SortKey::SinceAdded, "Since added", "w-20 text-right")}
                                    {head(SortKey::Added, "Added", "hidden w-20 text-right sm:block")}
                                    span { class: "hidden w-[4.5rem] sm:block" }
                                }
                                for row in rows {
                                    {
                                        let count = all_alerts.iter().filter(|a| a.ticker == row.item.ticker && a.is_active()).count();
                                        let tags = tags_of(row.item.ticker.as_str());
                                        rsx! {
                                            WatchRow {
                                                key: "{row.item.ticker}",
                                                ticker: row.item.ticker.clone(),
                                                price: row.price,
                                                day: row.day,
                                                since: row.since,
                                                added: row.item.added_at.get(..10).unwrap_or_default().to_string(),
                                                alerts: count,
                                                list: list_id,
                                                tags,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                AlertsCard { alerts: all_alerts.clone() }
                crate::alerts::NotificationsCard {}
            }
        }
    }
}

/// One line of the list, with the numbers it can be sorted by.
struct Row {
    item: dtos::watch::WatchItem,
    price: Option<Decimal>,
    day: Option<Decimal>,
    since: Option<Decimal>,
}

#[component]
fn WatchRow(
    ticker: TickerSymbol,
    price: Option<Decimal>,
    day: Option<Decimal>,
    since: Option<Decimal>,
    /// `YYYY-MM-DD`.
    added: String,
    alerts: usize,
    list: Option<Uuid>,
    tags: Vec<String>,
) -> Element {
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let (open, alert, remove) = (ticker.clone(), ticker.clone(), ticker.clone());
    rsx! {
        div {
            class: "group flex items-center gap-3 border-t border-ctp-surface0/60 px-4 py-3.5 cursor-pointer transition-colors hover:bg-ctp-surface0/30 sm:gap-4 sm:px-6",
            onclick: move |_| open_stock(&open),
            span { class: "w-20 truncate font-semibold text-ctp-text sm:w-24",
                "{ticker}"
                if alerts > 0 {
                    span { class: "ml-1 text-xs font-normal text-ctp-subtext0", title: tr("Active alerts"), "🔔{alerts}" }
                }
            }
            span { class: "hidden min-w-0 flex-1 gap-1 truncate sm:flex",
                for tag in tags {
                    span { key: "{tag}", class: "rounded-full bg-ctp-surface0 px-2 py-0.5 text-xs text-ctp-subtext0", "#{tag}" }
                }
            }
            span { class: "flex-1 text-right tabular-nums text-ctp-text",
                {price.map(|p| fmt_usd(p, 2)).unwrap_or_else(|| "—".into())}
            }
            span { class: "w-16 text-right text-sm tabular-nums sm:w-20 {day.map(signed_color).unwrap_or(\"text-ctp-overlay1\")}",
                {day.map(|d| format!("{d:+.2}%")).unwrap_or_else(|| "—".into())}
            }
            span { class: "w-20 text-right text-sm tabular-nums {since.map(signed_color).unwrap_or(\"text-ctp-overlay1\")}",
                {since.map(|d| format!("{d:+.1}%")).unwrap_or_else(|| "—".into())}
            }
            span { class: "hidden w-20 text-right text-xs tabular-nums text-ctp-subtext0 sm:block", "{added}" }
            span { class: "hidden gap-1 opacity-40 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 sm:flex",
                button {
                    class: "inline-flex h-8 min-w-8 items-center justify-center rounded-full px-2 text-sm text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                    title: tr("New alert"),
                    onclick: move |e| {
                        e.stop_propagation();
                        dialogs.open(Dialog::NewAlert(Some(alert.clone())));
                    },
                    "🔔"
                }
                button {
                    class: "inline-flex h-8 min-w-8 items-center justify-center rounded-full px-2 text-sm text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                    title: tr("Remove from this list"),
                    onclick: move |e| {
                        e.stop_propagation();
                        let t = remove.clone();
                        spawn(async move {
                            let done = match list {
                                Some(list) => api::unwatch_from(list, t).await,
                                None => api::unwatch_ticker(t).await,
                            };
                            if done.is_ok() {
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

/// Pills to switch lists, plus create / rename / delete.
#[component]
fn ListBar(lists: Vec<Watchlist>, current: Option<Uuid>, selected: Signal<Option<Uuid>>) -> Element {
    let refresh = use_context::<DataRefresh>();
    // Some(name) while typing a new list's name; `renaming` edits the current one.
    let mut naming = use_signal(|| None::<String>);
    let mut renaming = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let message = |e: ServerFnError| match e {
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    };
    let save_name = move |_| async move {
        let Some(name) = naming() else { return };
        let result = if renaming() {
            match current {
                Some(id) => api::rename_watchlist(id, name).await,
                None => Ok(()),
            }
        } else {
            api::create_watchlist(name).await.map(|id| selected.set(Some(id)))
        };
        match result {
            Ok(()) => {
                naming.set(None);
                renaming.set(false);
                error.set(None);
                refresh.reload();
            }
            Err(e) => error.set(Some(message(e))),
        }
    };
    let delete = move |_| async move {
        let Some(id) = current else { return };
        match api::delete_watchlist(id).await {
            Ok(()) => {
                selected.set(None);
                confirm_delete.set(false);
                refresh.reload();
            }
            Err(e) => error.set(Some(message(e))),
        }
    };
    let current_name = lists.iter().find(|l| Some(l.id) == current).map(|l| l.name.clone()).unwrap_or_default();
    rsx! {
        div { class: "flex flex-wrap items-center gap-2",
            for l in lists.iter().cloned() {
                button {
                    key: "{l.id}",
                    class: if Some(l.id) == current {
                        "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3.5 py-1.5 text-sm font-medium text-ctp-text cursor-pointer"
                    } else {
                        "rounded-full border border-ctp-surface0 px-3.5 py-1.5 text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
                    },
                    onclick: move |_| selected.set(Some(l.id)),
                    "{l.name} "
                    span { class: "text-xs text-ctp-overlay1", "{l.items.len()}" }
                }
            }
            if let Some(name) = naming() {
                form {
                    class: "flex items-center gap-2",
                    onsubmit: move |e| e.prevent_default(),
                    input {
                        class: "w-44 rounded-full border border-ctp-mauve bg-ctp-crust/40 px-3.5 py-1.5 text-sm text-ctp-text outline-none",
                        placeholder: tr("List name"),
                        autofocus: true,
                        value: "{name}",
                        oninput: move |e| naming.set(Some(e.value())),
                    }
                    GhostButton { label: if renaming() { tr("Rename") } else { tr("Create") }, onclick: save_name }
                    GhostButton { label: tr("Cancel"), onclick: move |_| { naming.set(None); renaming.set(false); error.set(None); } }
                }
            } else {
                GhostButton { label: tr("＋ New list"), onclick: move |_| naming.set(Some(String::new())) }
                if current.is_some() {
                    GhostButton { label: tr("Rename"), onclick: move |_| { renaming.set(true); naming.set(Some(current_name.clone())); } }
                    if lists.len() > 1 {
                        if confirm_delete() {
                            GhostButton { label: tr("Delete this list?"), onclick: delete }
                            GhostButton { label: tr("Keep"), onclick: move |_| confirm_delete.set(false) }
                        } else {
                            GhostButton { label: tr("Delete"), onclick: move |_| confirm_delete.set(true) }
                        }
                    }
                }
            }
            if let Some(e) = error() {
                span { class: "text-xs text-ctp-red", "{e}" }
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
            title: tr("Alerts"),
            subtitle: format!("{active} active · {} fired", alerts.len() - active),
            flush: true,
            if alerts.is_empty() {
                p { class: "px-6 pb-8 text-sm text-ctp-subtext0", {tr("No alerts. Create one with 🔔 New alert.")} }
            }
            for a in alerts {
                div { key: "{a.id}", class: "group flex items-center gap-4 border-t border-ctp-surface0/60 px-6 py-3",
                    span {
                        class: if a.is_active() {
                            "min-w-16 shrink-0 whitespace-nowrap rounded-full bg-ctp-green/15 px-2 py-0.5 text-center text-xs font-semibold text-ctp-green"
                        } else {
                            "min-w-16 shrink-0 whitespace-nowrap rounded-full bg-ctp-surface0 px-2 py-0.5 text-center text-xs font-semibold text-ctp-subtext0"
                        },
                        if a.is_active() { {tr("Active")} } else { {tr("Fired")} }
                    }
                    span { class: "flex-1 text-sm text-ctp-text", "{a.describe()}" }
                    if let Some(when) = &a.triggered_at {
                        span { class: "text-xs text-ctp-overlay1", "{when}" }
                    }
                    crate::components::card::DeleteButton {
                        title: tr("Delete alert"),
                        icon: "×",
                        onconfirm: move |_| {
                            spawn(async move {
                                if api::delete_alert(a.id).await.is_ok() {
                                    refresh.reload();
                                }
                            });
                        },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn candle(day: &str, close: Decimal) -> types::candle::Candle {
        let ts = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap().and_hms_opt(14, 30, 0).unwrap().and_utc();
        types::candle::Candle { ts, open: close, high: close, low: close, close, volume: None, adj_factor: None }
    }

    #[test]
    fn since_added_uses_the_first_close_on_or_after_the_day() {
        let candles = [candle("2026-03-02", dec!(90)), candle("2026-03-04", dec!(100)), candle("2026-03-05", dec!(110))];
        // Added on a day with no candle (a holiday): the next close counts.
        assert_eq!(since_added(&candles, "2026-03-03 09:15:00", dec!(120)), Some(dec!(20)));
        assert_eq!(since_added(&candles, "2026-03-02 18:00:00", dec!(45)), Some(dec!(-50)));
        assert_eq!(since_added(&candles, "2026-04-01 10:00:00", dec!(120)), None, "no close since");
        assert_eq!(since_added(&candles, "bad", dec!(120)), None);
    }
}
