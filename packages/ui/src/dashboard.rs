use crate::i18n::{tr, trf};
use crate::{
    income_tab::{IncomeTab, ReturnDrivers},
    app::PortfolioScope,
    components::{
        analysis::{stock::open_stock, RiskTab},
        card::{Card, Chevron, MenuItem, Segmented, ToggleButton},
        charts::{AllocationCard, ChartSection, StockHeatmap},
        color_schema::CHART_COLOR_CLASSES,
    },
    editors::{DeleteTransactionButton, Dialog, Dialogs},
    files::{print_report, ExportButtons},
    format::{fmt_signed, fmt_usd, signed_color},
    hooks::{scoped_data, use_portfolio, PortfolioState},
    page::{GhostButton, HeroStat, Page, PageHero},
    plan_tab::PlanTab,
    thesis::ThesisTab,
};
use dioxus::prelude::*;
use dtos::csv_export::{holdings_csv, transactions_csv};
use dtos::{portfolio::GetDashBoardResponse, transaction::Transaction, Position};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

/// A single holding above this share of the portfolio is flagged.
const CONCENTRATION_WARN_PCT: Decimal = dec!(25);

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Tab {
    Overview,
    Thesis,
    Income,
    Risk,
    Plan,
    Activity,
}

thread_local! {
    /// Tab to show the next time the page opens; see [`open_tab`].
    static OPEN_TAB: std::cell::Cell<Option<Tab>> = const { std::cell::Cell::new(None) };
}

/// Opens the portfolio page on `tab`, for links from other pages.
pub(crate) fn open_tab(tab: Tab) {
    OPEN_TAB.with(|t| t.set(Some(tab)));
    navigator().push("/portfolio");
}

// ─── Page ─────────────────────────────────────────────────────────────────────

