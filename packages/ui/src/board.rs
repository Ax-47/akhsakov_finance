//! The live board: the table a trading screen shows, one row per
//! instrument, in its own currency. Rows are refreshed in full every few
//! seconds, and between refreshes the rows on screen move with each trade
//! streamed from the exchange feed, flashing green or red.

use crate::{
    components::{analysis::stock::open_stock, card::Card},
    format::{fmt_clock, fmt_compact},
    i18n::{tr, tr_str, trf},
    page::GhostButton,
    LiveNumber,
};
use api::{events::quote_update_event::QuoteUpdateEvent, quote::quote::ClientEvent, quote_subscribe};
use dioxus::{
    fullstack::{use_websocket, WebSocketOptions},
    prelude::*,
};
use dtos::{
    board::{matches, sort_board, BoardQuote, BoardSort, Breadth, Move, Session, Tick, WORLD},
    market::MarketIndex,
};
use rust_decimal::prelude::ToPrimitive;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use types::ticker_symbol::TickerSymbol;

/// Rows shown at first and added by "Show more". Only rows on screen are
/// streamed.
const PAGE: usize = 50;
/// Seconds between full refreshes: a short list, and a long one (an index).
const REFRESH_MS: u32 = 15_000;
const REFRESH_LONG_MS: u32 = 45_000;
const LONG_LIST: usize = 120;
const RETRY_MS: u32 = 5_000;

/// Where a board's rows come from.
#[derive(Clone, PartialEq, Debug)]
pub enum BoardSource {
    Index(MarketIndex),
    Tickers(Vec<TickerSymbol>),
}

impl BoardSource {
    fn cache_key(&self) -> String {
        match self {
            Self::Index(index) => format!("board/index/{}", index.key()),
            Self::Tickers(tickers) => {
                format!("board/{}", tickers.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(","))
            }
        }
    }

    async fn fetch(&self) -> Result<Vec<BoardQuote>, String> {
        let result = match self {
            Self::Index(index) => api::get_index_board(*index).await,
            Self::Tickers(tickers) if tickers.is_empty() => return Ok(vec![]),
            Self::Tickers(tickers) => api::get_board(tickers.clone()).await,
        };
        result.map_err(|e| match e {
            ServerFnError::ServerError { message, .. } => message,
            e => e.to_string(),
        })
    }
}

/// A board's rows, kept current.
#[derive(Clone, Copy)]
pub struct BoardFeed {
    /// `None` until the first answer. An error only while nothing has
    /// loaded: a failed refresh keeps the rows shown.
    pub rows: Signal<Option<Result<Vec<BoardQuote>, String>>>,
    /// Which way each ticker's last streamed trade moved it, and a count
    /// that changes with every move (it restarts the flash).
    pub moves: Signal<HashMap<String, (Move, u32)>>,
    /// The tickers to stream trades for: set it to the rows on screen.
    pub watch: Signal<Vec<TickerSymbol>>,
    /// When the latest streamed trade happened, Unix seconds.
    pub last_trade: Signal<Option<i64>>,
}

