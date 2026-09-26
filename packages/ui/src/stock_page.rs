//! A stock's own page: header with price and key facts, then tabs for a
//! price summary, statistics, financial statements and analyst views.

use crate::i18n::tr;
use crate::{
    app::DataRefresh,
    components::{
        analysis::{
            compare::{CompareMeasures, MeasureVsMeasure},
            stock::StockReport,
        },
        card::{ActionButton, Card, MenuItem, MetricTile, Segmented, ToggleButton, INPUT},
        charts::bars::{BarChart, BarSeries, Sparkline, Unit},
    },
    editors::{Dialog, Dialogs},
    format::{fmt_compact, fmt_signed, fmt_usd, or_dash, signed_color},
    hooks::{use_portfolio, PortfolioState},
    page::{GhostButton, HeroStat, Page},
    stock_research::{DividendsAndSplits, NewsTab, OptionsTab, OwnershipTab, RatingChanges},
};
use api::quote::quote::get_quote;
use dioxus::prelude::*;
use dtos::fundamentals::{
    current_valuation, ttm, yoy_growth, Analysts, EpsSurprise, PeriodFinancials, StockFundamentals,
    ValuationPoint,
};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::ticker_symbol::TickerSymbol;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary,
    Statistics,
    Financials,
    Analysis,
    News,
    Ownership,
    Options,
    Compare,
}

/// Route target for `/stock/:ticker`.
#[component]
pub fn StockPage(ticker: String) -> Element {
    match TickerSymbol::new(&ticker.replace("%5E", "^")) {
        Ok(ticker) => rsx! { StockView { key: "{ticker}", ticker } },
        Err(_) => rsx! {
            Page { p { class: "text-ctp-subtext0", "“{ticker}” isn't a valid ticker." } }
        },
    }
}

