//! Markets: an index (S&P 500, Nasdaq-100, Dow, SET50) as a sector
//! heatmap, with breadth, sector moves, the day's biggest movers and a
//! stock table for a chosen sector.

use crate::i18n::tr;
use crate::{
    components::{
        analysis::stock::open_stock,
        card::{Card, MetricTile, Segmented, ToggleButton},
        charts::{HeatItem, HeatmapLegend, Treemap, DAY_SATURATION},
    },
    format::{fmt_compact, fmt_usd},
    page::{GhostButton, Page},
    stock_table::StockTable,
};
use dioxus::prelude::*;
use dtos::market::{sector_moves, weighted_change, HeatmapItem, MarketIndex, SectorMove};
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;

const REFRESH_MS: u32 = 60_000;
const RETRY_MS: u32 = 5_000;
const MOVERS: usize = 6;

#[component]
pub fn MarketPage() -> Element {
    let mut index = use_signal(|| MarketIndex::Sp500);
    rsx! {
        Page {
            header { class: "flex flex-wrap items-end justify-between gap-4 motion-safe:animate-rise",
                div {
                    h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                        {tr("Markets")}
                    }
                    p { class: "mt-2 text-sm text-ctp-subtext0", {tr("Today's move across an index. Refreshes every minute.")} }
                }
                Segmented {
                    for i in MarketIndex::ALL {
                        ToggleButton { key: "{i.key()}", label: tr(i.label()), active: index() == i, onclick: move |_| index.set(i) }
                    }
                }
            }
            // Keyed: switching index starts a fresh refresh loop.
            div { IndexView { key: "{index().key()}", index: index() } }
        }
    }
}

