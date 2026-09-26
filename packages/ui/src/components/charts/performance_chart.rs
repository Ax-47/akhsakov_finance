//! Performance card: compare any mix of all holdings, a single portfolio,
//! an index or a stock over a period. Each line is picked from a chip.

use crate::i18n::tr;
use super::{
    performance::{compare, Comparison, LabelStyle, Subject},
    GrowthChart, Series,
};
use crate::{
    components::card::{Card, Chevron, MenuItem, Segmented, ToggleButton},
    format::fmt_signed,
};
use api::quote::quote::get_charts;
use dioxus::prelude::*;
use dtos::Transaction;
use rust_decimal::Decimal;
use std::collections::HashSet;
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

const PERIODS: [Range; 5] = [Range::D1, Range::M1, Range::M6, Range::Ytd, Range::Y1];
const MAX_LINES: usize = 5;

/// Line colours by position (blue, yellow, mauve, green, peach).
const LINE_COLORS: [&str; MAX_LINES] = ["var(--catppuccin-color-blue)", "var(--catppuccin-color-yellow)", "var(--catppuccin-color-mauve)", "var(--catppuccin-color-green)", "var(--catppuccin-color-peach)"];

/// Indexes offered in the picker, as (Yahoo symbol, display name).
const INDEXES: [(&str, &str); 3] = [
    ("^GSPC", "S&P 500"),
    ("^NDX", "Nasdaq 100"),
    ("^DJI", "Dow Jones"),
];

/// What a chart line shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    AllHoldings,
    /// Portfolio id.
    Portfolio(String),
    Ticker(TickerSymbol),
}

/// `portfolios` is `(id, name)`. Compares the S&P 500 with `portfolio`
/// (all holdings when `None`); when `portfolio` changes, the holdings line
/// follows it and any other lines stay.
#[component]
pub fn ChartSection(
    transactions: ReadSignal<Vec<Transaction>>,
    portfolios: ReadSignal<Vec<(String, String)>>,
    height: Decimal,
    portfolio: ReadSignal<Option<String>>,
) -> Element {
    let mut period = use_signal(|| Range::Y1);
    let benchmark = use_context::<crate::app::AppSettings>().benchmark();
    let mut picks = use_signal(move || {
        vec![
            Pick::Ticker(benchmark),
            holdings_pick(portfolio.peek().clone()),
        ]
    });

    use_effect(move || {
        let mine = holdings_pick(portfolio());
        let current = picks.peek().clone();
        let holdings_line = current
            .iter()
            .position(|p| matches!(p, Pick::AllHoldings | Pick::Portfolio(_)));
        match holdings_line {
            Some(i) if current[i] == mine => {}
            Some(i) => picks.write()[i] = mine,
            None => picks.write().push(mine),
        }
    });
    let open_menu = use_signal(|| None::<usize>);

    let history = crate::cache::use_cached(
        move || {
            let txs = transactions.read();
            let fingerprint = txs.iter().fold(0u128, |a, t| a.wrapping_mul(31).wrapping_add(t.id.as_u128()));
            format!("performance/{:?}/{:?}/{}/{fingerprint}", picks(), period(), txs.len())
        },
        move || {
        let txs = transactions.read().clone();
        let picks = picks();
        let range = period();
        async move {
            let subjects: Vec<Subject> = picks.iter().map(|p| subject(p, &txs)).collect();
            let mut symbols: HashSet<TickerSymbol> = HashSet::new();
            for s in &subjects {
                match s {
                    Subject::Holdings(txs) => symbols.extend(
                        txs.iter()
                            .filter(|tx| !tx.is_cash())
                            .map(|tx| tx.ticker.clone()),
                    ),
                    Subject::Ticker(t) => {
                        symbols.insert(t.clone());
                    }
                }
            }
            // 1D fetches a few days so yesterday's close is included.
            let fetch_range = if range == Range::D1 { Range::D5 } else { range };
            let charts = get_charts(
                symbols.into_iter().collect(),
                fetch_range,
                interval_for(range),
                false,
            )
            .await
            .ok()?;
            Some(compare(
                &charts,
                &subjects,
                label_style(range),
                range == Range::D1,
            ))
        }
    },
    );

    // Outer None: loading. Inner None: request failed.
    let comparison = use_memo(move || history.read().clone());
    let chart_dates =
        use_memo(move || comparison().flatten().map(|c| c.labels).unwrap_or_default());
    let series = use_memo(move || {
        let names: Vec<String> = picks
            .read()
            .iter()
            .map(|p| pick_name(p, &portfolios.read()))
            .collect();
        comparison()
            .flatten()
            .map(|c| to_series(&c, &names))
            .unwrap_or_default()
    });

    let lines = comparison().flatten().map(|c| c.lines).unwrap_or_default();
    let has_data = comparison()
        .flatten()
        .is_some_and(|c: Comparison| !c.labels.is_empty());

    rsx! {
        document::Script { src: asset!("/assets/js/growth_chart.js") }
        Card {
            title: tr("Performance"),
            subtitle: period_phrase(period()).to_string(),
            actions: rsx! {
                Segmented {
                    for p in PERIODS {
                        ToggleButton {
                            key: "{p}",
                            label: period_label(p),
                            active: period() == p,
                            onclick: move |_| period.set(p),
                        }
                    }
                }
            },
            div { class: "flex flex-wrap items-center gap-2 mb-3",
                for (i, pick) in picks().into_iter().enumerate() {
                    PickChip {
                        key: "{i}",
                        index: i,
                        name: pick_name(&pick, &portfolios.read()),
                        change: lines.get(i).map(|l| l.change()),
                        gain: lines.get(i).and_then(|l| l.gain),
                        picks,
                        open_menu,
                        transactions,
                        portfolios,
                    }
                }
                if picks.read().len() < MAX_LINES {
                    AddLineButton { picks, open_menu, transactions }
                }
            }
            match comparison() {
                None => rsx! { ChartPlaceholder { height, text: "Loading…" } },
                Some(_) if has_data => rsx! { GrowthChart { chart_dates, series, height } },
                Some(_) => rsx! { ChartPlaceholder { height, text: "No data" } },
            }
        }
    }
}

