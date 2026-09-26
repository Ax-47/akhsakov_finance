//! Earnings and dividend dates for the stocks you hold and watch.

use crate::i18n::tr;
use crate::{
    app::DataRefresh,
    components::{
        analysis::stock::open_stock,
        card::{Card, Segmented, ToggleButton},
    },
    format::fmt_usd,
    page::Page,
};
use chrono::{Datelike, NaiveDate};
use dioxus::prelude::*;
use dtos::market::{CalendarEvent, EventKind};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};
use types::ticker_symbol::TickerSymbol;

#[derive(Clone, Copy, PartialEq)]
enum Show {
    All,
    Earnings,
    Dividends,
}

/// Holdings and watchlist tickers, and the calendar for them. Also returns
/// shares held per ticker.
fn use_my_calendar() -> (
    Memo<Option<Result<Vec<CalendarEvent>, String>>>,
    HashMap<String, Decimal>,
) {
    let refresh = use_context::<DataRefresh>();
    let held: HashMap<String, Decimal> = crate::hooks::use_held_shares()
        .read()
        .iter()
        .map(|(t, shares)| (t.to_string(), *shares))
        .collect();
    let mut held_tickers: Vec<String> = held.keys().cloned().collect();
    held_tickers.sort();
    let held_sig = use_memo(use_reactive!(|held_tickers| held_tickers));
    let watchlist = crate::cache::use_cached(|| "watchlist".into(), move || async move {
        let _reload = refresh.0();
        api::get_watchlist().await.unwrap_or_default()
    });
    let tickers = use_memo(move || {
        let mut all: Vec<String> = held_sig();
        if let Some(items) = &*watchlist.read() {
            all.extend(items.iter().map(|w| w.ticker.to_string()));
        }
        all.sort();
        all.dedup();
        all
    });
    let events = crate::cache::use_cached(move || format!("calendar/{}", tickers().join(",")), move || async move {
        let list: Vec<TickerSymbol> = tickers()
            .iter()
            .filter_map(|t| TickerSymbol::new(t).ok())
            .collect();
        if list.is_empty() {
            return Ok(vec![]);
        }
        api::get_calendar(list).await.map_err(|e| match e {
            ServerFnError::ServerError { message, .. } => message,
            e => e.to_string(),
        })
    });
    (events, held)
}