#[component]
fn IndexView(index: MarketIndex) -> Element {
    let cache_key = format!("index-heatmap/{}", index.key());
    // The last map for this index shows at once; the loop refreshes it.
    let mut data = use_signal({
        let k = cache_key.clone();
        move || crate::cache::get::<Vec<HeatmapItem>>(&k).map(Ok)
    });
    // A sector to zoom the map into.
    let mut focus = use_signal(|| None::<String>);

    use_future(move || {
        let cache_key = cache_key.clone();
        async move { loop {
            let result = api::get_index_heatmap(index).await.map_err(|e| match e {
                ServerFnError::ServerError { message, .. } => message,
                e => e.to_string(),
            });
            if let Ok(items) = &result {
                crate::cache::put(cache_key.clone(), items.clone());
            }
            let ok = result.is_ok();
            // Keep showing the last good map if a refresh fails.
            if ok || !matches!(*data.peek(), Some(Ok(_))) {
                data.set(Some(result));
            }
            // Failures are often a dropped connection: retry soon.
            crate::notify::poll_delay(if ok { REFRESH_MS } else { RETRY_MS }).await;
        } }
    });

    let items = use_memo(move || match &*data.read() {
        Some(Ok(items)) => items.clone(),
        _ => vec![],
    });
    let tiles = use_memo(move || {
        let focus = focus.read();
        items
            .read()
            .iter()
            .filter(|i| focus.is_none() || i.sector == *focus)
            .map(|i| HeatItem {
                ticker: i.ticker.clone(),
                name: i.name.clone(),
                size: i.market_cap.unwrap_or(0.0),
                change: i.change_pct,
                // Zoomed into one sector: no sector blocks.
                group: if focus.is_some() { None } else { i.sector.clone() },
                detail: format!(
                    "{} · {} market cap",
                    fmt_usd(Decimal::try_from(i.price).unwrap_or_default(), 2),
                    fmt_compact(i.market_cap.unwrap_or(0.0))
                ),
            })
            .collect::<Vec<_>>()
    });
    let sectors = use_memo(move || sector_moves(&items.read()));

    let all = items.read();
    let index_change = weighted_change(all.iter());
    let advancing = all.iter().filter(|i| i.change_pct.is_some_and(|c| c > 0.0)).count();
    let declining = all.iter().filter(|i| i.change_pct.is_some_and(|c| c < 0.0)).count();
    let best = sectors.read().iter().max_by(|a, b| a.change_pct.total_cmp(&b.change_pct)).cloned();
    let worst = sectors.read().iter().min_by(|a, b| a.change_pct.total_cmp(&b.change_pct)).cloned();
    let count = all.len();
    drop(all);

    let map_title = match focus() {
        Some(sector) => format!("{} · {sector}", tr(index.label())),
        None => format!("{} · {count} stocks", tr(index.label())),
    };
    let focused_rows: Vec<dtos::market::StockRow> = match focus() {
        Some(sector) => items
            .read()
            .iter()
            .filter(|i| i.sector.as_deref() == Some(sector.as_str()))
            .map(|i| i.row())
            .collect(),
        None => vec![],
    };

    rsx! {
            match data() {
                None => rsx! {
                    div { class: "mt-10 motion-safe:animate-rise",
                        Card { title: tr(index.label()),
                            div { class: "flex h-[560px] items-center justify-center text-sm text-ctp-subtext0", {tr("Loading prices…")} }
                        }
                    }
                },
                Some(Err(message)) => rsx! {
                    div { class: "mt-10 motion-safe:animate-rise",
                        Card { title: tr(index.label()),
                            p { class: "text-sm text-ctp-red", "Couldn't load prices: {message}" }
                            p { class: "mt-1 text-xs text-ctp-subtext0", {tr("Retrying in a few seconds…")} }
                        }
                    }
                },
                Some(Ok(_)) => rsx! {
                    div { class: "mt-10 grid grid-cols-2 gap-3 lg:grid-cols-4 motion-safe:animate-rise",
                        MetricTile {
                            label: tr(index.label()),
                            value: pct(index_change),
                            hint: tr("Weighted by market cap"),
                            tone: tone(index_change),
                        }
                        MetricTile {
                            label: tr("Breadth"),
                            value: format!("{advancing} ▲  {declining} ▼"),
                            hint: format!("{:.0}% of stocks up", advancing as f64 / count.max(1) as f64 * 100.0),
                        }
                        MetricTile {
                            label: tr("Best sector"),
                            value: pct(best.as_ref().map(|s| s.change_pct)),
                            hint: best.map(|s| s.sector).unwrap_or_default(),
                            tone: "text-ctp-green",
                        }
                        MetricTile {
                            label: tr("Worst sector"),
                            value: pct(worst.as_ref().map(|s| s.change_pct)),
                            hint: worst.map(|s| s.sector).unwrap_or_default(),
                            tone: "text-ctp-red",
                        }
                    }

                    div { class: "mt-5 motion-safe:animate-rise",
                        Card {
                            title: map_title,
                            subtitle: tr("Size is market cap, colour is today's change. Click a stock to open it.").to_string(),
                            actions: rsx! {
                                if focus().is_some() {
                                    GhostButton { label: tr("← All sectors"), onclick: move |_| focus.set(None) }
                                }
                            },
                            Treemap { items: tiles, saturation: DAY_SATURATION, height: 560 }
                            HeatmapLegend { saturation: DAY_SATURATION }
                        }
                    }

                    div { class: "mt-5 grid gap-5 lg:grid-cols-2 motion-safe:animate-rise",
                        Card { title: tr("Sectors"), subtitle: tr("Click one to zoom the map").to_string(),
                            SectorBars { sectors: sectors(), focus }
                        }
                        Card { title: tr("Movers"),
                            Movers { items: items() }
                        }
                    }

                    if let Some(sector) = focus() {
                        div { class: "mt-5 motion-safe:animate-rise",
                            Card { title: "{sector}", subtitle: format!("{} stocks, largest first", focused_rows.len()), flush: true,
                                StockTable { rows: focused_rows.clone() }
                                div { class: "h-3" }
                            }
                        }
                    }
                },
            }
    }
}