#[component]
pub fn Dashboard() -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    // Portfolio id, or None for all holdings; shared so other pages can
    // open a specific portfolio.
    let PortfolioScope(scope) = use_context::<PortfolioScope>();
    let PortfolioState {
        loaded,
        positions,
        realized,
        total_value,
        total_cost,
        total_pnl,
        day_change,
        pnl_pct,
        day_pct,
        allocation,
        mpt,
        cash,
        ticker_price_map,
        ..
    } = use_portfolio(scope());
    let mut tab = use_signal(|| OPEN_TAB.with(|t| t.take()).unwrap_or(Tab::Overview));
    let dialogs = use_context::<Dialogs>();
    let scope_id = scope().and_then(|id| Uuid::parse_str(&id).ok());

    // Price-independent: worked out when the data or scope changes, not on
    // every price update.
    let transactions = use_memo(move || scoped_data(&data.read(), scope().as_deref()).transactions);
    let portfolios = use_memo(move || {
        data.read()
            .portfolios
            .iter()
            .map(|p| (p.id.to_string(), p.name.clone(), p.ai))
            .collect::<Vec<(String, String, bool)>>()
    });
    let (transactions, portfolios) = (transactions(), portfolios());
    let current = scope().and_then(|id| portfolios.iter().find(|(pid, _, _)| *pid == id).cloned());
    let title = current
        .as_ref()
        .map(|(_, name, _)| name.clone())
        .unwrap_or_else(|| tr("Your portfolio").to_string());
    let badge = current
        .is_some_and(|(_, _, ai)| ai)
        .then(|| tr("AI-managed · paper money").to_string());
    let names: Vec<(String, String)> = portfolios.iter().map(|(id, name, _)| (id.clone(), name.clone())).collect();
    // The picker lists Claude's portfolio with an AI tag.
    let choices: Vec<(String, String)> = portfolios
        .iter()
        .map(|(id, name, ai)| (id.clone(), if *ai { format!("{name} · 🤖 AI") } else { name.clone() }))
        .collect();
    let empty = positions.is_empty() && transactions.is_empty();

    rsx! {
        Page {
            PageHero {
                title,
                badge,
                actions: rsx! {
                    div { class: "flex flex-wrap items-center gap-2",
                        GhostButton { label: tr("＋ Transaction"), primary: true, onclick: move |_| dialogs.open(Dialog::AddTransaction(scope_id)) }
                        GhostButton { label: tr("Print"), onclick: move |_| print_report() }
                        ScopePicker { scope, portfolios: choices.clone() }
                    }
                },
                loaded,
                total_value: total_value + cash.unwrap_or_default(),
                day_change,
                day_pct,
                HeroStat {
                    label: tr("Return"),
                    value: format!("{} ({pnl_pct:+.2}%)", fmt_signed(total_pnl, 2)),
                    color: signed_color(total_pnl),
                }
                HeroStat { label: tr("Invested"), value: fmt_usd(total_cost, 2) }
                HeroStat {
                    label: tr("Realized"),
                    value: fmt_signed(realized, 2),
                    color: signed_color(realized),
                }
                if let Some(cash) = cash {
                    HeroStat { label: tr("Cash"), value: fmt_usd(cash, 2) }
                }
            }

            if empty {
                EmptyState {}
            } else {
                Insights { positions: positions.clone(), total_value }

                div { class: "mt-10 motion-safe:animate-rise",
                    ChartSection {
                        transactions: data.read().transactions.clone(),
                        portfolios: names.clone(),
                        portfolio: scope(),
                        height: dec!(260),
                    }
                }

                nav { class: "flex items-center justify-between gap-4 mt-10 mb-5 overflow-x-auto",
                    Segmented {
                        ToggleButton {
                            label: tr("Overview"),
                            active: tab() == Tab::Overview,
                            onclick: move |_| tab.set(Tab::Overview),
                        }
                        ToggleButton {
                            label: tr("Thesis"),
                            active: tab() == Tab::Thesis,
                            onclick: move |_| tab.set(Tab::Thesis),
                        }
                        ToggleButton {
                            label: tr("Income"),
                            active: tab() == Tab::Income,
                            onclick: move |_| tab.set(Tab::Income),
                        }
                        ToggleButton {
                            label: tr("Risk"),
                            active: tab() == Tab::Risk,
                            onclick: move |_| tab.set(Tab::Risk),
                        }
                        ToggleButton {
                            label: tr("Plan"),
                            active: tab() == Tab::Plan,
                            onclick: move |_| tab.set(Tab::Plan),
                        }
                        ToggleButton {
                            label: tr("Activity"),
                            active: tab() == Tab::Activity,
                            onclick: move |_| tab.set(Tab::Activity),
                        }
                    }
                    span { class: "hidden sm:block text-xs text-ctp-overlay1",
                        "{positions.len()} holdings · {transactions.len()} transactions"
                    }
                }

                match tab() {
                    Tab::Overview => rsx! {
                        TabPanel {
                            HoldingsTable {
                                positions: positions.clone(),
                                allocation: allocation.clone(),
                                total_value,
                            }
                            div { class: "grid gap-5 lg:grid-cols-[1fr_300px]",
                                StockHeatmap { positions: positions.clone() }
                                AllocationCard { allocation: allocation.clone() }
                            }
                            crate::assets_card::AssetsCard {}
                            ReturnDrivers {
                                transactions: transactions.clone(),
                                positions: positions.clone(),
                                prices: ticker_price_map.clone(),
                            }
                        }
                    },
                    Tab::Thesis => rsx! {
                        TabPanel {
                            ThesisTab { portfolio: scope_id }
                        }
                    },
                    Tab::Income => rsx! {
                        TabPanel {
                            IncomeTab { positions: positions.clone(), transactions: transactions.clone(), total_value }
                        }
                    },
                    Tab::Risk => rsx! {
                        TabPanel {
                            RiskTab {
                                allocation: allocation.clone(),
                                positions: positions.clone(),
                                total_value,
                                mpt,
                                transactions: transactions.clone(),
                                cash,
                            }
                        }
                    },
                    Tab::Plan => rsx! {
                        TabPanel {
                            PlanTab {
                                positions: positions.clone(),
                                transactions: transactions.clone(),
                                total_value: total_value + cash.unwrap_or_default(),
                                portfolio: scope_id,
                                prices: ticker_price_map.clone(),
                            }
                        }
                    },
                    Tab::Activity => rsx! {
                        TabPanel {
                            crate::report_card::ReportCard { portfolio: scope_id }
                            TransactionList { transactions, portfolio: scope_id }
                        }
                    },
                }
            }
        }
    }
}

