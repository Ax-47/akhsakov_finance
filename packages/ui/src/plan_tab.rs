//! Portfolio page "Plan" tab: rebalancing, tax, money-weighted return and
//! goals. The maths lives in `dtos::planning`.

use crate::i18n::tr;
use crate::{
    app::DataRefresh,
    components::card::{ActionButton, Card, MetricTile, Stepper},
    editors::{Dialog, Dialogs},
    format::{fmt_compact, fmt_signed, fmt_usd, signed_color},
    hooks::scoped_data,
    notify::today,
};
use dioxus::prelude::*;
use dtos::{
    planning::{
        days_between, fifo_lots, investor_cash_flows, project_goal, realized_by_year, rebalance,
        xirr, TargetWeight,
    },
    portfolio::GetDashBoardResponse,
    position::compute_positions,
    Position, Transaction,
};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::{collections::HashMap, str::FromStr};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[component]
pub fn PlanTab(
    positions: Vec<Position>,
    transactions: Vec<Transaction>,
    total_value: Decimal,
    /// Selected portfolio; rebalancing targets are per portfolio.
    portfolio: Option<Uuid>,
    /// Live prices for every ticker, for goals on other portfolios.
    prices: HashMap<TickerSymbol, Decimal>,
) -> Element {
    let date = use_resource(today);
    let today = date.read().clone().flatten();
    rsx! {
        div { class: "grid gap-5",
            RebalanceCard { positions: positions.clone(), portfolio }
            div { class: "grid gap-5 lg:grid-cols-[1fr_1fr]",
                ReturnsCard { transactions: transactions.clone(), total_value, today: today.clone() }
                GoalsCard { prices, today: today.clone() }
            }
            TaxCard { transactions, positions, today }
        }
    }
}

// ─── Rebalance ────────────────────────────────────────────────────────────────

#[component]
fn RebalanceCard(positions: Vec<Position>, portfolio: Option<Uuid>) -> Element {
    let Some(portfolio_id) = portfolio else {
        return rsx! {
            Card { title: tr("Rebalance"),
                p { class: "text-sm text-ctp-subtext0", {tr("Choose a portfolio at the top of the page to set target weights and rebalance it.")} }
            }
        };
    };
    rsx! { RebalanceEditor { key: "{portfolio_id}", positions, portfolio_id } }
}