#[component]
fn StockView(ticker: TickerSymbol) -> Element {
    let PortfolioState {
        positions,
        total_value,
        ..
    } = use_portfolio(None);
    let position = positions.iter().find(|p| p.ticker == ticker).cloned();
    let mut tab = use_signal(|| Tab::Summary);
    let dialogs = use_context::<Dialogs>();
    // Compare tab: other stocks, or two measures of this one over time.
    let mut compare_over_time = use_signal(|| false);

    let symbol = ticker.clone();
    let key = ticker.clone();
    let fundamentals = crate::cache::use_cached(move || format!("fundamentals/{key}"), move || {
        let t = symbol.clone();
        async move { api::get_fundamentals(t).await.ok() }
    });
    let symbol = ticker.clone();
    let key = ticker.clone();
    let quote = crate::cache::use_cached(move || format!("quote/{key}"), move || {
        let t = symbol.clone();
        async move { get_quote(t).await.ok() }
    });

    let loading = fundamentals.read().is_none();
    let f: Option<StockFundamentals> = fundamentals.read().clone().flatten();
    let q = quote.read().clone().flatten();

    let price = q.as_ref().map(|q| q.current_price).or_else(|| {
        f.as_ref()
            .and_then(|f| f.stats.price)
            .and_then(|p| Decimal::try_from(p).ok())
    });
    let day = q.as_ref().and_then(|q| {
        (q.previous_close_price > Decimal::ZERO).then(|| {
            (
                q.current_price - q.previous_close_price,
                (q.current_price / q.previous_close_price - Decimal::ONE) * Decimal::ONE_HUNDRED,
            )
        })
    });
    let name = f
        .as_ref()
        .and_then(|f| f.profile.name.clone())
        .unwrap_or_else(|| ticker.to_string());
    let tags: Vec<String> = f
        .as_ref()
        .map(|f| {
            [f.profile.sector.clone(), f.profile.industry.clone()]
                .into_iter()
                .flatten()
                .collect()
        })
        .unwrap_or_default();

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                div { class: "flex items-center justify-between gap-4 mb-4",
                    div { class: "flex flex-wrap items-center gap-2 text-xs text-ctp-subtext0",
                        span { class: "rounded-lg bg-ctp-surface0 px-2 py-0.5 font-mono font-semibold text-ctp-text", "{ticker}" }
                        for t in tags {
                            span { class: "rounded-full border border-ctp-surface0 px-2.5 py-0.5", "{t}" }
                        }
                    }
                    div { class: "flex flex-wrap gap-2",
                        WatchButton { ticker: ticker.clone() }
                        GhostButton {
                            label: tr("🔔 Alert"),
                            onclick: {
                                let t = ticker.clone();
                                move |_| dialogs.open(Dialog::NewAlert(Some(t.clone())))
                            },
                        }
                        GhostButton { label: tr("← Back"), onclick: move |_| navigator().go_back() }
                    }
                }
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 \
                             bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    "{name}"
                }
                div { class: "mt-4 flex flex-wrap items-baseline gap-x-4 gap-y-2",
                    span { class: "text-5xl font-semibold tracking-tight tabular-nums text-ctp-text",
                        {or_dash(price, |p| fmt_usd(p, 2))}
                    }
                    if let Some((abs, pct)) = day {
                        span {
                            class: if abs >= Decimal::ZERO {
                                "rounded-full px-3 py-1 text-sm font-semibold bg-ctp-green/15 text-ctp-green"
                            } else {
                                "rounded-full px-3 py-1 text-sm font-semibold bg-ctp-red/15 text-ctp-red"
                            },
                            "{fmt_signed(abs, 2)} ({pct:+.2}%) today"
                        }
                    }
                }
                if let Some(f) = &f {
                    div { class: "mt-5 flex flex-wrap items-center gap-x-6 gap-y-3 text-sm",
                        HeroStat { label: tr("Market cap"), value: or_dash(f.stats.market_cap, fmt_compact) }
                        HeroStat { label: tr("P/E"), value: or_dash(f.stats.pe_ttm, |v| format!("{v:.1}")) }
                        HeroStat { label: tr("Dividend"), value: or_dash(f.stats.dividend_yield, |v| format!("{:.2}%", v * 100.0)) }
                        HeroStat { label: tr("Next earnings"), value: f.stats.next_earnings.clone().unwrap_or_else(|| "—".into()) }
                    }
                }
                if let Some(p) = &position {
                    div { class: "mt-4 inline-flex flex-wrap items-center gap-x-3 gap-y-1 rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60 px-4 py-2 text-sm",
                        span { class: "text-ctp-subtext0", {tr("You own")} }
                        span { class: "font-medium tabular-nums text-ctp-text", "{p.shares.normalize()} shares · {fmt_usd(p.market_value(), 2)}" }
                        span { class: "font-medium tabular-nums {signed_color(p.unrealized_pnl())}",
                            "{fmt_signed(p.unrealized_pnl(), 2)} ({p.unrealized_pnl_pct():+.2}%)"
                        }
                    }
                }
            }

            nav { class: "mt-10 mb-5 overflow-x-auto",
                Segmented {
                    ToggleButton { label: tr("Summary"), active: tab() == Tab::Summary, onclick: move |_| tab.set(Tab::Summary) }
                    ToggleButton { label: tr("Statistics"), active: tab() == Tab::Statistics, onclick: move |_| tab.set(Tab::Statistics) }
                    ToggleButton { label: tr("Financials"), active: tab() == Tab::Financials, onclick: move |_| tab.set(Tab::Financials) }
                    ToggleButton { label: tr("Analysis"), active: tab() == Tab::Analysis, onclick: move |_| tab.set(Tab::Analysis) }
                    ToggleButton { label: tr("News"), active: tab() == Tab::News, onclick: move |_| tab.set(Tab::News) }
                    ToggleButton { label: tr("Ownership"), active: tab() == Tab::Ownership, onclick: move |_| tab.set(Tab::Ownership) }
                    ToggleButton { label: tr("Options"), active: tab() == Tab::Options, onclick: move |_| tab.set(Tab::Options) }
                    ToggleButton { label: tr("Compare"), active: tab() == Tab::Compare, onclick: move |_| tab.set(Tab::Compare) }
                }
            }

            match (tab(), &f) {
                (Tab::Summary, _) => rsx! {
                    div { class: "grid gap-5 motion-safe:animate-rise",
                        crate::components::charts::TechnicalChart { ticker: ticker.clone() }
                        if let Some(f) = &f {
                            About { fundamentals: f.clone() }
                        }
                        StockReport { ticker: ticker.clone(), position: position.clone(), total_value }
                        Peers { ticker: ticker.clone() }
                        NotesCard { ticker: ticker.clone() }
                    }
                },
                (Tab::News, _) => rsx! { NewsTab { ticker: ticker.clone() } },
                (Tab::Ownership, _) => rsx! { OwnershipTab { ticker: ticker.clone() } },
                (Tab::Options, _) => rsx! { OptionsTab { ticker: ticker.clone() } },
                (Tab::Compare, _) => rsx! {
                    nav { class: "mb-5 -mt-1",
                        Segmented {
                            ToggleButton { label: tr("Other stocks"), active: !compare_over_time(), onclick: move |_| compare_over_time.set(false) }
                            ToggleButton { label: tr("Measure vs measure"), active: compare_over_time(), onclick: move |_| compare_over_time.set(true) }
                        }
                    }
                    if !compare_over_time() {
                        CompareMeasures {
                            ticker: ticker.clone(),
                            peers: positions.iter().map(|p| p.ticker.clone()).collect::<Vec<_>>(),
                        }
                    } else if let Some(f) = &f {
                        MeasureVsMeasure { fundamentals: f.clone() }
                    } else if loading {
                        Unavailable { text: "Loading fundamentals…" }
                    } else {
                        Unavailable { text: "Fundamentals aren't available for {ticker} right now." }
                    }
                },
                (_, None) if loading => rsx! { Unavailable { text: "Loading fundamentals…" } },
                (_, None) => rsx! {
                    Unavailable { text: "Fundamentals aren't available for {ticker} right now. Yahoo may be rate-limiting; try again shortly." }
                },
                (Tab::Statistics, Some(f)) => rsx! { Statistics { fundamentals: f.clone() } },
                (Tab::Financials, Some(f)) => rsx! {
                    div { class: "grid gap-5",
                        Financials { fundamentals: f.clone() }
                        DividendsAndSplits { ticker: ticker.clone() }
                    }
                },
                (Tab::Analysis, Some(f)) => rsx! {
                    div { class: "grid gap-5",
                        AnalysisTab { fundamentals: f.clone(), price: price.and_then(|p| p.to_f64()) }
                        RatingChanges { ticker: ticker.clone() }
                    }
                },
            }
        }
    }
}