/// Switches the whole page between all holdings and a single portfolio.
/// A searchable dropdown, so it stays usable with many portfolios.
#[component]
fn ScopePicker(mut scope: Signal<Option<String>>, portfolios: Vec<(String, String)>) -> Element {
    let mut open = use_signal(|| false);
    let mut query = use_signal(String::new);
    if portfolios.len() < 2 {
        return rsx! {};
    }

    let current = scope()
        .and_then(|id| portfolios.iter().find(|(pid, _)| *pid == id))
        .map_or("All holdings".to_string(), |(_, name)| name.clone());
    let needle = query().trim().to_lowercase();
    let matches: Vec<(String, String)> = portfolios
        .iter()
        .filter(|(_, name)| name.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    let mut choose = move |pick: Option<String>| {
        scope.set(pick);
        open.set(false);
        query.set(String::new());
    };

    rsx! {
        div { class: "relative",
            button {
                class: "inline-flex items-center gap-2 rounded-full border border-ctp-surface1 bg-ctp-crust/40 \
                        pl-3.5 pr-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:border-ctp-mauve",
                aria_expanded: open(),
                onclick: move |_| open.toggle(),
                span { class: "font-medium text-ctp-text", "{current}" }
                Chevron { open: open() }
            }
            if open() {
                div { class: "fixed inset-0 z-20", onclick: move |_| open.set(false) }
                div { class: "absolute right-0 top-full z-30 mt-2 w-72 rounded-2xl border border-ctp-surface0 \
                              bg-ctp-mantle p-1.5 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                    input {
                        class: "mb-1 w-full rounded-xl border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1.5 text-sm \
                                text-ctp-text placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                        placeholder: tr("Search {n} portfolios…").replace("{n}", &portfolios.len().to_string()),
                        autofocus: true,
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                    MenuItem {
                        label: tr("All holdings"),
                        selected: scope().is_none(),
                        taken: false,
                        onclick: move |_| choose(None),
                    }
                    div { class: "max-h-72 overflow-y-auto",
                        for (id, name) in matches.clone() {
                            MenuItem {
                                key: "{id}",
                                label: name,
                                selected: scope().as_deref() == Some(id.as_str()),
                                taken: false,
                                onclick: move |_| choose(Some(id.clone())),
                            }
                        }
                        if matches.is_empty() {
                            p { class: "px-2.5 py-2 text-sm text-ctp-overlay1", "No portfolio matches “{query}”" }
                        }
                    }
                }
            }
        }
    }
}

/// Tab content; rises in each time a tab is opened.
#[component]
pub(crate) fn TabPanel(children: Element) -> Element {
    rsx! {
        div { class: "grid min-w-0 grid-cols-[minmax(0,1fr)] gap-5 motion-safe:animate-rise", {children} }
    }
}

// ─── Insights ─────────────────────────────────────────────────────────────────

/// Quick answers: what moved most today, where the money is concentrated,
/// and how many holdings are in the green.
#[component]
fn Insights(positions: Vec<Position>, total_value: Decimal) -> Element {
    let priced: Vec<&Position> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .collect();
    if priced.is_empty() {
        return rsx! {};
    }

    let best = priced.iter().max_by_key(|p| p.daily_change_pct).copied();
    let worst = priced.iter().min_by_key(|p| p.daily_change_pct).copied();
    let largest = priced.iter().max_by_key(|p| p.market_value()).copied();
    let winners = priced
        .iter()
        .filter(|p| p.unrealized_pnl() > Decimal::ZERO)
        .count();
    let top_return = priced
        .iter()
        .max_by_key(|p| p.unrealized_pnl_pct())
        .copied();

    let largest_weight = largest
        .map(|p| p.market_value() / total_value.max(dec!(1)) * dec!(100))
        .unwrap_or_default();
    let concentrated = largest_weight > CONCENTRATION_WARN_PCT;
    let (largest_color, largest_note) = if concentrated {
        ("text-ctp-yellow", "Concentrated — consider trimming")
    } else {
        ("text-ctp-text", "Well spread")
    };

    rsx! {
        div { class: "mt-10 grid grid-cols-2 lg:grid-cols-4 gap-3 motion-safe:animate-rise",
            if let Some(p) = best {
                InsightTile {
                    label: tr("Best today"),
                    ticker: p.ticker.to_string(),
                    value: format!("{:+.2}%", p.daily_change_pct),
                    color: signed_color(p.daily_change_pct),
                }
            }
            if let Some(p) = worst {
                InsightTile {
                    label: tr("Worst today"),
                    ticker: p.ticker.to_string(),
                    value: format!("{:+.2}%", p.daily_change_pct),
                    color: signed_color(p.daily_change_pct),
                }
            }
            if let Some(p) = largest {
                InsightTile {
                    label: tr("Largest holding"),
                    ticker: p.ticker.to_string(),
                    value: format!("{largest_weight:.1}%"),
                    color: largest_color,
                    note: largest_note,
                }
            }
            InsightTile {
                label: tr("In profit"),
                ticker: format!("{winners} / {}", priced.len()),
                value: String::new(),
                note: top_return
                    .map(|p| format!("Top: {} {:+.1}%", p.ticker, p.unrealized_pnl_pct()))
                    .unwrap_or_default(),
            }
        }
    }
}

#[component]
fn InsightTile(
    label: String,
    ticker: String,
    value: String,
    #[props(default = "text-ctp-text")] color: &'static str,
    #[props(default)] note: String,
) -> Element {
    rsx! {
        div { class: "rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60 px-4 py-3.5 \
                      transition-transform hover:-translate-y-0.5",
            div { class: "text-xs text-ctp-subtext0", "{label}" }
            div { class: "mt-1.5 flex items-baseline justify-between gap-2",
                span { class: "text-lg font-semibold text-ctp-text", "{ticker}" }
                span { class: "text-sm font-semibold tabular-nums {color}", "{value}" }
            }
            if !note.is_empty() {
                div { class: "mt-1 text-xs text-ctp-overlay1 truncate", "{note}" }
            }
        }
    }
}

// ─── Holdings ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Ticker,
    Price,
    Day,
    Value,
    Return,
}