// ─── Chips and menu ───────────────────────────────────────────────────────────

/// One line's selector, like "● All holdings +15.14% ⌄".
#[component]
fn PickChip(
    index: usize,
    name: String,
    change: Option<f64>,
    gain: Option<f64>,
    picks: Signal<Vec<Pick>>,
    mut open_menu: Signal<Option<usize>>,
    transactions: ReadSignal<Vec<Transaction>>,
    portfolios: ReadSignal<Vec<(String, String)>>,
) -> Element {
    let color = LINE_COLORS[index % LINE_COLORS.len()];
    let is_open = open_menu() == Some(index);
    let gain = gain.map(|g| {
        format!(
            "{} · ",
            fmt_signed(Decimal::try_from(g).unwrap_or_default(), 2)
        )
    });

    rsx! {
        div { class: "relative",
            button {
                class: if is_open {
                    "inline-flex items-center gap-2 rounded-full border border-ctp-mauve bg-ctp-crust/40 pl-3 pr-2 py-1.5 text-sm cursor-pointer transition-colors"
                } else {
                    "inline-flex items-center gap-2 rounded-full border border-ctp-surface1 bg-ctp-crust/40 pl-3 pr-2 py-1.5 text-sm cursor-pointer transition-colors hover:border-ctp-surface2"
                },
                aria_expanded: is_open,
                onclick: move |_| open_menu.set(if is_open { None } else { Some(index) }),
                span { class: "h-2.5 w-2.5 rounded-full", style: "background:{color};" }
                span { class: "font-medium text-ctp-text", "{name}" }
                if let Some(change) = change {
                    span {
                        class: if change >= 0.0 { "tabular-nums text-xs font-semibold text-ctp-green" } else { "tabular-nums text-xs font-semibold text-ctp-red" },
                        if let Some(gain) = gain { "{gain}" }
                        "{change:+.2}%"
                    }
                }
                Chevron { open: is_open }
            }
            if is_open {
                PickMenu { index, picks, open_menu, transactions, portfolios }
            }
        }
    }
}