/// Loads `source` and keeps it current: a full refresh every few seconds
/// (less often for long lists and in Lite), and streamed trades in between
/// for the tickers in [`BoardFeed::watch`], applied a couple of times a
/// second.
pub fn use_board_feed(source: BoardSource) -> BoardFeed {
    let key = source.cache_key();
    let mut rows = use_signal({
        let key = key.clone();
        move || crate::cache::get::<Vec<BoardQuote>>(&key).map(Ok)
    });
    let moves = use_signal(HashMap::<String, (Move, u32)>::new);
    let watch = use_signal(Vec::<TickerSymbol>::new);
    let last_trade = use_signal(|| None::<i64>);
    // The newest streamed trade per ticker. A refresh can be older than the
    // stream; these are put back on top of it.
    let latest = use_hook(|| Rc::new(RefCell::new(HashMap::<String, Tick>::new())));

    use_future({
        let latest = latest.clone();
        move || {
            let (source, key, latest) = (source.clone(), key.clone(), latest.clone());
            async move {
                loop {
                    let result = source.fetch().await;
                    let ok = result.is_ok();
                    let mut long = false;
                    match result {
                        Ok(mut fresh) => {
                            for row in &mut fresh {
                                if let Some(tick) = latest.borrow().get(&row.ticker) {
                                    row.apply(*tick);
                                }
                            }
                            long = fresh.len() > LONG_LIST;
                            crate::cache::put(key.clone(), fresh.clone());
                            rows.set(Some(Ok(fresh)));
                        }
                        Err(e) if !matches!(*rows.peek(), Some(Ok(_))) => rows.set(Some(Err(e))),
                        Err(_) => {}
                    }
                    let wait = match (ok, long) {
                        (false, _) => RETRY_MS,
                        (true, true) => REFRESH_LONG_MS,
                        (true, false) => REFRESH_MS,
                    };
                    crate::notify::poll_delay(if crate::perf::lite_now() { wait * 2 } else { wait }).await;
                }
            }
        }
    });

    let mut socket = use_websocket(|| quote_subscribe(WebSocketOptions::new()));
    use_effect(move || {
        let list = watch();
        spawn(async move {
            let _ = socket.send(ClientEvent::WatchRaw(list)).await;
        });
    });

    // Trades waiting for the next flush. Not reactive: the stream can send
    // several a second per ticker, and each write redraws the board.
    let pending = use_hook(|| Rc::new(RefCell::new(HashMap::<String, Tick>::new())));
    use_future(move || {
        let (pending, latest) = (pending.clone(), latest.clone());
        async move {
            let scheduled = Rc::new(Cell::new(false));
            while let Ok(QuoteUpdateEvent::QuoteUpdate(update)) = socket.recv().await {
                let price = update.native_price.unwrap_or(update.current_price).to_f64().unwrap_or(0.0);
                if price <= 0.0 {
                    continue;
                }
                pending.borrow_mut().insert(
                    update.ticker_symbol.to_string(),
                    Tick { price, volume: update.volume.map(|v| v as f64), time: update.timestamp },
                );
                // Only the first trade of a quiet spell schedules a flush.
                if !scheduled.replace(true) {
                    let (pending, scheduled, latest) = (pending.clone(), scheduled.clone(), latest.clone());
                    spawn(async move {
                        crate::notify::poll_delay(crate::perf::price_flush_ms() / 2).await;
                        scheduled.set(false);
                        let ticks = std::mem::take(&mut *pending.borrow_mut());
                        let feed = BoardFeed { rows, moves, watch, last_trade };
                        feed.apply(&mut latest.borrow_mut(), ticks);
                    });
                }
            }
        }
    });

    BoardFeed { rows, moves, watch, last_trade }
}

impl BoardFeed {
    /// Applies streamed trades to the rows. Writes only what changed:
    /// every write redraws whatever shows it.
    fn apply(mut self, latest: &mut HashMap<String, Tick>, ticks: HashMap<String, Tick>) {
        let Some(Ok(mut list)) = self.rows.peek().clone() else {
            return;
        };
        let mut moved = Vec::new();
        let mut changed = false;
        let mut newest = *self.last_trade.peek();
        for row in &mut list {
            let Some(tick) = ticks.get(&row.ticker) else {
                continue;
            };
            let before = (row.price, row.ext_price, row.volume, row.time);
            if let Some(direction) = row.apply(*tick) {
                moved.push((row.ticker.clone(), direction));
            }
            if before != (row.price, row.ext_price, row.volume, row.time) {
                changed = true;
                newest = newest.max(Some(tick.time));
                latest.insert(row.ticker.clone(), *tick);
            }
        }
        if changed {
            self.rows.set(Some(Ok(list)));
        }
        if newest != *self.last_trade.peek() {
            self.last_trade.set(newest);
        }
        if !moved.is_empty() {
            self.moves.with_mut(|moves| {
                for (ticker, direction) in moved {
                    let count = moves.get(&ticker).map_or(0, |(_, n)| n.wrapping_add(1));
                    moves.insert(ticker, (direction, count));
                }
            });
        }
    }
}

// ─── Numbers ──────────────────────────────────────────────────────────────────