#[component]
pub(crate) fn HoldingsTable(
    positions: Vec<Position>,
    allocation: Vec<(TickerSymbol, Decimal)>,
    total_value: Decimal,
) -> Element {
    let mut sort = use_signal(|| (SortKey::Value, true));
    let (key, desc) = sort();

    let mut rows = positions.clone();
    rows.sort_by(|a, b| {
        let ord = match key {
            SortKey::Ticker => a.ticker.as_str().cmp(b.ticker.as_str()),
            SortKey::Price => a.current_price.cmp(&b.current_price),
            SortKey::Day => a.daily_change_pct.cmp(&b.daily_change_pct),
            SortKey::Value => a.market_value().cmp(&b.market_value()),
            SortKey::Return => a.unrealized_pnl_pct().cmp(&b.unrealized_pnl_pct()),
        };
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });

    // Same colours as the allocation legend.
    let color_of = |ticker: &TickerSymbol| {
        allocation
            .iter()
            .position(|(t, _)| t == ticker)
            .map(|i| CHART_COLOR_CLASSES[i % CHART_COLOR_CLASSES.len()])
            .unwrap_or("bg-ctp-overlay0")
    };
    let mut toggle = move |k: SortKey| {
        let (cur, desc) = sort();
        // Text sorts A→Z first; numbers biggest first.
        sort.set(if cur == k {
            (k, !desc)
        } else {
            (k, k != SortKey::Ticker)
        });
    };

    rsx! {
        Card {
            title: tr("Holdings"),
            subtitle: tr("Click a column to sort").to_string(),
            flush: true,
            actions: rsx! {
                ExportButtons { filename: "akhsakov-holdings.csv", csv: move || holdings_csv(&positions) }
            },
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            SortHeader {
                                label: tr("Asset"),
                                align_left: true,
                                active: key == SortKey::Ticker,
                                desc,
                                onclick: move |_| toggle(SortKey::Ticker),
                            }
                            SortHeader {
                                label: tr("Price"),
                                active: key == SortKey::Price,
                                desc,
                                onclick: move |_| toggle(SortKey::Price),
                            }
                            SortHeader {
                                label: tr("Today"),
                                active: key == SortKey::Day,
                                desc,
                                onclick: move |_| toggle(SortKey::Day),
                            }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Shares")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Avg cost")} }
                            SortHeader {
                                label: tr("Value"),
                                active: key == SortKey::Value,
                                desc,
                                onclick: move |_| toggle(SortKey::Value),
                            }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Weight")} }
                            SortHeader {
                                label: tr("Return"),
                                active: key == SortKey::Return,
                                desc,
                                onclick: move |_| toggle(SortKey::Return),
                            }
                        }
                    }
                    tbody {
                        for pos in rows {
                            HoldingRow {
                                key: "{pos.ticker}",
                                weight: pos.market_value() / total_value.max(dec!(1)) * dec!(100),
                                color: color_of(&pos.ticker),
                                pos,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SortHeader(
    label: String,
    active: bool,
    desc: bool,
    #[props(default)] align_left: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let arrow = match (active, desc) {
        (false, _) => "",
        (true, true) => " ↓",
        (true, false) => " ↑",
    };
    rsx! {
        th { class: if align_left { "pl-6 pr-4 py-2.5 text-left font-medium" } else { "px-4 py-2.5 text-right font-medium" },
            button {
                class: if active { "text-ctp-text cursor-pointer" } else { "hover:text-ctp-text cursor-pointer transition-colors" },
                onclick: move |e| onclick.call(e),
                "{label}{arrow}"
            }
        }
    }
}

#[component]
fn HoldingRow(pos: Position, weight: Decimal, color: &'static str) -> Element {
    let priced = pos.current_price > Decimal::ZERO;
    let cell = "px-4 py-3.5 text-right tabular-nums";
    let ticker = pos.ticker.clone();
    rsx! {
        tr {
            class: "group border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors cursor-pointer",
            title: "Open {pos.ticker}",
            onclick: move |_| open_stock(&ticker),
            td { class: "pl-6 pr-4 py-3.5",
                span { class: "flex items-center gap-2.5",
                    span { class: "h-2.5 w-2.5 rounded-full shrink-0 {color}" }
                    span { class: "font-semibold text-ctp-text", "{pos.ticker}" }
                    span { class: "text-ctp-overlay1 opacity-40 transition-opacity group-hover:opacity-100 focus-visible:opacity-100", "→" }
                }
            }
            if priced {
                td { class: cell, "{fmt_usd(pos.current_price, 2)}" }
                td { class: "{cell} {signed_color(pos.daily_change_pct)}",
                    "{pos.daily_change_pct:+.2}%"
                }
            } else {
                td { class: "{cell} text-ctp-overlay1", "—" }
                td { class: "{cell} text-ctp-overlay1", "—" }
            }
            td { class: "{cell} text-ctp-subtext0", {crate::format::fmt_shares(pos.shares)} }
            td { class: "{cell} text-ctp-subtext0", "{fmt_usd(pos.avg_cost, 2)}" }
            if priced {
                td { class: "{cell} font-medium text-ctp-text", "{fmt_usd(pos.market_value(), 2)}" }
                td { class: cell,
                    span { class: "inline-flex items-center justify-end gap-2",
                        span { class: "h-1 w-14 rounded-full bg-ctp-surface0 overflow-hidden",
                            span {
                                class: "block h-full rounded-full {color}",
                                style: "width:{weight.min(dec!(100)):.0}%;",
                            }
                        }
                        span { class: "w-11 text-ctp-subtext0", "{weight:.1}%" }
                    }
                }
                td { class: "{cell} pr-6 {signed_color(pos.unrealized_pnl())}",
                    div { class: "font-medium", "{fmt_signed(pos.unrealized_pnl(), 2)}" }
                    div { class: "text-xs opacity-75", "{pos.unrealized_pnl_pct():+.2}%" }
                }
            } else {
                td { class: "{cell} text-ctp-overlay1", "—" }
                td { class: "{cell} text-ctp-overlay1", "—" }
                td { class: "{cell} pr-6 text-ctp-overlay1", "—" }
            }
        }
    }
}

// ─── Activity ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum TxFilter {
    All,
    Buy,
    Sell,
}

/// Rows shown at first and added by "Show more", so a long history opens
/// and scrolls quickly.
const TX_PAGE: usize = 50;

#[component]
fn TransactionList(transactions: Vec<Transaction>, portfolio: Option<Uuid>) -> Element {
    let mut filter = use_signal(|| TxFilter::All);
    let mut query = use_signal(String::new);
    let mut limit = use_signal(|| TX_PAGE);
    let dialogs = use_context::<Dialogs>();
    let data = use_context::<Signal<GetDashBoardResponse>>();

    let needle = query().trim().to_uppercase();
    let mut shown: Vec<Transaction> = transactions
        .into_iter()
        .filter(|tx| match filter() {
            TxFilter::All => true,
            TxFilter::Buy => tx.transaction_type == TransactionType::Buy,
            TxFilter::Sell => tx.transaction_type == TransactionType::Sell,
        })
        .filter(|tx| needle.is_empty() || tx.ticker.as_str().contains(&needle))
        .collect();
    shown.sort_by(|a, b| b.date.cmp(&a.date));
    let total: Decimal = shown.iter().map(|tx| tx.shares * tx.usd_price()).sum();
    let count = shown.len();
    let hidden = count.saturating_sub(limit());
    // The export has every matching row, not just the ones on screen.
    let for_csv = shown.clone();
    let csv = move || {
        transactions_csv(&for_csv, |t| {
            data.peek()
                .portfolios
                .iter()
                .find(|p| p.id == t.portfolio_id)
                .map(|p| p.name.clone())
                .unwrap_or_default()
        })
    };
    let mut set_filter = move |f: TxFilter| {
        filter.set(f);
        limit.set(TX_PAGE);
    };

    rsx! {
        Card {
            title: tr("Transactions"),
            subtitle: format!("{count} shown · {} total", fmt_usd(total, 2)),
            flush: true,
            actions: rsx! {
                div { class: "flex flex-wrap items-center gap-2",
                GhostButton { label: tr("＋ Add"), onclick: move |_| dialogs.open(Dialog::AddTransaction(portfolio)) }
                GhostButton { label: tr("Import CSV"), onclick: move |_| dialogs.open(Dialog::Import(portfolio)) }
                ExportButtons { filename: "akhsakov-transactions.csv", csv }
                input {
                    class: "w-32 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm uppercase text-ctp-text \
                            placeholder:normal-case placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                    r#type: "search",
                    placeholder: tr("Ticker…"),
                    aria_label: tr("Filter by ticker"),
                    value: "{query}",
                    oninput: move |e| {
                        query.set(e.value());
                        limit.set(TX_PAGE);
                    },
                }
                Segmented {
                    ToggleButton {
                        label: tr("All"),
                        active: filter() == TxFilter::All,
                        onclick: move |_| set_filter(TxFilter::All),
                    }
                    ToggleButton {
                        label: tr("Buy"),
                        active: filter() == TxFilter::Buy,
                        onclick: move |_| set_filter(TxFilter::Buy),
                    }
                    ToggleButton {
                        label: tr("Sell"),
                        active: filter() == TxFilter::Sell,
                        onclick: move |_| set_filter(TxFilter::Sell),
                    }
                }
                }
            },
            if shown.is_empty() {
                div { class: "px-6 pb-8 pt-2 text-sm text-ctp-overlay1", {tr("Nothing here yet.")} }
            }
            for tx in shown.into_iter().take(limit()) {
                TransactionRow { key: "{tx.id}", tx }
            }
            if hidden > 0 {
                div { class: "flex flex-wrap items-center justify-center gap-2 border-t border-ctp-surface0/60 px-6 py-3",
                    GhostButton {
                        label: trf("Show {} more", &[&hidden.min(TX_PAGE)]),
                        onclick: move |_| limit.set(limit().saturating_add(TX_PAGE)),
                    }
                    GhostButton {
                        label: trf("Show all {}", &[&count]),
                        onclick: move |_| limit.set(usize::MAX),
                    }
                }
            }
        }
    }
}

#[component]
fn TransactionRow(tx: Transaction) -> Element {
    let dialogs = use_context::<Dialogs>();
    let editing = tx.clone();
    let (badge, title, detail, amount) = describe(&tx);
    rsx! {
        div { class: "group flex items-center gap-4 px-6 py-3.5 border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
            span { class: "min-w-20 shrink-0 whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-semibold {badge}",
                "{tx.transaction_type}"
            }
            div { class: "flex-1 min-w-0",
                div { class: "font-semibold text-ctp-text", "{title}" }
                div { class: "text-xs text-ctp-subtext0 truncate", "{detail}" }
            }
            div { class: "text-right shrink-0",
                div { class: "text-sm font-medium tabular-nums text-ctp-text", "{amount}" }
                div { class: "text-xs text-ctp-overlay1", "{tx.date}" }
            }
            span { class: "flex shrink-0 gap-1 opacity-40 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 focus-within:opacity-100",
                button {
                    class: "inline-flex h-8 min-w-8 items-center justify-center rounded-full px-2 text-sm text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                    title: tr("Edit"),
                    onclick: move |_| dialogs.open(Dialog::EditTransaction(editing.clone())),
                    "✎"
                }
                DeleteTransactionButton { id: tx.id }
            }
        }
    }
}

/// Badge style, title, detail line and signed cash effect for a row. The
/// detail shows the trade's own currency; the cash effect is converted.
fn describe(tx: &Transaction) -> (&'static str, String, String, String) {
    // Amounts in the trade's currency, as the broker showed them.
    let native = |v: Decimal| {
        if tx.is_usd() {
            fmt_usd(v, 2)
        } else {
            format!("{:.2} {}", v, tx.currency)
        }
    };
    let (gross, fee_usd) = (tx.shares * tx.usd_price(), tx.usd_fee());
    let fee = if tx.fee > Decimal::ZERO {
        format!(" · fee {}", native(tx.fee))
    } else {
        String::new()
    };
    let money = |sign: &str, v: Decimal| format!("{sign}{}", fmt_usd(v, 2));
    match tx.transaction_type {
        TransactionType::Buy => (
            "bg-ctp-green/15 text-ctp-green",
            tx.ticker.to_string(),
            format!(
                "{} shares @ {}{fee}",
                crate::format::fmt_shares(tx.shares),
                native(tx.price)
            ),
            money("−", gross + fee_usd),
        ),
        TransactionType::Sell => (
            "bg-ctp-red/15 text-ctp-red",
            tx.ticker.to_string(),
            format!(
                "{} shares @ {}{fee}",
                crate::format::fmt_shares(tx.shares),
                native(tx.price)
            ),
            money("+", gross - fee_usd),
        ),
        TransactionType::Dividend => (
            "bg-ctp-teal/15 text-ctp-teal",
            tx.ticker.to_string(),
            format!("{}{}", tr("Dividend received"), fee.replace("fee", "tax")),
            money("+", tx.usd_price() - fee_usd),
        ),
        TransactionType::Split => (
            "bg-ctp-lavender/15 text-ctp-lavender",
            tx.ticker.to_string(),
            format!("{}-for-1 split", tx.shares.normalize()),
            "—".into(),
        ),
        TransactionType::Deposit => (
            "bg-ctp-blue/15 text-ctp-blue",
            "Cash".into(),
            format!("Deposit{fee}"),
            money("+", gross - fee_usd),
        ),
        TransactionType::Withdrawal => (
            "bg-ctp-peach/15 text-ctp-peach",
            "Cash".into(),
            format!("Withdrawal{fee}"),
            money("−", gross + fee_usd),
        ),
        TransactionType::Transfer => (
            "bg-ctp-surface1 text-ctp-subtext1",
            tx.ticker.to_string(),
            "Transfer".into(),
            "—".into(),
        ),
    }
}

#[component]
fn EmptyState() -> Element {
    rsx! {
        div { class: "mt-10 rounded-3xl border border-dashed border-ctp-surface1 px-6 py-16 text-center",
            div { class: "text-lg font-semibold text-ctp-text", {tr("Nothing here yet")} }
            p { class: "mt-1 text-sm text-ctp-subtext0",
                {tr("Add your first transaction to start tracking.")}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::DataRefresh;
    use futures::FutureExt;

    fn trade(i: u32) -> Transaction {
        Transaction {
            id: Uuid::from_u128(i as u128 + 1),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new(if i.is_multiple_of(2) { "VOO" } else { "QQQ" }).unwrap(),
            transaction_type: TransactionType::Buy,
            shares: Decimal::ONE,
            price: dec!(100),
            date: format!("2025-01-{:02}", i % 28 + 1),
            fee: Decimal::ZERO,
            currency: "USD".into(),
            fx_to_usd: Decimal::ONE,
        }
    }

    #[component]
    fn Harness(count: u32) -> Element {
        use_context_provider(|| Signal::new(GetDashBoardResponse::default()));
        use_context_provider(|| Dialogs(Signal::new(None)));
        use_context_provider(|| DataRefresh(Signal::new(0)));
        rsx! {
            TransactionList { transactions: (0..count).map(trade).collect::<Vec<_>>(), portfolio: None }
        }
    }

    fn render(count: u32) -> String {
        let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { count });
        dom.rebuild_in_place();
        for _ in 0..10 {
            if dom.wait_for_work().now_or_never().is_none() {
                break;
            }
            dom.render_immediate(&mut dioxus_core::NoOpMutations);
        }
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn long_histories_show_a_page_at_a_time() {
        let html = render(120);
        assert_eq!(html.matches("title=\"Edit\"").count(), TX_PAGE, "one page of rows");
        assert!(html.contains("120 shown"), "the count covers every row");
        assert!(html.contains("Show 50 more") && html.contains("Show all 120"), "{html}");

        let html = render(TX_PAGE as u32);
        assert_eq!(html.matches("title=\"Edit\"").count(), TX_PAGE);
        assert!(!html.contains("Show all"), "nothing hidden, no button");
    }
}