#[component]
fn AddLineButton(
    mut picks: Signal<Vec<Pick>>,
    mut open_menu: Signal<Option<usize>>,
    transactions: ReadSignal<Vec<Transaction>>,
) -> Element {
    rsx! {
        button {
            class: "flex h-9 w-9 items-center justify-center rounded-full border border-ctp-surface1 \
                    text-lg text-ctp-subtext0 cursor-pointer transition-colors hover:border-ctp-mauve hover:text-ctp-text",
            "aria-label": "Compare another line",
            title: tr("Compare another line"),
            onclick: move |_| {
                let next = next_unused(&picks.read(), &transactions.read());
                picks.write().push(next);
                open_menu.set(Some(picks.read().len() - 1));
            },
            "+"
        }
    }
}

#[component]
fn PickMenu(
    index: usize,
    mut picks: Signal<Vec<Pick>>,
    mut open_menu: Signal<Option<usize>>,
    transactions: ReadSignal<Vec<Transaction>>,
    portfolios: ReadSignal<Vec<(String, String)>>,
) -> Element {
    let mut custom = use_signal(String::new);
    let mut choose = move |pick: Pick| {
        picks.write()[index] = pick;
        open_menu.set(None);
    };

    let current = picks.read().get(index).cloned();
    let taken: Vec<Pick> = picks
        .read()
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, p)| p.clone())
        .collect();

    let mut stocks: Vec<TickerSymbol> = transactions
        .read()
        .iter()
        .filter(|tx| !tx.is_cash())
        .map(|tx| tx.ticker.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    stocks.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let mut holdings = vec![(Pick::AllHoldings, "All holdings".to_string())];
    holdings.extend(
        portfolios
            .read()
            .iter()
            .map(|(id, name)| (Pick::Portfolio(id.clone()), name.clone())),
    );
    let indexes: Vec<(Pick, String)> = INDEXES
        .iter()
        .filter_map(|(sym, name)| {
            Some((Pick::Ticker(TickerSymbol::new(sym).ok()?), name.to_string()))
        })
        .collect();
    let stocks: Vec<(Pick, String)> = stocks
        .into_iter()
        .map(|t| (Pick::Ticker(t.clone()), t.to_string()))
        .collect();
    let can_remove = picks.read().len() > 1;

    rsx! {
        // Click anywhere else to close.
        div { class: "fixed inset-0 z-20", onclick: move |_| open_menu.set(None) }
        div { class: "absolute left-0 top-full z-30 mt-2 w-64 max-h-96 overflow-y-auto rounded-2xl \
                      border border-ctp-surface0 bg-ctp-mantle p-1.5 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
            for (title, options) in [("Portfolios", holdings), ("Indexes", indexes), ("Your stocks", stocks)] {
                if !options.is_empty() {
                    div { class: "px-2.5 pt-2 pb-1 text-xs text-ctp-overlay1", "{title}" }
                    for (pick, label) in options {
                        MenuItem {
                            label,
                            selected: current.as_ref() == Some(&pick),
                            taken: taken.contains(&pick),
                            onclick: move |_| choose(pick.clone()),
                        }
                    }
                }
            }
            div { class: "mx-1 mt-2 mb-1 border-t border-ctp-surface0" }
            form {
                class: "px-1 py-1",
                onsubmit: move |e| {
                    e.prevent_default();
                    if let Ok(t) = TickerSymbol::new(&custom()) {
                        custom.set(String::new());
                        choose(Pick::Ticker(t));
                    }
                },
                input {
                    class: "w-full rounded-xl border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1.5 text-sm \
                            text-ctp-text placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                    placeholder: tr("Other ticker, e.g. AAPL ↵"),
                    value: "{custom}",
                    oninput: move |e| custom.set(e.value()),
                }
            }
            if can_remove {
                button {
                    class: "mt-1 w-full rounded-xl px-2.5 py-1.5 text-left text-sm text-ctp-red cursor-pointer \
                            transition-colors hover:bg-ctp-red/10",
                    onclick: move |_| {
                        picks.write().remove(index);
                        open_menu.set(None);
                    },
                    {tr("Remove line")}
                }
            }
        }
    }
}