/// A price as a board prints it: two decimals, more for small prices so
/// the last digits that move still show; thousands grouped.
pub fn fmt_price(v: f64) -> String {
    let decimals = match v.abs() {
        a if a >= 1.0 => 2,
        a if a >= 0.01 => 4,
        _ => 6,
    };
    let text = format!("{:.*}", decimals, v.abs());
    let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut out = String::with_capacity(text.len() + 4);
    if v < 0.0 {
        out.push('-');
    }
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out.push('.');
    out.push_str(frac);
    out
}

/// A count of shares or money, compact and without a currency: `41.2M`.
pub fn fmt_count(v: f64) -> String {
    match v.abs() {
        a if a >= 1e12 => format!("{:.2}T", v / 1e12),
        a if a >= 1e9 => format!("{:.2}B", v / 1e9),
        a if a >= 1e6 => format!("{:.2}M", v / 1e6),
        a if a >= 1e3 => format!("{:.1}K", v / 1e3),
        _ => format!("{v:.0}"),
    }
}

fn signed_text(v: Option<f64>) -> &'static str {
    match v {
        Some(v) if v > 0.0 => "text-ctp-green",
        Some(v) if v < 0.0 => "text-ctp-red",
        _ => "text-ctp-subtext0",
    }
}

fn dash() -> String {
    "—".to_string()
}

// ─── Board ────────────────────────────────────────────────────────────────────