#[component]
fn RebalanceEditor(positions: Vec<Position>, portfolio_id: Uuid) -> Element {
    let refresh = use_context::<DataRefresh>();
    let saved = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_targets(portfolio_id).await.ok()
    });
    // Editable rows: (ticker, weight %) as typed.
    let mut rows = use_signal(Vec::<(String, String)>::new);
    let mut loaded = use_signal(|| false);
    let mut new_cash = use_signal(String::new);
    let mut adding = use_signal(String::new);
    let mut status = use_signal(|| None::<Result<(), String>>);

    let priced: Vec<&Position> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .collect();
    let total: Decimal = priced.iter().map(|p| p.market_value()).sum();
    let current_pct = |t: &str| {
        priced
            .iter()
            .find(|p| p.ticker.as_str() == t)
            .and_then(|p| {
                (total > Decimal::ZERO)
                    .then(|| (p.market_value() / total * Decimal::ONE_HUNDRED).to_f64())
                    .flatten()
            })
            .unwrap_or(0.0)
    };

    // Seed once from saved targets.
    if !loaded() {
        if let Some(Some(targets)) = saved.read().clone() {
            rows.set(
                targets
                    .iter()
                    .map(|t| (t.ticker.to_string(), t.weight.normalize().to_string()))
                    .collect(),
            );
            loaded.set(true);
        }
    }

    let parsed: Vec<TargetWeight> = rows
        .read()
        .iter()
        .filter_map(|(t, w)| {
            Some(TargetWeight {
                ticker: TickerSymbol::new(t).ok()?,
                weight: Decimal::from_str(w.trim()).ok()?,
            })
        })
        .collect();
    let target_total: Decimal = parsed.iter().map(|t| t.weight).sum();
    let holdings: Vec<(TickerSymbol, f64, f64)> = priced
        .iter()
        .map(|p| {
            (
                p.ticker.clone(),
                p.market_value().to_f64().unwrap_or(0.0),
                p.current_price.to_f64().unwrap_or(0.0),
            )
        })
        .collect();
    let cash = new_cash().trim().parse::<f64>().unwrap_or(0.0);
    let trades = if parsed.is_empty() {
        vec![]
    } else {
        rebalance(&holdings, &parsed, cash, 1.0)
    };

    let current_rows = priced_weights(&positions);
    let use_current = move |_| rows.set(current_rows.clone());
    let save = move |_| async move {
        let targets: Vec<TargetWeight> = rows
            .read()
            .iter()
            .filter_map(|(t, w)| {
                Some(TargetWeight {
                    ticker: TickerSymbol::new(t).ok()?,
                    weight: Decimal::from_str(w.trim()).ok()?,
                })
            })
            .collect();
        match api::set_targets(portfolio_id, targets).await {
            Ok(()) => {
                status.set(Some(Ok(())));
                refresh.reload();
            }
            Err(ServerFnError::ServerError { message, .. }) => status.set(Some(Err(message))),
            Err(e) => status.set(Some(Err(e.to_string()))),
        }
    };

    rsx! {
        Card {
            title: tr("Rebalance"),
            subtitle: tr("Set target weights, see the trades that get you there").to_string(),
            actions: rsx! {
                div { class: "flex flex-wrap gap-2",
                    ActionButton { label: tr("Use current weights"), tone: crate::components::card::ButtonTone::Quiet, onclick: use_current }
                    ActionButton { label: tr("Save targets"), onclick: save }
                }
            },
            div { class: "grid gap-6 lg:grid-cols-[1fr_1fr]",
                div {
                    div { class: "mb-2 flex justify-between text-xs text-ctp-subtext0",
                        span { {tr("Holding · now → target")} }
                        span { class: if target_total > Decimal::ONE_HUNDRED { "text-ctp-red" } else { "" }, "Total {target_total.normalize()}%" }
                    }
                    div { class: "flex flex-col gap-2",
                        for (i, (ticker, weight)) in rows().into_iter().enumerate() {
                            div { key: "{ticker}", class: "flex items-center gap-3",
                                span { class: "w-16 font-semibold text-ctp-text", "{ticker}" }
                                span { class: "w-14 text-right text-xs tabular-nums text-ctp-subtext0", "{current_pct(&ticker):.1}%" }
                                span { class: "text-ctp-overlay1", "→" }
                                Stepper {
                                    aria_label: "Target weight for {ticker}",
                                    value: weight,
                                    step: 1.0,
                                    decimals: 0,
                                    suffix: "%",
                                    on_change: move |v| rows.write()[i].1 = v,
                                }
                                button {
                                    class: "ml-auto inline-flex h-8 min-w-8 items-center justify-center rounded-full px-2 text-sm text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                                    "aria-label": "Remove {ticker}",
                                    onclick: move |_| {
                                        rows.write().remove(i);
                                    },
                                    "×"
                                }
                            }
                        }
                    }
                    form {
                        class: "mt-3 flex gap-2",
                        onsubmit: move |e| {
                            e.prevent_default();
                            if let Ok(t) = TickerSymbol::new(&adding()) {
                                if !rows.read().iter().any(|r| r.0 == t.as_str()) {
                                    rows.write().push((t.to_string(), "0".into()));
                                }
                                adding.set(String::new());
                            }
                        },
                        input {
                            class: "w-36 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1.5 text-sm uppercase text-ctp-text placeholder:normal-case placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                            placeholder: tr("Add ticker ↵"),
                            value: "{adding}",
                            oninput: move |e| adding.set(e.value()),
                        }
                    }
                    if let Some(result) = status() {
                        p { class: if result.is_ok() { "mt-3 text-xs text-ctp-green" } else { "mt-3 text-xs text-ctp-red" },
                            {result.as_ref().err().cloned().unwrap_or_else(|| "Targets saved.".into())}
                        }
                    }
                }
                div {
                    div { class: "mb-2 flex items-center justify-between gap-3 text-xs text-ctp-subtext0",
                        span { {tr("Suggested trades")} }
                        label { class: "flex items-center gap-2",
                            {tr("New cash to invest $")}
                            input {
                                class: "w-24 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                                inputmode: "decimal",
                                placeholder: "0",
                                value: "{new_cash}",
                                oninput: move |e| new_cash.set(e.value()),
                            }
                        }
                    }
                    if parsed.is_empty() {
                        p { class: "rounded-2xl bg-ctp-base/40 px-4 py-6 text-center text-sm text-ctp-subtext0",
                            {tr("Add targets (or use current weights) to see trades.")}
                        }
                    } else if trades.is_empty() {
                        p { class: "rounded-2xl bg-ctp-base/40 px-4 py-6 text-center text-sm text-ctp-green", {tr("Already on target.")} }
                    } else {
                        div { class: "flex flex-col gap-1.5",
                            for t in trades {
                                div { key: "{t.ticker}", class: "flex items-center gap-3 rounded-xl bg-ctp-base/40 px-3 py-2 text-sm",
                                    span {
                                        class: if t.amount >= 0.0 { "w-12 rounded-full bg-ctp-green/15 py-0.5 text-center text-xs font-semibold text-ctp-green" } else { "w-12 rounded-full bg-ctp-red/15 py-0.5 text-center text-xs font-semibold text-ctp-red" },
                                        if t.amount >= 0.0 { {tr("Buy")} } else { {tr("Sell")} }
                                    }
                                    span { class: "w-16 font-semibold text-ctp-text", "{t.ticker}" }
                                    span { class: "flex-1 text-xs text-ctp-subtext0", "{t.current_pct:.1}% → {t.target_pct:.1}%" }
                                    span { class: "tabular-nums text-ctp-text", "{fmt_compact(t.amount.abs())}" }
                                    span { class: "w-24 text-right text-xs tabular-nums text-ctp-subtext0",
                                        {t.shares.map(|s| format!("{:.4} sh", s.abs())).unwrap_or_default()}
                                    }
                                }
                            }
                        }
                    }
                    p { class: "mt-3 text-xs text-ctp-overlay1", {tr("Uses live prices; fees and taxes aren't included. Trades under $1 are skipped.")} }
                }
            }
        }
    }
}

