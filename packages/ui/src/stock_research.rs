//! Stock page sections backed by the research API: news, rating changes,
//! dividends & splits, ownership and options.

use crate::i18n::tr;
use crate::{
    components::{
        analysis::stats::day_label,
        card::{Card, Segmented, ToggleButton},
        charts::bars::{BarChart, BarSeries, Unit},
    },
    format::{fmt_compact, or_dash},
};
use dioxus::prelude::*;
use dtos::research::{CorporateAction, OptionQuote};
use std::collections::BTreeMap;
use types::ticker_symbol::TickerSymbol;

#[component]
fn Loading(title: String) -> Element {
    rsx! {
        Card { title, p { class: "py-10 text-center text-sm text-ctp-overlay1", {tr("Loading…")} } }
    }
}

#[component]
fn Empty(title: String, text: String) -> Element {
    rsx! {
        Card { title, p { class: "py-10 text-center text-sm text-ctp-overlay1", "{text}" } }
    }
}

const UNAVAILABLE: &str =
    "Not available right now — Yahoo may be rate-limiting. Try again shortly.";

// ─── News ─────────────────────────────────────────────────────────────────────

#[component]
pub fn NewsTab(ticker: TickerSymbol) -> Element {
    let news = use_resource(move || {
        let t = ticker.clone();
        async move { api::get_news(t).await.ok() }
    });
    let result = news.read().clone();
    match result {
        None => rsx! { Loading { title: tr("News") } },
        Some(None) => rsx! { Empty { title: tr("News"), text: UNAVAILABLE } },
        Some(Some(items)) if items.is_empty() => {
            rsx! { Empty { title: tr("News"), text: "No recent news." } }
        }
        Some(Some(items)) => rsx! {
            Card { title: tr("News"), subtitle: format!("{} latest stories", items.len()), flush: true,
                for n in items {
                    a {
                        key: "{n.title}",
                        class: "group block border-t border-ctp-surface0/60 px-6 py-3.5 transition-colors hover:bg-ctp-surface0/30",
                        href: n.link.clone().unwrap_or_default(),
                        target: "_blank",
                        rel: "noopener noreferrer",
                        div { class: "text-sm font-medium text-ctp-text group-hover:text-ctp-mauve", "{n.title}" }
                        div { class: "mt-1 text-xs text-ctp-overlay1",
                            {[n.publisher.clone().unwrap_or_default(), n.published_at.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")}
                        }
                    }
                }
            }
        },
    }
}

// ─── Rating changes ───────────────────────────────────────────────────────────

#[component]
pub fn RatingChanges(ticker: TickerSymbol) -> Element {
    let rows = use_resource(move || {
        let t = ticker.clone();
        async move { api::get_rating_changes(t).await.ok() }
    });
    let title = "Rating changes";
    let result = rows.read().clone();
    match result {
        None => rsx! { Loading { title } },
        Some(None) => rsx! { Empty { title, text: UNAVAILABLE } },
        Some(Some(rows)) if rows.is_empty() => {
            rsx! { Empty { title, text: "No recent upgrades or downgrades." } }
        }
        Some(Some(rows)) => rsx! {
            Card { title, subtitle: tr("Analyst upgrades and downgrades").to_string(), flush: true,
                div { class: "overflow-x-auto",
                    table { class: "w-full text-sm whitespace-nowrap",
                        tbody {
                            for (i, r) in rows.into_iter().enumerate() {
                                {
                                    let action = r.action.clone().unwrap_or_default();
                                    let tone = match action.to_lowercase().as_str() {
                                        a if a.starts_with("up") => "bg-ctp-green/15 text-ctp-green",
                                        a if a.starts_with("down") => "bg-ctp-red/15 text-ctp-red",
                                        _ => "bg-ctp-surface0 text-ctp-subtext1",
                                    };
                                    rsx! {
                                        tr { key: "{i}", class: "border-t border-ctp-surface0/60 first:border-t-0",
                                            td { class: "pl-6 pr-4 py-2.5 text-xs text-ctp-overlay1", "{r.date}" }
                                            td { class: "px-4 py-2.5 text-ctp-text", {r.firm.clone().unwrap_or_default()} }
                                            td { class: "px-4 py-2.5 text-ctp-subtext0",
                                                {match (&r.from, &r.to) {
                                                    (Some(f), Some(t)) if f != t => format!("{f} → {t}"),
                                                    (_, Some(t)) => t.clone(),
                                                    _ => String::new(),
                                                }}
                                            }
                                            td { class: "pl-4 pr-6 py-2.5 text-right",
                                                if !action.is_empty() {
                                                    span { class: "rounded-full px-2.5 py-0.5 text-xs font-semibold {tone}", "{action}" }
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
        },
    }
}

// ─── Dividends & splits ───────────────────────────────────────────────────────

#[component]
pub fn DividendsAndSplits(ticker: TickerSymbol) -> Element {
    let actions = use_resource(move || {
        let t = ticker.clone();
        async move { api::get_corporate_actions(t).await.ok() }
    });
    let title = "Dividends & splits";
    let Some(result) = actions.read().clone() else {
        return rsx! { Loading { title } };
    };
    let Some(actions) = result else {
        return rsx! { Empty { title, text: UNAVAILABLE } };
    };
    if actions.is_empty() {
        return rsx! { Empty { title, text: "No dividends or splits on record." } };
    }

    // Dividends summed per calendar year, last 10 years.
    let mut per_year: BTreeMap<String, f64> = BTreeMap::new();
    for a in &actions {
        if let CorporateAction::Dividend { date, amount } = a {
            *per_year.entry(date[..4].to_string()).or_default() += amount;
        }
    }
    let years: Vec<(String, f64)> = per_year
        .into_iter()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let splits: Vec<(String, u32, u32)> = actions
        .iter()
        .filter_map(|a| match a {
            CorporateAction::Split {
                date,
                numerator,
                denominator,
            } => Some((date.clone(), *numerator, *denominator)),
            _ => None,
        })
        .collect();

    rsx! {
        Card { title, subtitle: tr("Dividends per share, by year").to_string(),
            if years.is_empty() {
                p { class: "text-sm text-ctp-overlay1", {tr("This stock hasn't paid dividends.")} }
            } else {
                BarChart {
                    labels: years.iter().map(|y| y.0.clone()).collect::<Vec<_>>(),
                    series: vec![BarSeries { name: "Dividends per share".into(), color: "var(--catppuccin-color-teal)", values: years.iter().map(|y| Some(y.1)).collect() }],
                    unit: Unit::Money,
                }
            }
            if !splits.is_empty() {
                div { class: "mt-5",
                    div { class: "mb-2 text-xs text-ctp-overlay1", {tr("Splits")} }
                    div { class: "flex flex-wrap gap-2",
                        for (date, n, d) in splits {
                            span { class: "rounded-full border border-ctp-surface0 px-3 py-1 text-xs text-ctp-subtext1",
                                span { class: "font-semibold text-ctp-text", "{n}-for-{d}" }
                                " · {date}"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Ownership ────────────────────────────────────────────────────────────────

#[component]
pub fn OwnershipTab(ticker: TickerSymbol) -> Element {
    let holders = use_resource(move || {
        let t = ticker.clone();
        async move { api::get_holders(t).await.ok() }
    });
    let Some(result) = holders.read().clone() else {
        return rsx! { Loading { title: tr("Ownership") } };
    };
    let Some(h) = result else {
        return rsx! { Empty { title: tr("Ownership"), text: UNAVAILABLE } };
    };
    let pct = |v: f64| format!("{:.2}%", v * 100.0);
    let shares = |v: f64| fmt_compact(v).replacen('$', "", 1);

    rsx! {
        div { class: "grid gap-5 motion-safe:animate-rise",
            if !h.breakdown.is_empty() {
                div { class: "grid grid-cols-2 lg:grid-cols-4 gap-3",
                    for (category, value) in h.breakdown.clone() {
                        div { key: "{category}", class: "rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60 px-4 py-3",
                            div { class: "text-xs text-ctp-overlay1", "{holder_label(&category)}" }
                            div { class: "mt-1 text-xl font-semibold tabular-nums text-ctp-text",
                                {if value <= 1.0 { pct(value) } else { shares(value) }}
                            }
                        }
                    }
                }
            }
            Card { title: tr("Top institutions"), flush: true,
                if h.institutions.is_empty() {
                    p { class: "px-6 pb-6 text-sm text-ctp-overlay1", {tr("No institutional holders reported.")} }
                } else {
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm whitespace-nowrap",
                            thead {
                                tr { class: "text-xs text-ctp-overlay1",
                                    th { class: "pl-6 pr-4 py-2.5 text-left font-medium", {tr("Holder")} }
                                    th { class: "px-4 py-2.5 text-right font-medium", {tr("Shares")} }
                                    th { class: "px-4 py-2.5 text-right font-medium", "% held" }
                                    th { class: "px-4 py-2.5 text-right font-medium", {tr("Value")} }
                                    th { class: "pl-4 pr-6 py-2.5 text-right font-medium", {tr("Reported")} }
                                }
                            }
                            tbody {
                                for i in h.institutions.clone() {
                                    tr { key: "{i.name}", class: "border-t border-ctp-surface0/60",
                                        td { class: "pl-6 pr-4 py-2.5 text-ctp-text", "{i.name}" }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", {or_dash(i.shares, shares)} }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", {or_dash(i.pct_held, pct)} }
                                        td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", {or_dash(i.value, fmt_compact)} }
                                        td { class: "pl-4 pr-6 py-2.5 text-right text-xs text-ctp-overlay1", "{i.date}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Card { title: tr("Insider trades"), subtitle: tr("Executives and directors buying or selling").to_string(), flush: true,
                if h.insider_trades.is_empty() {
                    p { class: "px-6 pb-6 text-sm text-ctp-overlay1", {tr("No recent insider trades.")} }
                } else {
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm whitespace-nowrap",
                            tbody {
                                for (n, t) in h.insider_trades.clone().into_iter().take(25).enumerate() {
                                    {
                                        let tone = match t.kind.as_str() {
                                            "Buy" => "bg-ctp-green/15 text-ctp-green",
                                            "Sell" => "bg-ctp-red/15 text-ctp-red",
                                            _ => "bg-ctp-surface0 text-ctp-subtext1",
                                        };
                                        rsx! {
                                            tr { key: "{n}", class: "border-t border-ctp-surface0/60 first:border-t-0",
                                                td { class: "pl-6 pr-4 py-2.5 text-xs text-ctp-overlay1", "{t.date}" }
                                                td { class: "px-4 py-2.5",
                                                    div { class: "text-ctp-text", "{t.name}" }
                                                    div { class: "text-xs text-ctp-overlay1", "{t.position}" }
                                                }
                                                td { class: "px-4 py-2.5", span { class: "rounded-full px-2.5 py-0.5 text-xs font-semibold {tone}", "{t.kind}" } }
                                                td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0", {or_dash(t.shares, shares)} }
                                                td { class: "pl-4 pr-6 py-2.5 text-right tabular-nums text-ctp-subtext0", {or_dash(t.value, fmt_compact)} }
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
}

/// Yahoo's holder category keys as readable labels.
fn holder_label(key: &str) -> String {
    match key {
        "insidersPercentHeld" => tr("Held by insiders").into(),
        "institutionsPercentHeld" => tr("Held by institutions").into(),
        "institutionsFloatPercentHeld" => tr("Institutions (of float)").into(),
        "institutionsCount" => tr("Institutions").into(),
        other => dtos::research::humanize(other),
    }
}

// ─── Options ──────────────────────────────────────────────────────────────────

#[component]
pub fn OptionsTab(ticker: TickerSymbol) -> Element {
    let mut expiration = use_signal(|| None::<i64>);
    let mut side = use_signal(|| true); // calls
    let chain = use_resource(move || {
        let t = ticker.clone();
        let e = expiration();
        async move { api::get_option_chain(t, e).await.ok() }
    });
    let Some(result) = chain.read().clone() else {
        return rsx! { Loading { title: tr("Options") } };
    };
    let Some(view) = result else {
        return rsx! { Empty { title: tr("Options"), text: UNAVAILABLE } };
    };
    if view.expirations.is_empty() {
        return rsx! { Empty { title: tr("Options"), text: "No listed options." } };
    }
    let rows = if side() {
        view.calls.clone()
    } else {
        view.puts.clone()
    };

    rsx! {
        Card {
            title: tr("Options"),
            subtitle: format!("Expiring {}", view.expiration.map(|e| day_label(e.div_euclid(86_400))).unwrap_or_default()),
            flush: true,
            actions: rsx! {
                Segmented {
                    ToggleButton { label: tr("Calls"), active: side(), onclick: move |_| side.set(true) }
                    ToggleButton { label: tr("Puts"), active: !side(), onclick: move |_| side.set(false) }
                }
            },
            div { class: "flex gap-1.5 overflow-x-auto px-6 pb-3",
                for e in view.expirations.clone().into_iter().take(16) {
                    button {
                        key: "{e}",
                        class: if view.expiration == Some(e) {
                            "shrink-0 rounded-full bg-ctp-mauve/15 px-3 py-1 text-xs font-medium text-ctp-mauve cursor-pointer"
                        } else {
                            "shrink-0 rounded-full px-3 py-1 text-xs text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0/60 hover:text-ctp-text"
                        },
                        onclick: move |_| expiration.set(Some(e)),
                        {day_label(e.div_euclid(86_400))}
                    }
                }
            }
            OptionTable { rows }
            p { class: "px-6 py-3 text-[0.7rem] text-ctp-overlay0", {tr("Highlighted rows are in the money.")} }
        }
    }
}

#[component]
fn OptionTable(rows: Vec<OptionQuote>) -> Element {
    let money = |v: Option<f64>| or_dash(v, |v| format!("{v:.2}"));
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "w-full text-sm whitespace-nowrap",
                thead {
                    tr { class: "text-xs text-ctp-overlay1",
                        for h in ["Strike", "Last", "Bid", "Ask", "Volume", "Open interest", "Implied vol."] {
                            th { class: "px-4 py-2.5 text-right font-medium first:pl-6 last:pr-6", "{h}" }
                        }
                    }
                }
                tbody {
                    for (i, q) in rows.into_iter().enumerate() {
                        tr {
                            key: "{i}",
                            class: if q.in_the_money { "border-t border-ctp-surface0/60 bg-ctp-mauve/5" } else { "border-t border-ctp-surface0/60" },
                            td { class: "pl-6 pr-4 py-2 text-right font-semibold tabular-nums text-ctp-text", "{q.strike:.2}" }
                            td { class: "px-4 py-2 text-right tabular-nums text-ctp-subtext0", {money(q.last)} }
                            td { class: "px-4 py-2 text-right tabular-nums text-ctp-subtext0", {money(q.bid)} }
                            td { class: "px-4 py-2 text-right tabular-nums text-ctp-subtext0", {money(q.ask)} }
                            td { class: "px-4 py-2 text-right tabular-nums text-ctp-subtext0", {or_dash(q.volume, |v| v.to_string())} }
                            td { class: "px-4 py-2 text-right tabular-nums text-ctp-subtext0", {or_dash(q.open_interest, |v| v.to_string())} }
                            td { class: "pl-4 pr-6 py-2 text-right tabular-nums text-ctp-subtext0", {or_dash(q.implied_volatility, |v| format!("{:.1}%", v * 100.0))} }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holder_labels() {
        assert_eq!(holder_label("insidersPercentHeld"), "Held by insiders");
        assert_eq!(holder_label("SomethingNew"), "Something new");
    }
}