/// The board for `source`. Click a header to sort (again to reverse, a
/// third time for the original order), type to filter, click a row to open
/// the stock.
#[component]
pub fn LiveBoard(source: BoardSource, title: String) -> Element {
    let feed = use_board_feed(source);
    let mut watch = feed.watch;
    let mut sort = use_signal(|| None::<(BoardSort, bool)>);
    let mut query = use_signal(String::new);
    let mut limit = use_signal(|| PAGE);

    let shown = use_memo(move || {
        let needle = query();
        let mut rows: Vec<BoardQuote> = match &*feed.rows.read() {
            Some(Ok(rows)) => rows.iter().filter(|r| matches(r, &needle)).cloned().collect(),
            _ => vec![],
        };
        if let Some((key, descending)) = sort() {
            sort_board(&mut rows, key, descending);
        }
        rows
    });
    let breadth = use_memo(move || match &*feed.rows.read() {
        Some(Ok(rows)) => Breadth::of(rows),
        _ => Breadth::default(),
    });
    // Stream the rows on screen, and only those.
    use_effect(move || {
        let mut list: Vec<TickerSymbol> =
            shown.read().iter().take(limit()).filter_map(|r| TickerSymbol::new(&r.ticker).ok()).collect();
        list.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        if *watch.peek() != list {
            watch.set(list);
        }
    });

    let state = match &*feed.rows.read() {
        None => Loaded::Loading,
        Some(Err(message)) => Loaded::Failed(message.clone()),
        Some(Ok(rows)) => Loaded::Rows(rows.len()),
    };
    let rows = shown.read();
    let hidden = rows.len().saturating_sub(limit());
    let moves = feed.moves.read();
    let with_sector = rows.iter().any(|r| r.sector.is_some());
    let status = match (feed.last_trade)() {
        Some(at) => rsx! {
            span { class: "inline-flex items-center gap-1.5 text-xs text-ctp-green", title: tr("Prices move with each trade streamed from the exchange feed"),
                span { class: "h-2 w-2 rounded-full bg-ctp-green motion-safe:animate-pulse" }
                {trf("Live · last trade {}", &[&fmt_clock(at)])}
            }
        },
        None => rsx! {
            span { class: "inline-flex items-center gap-1.5 text-xs text-ctp-subtext0", title: tr("No trade has streamed in yet: the market may be closed. Rows still refresh every few seconds."),
                span { class: "h-2 w-2 rounded-full bg-ctp-overlay0" }
                {tr("Waiting for trades")}
            }
        },
    };
    let head = move |key: BoardSort, label: &'static str, class: &'static str| {
        let on = sort().is_some_and(|(k, _)| k == key);
        let down = sort().is_some_and(|(_, d)| d);
        let (arrow, sorted) = match (on, down) {
            (true, true) => (" ↓", "descending"),
            (true, false) => (" ↑", "ascending"),
            _ => ("", "none"),
        };
        rsx! {
            th { class: "{class} py-2 font-normal",
                "aria-sort": "{sorted}",
                button {
                    r#type: "button",
                    class: if on { "cursor-pointer select-none text-ctp-text" } else { "cursor-pointer select-none hover:text-ctp-text" },
                    onclick: move |_| {
                        // Numbers start largest first, names A to Z.
                        let first = key != BoardSort::Ticker;
                        sort.set(match sort() {
                            Some((k, d)) if k == key && d == first => Some((key, !first)),
                            Some((k, _)) if k == key => None,
                            _ => Some((key, first)),
                        });
                    },
                    "{tr_str(label)}{arrow}"
                }
            }
        }
    };

    rsx! {
        Card {
            title,
            subtitle: match state {
                Loaded::Rows(n) => trf("{} rows · each in its own currency", &[&n]),
                _ => String::new(),
            },
            flush: true,
            actions: rsx! {
                div { class: "flex flex-wrap items-center gap-3",
                    {status}
                    input {
                        class: "w-44 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm text-ctp-text \
                                placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                        r#type: "search",
                        placeholder: tr("Symbol or name…"),
                        aria_label: tr("Filter the board"),
                        value: "{query}",
                        oninput: move |e| {
                            query.set(e.value());
                            limit.set(PAGE);
                        },
                    }
                }
            },
            match state {
                Loaded::Loading => rsx! {
                    p { class: "px-6 pb-8 text-sm text-ctp-subtext0", role: "status", {tr("Loading prices…")} }
                },
                Loaded::Failed(message) => rsx! {
                    div { class: "px-6 pb-8",
                        p { class: "text-sm text-ctp-red", title: "{message}", {tr("Couldn't load prices right now. The data source may be busy; try again shortly.")} }
                        p { class: "mt-1 text-xs text-ctp-subtext0", {tr("Retrying in a few seconds…")} }
                    }
                },
                Loaded::Rows(0) => rsx! {
                    p { class: "px-6 pb-8 text-sm text-ctp-subtext0", {tr("Nothing to show here yet.")} }
                },
                Loaded::Rows(_) => rsx! {
                    BreadthBar { breadth: breadth() }
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm tabular-nums",
                            thead {
                                tr { class: "border-t border-ctp-surface0/60 text-xs text-ctp-subtext0",
                                    {head(BoardSort::Ticker, "Symbol", "pl-4 pr-1.5 text-left sm:pl-6")}
                                    {head(BoardSort::Price, "Last", "px-1.5 text-right")}
                                    {head(BoardSort::ChangePct, "Change", "px-1.5 text-right")}
                                    th { class: "hidden px-1.5 py-2 text-right font-normal md:table-cell", {tr("Bid / Ask")} }
                                    {head(BoardSort::Volume, "Vol", "hidden px-1.5 text-right sm:table-cell")}
                                    {head(BoardSort::Turnover, "Value", "hidden px-1.5 text-right md:table-cell")}
                                    {head(BoardSort::RelativeVolume, "× avg", "hidden px-1.5 text-right lg:table-cell")}
                                    {head(BoardSort::DayPosition, "Day range", "hidden px-1.5 text-left lg:table-cell")}
                                    {head(BoardSort::YearPosition, "52-week range", "hidden px-1.5 text-left lg:table-cell")}
                                    {head(BoardSort::MarketCap, "Market cap", "hidden pl-1.5 pr-4 text-right sm:pr-6 xl:table-cell")}
                                }
                            }
                            tbody {
                                for row in rows.iter().take(limit()) {
                                    BoardRow {
                                        key: "{row.ticker}",
                                        flash: moves.get(&row.ticker).copied(),
                                        with_sector,
                                        row: row.clone(),
                                    }
                                }
                            }
                        }
                    }
                    if rows.is_empty() {
                        p { class: "px-6 py-6 text-sm text-ctp-subtext0", {tr("No row matches that.")} }
                    }
                    if hidden > 0 {
                        div { class: "flex flex-wrap items-center justify-center gap-2 border-t border-ctp-surface0/60 px-6 py-3",
                            GhostButton {
                                label: trf("Show {} more", &[&hidden.min(PAGE)]),
                                onclick: move |_| limit.set(limit().saturating_add(PAGE)),
                            }
                            span { class: "text-xs text-ctp-overlay1", {tr("Only the rows shown are streamed live.")} }
                        }
                    } else {
                        div { class: "h-3" }
                    }
                },
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum Loaded {
    Loading,
    Failed(String),
    /// How many rows the board has, before filtering.
    Rows(usize),
}

/// How many rows are up, down and unchanged, as counts and one bar.
#[component]
fn BreadthBar(breadth: Breadth) -> Element {
    let total = breadth.counted().max(1) as f64;
    let share = |n: usize| n as f64 / total * 100.0;
    rsx! {
        div { class: "flex flex-wrap items-center gap-x-4 gap-y-1 px-4 pb-3 text-xs sm:px-6",
            span { class: "text-ctp-green", title: tr("Rows up on the day"), "▲ {breadth.up}" }
            span { class: "text-ctp-red", title: tr("Rows down on the day"), "▼ {breadth.down}" }
            span { class: "text-ctp-subtext0", title: tr("Rows unchanged"), "＝ {breadth.flat}" }
            span { class: "flex h-1.5 min-w-24 flex-1 overflow-hidden rounded-full bg-ctp-surface0",
                role: "img",
                "aria-label": trf("{} up, {} down, {} unchanged", &[&breadth.up, &breadth.down, &breadth.flat]),
                span { class: "bg-ctp-green/80", style: "width:{share(breadth.up):.2}%" }
                span { class: "bg-ctp-overlay0/60", style: "width:{share(breadth.flat):.2}%" }
                span { class: "bg-ctp-red/80", style: "width:{share(breadth.down):.2}%" }
            }
            span { class: "text-ctp-subtext0", {trf("52-week highs {} · lows {}", &[&breadth.year_highs, &breadth.year_lows])} }
        }
    }
}

/// `flash`: which way the row's last streamed trade moved it, and a count
/// that changes with every move.
#[component]
fn BoardRow(row: BoardQuote, flash: Option<(Move, u32)>, with_sector: bool) -> Element {
    let ticker = TickerSymbol::new(&row.ticker).ok();
    let change_pct = row.change_pct();
    let tone = signed_text(change_pct);
    let pill = match change_pct {
        Some(c) if c > 0.0 => "bg-ctp-green/15 text-ctp-green",
        Some(c) if c < 0.0 => "bg-ctp-red/15 text-ctp-red",
        _ => "bg-ctp-surface0 text-ctp-subtext0",
    };
    // Lite: no flash.
    let (flash_class, flash_key) = match flash.filter(|_| !crate::perf::lite()) {
        Some((Move::Up, n)) => ("tick-up", n),
        Some((Move::Down, n)) => ("tick-down", n),
        None => ("", 0),
    };
    let mark = match flash {
        Some((Move::Up, _)) => "▲",
        Some((Move::Down, _)) => "▼",
        None => "",
    };
    let mark_tone = match flash {
        Some((Move::Up, _)) => "text-ctp-green",
        _ => "text-ctp-red",
    };
    let extended = row.ext_price.filter(|_| row.session != Session::Regular).zip(row.ext_change_pct());
    let relative = row.relative_volume();
    let under = match (&row.sector, with_sector) {
        (Some(sector), true) => format!("{} · {sector}", row.name),
        _ => row.name.clone(),
    };
    let traded = row.time.map(|t| trf("Last trade {}", &[&fmt_clock(t)])).unwrap_or_default();
    rsx! {
        tr {
            class: "cursor-pointer border-t border-ctp-surface0/60 transition-colors hover:bg-ctp-surface0/30",
            title: "{traded}",
            onclick: move |_| {
                if let Some(t) = &ticker {
                    open_stock(t);
                }
            },
            td { class: "max-w-[9rem] py-2 pl-4 pr-1.5 sm:max-w-[12rem] sm:pl-6",
                div { class: "flex items-center gap-1.5",
                    span { class: "font-semibold text-ctp-text", "{row.ticker}" }
                    if let Some(badge) = row.session.badge() {
                        span { class: "whitespace-nowrap rounded bg-ctp-surface0 px-1 text-[10px] font-medium leading-4 text-ctp-subtext0", {tr_str(badge)} }
                    }
                    if row.at_year_high() {
                        span { class: "rounded bg-ctp-green/15 px-1 text-[10px] font-medium leading-4 text-ctp-green", title: tr("At its 52-week high"), "52H" }
                    } else if row.at_year_low() {
                        span { class: "rounded bg-ctp-red/15 px-1 text-[10px] font-medium leading-4 text-ctp-red", title: tr("At its 52-week low"), "52L" }
                    }
                }
                div { class: "truncate text-xs text-ctp-subtext0", "{under}" }
            }
            td { class: "whitespace-nowrap px-1.5 py-2 text-right",
                div { class: "flex items-center justify-end gap-1",
                    span { class: "w-2 text-[9px] {mark_tone}", "aria-hidden": "true", "{mark}" }
                    // Re-created on every move, which restarts the flash.
                    // Outside regular hours it's the line below that moved.
                    Ticking {
                        value: row.price,
                        text: fmt_price(row.price),
                        class: "px-1 font-medium text-ctp-text",
                        flash_class: if extended.is_some() { "" } else { flash_class },
                        flash_key,
                    }
                }
                if let Some((price, change)) = extended {
                    div { class: "flex justify-end", title: tr("Traded outside regular hours"),
                        Ticking {
                            value: price,
                            text: format!("{} {change:+.2}%", fmt_price(price)),
                            class: format!("px-1 text-[11px] {}", signed_text(Some(change))),
                            flash_class,
                            flash_key,
                        }
                    }
                } else {
                    div { class: "pr-1 text-[11px] text-ctp-overlay1", "{row.currency}" }
                }
            }
            td { class: "whitespace-nowrap px-1.5 py-2 text-right",
                span { class: "inline-flex min-w-[4.25rem] justify-end rounded-md px-1.5 py-0.5 text-xs font-semibold {pill}",
                    match change_pct {
                        Some(c) => rsx! { LiveNumber { value: decimal(c), text: format!("{c:+.2}%") } },
                        None => rsx! { "—" },
                    }
                }
                div { class: "pr-1.5 text-[11px] {tone}",
                    {row.change().map_or_else(dash, |c| format!("{}{}", if c > 0.0 { "+" } else { "" }, fmt_price(c)))}
                }
            }
            td { class: "hidden whitespace-nowrap px-1.5 py-2 text-right text-ctp-subtext1 md:table-cell",
                div {
                    {row.bid.map_or_else(dash, fmt_price)}
                    span { class: "px-1 text-ctp-overlay0", "/" }
                    {row.ask.map_or_else(dash, fmt_price)}
                }
                if let Some(spread) = row.spread_pct() {
                    div { class: "text-[11px] text-ctp-overlay1", title: tr("Ask minus bid, as a share of the price"), {trf("spread {}%", &[&format!("{spread:.2}")])} }
                }
            }
            td { class: "hidden px-1.5 py-2 text-right text-ctp-subtext1 sm:table-cell", {row.volume.map_or_else(dash, fmt_count)} }
            td { class: "hidden px-1.5 py-2 text-right text-ctp-subtext1 md:table-cell", {row.turnover().map_or_else(dash, fmt_count)} }
            td {
                class: match relative {
                    Some(r) if r >= 2.0 => "hidden px-1.5 py-2 text-right font-semibold text-ctp-peach lg:table-cell",
                    Some(r) if r >= 1.0 => "hidden px-1.5 py-2 text-right text-ctp-text lg:table-cell",
                    _ => "hidden px-1.5 py-2 text-right text-ctp-subtext0 lg:table-cell",
                },
                title: tr("Today's volume against an average day's (three months)"),
                {relative.map_or_else(dash, |r| format!("{r:.2}×"))}
            }
            td { class: "hidden px-1.5 py-2 lg:table-cell",
                RangeBar { low: row.day_low, high: row.day_high, at: row.day_position(), mark: row.open.zip(row.day_low).zip(row.day_high).and_then(|((o, l), h)| (h > l).then(|| ((o - l) / (h - l)).clamp(0.0, 1.0))) }
            }
            td { class: "hidden px-1.5 py-2 lg:table-cell",
                RangeBar { low: row.low_52w, high: row.high_52w, at: row.year_position() }
            }
            td { class: "hidden whitespace-nowrap py-2 pl-1.5 pr-4 text-right text-ctp-subtext1 sm:pr-6 xl:table-cell",
                {row.market_cap_usd.map_or_else(dash, fmt_compact)}
            }
        }
    }
}

/// A number that rolls to its new digits, like the totals on the dashboard,
/// over a flash of the way the trade moved it. The flash is a backdrop
/// re-created on every move, which restarts it; the number stays mounted so
/// only its changed digits roll.
#[component]
fn Ticking(value: f64, text: String, #[props(into, default)] class: String, flash_class: &'static str, flash_key: u32) -> Element {
    rsx! {
        span { class: "relative inline-block tabular-nums {class}",
            for n in [flash_key] {
                span { key: "{n}", class: "absolute inset-0 {flash_class}", "aria-hidden": "true" }
            }
            span { class: "relative", LiveNumber { value: decimal(value), text } }
        }
    }
}

fn decimal(v: f64) -> rust_decimal::Decimal {
    rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default()
}

/// A low–high track with a dot where the price is (`at`, 0 to 1) and,
/// optionally, a tick where the day opened (`mark`).
#[component]
fn RangeBar(low: Option<f64>, high: Option<f64>, at: Option<f64>, #[props(default)] mark: Option<f64>) -> Element {
    let (Some(low), Some(high), Some(at)) = (low, high, at) else {
        return rsx! { span { class: "text-ctp-overlay1", "—" } };
    };
    rsx! {
        div { class: "w-24",
            div { class: "relative h-1.5 rounded-full bg-ctp-surface0",
                if let Some(open) = mark {
                    span { class: "absolute top-[-2px] h-[10px] w-px bg-ctp-overlay1", style: "left:{open * 100.0:.1}%", title: tr("Where the day opened") }
                }
                span { class: "absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border border-ctp-mantle bg-ctp-mauve", style: "left:{at * 100.0:.1}%" }
            }
            div { class: "mt-1 flex justify-between text-[10px] leading-none text-ctp-overlay1",
                span { "{fmt_price(low)}" }
                span { "{fmt_price(high)}" }
            }
        }
    }
}

// ─── World strip ──────────────────────────────────────────────────────────────

/// Markets around the world, live: indices, volatility, rates, the
/// dollar, gold, oil and Bitcoin.
#[component]
pub fn WorldStrip() -> Element {
    let tickers: Vec<TickerSymbol> = WORLD.iter().filter_map(|(symbol, _)| TickerSymbol::new(symbol).ok()).collect();
    let all = tickers.clone();
    let feed = use_board_feed(BoardSource::Tickers(tickers));
    let mut watch = feed.watch;
    use_effect(move || {
        if matches!(*feed.rows.read(), Some(Ok(_))) && watch.peek().is_empty() {
            watch.set(all.clone());
        }
    });
    let rows = match &*feed.rows.read() {
        Some(Ok(rows)) => rows.clone(),
        _ => vec![],
    };
    let moves = feed.moves.read();
    if rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "flex gap-2 overflow-x-auto pb-1 sm:grid sm:grid-cols-4 sm:overflow-visible lg:grid-cols-8",
            "aria-label": tr("Markets around the world"),
            for row in rows {
                WorldTile {
                    key: "{row.ticker}",
                    name: WORLD.iter().find(|(symbol, _)| *symbol == row.ticker).map_or(row.name.clone(), |(_, name)| tr_str(name).to_string()),
                    flash: moves.get(&row.ticker).copied(),
                    row,
                }
            }
        }
    }
}