#[component]
pub fn CalendarPage() -> Element {
    let (events, held) = use_my_calendar();
    let mut show = use_signal(|| Show::All);
    let today = use_today();

    let body = match &*events.read() {
        None => rsx! { p { class: "py-10 text-center text-sm text-ctp-subtext0", {tr("Loading dates…")} } },
        Some(Err(message)) => rsx! { p { class: "text-sm text-ctp-red", "Couldn't load the calendar: {message}" } },
        Some(Ok(list)) if list.is_empty() => rsx! {
            p { class: "py-6 text-sm text-ctp-subtext0",
                {tr("No upcoming earnings or dividends. Add stocks to your portfolio or watchlist to see their dates here.")}
            }
        },
        Some(Ok(list)) => {
            let shown: Vec<CalendarEvent> = list
                .iter()
                .filter(|e| match show() {
                    Show::All => true,
                    Show::Earnings => e.kind == EventKind::Earnings,
                    Show::Dividends => e.kind != EventKind::Earnings,
                })
                .cloned()
                .collect();
            // Month → day → events.
            let mut months: BTreeMap<String, BTreeMap<String, Vec<CalendarEvent>>> = BTreeMap::new();
            for e in shown {
                months
                    .entry(e.date.get(..7).unwrap_or_default().to_string())
                    .or_default()
                    .entry(e.date.clone())
                    .or_default()
                    .push(e);
            }
            rsx! {
                for (month, days) in months {
                    div { key: "{month}", class: "mb-6 last:mb-0",
                        h3 { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-ctp-subtext0", {month_label(&month)} }
                        div { class: "grid gap-2",
                            for (date, day_events) in days {
                                DayRow { key: "{date}", date: date.clone(), events: day_events, held: held.clone(), today }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("Calendar")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0",
                    {tr("Earnings reports and dividend dates for the stocks you hold and watch, from two weeks ago to four months ahead.")}
                }
            }
            div { class: "mt-10 motion-safe:animate-rise",
                Card {
                    title: tr("Upcoming"),
                    actions: rsx! {
                        Segmented {
                            ToggleButton { label: tr("All"), active: show() == Show::All, onclick: move |_| show.set(Show::All) }
                            ToggleButton { label: tr("Earnings"), active: show() == Show::Earnings, onclick: move |_| show.set(Show::Earnings) }
                            ToggleButton { label: tr("Dividends"), active: show() == Show::Dividends, onclick: move |_| show.set(Show::Dividends) }
                        }
                    },
                    {body}
                }
            }
        }
    }
}

/// The next few events, for the dashboard.
#[component]
pub fn UpcomingEvents(limit: usize) -> Element {
    let (events, held) = use_my_calendar();
    let today = use_today();
    let upcoming: Vec<CalendarEvent> = match &*events.read() {
        Some(Ok(list)) => list
            .iter()
            .filter(|e| parse(&e.date).is_some_and(|d| d >= today))
            .take(limit)
            .cloned()
            .collect(),
        _ => vec![],
    };
    rsx! {
        Card {
            title: tr("Coming up"),
            subtitle: tr("Earnings and dividends for your stocks").to_string(),
            actions: rsx! {
                button {
                    class: "text-xs text-ctp-subtext0 cursor-pointer hover:text-ctp-text",
                    onclick: move |_| {
                        navigator().push("/calendar");
                    },
                    {tr("Calendar →")}
                }
            },
            if upcoming.is_empty() {
                p { class: "text-sm text-ctp-subtext0",
                    if events.read().is_none() { {tr("Loading…")} } else { {tr("Nothing in the next four months.")} }
                }
            }
            div { class: "grid gap-2",
                for e in upcoming {
                    EventRow { key: "{e.date}{e.ticker}{e.kind:?}", event: e.clone(), shares: held.get(&e.ticker).copied(), show_date: true, past: false }
                }
            }
        }
    }
}

#[component]
fn DayRow(date: String, events: Vec<CalendarEvent>, held: HashMap<String, Decimal>, today: NaiveDate) -> Element {
    let day = parse(&date);
    let past = day.is_some_and(|d| d < today);
    let is_today = day == Some(today);
    let (weekday, number) = day.map_or((String::new(), date.clone()), |d| {
        (d.format("%a").to_string(), d.day().to_string())
    });
    rsx! {
        div { class: if past { "flex gap-4 opacity-50" } else { "flex gap-4" },
            div {
                class: if is_today {
                    "flex w-12 shrink-0 flex-col items-center rounded-xl bg-ctp-mauve/20 py-1.5 text-ctp-mauve"
                } else {
                    "flex w-12 shrink-0 flex-col items-center rounded-xl bg-ctp-surface0/50 py-1.5 text-ctp-subtext1"
                },
                span { class: "text-xs uppercase", "{weekday}" }
                span { class: "text-lg font-semibold leading-none", "{number}" }
            }
            div { class: "grid flex-1 gap-1.5",
                for e in events {
                    EventRow { key: "{e.ticker}{e.kind:?}", shares: held.get(&e.ticker).copied(), event: e, show_date: false, past }
                }
            }
        }
    }
}

#[component]
fn EventRow(event: CalendarEvent, shares: Option<Decimal>, show_date: bool, past: bool) -> Element {
    let badge = match event.kind {
        EventKind::Earnings => "bg-ctp-sky/15 text-ctp-sky",
        EventKind::ExDividend => "bg-ctp-peach/15 text-ctp-peach",
        EventKind::DividendPayment => "bg-ctp-green/15 text-ctp-green",
    };
    let detail = match event.kind {
        EventKind::Earnings if event.estimated => tr("Date not confirmed yet").to_string(),
        EventKind::Earnings => tr("Quarterly results").to_string(),
        EventKind::ExDividend if !past => tr("Own it before this date to get the next dividend").to_string(),
        EventKind::ExDividend => tr("Went ex-dividend").to_string(),
        EventKind::DividendPayment => tr("Dividend paid to shareholders").to_string(),
    };
    let dividend = event.annual_dividend.filter(|d| *d > 0.0).map(|annual| {
        let per_share = fmt_usd(Decimal::try_from(annual).unwrap_or_default(), 2);
        match shares {
            Some(s) => {
                let total = Decimal::try_from(annual).unwrap_or_default() * s;
                crate::i18n::trf("{}/share a year · ≈ {} a year on your {} shares", &[&per_share, &fmt_usd(total, 2), &crate::format::fmt_shares(s)])
            }
            None => format!("{per_share}/share a year"),
        }
    });
    let date = parse(&event.date).map_or(event.date.clone(), |d| d.format("%a %-d %b").to_string());
    let ticker = TickerSymbol::new(&event.ticker).ok();
    rsx! {
        button {
            class: "flex w-full items-center gap-3 rounded-xl border border-ctp-surface0/60 px-3 py-2 text-left cursor-pointer transition-colors hover:bg-ctp-surface0/30",
            onclick: move |_| {
                if let Some(t) = &ticker {
                    open_stock(t);
                }
            },
            span { class: "w-28 shrink-0 whitespace-nowrap rounded-full px-2 py-0.5 text-center text-xs font-semibold {badge}", {tr(event.kind.label())} }
            div { class: "min-w-0 flex-1",
                div { class: "flex items-baseline gap-2",
                    span { class: "font-semibold text-ctp-text", "{event.ticker}" }
                    span { class: "truncate text-xs text-ctp-subtext0", "{event.name}" }
                    if shares.is_some() {
                        span { class: "rounded-full bg-ctp-surface0 px-1.5 text-xs text-ctp-subtext0", "held" }
                    }
                }
                div { class: "text-xs text-ctp-subtext0", {dividend.unwrap_or(detail)} }
            }
            if show_date {
                span { class: "shrink-0 text-xs tabular-nums text-ctp-subtext0", "{date}" }
            }
        }
    }
}

/// Today in the viewer's time zone (from the webview); the earliest date
/// until it's known.
fn use_today() -> NaiveDate {
    let today = use_resource(|| async { crate::notify::today().await });
    let date = today.read().clone().flatten();
    date.and_then(|d| parse(&d)).unwrap_or(NaiveDate::MIN)
}

fn parse(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// `2026-10` → `October 2026`.
fn month_label(month: &str) -> String {
    parse(&format!("{month}-01")).map_or(month.to_string(), |d| d.format("%B %Y").to_string())
}