/// ☆ Watch menu: tick the lists this stock is on.
#[component]
fn WatchButton(ticker: TickerSymbol) -> Element {
    let refresh = use_context::<DataRefresh>();
    let mut open = use_signal(|| false);
    let lists = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_watchlists().await.unwrap_or_default()
    });
    let lists = lists.read().clone().unwrap_or_default();
    let on_lists = lists.iter().filter(|l| l.items.iter().any(|i| i.ticker == ticker)).count();
    let label = match on_lists {
        0 => tr("☆ Watch").to_string(),
        1 => tr("★ Watching").to_string(),
        n => format!("★ On {n} lists"),
    };
    rsx! {
        div { class: "relative",
            GhostButton { label, onclick: move |_| open.toggle() }
            if open() {
                div { class: "fixed inset-0 z-20", onclick: move |_| open.set(false) }
                div { class: "absolute right-0 top-full z-30 mt-2 w-56 rounded-2xl border border-ctp-surface0 bg-ctp-mantle p-1.5 shadow-2xl shadow-ctp-crust/60",
                    div { class: "px-2.5 pt-1.5 pb-1 text-xs text-ctp-overlay1", {tr("Watchlists")} }
                    for l in lists {
                        {
                            let has = l.items.iter().any(|i| i.ticker == ticker);
                            let t = ticker.clone();
                            rsx! {
                                MenuItem {
                                    key: "{l.id}",
                                    label: l.name.clone(),
                                    selected: has,
                                    taken: false,
                                    onclick: move |_| {
                                        let t = t.clone();
                                        spawn(async move {
                                            let done = if has { api::unwatch_from(l.id, t).await } else { api::watch_in(l.id, t).await };
                                            if done.is_ok() {
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
        }
    }
}

/// Your notes and tags on this stock.
#[component]
fn NotesCard(ticker: TickerSymbol) -> Element {
    let refresh = use_context::<DataRefresh>();
    let t = ticker.clone();
    let saved = use_resource(move || {
        let t = t.clone();
        async move {
            let _reload = refresh.0();
            api::get_notes()
                .await
                .ok()
                .and_then(|n| n.into_iter().find(|n| n.ticker == t.as_str()))
        }
    });
    let mut text = use_signal(String::new);
    let mut tags = use_signal(String::new);
    let mut loaded = use_signal(|| false);
    let mut status = use_signal(|| None::<Result<(), String>>);
    // Fill the fields once the saved note arrives.
    use_effect(move || {
        if let Some(note) = &*saved.read() {
            if !loaded() {
                let note = note.clone().unwrap_or_default();
                text.set(note.text);
                tags.set(note.tags.join(", "));
                loaded.set(true);
            }
        }
    });
    let updated = saved.read().clone().flatten().map(|n| n.updated_at).filter(|u| !u.is_empty());
    let save = move |_| {
        let t = ticker.clone();
        async move {
            let tag_list: Vec<String> = tags().split(',').map(str::to_string).collect();
            match api::save_note(t, text(), tag_list).await {
                Ok(()) => {
                    status.set(Some(Ok(())));
                    loaded.set(false);
                    refresh.reload();
                }
                Err(e) => status.set(Some(Err(e.to_string()))),
            }
        }
    };
    rsx! {
        Card {
            title: tr("Your notes"),
            subtitle: updated.map_or(tr("Why you own it, what to watch for").to_string(), |u| crate::i18n::trf("Last saved {}", &[&u])),
            div { class: "grid gap-3",
                textarea {
                    class: "{INPUT} min-h-28 resize-y",
                    placeholder: tr("e.g. Buy more below $150. Watch margins in the next report."),
                    value: "{text}",
                    oninput: move |e| { text.set(e.value()); status.set(None); },
                }
                div { class: "flex flex-wrap items-center gap-3",
                    input {
                        class: "{INPUT} flex-1",
                        placeholder: tr("Tags, comma separated: dividend, long term"),
                        value: "{tags}",
                        oninput: move |e| { tags.set(e.value()); status.set(None); },
                    }
                    ActionButton { label: tr("Save"), onclick: save }
                    match status() {
                        Some(Ok(())) => rsx! { span { class: "text-sm text-ctp-green", {tr("Saved")} } },
                        Some(Err(e)) => rsx! { span { class: "text-sm text-ctp-red", "{e}" } },
                        None => rsx! {},
                    }
                }
            }
        }
    }
}

#[component]
fn Unavailable(text: String) -> Element {
    rsx! {
        Card { title: tr("Fundamentals"), p { class: "py-12 text-center text-sm text-ctp-subtext0", "{text}" } }
    }
}

// ─── Summary ──────────────────────────────────────────────────────────────────

/// The largest companies in the same sector of the first index listing
/// this stock; nothing if no index does.
#[component]
fn Peers(ticker: TickerSymbol) -> Element {
    let (t, key) = (ticker.clone(), ticker.clone());
    let group = crate::cache::use_cached(move || format!("peers/{key}"), move || {
        let t = t.clone();
        async move { api::get_peers(t).await.ok().flatten() }
    });
    let Some(Some(group)) = group.read().clone() else {
        return rsx! {};
    };
    rsx! {
        Card {
            title: tr("Peers"),
            subtitle: format!("Largest {} companies in the {}", group.sector, tr(group.index.label())),
            flush: true,
            crate::stock_table::StockTable { rows: group.rows.clone(), highlight: Some(ticker.to_string()) }
            div { class: "h-3" }
        }
    }
}

#[component]
fn About(fundamentals: StockFundamentals) -> Element {
    let mut expanded = use_signal(|| false);
    let Some(summary) = fundamentals.profile.summary.clone() else {
        return rsx! {};
    };
    rsx! {
        Card { title: tr("About"),
            p {
                class: if expanded() { "text-sm leading-relaxed text-ctp-subtext0" } else { "text-sm leading-relaxed text-ctp-subtext0 line-clamp-3" },
                "{summary}"
            }
            div { class: "mt-2 flex items-center gap-4 text-xs",
                button { class: "text-ctp-mauve cursor-pointer hover:underline", onclick: move |_| expanded.toggle(),
                    if expanded() { {tr("Show less")} } else { {tr("Read more")} }
                }
                if let Some(site) = &fundamentals.profile.website {
                    span { class: "text-ctp-subtext0", "{site}" }
                }
            }
        }
    }
}

// ─── Statistics ───────────────────────────────────────────────────────────────

type Getter = fn(&ValuationPoint) -> Option<f64>;

#[component]
fn Statistics(fundamentals: StockFundamentals) -> Element {
    let f = &fundamentals;
    let current = current_valuation(&f.stats, &f.quarterly);
    let history: Vec<ValuationPoint> = f.valuation.iter().take(5).cloned().collect();
    // Columns: Current, then quarter ends newest first.
    let columns: Vec<ValuationPoint> = std::iter::once(current).chain(history).collect();

    let rows: [(&str, Getter, Unit); 6] = [
        ("Market cap", |v| v.market_cap, Unit::Money),
        ("Enterprise value", |v| v.enterprise_value, Unit::Money),
        ("Trailing P/E", |v| v.pe_ttm, Unit::Number),
        ("Price / sales", |v| v.price_to_sales, Unit::Number),
        ("Price / book", |v| v.price_to_book, Unit::Number),
        ("EV / revenue", |v| v.ev_to_revenue, Unit::Number),
    ];

    rsx! {
        div { class: "grid gap-5 motion-safe:animate-rise",
            Card {
                title: tr("Valuation measures"),
                subtitle: tr("Current, and at each quarter end from reported statements").to_string(),
                flush: true,
                div { class: "overflow-x-auto",
                    table { class: "w-full text-sm whitespace-nowrap",
                        thead {
                            tr { class: "text-xs text-ctp-subtext0",
                                th { class: "pl-6 pr-4 py-2.5 text-left font-medium", "" }
                                for c in columns.iter() {
                                    th { class: "px-4 py-2.5 text-right font-medium", "{short_date(&c.date)}" }
                                }
                                th { class: "pl-4 pr-6 py-2.5 text-right font-medium", {tr("Trend")} }
                            }
                        }
                        tbody {
                            for (label, get, unit) in rows {
                                tr { class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
                                    td { class: "pl-6 pr-4 py-3 text-ctp-subtext1", "{label}" }
                                    for (i, c) in columns.iter().enumerate() {
                                        td {
                                            class: if i == 0 { "px-4 py-3 text-right tabular-nums font-semibold text-ctp-text" } else { "px-4 py-3 text-right tabular-nums text-ctp-subtext0" },
                                            {or_dash(get(c), |v| unit.format(v))}
                                        }
                                    }
                                    td { class: "pl-4 pr-6 py-2 text-right",
                                        Sparkline { values: columns.iter().rev().map(get).collect::<Vec<_>>() }
                                    }
                                }
                            }
                            tr { class: "border-t border-ctp-surface0/60",
                                td { class: "pl-6 pr-4 py-3 text-ctp-subtext1", {tr("Forward P/E")} }
                                td { class: "px-4 py-3 text-right tabular-nums font-semibold text-ctp-text",
                                    {or_dash(f.stats.forward_pe, |v| format!("{v:.2}"))}
                                }
                                td { class: "px-4 py-3 text-xs text-ctp-overlay1", colspan: "{columns.len()}",
                                    {tr("Price ÷ analysts' next-year EPS")}
                                }
                            }
                        }
                    }
                }
            }
            Highlights { fundamentals: f.clone() }
        }
    }
}

#[component]
fn Highlights(fundamentals: StockFundamentals) -> Element {
    let f = &fundamentals;
    let q = &f.quarterly;
    let latest = q.first().cloned().unwrap_or_default();
    let pct = |v: f64| format!("{:.2}%", v * 100.0);
    let revenue = ttm(q, |p| p.revenue);
    let net = ttm(q, |p| p.net_income);
    let div = |a: Option<f64>, b: Option<f64>| Some(a? / b.filter(|b| *b != 0.0)?);

    let groups: Vec<(&str, Vec<(&str, String)>)> = vec![
        (
            "Profitability (TTM)",
            vec![
                (
                    "Gross margin",
                    or_dash(div(ttm(q, |p| p.gross_profit), revenue), pct),
                ),
                (
                    "Operating margin",
                    or_dash(div(ttm(q, |p| p.operating_income), revenue), pct),
                ),
                ("Net margin", or_dash(div(net, revenue), pct)),
                (
                    "Return on equity",
                    or_dash(div(net, latest.total_equity), pct),
                ),
                (
                    "Return on assets",
                    or_dash(div(net, latest.total_assets), pct),
                ),
            ],
        ),
        (
            "Income statement (TTM)",
            vec![
                ("Revenue", or_dash(revenue, fmt_compact)),
                (
                    "Revenue growth (YoY quarter)",
                    or_dash(yoy_growth(q, |p| p.revenue), pct),
                ),
                ("Net income", or_dash(net, fmt_compact)),
                (
                    "Earnings growth (YoY quarter)",
                    or_dash(yoy_growth(q, |p| p.net_income), pct),
                ),
                (
                    "EPS (TTM)",
                    or_dash(f.stats.eps_ttm, |v| format!("${v:.2}")),
                ),
                (
                    "EPS (next year, est.)",
                    or_dash(f.stats.forward_eps, |v| format!("${v:.2}")),
                ),
            ],
        ),
        (
            "Balance sheet (latest quarter)",
            vec![
                ("Cash", or_dash(latest.cash, fmt_compact)),
                (
                    "Long-term debt",
                    or_dash(latest.long_term_debt, fmt_compact),
                ),
                (
                    "Debt / equity",
                    or_dash(latest.debt_to_equity(), |v| format!("{v:.2}")),
                ),
                (
                    "Current ratio",
                    or_dash(latest.current_ratio(), |v| format!("{v:.2}")),
                ),
                (
                    "Book value / share",
                    or_dash(div(latest.total_equity, latest.shares), |v| {
                        format!("${v:.2}")
                    }),
                ),
            ],
        ),
        (
            "Cash flow (TTM)",
            vec![
                (
                    "Operating cash flow",
                    or_dash(ttm(q, |p| p.operating_cashflow), fmt_compact),
                ),
                (
                    "Free cash flow",
                    or_dash(ttm(q, |p| p.free_cash_flow), fmt_compact),
                ),
                (
                    "Free cash flow margin",
                    or_dash(div(ttm(q, |p| p.free_cash_flow), revenue), pct),
                ),
            ],
        ),
        (
            "Trading information",
            vec![
                (
                    "52-week high",
                    or_dash(f.stats.high_52w, |v| format!("${v:.2}")),
                ),
                (
                    "52-week low",
                    or_dash(f.stats.low_52w, |v| format!("${v:.2}")),
                ),
                (
                    "50-day average",
                    or_dash(f.stats.sma50, |v| format!("${v:.2}")),
                ),
                (
                    "200-day average",
                    or_dash(f.stats.sma200, |v| format!("${v:.2}")),
                ),
                (
                    "Shares outstanding",
                    or_dash(f.stats.shares_outstanding, |v| {
                        fmt_compact(v).replacen('$', "", 1)
                    }),
                ),
            ],
        ),
        (
            "Dividends & dates",
            vec![
                ("Dividend yield", or_dash(f.stats.dividend_yield, pct)),
                (
                    "Ex-dividend date",
                    f.stats
                        .ex_dividend_date
                        .clone()
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    "Next earnings",
                    f.stats.next_earnings.clone().unwrap_or_else(|| "—".into()),
                ),
            ],
        ),
    ];

    rsx! {
        div { class: "grid gap-5 md:grid-cols-2",
            for (title, rows) in groups {
                Card { key: "{title}", title: crate::i18n::tr_str(title),
                    div { class: "flex flex-col",
                        for (label, value) in rows {
                            div { class: "flex items-center justify-between gap-4 border-t border-dashed border-ctp-surface0 py-2 text-sm first:border-t-0",
                                span { class: "text-ctp-subtext0", {crate::i18n::tr_str(label)} }
                                span { class: "font-medium tabular-nums text-ctp-text", "{value}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Financials ───────────────────────────────────────────────────────────────

#[component]
fn Financials(fundamentals: StockFundamentals) -> Element {
    let mut quarterly = use_signal(|| true);
    let rows: Vec<PeriodFinancials> = if quarterly() {
        fundamentals.quarterly.iter().take(8).cloned().collect()
    } else {
        fundamentals.annual.iter().take(5).cloned().collect()
    };
    // Charts read oldest → newest.
    let oldest_first: Vec<PeriodFinancials> = rows.iter().rev().cloned().collect();
    let labels: Vec<String> = oldest_first
        .iter()
        .map(|p| period_label(p, quarterly()))
        .collect();
    let series =
        |name: &str, color: &'static str, get: fn(&PeriodFinancials) -> Option<f64>| BarSeries {
            name: name.into(),
            color,
            values: oldest_first.iter().map(get).collect(),
        };

    let toggle = rsx! {
        Segmented {
            ToggleButton { label: tr("Quarterly"), active: quarterly(), onclick: move |_| quarterly.set(true) }
            ToggleButton { label: tr("Annual"), active: !quarterly(), onclick: move |_| quarterly.set(false) }
        }
    };

    type Row = (&'static str, fn(&PeriodFinancials) -> Option<f64>, Unit);
    let table: [Row; 11] = [
        ("Revenue", |p| p.revenue, Unit::Money),
        ("Gross profit", |p| p.gross_profit, Unit::Money),
        ("Operating income", |p| p.operating_income, Unit::Money),
        ("Net income", |p| p.net_income, Unit::Money),
        ("Gross margin", |p| p.gross_margin(), Unit::Percent),
        ("Net margin", |p| p.net_margin(), Unit::Percent),
        ("Operating cash flow", |p| p.operating_cashflow, Unit::Money),
        ("Free cash flow", |p| p.free_cash_flow, Unit::Money),
        ("Cash", |p| p.cash, Unit::Money),
        ("Long-term debt", |p| p.long_term_debt, Unit::Money),
        ("Shareholders' equity", |p| p.total_equity, Unit::Money),
    ];

    rsx! {
        div { class: "grid gap-5 motion-safe:animate-rise",
            div { class: "grid gap-5 md:grid-cols-2",
                Card { title: tr("Revenue & net income"), actions: toggle,
                    BarChart {
                        labels: labels.clone(),
                        series: vec![series("Revenue", "var(--catppuccin-color-blue)", |p| p.revenue), series("Net income", "var(--catppuccin-color-mauve)", |p| p.net_income)],
                        unit: Unit::Money,
                    }
                }
                Card { title: tr("Margins"),
                    BarChart {
                        labels: labels.clone(),
                        series: vec![
                            series("Gross", "var(--catppuccin-color-green)", |p| p.gross_margin()),
                            series("Operating", "var(--catppuccin-color-yellow)", |p| p.operating_margin()),
                            series("Net", "var(--catppuccin-color-mauve)", |p| p.net_margin()),
                        ],
                        unit: Unit::Percent,
                    }
                }
                Card { title: tr("Cash flow"),
                    BarChart {
                        labels: labels.clone(),
                        series: vec![
                            series("Operating cash flow", "var(--catppuccin-color-sky)", |p| p.operating_cashflow),
                            series("Free cash flow", "var(--catppuccin-color-teal)", |p| p.free_cash_flow),
                        ],
                        unit: Unit::Money,
                    }
                }
                Card { title: tr("Balance sheet"),
                    BarChart {
                        labels: labels.clone(),
                        series: vec![
                            series("Cash", "var(--catppuccin-color-green)", |p| p.cash),
                            series("Long-term debt", "var(--catppuccin-color-red)", |p| p.long_term_debt),
                            series("Equity", "var(--catppuccin-color-lavender)", |p| p.total_equity),
                        ],
                        unit: Unit::Money,
                    }
                }
            }
            Card { title: tr("Statements"), subtitle: tr("Newest first").to_string(), flush: true,
                div { class: "overflow-x-auto",
                    table { class: "w-full text-sm whitespace-nowrap",
                        thead {
                            tr { class: "text-xs text-ctp-subtext0",
                                th { class: "pl-6 pr-4 py-2.5 text-left font-medium", "" }
                                for p in rows.iter() {
                                    th { class: "px-4 py-2.5 text-right font-medium", "{period_label(p, quarterly())}" }
                                }
                            }
                        }
                        tbody {
                            for (label, get, unit) in table {
                                tr { class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
                                    td { class: "pl-6 pr-4 py-3 text-ctp-subtext1", "{label}" }
                                    for p in rows.iter() {
                                        td { class: "px-4 py-3 text-right tabular-nums text-ctp-subtext0",
                                            {or_dash(get(p), |v| unit.format(v))}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Analysis ─────────────────────────────────────────────────────────────────

#[component]
fn AnalysisTab(fundamentals: StockFundamentals, price: Option<f64>) -> Element {
    rsx! {
        div { class: "grid gap-5 motion-safe:animate-rise",
            div { class: "grid gap-5 md:grid-cols-2",
                if let Some(a) = fundamentals.analysts.clone() {
                    Ratings { analysts: a.clone() }
                    PriceTarget { analysts: a, price }
                } else {
                    Card { title: tr("Analysts"), p { class: "py-8 text-center text-sm text-ctp-subtext0", {tr("No analyst coverage.")} } }
                }
            }
            EpsChart { surprises: fundamentals.eps_surprises.clone() }
        }
    }
}

const RATING_COLORS: [(&str, &str); 5] = [
    ("Strong buy", "var(--catppuccin-color-green)"),
    ("Buy", "var(--catppuccin-color-teal)"),
    ("Hold", "var(--catppuccin-color-yellow)"),
    ("Sell", "var(--catppuccin-color-peach)"),
    ("Strong sell", "var(--catppuccin-color-red)"),
];

#[component]
fn Ratings(analysts: Analysts) -> Element {
    let a = &analysts;
    let counts = [a.strong_buy, a.buy, a.hold, a.sell, a.strong_sell];
    let total: u32 = counts.iter().sum();
    let rating = a.rating.clone().unwrap_or_default();
    // Mean runs 1 (strong buy) … 5 (strong sell).
    let marker = a.mean.map(|m| ((m - 1.0) / 4.0 * 100.0).clamp(0.0, 100.0));

    rsx! {
        Card { title: tr("Analyst ratings"), subtitle: format!("{total} analysts"),
            div { class: "flex items-baseline gap-3",
                span { class: "text-3xl font-semibold capitalize text-ctp-text", "{rating}" }
                if let Some(m) = a.mean {
                    span { class: "text-sm text-ctp-subtext0", "{m:.1} on a 1–5 scale" }
                }
            }
            if let Some(pos) = marker {
                div { class: "relative mt-4 h-2 rounded-full bg-gradient-to-r from-ctp-green via-ctp-yellow to-ctp-red",
                    span { class: "absolute top-1/2 h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ctp-base bg-ctp-text", style: "left:{pos:.1}%;" }
                }
                div { class: "mt-1.5 flex justify-between text-xs text-ctp-overlay1",
                    span { {tr("Strong buy")} }
                    span { {tr("Hold")} }
                    span { {tr("Strong sell")} }
                }
            }
            if total > 0 {
                div { class: "mt-5 flex h-3 gap-0.5 overflow-hidden rounded-full",
                    for (i, n) in counts.iter().enumerate() {
                        if *n > 0 {
                            div {
                                class: "h-full first:rounded-l-full last:rounded-r-full",
                                style: "width:{*n as f64 / total as f64 * 100.0:.1}%;background:{RATING_COLORS[i].1};",
                                title: "{RATING_COLORS[i].0}: {n}",
                            }
                        }
                    }
                }
                div { class: "mt-3 grid grid-cols-5 gap-1 text-center text-xs",
                    for (i, n) in counts.iter().enumerate() {
                        div {
                            div { class: "font-semibold tabular-nums text-ctp-text", "{n}" }
                            div { class: "text-ctp-subtext0", style: "color:{RATING_COLORS[i].1};", "{RATING_COLORS[i].0}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PriceTarget(analysts: Analysts, price: Option<f64>) -> Element {
    let a = &analysts;
    let (Some(low), Some(high)) = (a.target_low, a.target_high) else {
        return rsx! {
            Card { title: tr("Price target"), p { class: "py-8 text-center text-sm text-ctp-subtext0", {tr("No price targets.")} } }
        };
    };
    let lo = low.min(price.unwrap_or(low));
    let hi = high.max(price.unwrap_or(high));
    let span = (hi - lo).max(f64::EPSILON);
    let at = |v: f64| (v - lo) / span * 100.0;
    let upside = a.target_mean.zip(price).map(|(m, p)| (m / p - 1.0) * 100.0);
    let upside_tone = if upside.unwrap_or(0.0) >= 0.0 {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    };

    rsx! {
        Card { title: tr("Price target"), subtitle: a.count.map(|n| format!("{n} analysts")).unwrap_or_default(),
            div { class: "grid grid-cols-2 gap-3",
                MetricTile { label: tr("Average target"), value: or_dash(a.target_mean, |v| format!("${v:.2}")) }
                MetricTile {
                    label: tr("Upside"),
                    value: or_dash(upside, |v| format!("{v:+.1}%")),
                    tone: upside_tone,
                }
            }
            div { class: "relative mt-8 mb-6 h-1.5 rounded-full bg-ctp-surface1",
                span {
                    class: "absolute top-0 h-full rounded-full bg-ctp-mauve/50",
                    style: "left:{at(low):.1}%;width:{at(high) - at(low):.1}%;",
                }
                if let Some(m) = a.target_mean {
                    span { class: "absolute -top-6 -translate-x-1/2 text-xs font-semibold text-ctp-mauve", style: "left:{at(m):.1}%;", "Avg ${m:.0}" }
                    span { class: "absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-ctp-mauve", style: "left:{at(m):.1}%;" }
                }
                if let Some(p) = price {
                    span { class: "absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ctp-base bg-ctp-text", style: "left:{at(p):.1}%;" }
                    span { class: "absolute top-4 -translate-x-1/2 text-xs text-ctp-subtext0", style: "left:{at(p):.1}%;", "Now ${p:.0}" }
                }
            }
            div { class: "flex justify-between text-xs tabular-nums text-ctp-subtext0",
                span { "Low ${low:.0}" }
                span { "High ${high:.0}" }
            }
        }
    }
}

/// Reported vs estimated EPS per quarter; green beat, red miss.
#[component]
fn EpsChart(surprises: Vec<EpsSurprise>) -> Element {
    const W: f64 = 600.0;
    const H: f64 = 200.0;
    let points: Vec<&EpsSurprise> = surprises
        .iter()
        .filter(|s| s.actual.is_some() || s.estimate.is_some())
        .collect();
    if points.is_empty() {
        return rsx! {
            Card { title: tr("Earnings per share"), p { class: "py-8 text-center text-sm text-ctp-subtext0", {tr("No earnings history.")} } }
        };
    }
    let values: Vec<f64> = points
        .iter()
        .flat_map(|s| [s.actual, s.estimate])
        .flatten()
        .collect();
    let lo = values.iter().cloned().fold(f64::MAX, f64::min);
    let hi = values.iter().cloned().fold(f64::MIN, f64::max);
    let pad = ((hi - lo) * 0.25).max(0.05);
    let (lo, hi) = (lo - pad, hi + pad);
    let x = |i: usize| 50.0 + (W - 100.0) * (i as f64 + 0.5) / points.len() as f64;
    let y = |v: f64| 20.0 + (hi - v) / (hi - lo) * (H - 60.0);

    rsx! {
        Card { title: tr("Earnings per share"), subtitle: tr("Reported vs analyst estimate").to_string(),
            div { class: "mb-2 flex gap-4 text-xs text-ctp-subtext0",
                span { class: "flex items-center gap-1.5", span { class: "h-2.5 w-2.5 rounded-full border-2 border-ctp-overlay1" } "Estimate" }
                span { class: "flex items-center gap-1.5", span { class: "h-2.5 w-2.5 rounded-full bg-ctp-green" } "Beat" }
                span { class: "flex items-center gap-1.5", span { class: "h-2.5 w-2.5 rounded-full bg-ctp-red" } "Missed" }
            }
            svg { class: "w-full", view_box: "0 0 {W} {H}",
                for (i, s) in points.iter().enumerate() {
                    {
                        let beat = s.actual.zip(s.estimate).map(|(a, e)| a >= e);
                        let color = match beat { Some(false) => "var(--catppuccin-color-red)", _ => "var(--catppuccin-color-green)" };
                        let surprise = s.actual.zip(s.estimate).filter(|(_, e)| *e != 0.0).map(|(a, e)| (a / e.abs() - e.signum()) * 100.0);
                        rsx! {
                            g { key: "{s.period}",
                                if let Some(e) = s.estimate {
                                    circle { cx: "{x(i):.1}", cy: "{y(e):.1}", r: "9", fill: "none", stroke: "var(--catppuccin-color-overlay1)", stroke_width: "2", stroke_dasharray: "3 2" }
                                }
                                if let Some(a) = s.actual {
                                    circle { cx: "{x(i):.1}", cy: "{y(a):.1}", r: "7", fill: color }
                                    text { x: "{x(i):.1}", y: "{y(a) - 14.0:.1}", text_anchor: "middle", font_size: "11", font_weight: "600", fill: "var(--catppuccin-color-text)", "${a:.2}" }
                                }
                                if let Some(pct) = surprise {
                                    text { x: "{x(i):.1}", y: "{H - 26.0}", text_anchor: "middle", font_size: "11", font_weight: "600", fill: color, "{pct:+.1}%" }
                                }
                                text { x: "{x(i):.1}", y: "{H - 8.0}", text_anchor: "middle", font_size: "11", fill: "var(--catppuccin-color-overlay1)", "{s.period}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// `2026-07-31` → `7/31/2026`; other labels unchanged.
fn short_date(date: &str) -> String {
    let parts: Vec<&str> = date.split('-').collect();
    match parts.as_slice() {
        [y, m, d] => format!(
            "{}/{}/{y}",
            m.trim_start_matches('0'),
            d.trim_start_matches('0')
        ),
        _ => date.to_string(),
    }
}

/// `Q2 '26` for quarters, `2025` for years.
fn period_label(p: &PeriodFinancials, quarterly: bool) -> String {
    let Some(date) = &p.end_date else {
        return p.period.clone();
    };
    let (year, month) = (&date[..4], date[5..7].parse::<u32>().unwrap_or(12));
    if quarterly {
        format!("Q{} '{}", month.div_ceil(3), &year[2..])
    } else {
        year.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(short_date("2026-07-31"), "7/31/2026");
        assert_eq!(short_date("Current"), "Current");
        let p = PeriodFinancials {
            period: "x".into(),
            end_date: Some("2026-06-30".into()),
            ..Default::default()
        };
        assert_eq!(period_label(&p, true), "Q2 '26");
        assert_eq!(period_label(&p, false), "2026");
    }
}