/// Current weights, rounded to whole percents, as editable rows.
fn priced_weights(positions: &[Position]) -> Vec<(String, String)> {
    let total: Decimal = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| p.market_value())
        .sum();
    if total <= Decimal::ZERO {
        return vec![];
    }
    let mut rows: Vec<(String, Decimal)> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| {
            (
                p.ticker.to_string(),
                (p.market_value() / total * Decimal::ONE_HUNDRED).round(),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    rows.into_iter().map(|(t, w)| (t, w.to_string())).collect()
}

// ─── Returns ──────────────────────────────────────────────────────────────────

#[component]
fn ReturnsCard(
    transactions: Vec<Transaction>,
    total_value: Decimal,
    today: Option<String>,
) -> Element {
    let value = total_value.to_f64().unwrap_or(0.0);
    let mwr = today
        .as_deref()
        .and_then(|d| xirr(&investor_cash_flows(&transactions, value, d)));
    let invested: f64 = transactions
        .iter()
        .filter(|t| t.transaction_type == types::transaction_type::TransactionType::Buy)
        .filter_map(|t| (t.shares * t.usd_price() + t.usd_fee()).to_f64())
        .sum();
    let first = transactions
        .iter()
        .map(|t| t.date.as_str())
        .min()
        .unwrap_or_default()
        .to_string();
    let years = today
        .as_deref()
        .and_then(|d| days_between(&first, d))
        .map(|d| d as f64 / 365.0);
    let mwr_tone = if mwr.unwrap_or(0.0) >= 0.0 {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    };

    rsx! {
        Card { title: tr("Your return"), subtitle: tr("Money-weighted: counts when you added money").to_string(),
            div { class: "grid grid-cols-2 gap-3",
                MetricTile {
                    label: tr("Money-weighted (per year)"),
                    value: mwr.map(|r| format!("{:+.2}%", r * 100.0)).unwrap_or_else(|| "—".into()),
                    tone: mwr_tone,
                    hint: tr("Like a savings rate: your actual yearly growth").to_string(),
                }
                MetricTile {
                    label: tr("Invested so far"),
                    value: fmt_compact(invested),
                    hint: years.map(|y| format!("over {:.1} years", y)).unwrap_or_default(),
                }
            }
            p { class: "mt-4 text-xs leading-relaxed text-ctp-subtext0",
                {tr("The Performance chart shows the time-weighted return, which ignores when you invested — best for comparing with the market. ")}
                {tr("This money-weighted return reflects your timing: buying more before a rise lifts it; before a fall lowers it.")}
            }
            if mwr.is_none() {
                p { class: "mt-2 text-xs text-ctp-overlay1", {tr("Needs at least one purchase and a current value.")} }
            }
        }
    }
}

// ─── Goals ────────────────────────────────────────────────────────────────────

#[component]
fn GoalsCard(prices: HashMap<TickerSymbol, Decimal>, today: Option<String>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let assumed = use_context::<crate::app::AppSettings>()
        .0
        .read()
        .assumed_return
        .to_f64()
        .unwrap_or(7.0)
        / 100.0;
    let goals = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_goals().await.unwrap_or_default()
    });
    let list = goals.read().clone().unwrap_or_default();
    let price_map: HashMap<TickerSymbol, (Decimal, Decimal)> = prices
        .iter()
        .map(|(t, p)| (t.clone(), (*p, Decimal::ZERO)))
        .collect();
    let value_of = |portfolio: Option<Uuid>| -> f64 {
        let scoped = scoped_data(&data.read(), portfolio.map(|p| p.to_string()).as_deref());
        let holdings: Decimal = compute_positions(&scoped, &price_map)
            .iter()
            .map(|p| p.market_value())
            .sum();
        (holdings + dtos::position::cash_balance(&scoped.transactions).unwrap_or_default())
            .to_f64()
            .unwrap_or(0.0)
    };

    rsx! {
        Card {
            title: tr("Goals"),
            subtitle: format!("Projected at {:.1}% a year (see Settings)", assumed * 100.0),
            actions: rsx! {
                ActionButton { label: tr("＋ Goal"), tone: crate::components::card::ButtonTone::Quiet, onclick: move |_| dialogs.open(Dialog::Goal(None)) }
            },
            if list.is_empty() {
                p { class: "text-sm text-ctp-subtext0", {tr("Set a target, like a house deposit by 2030, and see if you're on track.")} }
            }
            div { class: "flex flex-col gap-4",
                for g in list {
                    {
                        let current = value_of(g.portfolio_id);
                        let target = g.target.to_f64().unwrap_or(1.0).max(1.0);
                        let progress = (current / target * 100.0).clamp(0.0, 100.0);
                        let p = today.as_deref().map(|d| project_goal(&g, current, d, assumed));
                        let edit = g.clone();
                        rsx! {
                            div { key: "{g.id}", class: "group",
                                div { class: "flex items-baseline justify-between gap-2",
                                    span { class: "font-semibold text-ctp-text", "{g.name}" }
                                    span { class: "text-xs text-ctp-subtext0", "{fmt_compact(current)} of {fmt_compact(target)} · by {g.date}" }
                                }
                                div { class: "mt-2 h-2 overflow-hidden rounded-full bg-ctp-surface0",
                                    div { class: "h-full rounded-full bg-gradient-to-r from-ctp-mauve to-ctp-sky", style: "width:{progress:.1}%;" }
                                }
                                div { class: "mt-2 flex flex-wrap items-center gap-2 text-xs",
                                    if let Some(p) = &p {
                                        span {
                                            class: if p.on_track { "rounded-full bg-ctp-green/15 px-2 py-0.5 font-semibold text-ctp-green" } else { "rounded-full bg-ctp-peach/15 px-2 py-0.5 font-semibold text-ctp-peach" },
                                            if p.on_track { {tr("On track")} } else { {tr("Behind")} }
                                        }
                                        span { class: "text-ctp-subtext0", "Projected {fmt_compact(p.projected)} in {p.months} months" }
                                        if let Some(r) = p.required_return {
                                            span { class: "text-ctp-overlay1", "· needs {r * 100.0:.1}%/yr" }
                                        }
                                    }
                                    span { class: "ml-auto flex gap-1 opacity-40 transition-opacity group-hover:opacity-100 focus-visible:opacity-100",
                                        button {
                                            class: "rounded-full px-2 py-0.5 text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                                            onclick: move |_| dialogs.open(Dialog::Goal(Some(edit.clone()))),
                                            "✎"
                                        }
                                        button {
                                            class: "rounded-full px-2 py-0.5 text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                                            onclick: move |_| async move {
                                                if api::delete_goal(g.id).await.is_ok() {
                                                    refresh.reload();
                                                }
                                            },
                                            "🗑"
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

// ─── Tax ──────────────────────────────────────────────────────────────────────

#[component]
fn TaxCard(
    transactions: Vec<Transaction>,
    positions: Vec<Position>,
    today: Option<String>,
) -> Element {
    let years = realized_by_year(&transactions);
    let (lots, _) = fifo_lots(&transactions);
    let price = |t: &TickerSymbol| {
        positions
            .iter()
            .find(|p| &p.ticker == t)
            .map(|p| p.current_price)
            .filter(|p| *p > Decimal::ZERO)
    };

    rsx! {
        Card { title: tr("Tax"), subtitle: tr("First-in, first-out lots · long term = held over a year").to_string(), flush: true,
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            for h in ["Year", "Short-term gains", "Long-term gains", "Dividends", "Total"] {
                                th { class: "px-4 py-2.5 text-right font-medium first:pl-6 first:text-left last:pr-6", "{h}" }
                            }
                        }
                    }
                    tbody {
                        if years.is_empty() {
                            tr { td { class: "px-6 py-4 text-sm text-ctp-subtext0", colspan: "5", {tr("No sales or dividends yet.")} } }
                        }
                        for (year, short, long, divs) in years {
                            tr { key: "{year}", class: "border-t border-ctp-surface0/60",
                                td { class: "pl-6 pr-4 py-2.5 font-semibold text-ctp-text", "{year}" }
                                td { class: "px-4 py-2.5 text-right tabular-nums {signed_color(short)}", "{fmt_signed(short, 2)}" }
                                td { class: "px-4 py-2.5 text-right tabular-nums {signed_color(long)}", "{fmt_signed(long, 2)}" }
                                td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", "{fmt_usd(divs, 2)}" }
                                td { class: "pl-4 pr-6 py-2.5 text-right font-semibold tabular-nums {signed_color(short + long + divs)}", "{fmt_signed(short + long + divs, 2)}" }
                            }
                        }
                    }
                }
            }
            div { class: "mt-4 px-6 pb-2 text-xs text-ctp-subtext0", {tr("Open lots")} }
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            for h in ["Asset", "Bought", "Shares", "Cost / share", "Unrealized", "Term"] {
                                th { class: "px-4 py-2.5 text-right font-medium first:pl-6 first:text-left last:pr-6", "{h}" }
                            }
                        }
                    }
                    tbody {
                        for (i, lot) in lots.into_iter().enumerate() {
                            {
                                let gain = price(&lot.ticker).map(|p| (p - lot.cost) * lot.shares);
                                let long = today.as_deref().and_then(|d| days_between(&lot.date, d)).is_some_and(|d| d > 365);
                                rsx! {
                                    tr { key: "{i}", class: "border-t border-ctp-surface0/60",
                                        td { class: "pl-6 pr-4 py-2.5 font-semibold text-ctp-text", "{lot.ticker}" }
                                        td { class: "px-4 py-2.5 text-right text-xs text-ctp-subtext0", "{lot.date}" }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", {crate::format::fmt_shares(lot.shares)} }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", "{fmt_usd(lot.cost, 2)}" }
                                        td { class: "px-4 py-2.5 text-right tabular-nums {gain.map(signed_color).unwrap_or(\"text-ctp-overlay1\")}",
                                            {gain.map(|g| fmt_signed(g, 2)).unwrap_or_else(|| "—".into())}
                                        }
                                        td { class: "pl-4 pr-6 py-2.5 text-right",
                                            span { class: if long { "rounded-full bg-ctp-green/15 px-2 py-0.5 text-xs text-ctp-green" } else { "rounded-full bg-ctp-surface0 px-2 py-0.5 text-xs text-ctp-subtext1" },
                                                if long { {tr("Long")} } else { {tr("Short")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            p { class: "px-6 py-3 text-xs text-ctp-overlay1", {tr("For information only — check your country's rules and your broker's tax statement.")} }
        }
    }
}