#[component]
fn SectorBars(sectors: Vec<SectorMove>, focus: Signal<Option<String>>) -> Element {
    let scale = sectors
        .iter()
        .map(|s| s.change_pct.abs())
        .fold(0.5, f64::max);
    rsx! {
        div { class: "grid gap-1",
            for s in sectors {
                {
                    let width = s.change_pct.abs() / scale * 100.0;
                    let bar = if s.change_pct >= 0.0 { "bg-ctp-green/70" } else { "bg-ctp-red/70" };
                    let text = if s.change_pct >= 0.0 { "text-ctp-green" } else { "text-ctp-red" };
                    let selected = focus.read().as_deref() == Some(s.sector.as_str());
                    let row = if selected {
                        "grid grid-cols-[9rem_1fr_4rem] items-center gap-3 rounded-xl bg-ctp-surface0 px-2.5 py-1.5 text-left cursor-pointer"
                    } else {
                        "grid grid-cols-[9rem_1fr_4rem] items-center gap-3 rounded-xl px-2.5 py-1.5 text-left cursor-pointer hover:bg-ctp-surface0/50"
                    };
                    let sector = s.sector.clone();
                    rsx! {
                        button {
                            key: "{s.sector}",
                            class: row,
                            onclick: move |_| focus.set(if selected { None } else { Some(sector.clone()) }),
                            span { class: "truncate text-sm text-ctp-subtext1", "{s.sector}" }
                            span { class: "h-2 rounded-full bg-ctp-surface0/60",
                                span { class: "block h-2 rounded-full {bar}", style: "width:{width:.1}%;" }
                            }
                            span { class: "text-right text-sm tabular-nums {text}", "{s.change_pct:+.2}%" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Movers(items: Vec<HeatmapItem>) -> Element {
    let mut sorted: Vec<HeatmapItem> = items.into_iter().filter(|i| i.change_pct.is_some()).collect();
    sorted.sort_by(|a, b| b.change_pct.unwrap_or(0.0).total_cmp(&a.change_pct.unwrap_or(0.0)));
    let gainers: Vec<HeatmapItem> = sorted.iter().take(MOVERS).cloned().collect();
    let losers: Vec<HeatmapItem> = sorted.iter().rev().take(MOVERS).cloned().collect();
    rsx! {
        div { class: "grid gap-6 sm:grid-cols-2",
            MoverList { title: tr("Top gainers"), items: gainers }
            MoverList { title: tr("Top losers"), items: losers }
        }
    }
}

#[component]
fn MoverList(title: String, items: Vec<HeatmapItem>) -> Element {
    rsx! {
        div {
            div { class: "mb-2 text-xs text-ctp-subtext0", "{title}" }
            for i in items {
                {
                    let change = i.change_pct.unwrap_or(0.0);
                    let color = if change >= 0.0 { "text-ctp-green" } else { "text-ctp-red" };
                    let ticker = TickerSymbol::new(&i.ticker).ok();
                    rsx! {
                        button {
                            key: "{i.ticker}",
                            class: "flex w-full items-center gap-3 rounded-xl px-2 py-1.5 text-left cursor-pointer hover:bg-ctp-surface0/50",
                            onclick: move |_| {
                                if let Some(t) = &ticker {
                                    open_stock(t);
                                }
                            },
                            span { class: "w-14 text-sm font-semibold text-ctp-text", "{i.ticker}" }
                            span { class: "min-w-0 flex-1 truncate text-xs text-ctp-subtext0", "{i.name}" }
                            span { class: "text-sm tabular-nums {color}", "{change:+.2}%" }
                        }
                    }
                }
            }
        }
    }
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "—".into(), |v| format!("{v:+.2}%"))
}

fn tone(value: Option<f64>) -> &'static str {
    match value {
        Some(v) if v >= 0.0 => "text-ctp-green",
        Some(_) => "text-ctp-red",
        None => "text-ctp-text",
    }
}