#[component]
fn ChartPlaceholder(height: Decimal, text: String) -> Element {
    rsx! {
        div {
            class: "flex items-center justify-center text-ctp-subtext0 text-sm",
            style: "height:{height}px;",
            "{text}"
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn holdings_pick(portfolio: Option<String>) -> Pick {
    portfolio.map_or(Pick::AllHoldings, Pick::Portfolio)
}

fn subject(pick: &Pick, transactions: &[Transaction]) -> Subject {
    match pick {
        Pick::AllHoldings => Subject::Holdings(transactions.to_vec()),
        Pick::Portfolio(id) => Subject::Holdings(
            transactions
                .iter()
                .filter(|tx| tx.portfolio_id.to_string() == *id)
                .cloned()
                .collect(),
        ),
        Pick::Ticker(t) => Subject::Ticker(t.clone()),
    }
}

fn pick_name(pick: &Pick, portfolios: &[(String, String)]) -> String {
    match pick {
        Pick::AllHoldings => tr("All holdings").into(),
        Pick::Portfolio(id) => portfolios
            .iter()
            .find(|(pid, _)| pid == id)
            .map_or_else(|| "Portfolio".into(), |(_, name)| name.clone()),
        Pick::Ticker(t) => INDEXES
            .iter()
            .find(|(sym, _)| *sym == t.as_str())
            .map_or_else(|| t.to_string(), |(_, name)| name.to_string()),
    }
}

/// A sensible new line: the first index or stock not already shown.
fn next_unused(picks: &[Pick], transactions: &[Transaction]) -> Pick {
    INDEXES
        .iter()
        .filter_map(|(sym, _)| TickerSymbol::new(sym).ok())
        .chain(
            transactions
                .iter()
                .filter(|tx| !tx.is_cash())
                .map(|tx| tx.ticker.clone()),
        )
        .map(Pick::Ticker)
        .find(|p| !picks.contains(p))
        .unwrap_or(Pick::AllHoldings)
}

fn to_series(c: &Comparison, names: &[String]) -> Vec<Series> {
    c.lines
        .iter()
        .enumerate()
        .map(|(i, line)| Series {
            name: names.get(i).cloned().unwrap_or_default(),
            color: LINE_COLORS[i % LINE_COLORS.len()].into(),
            values: line
                .values
                .iter()
                .map(|v| Decimal::try_from(*v).ok())
                .collect(),
        })
        .collect()
}

fn period_label(period: Range) -> &'static str {
    match period {
        Range::D1 => "1D",
        Range::D5 => "1W",
        Range::M1 => "1M",
        Range::M3 => "3M",
        Range::M6 => "6M",
        Range::Ytd => tr("YTD"),
        Range::Y1 => "1Y",
        Range::Y5 => "5Y",
        Range::Max => tr("All"),
        other => other.code(),
    }
}

fn period_phrase(period: Range) -> &'static str {
    match period {
        Range::D1 => tr("Today"),
        Range::D5 => tr("Past week"),
        Range::M1 => tr("Past month"),
        Range::M6 => tr("Past 6 months"),
        Range::Ytd => tr("Year to date"),
        Range::Y1 => tr("Past year"),
        Range::Y5 => tr("Past 5 years"),
        _ => tr("All time"),
    }
}

/// Candle resolution that gives a smooth line for each range.
fn interval_for(range: Range) -> Interval {
    match range {
        Range::D1 => Interval::I5m,
        Range::D5 => Interval::I30m,
        Range::M1 => Interval::I1h,
        Range::M6 | Range::Ytd | Range::Y1 => Interval::D1,
        _ => Interval::W1,
    }
}

fn label_style(range: Range) -> LabelStyle {
    match range {
        Range::D1 => LabelStyle::Time,
        Range::D5 | Range::M1 => LabelStyle::DayTime,
        _ => LabelStyle::Date,
    }
}