#[component]
fn WorldTile(row: BoardQuote, name: String, flash: Option<(Move, u32)>) -> Element {
    let ticker = TickerSymbol::new(&row.ticker).ok();
    let change = row.change_pct();
    let (flash_class, flash_key) = match flash.filter(|_| !crate::perf::lite()) {
        Some((Move::Up, n)) => ("tick-up", n),
        Some((Move::Down, n)) => ("tick-down", n),
        None => ("", 0),
    };
    rsx! {
        button {
            r#type: "button",
            class: "min-w-[8.5rem] shrink-0 rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60 px-3 py-2 text-left cursor-pointer \
                    transition-colors hover:border-ctp-surface1 sm:min-w-0",
            onclick: move |_| {
                if let Some(t) = &ticker {
                    open_stock(t);
                }
            },
            div { class: "flex items-center justify-between gap-2",
                span { class: "truncate text-xs text-ctp-subtext0", "{name}" }
                if row.session != Session::Regular {
                    span { class: "h-1.5 w-1.5 shrink-0 rounded-full bg-ctp-overlay0", title: tr("Market closed") }
                } else {
                    span { class: "h-1.5 w-1.5 shrink-0 rounded-full bg-ctp-green", title: tr("Market open") }
                }
            }
            div { class: "mt-0.5",
                Ticking { value: row.price, text: fmt_price(row.price), class: "text-sm font-semibold text-ctp-text", flash_class, flash_key }
            }
            div { class: "text-xs tabular-nums {signed_text(change)}",
                match change {
                    Some(c) => rsx! { LiveNumber { value: decimal(c), text: format!("{c:+.2}%") } },
                    None => rsx! { "—" },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_keep_the_digits_that_move() {
        assert_eq!(fmt_price(6812.345), "6,812.35");
        assert_eq!(fmt_price(236.5), "236.50");
        assert_eq!(fmt_price(1_234_567.0), "1,234,567.00");
        assert_eq!(fmt_price(0.4321), "0.4321");
        assert_eq!(fmt_price(0.00001234), "0.000012");
        assert_eq!(fmt_price(-1.5), "-1.50");
    }

    #[test]
    fn counts_are_compact() {
        assert_eq!(fmt_count(41_230_000.0), "41.23M");
        assert_eq!(fmt_count(9_700_000_000.0), "9.70B");
        assert_eq!(fmt_count(15_300.0), "15.3K");
        assert_eq!(fmt_count(812.0), "812");
        assert_eq!(fmt_count(2.5e12), "2.50T");
    }

    fn row(ticker: &str, price: f64, session: Session) -> BoardQuote {
        BoardQuote {
            ticker: ticker.into(),
            name: format!("{ticker} Inc."),
            currency: "USD".into(),
            price,
            previous_close: Some(100.0),
            session,
            time: Some(100),
            ..Default::default()
        }
    }

    /// A feed with two rows, fed the trades in `ticks`.
    #[component]
    fn Fed(ticks: Vec<(String, f64, i64)>) -> Element {
        let feed = BoardFeed {
            rows: use_signal(|| Some(Ok(vec![row("AMD", 110.0, Session::Regular), row("CAT", 90.0, Session::Post)]))),
            moves: use_signal(HashMap::new),
            watch: use_signal(Vec::new),
            last_trade: use_signal(|| None),
        };
        use_hook(|| {
            let mut latest = HashMap::new();
            for (ticker, price, time) in &ticks {
                let tick = Tick { price: *price, volume: None, time: *time };
                feed.apply(&mut latest, HashMap::from([(ticker.clone(), tick)]));
            }
        });
        let rows = feed.rows.read().clone().and_then(Result::ok).unwrap_or_default();
        let moves = feed.moves.read().clone();
        let last = *feed.last_trade.read();
        rsx! {
            for r in rows {
                p { "{r.ticker} {r.price} {r.ext_price:?} {moves.get(&r.ticker):?}" }
            }
            p { "last {last:?}" }
        }
    }

    fn fed(ticks: &[(&str, f64, i64)]) -> String {
        let ticks = ticks.iter().map(|(t, p, at)| (t.to_string(), *p, *at)).collect();
        let mut dom = VirtualDom::new_with_props(Fed, FedProps { ticks });
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn streamed_trades_move_rows_and_count_each_move() {
        let html = fed(&[("AMD", 111.0, 101), ("AMD", 112.0, 102), ("CAT", 89.0, 103), ("XYZ", 5.0, 104), ("AMD", 1.0, 50)]);
        assert!(html.contains("AMD 112 None Some((Up, 1))"), "two moves up, the late trade ignored: {html}");
        assert!(html.contains("CAT 90 Some(89.0) Some((Down, 0))"), "after hours: the close stays, the extended price moves: {html}");
        assert!(html.contains("last Some(103)"), "the newest trade applied to a row: {html}");
    }

    #[test]
    fn a_trade_at_the_same_price_writes_nothing() {
        let html = fed(&[("AMD", 110.0, 100)]);
        assert!(html.contains("AMD 110 None None") && html.contains("last None"), "{html}");
    }
}
