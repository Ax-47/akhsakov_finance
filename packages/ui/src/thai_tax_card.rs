//! Plan tab "Thai tax" card: a year's dividends, tax withheld, gains and
//! SSF / RMF / Thai ESG room in baht, for filing (ภ.ง.ด. 90/91). The maths
//! lives in `dtos::thai_tax`.

use crate::i18n::{tr, trf};
use crate::{
    assets_card::use_assets,
    components::card::{ActionButton, ButtonTone, Card, MetricTile},
    files::{copy_to_clipboard, download},
    format::fmt_money,
};
use dioxus::prelude::*;
use dtos::{
    assets::TaxWrapper,
    thai_tax::{dates_needing_rates, early_sales, fund_room, in_baht, report_text, tax_years},
    Transaction,
};
use rust_decimal::Decimal;
use std::{collections::HashMap, str::FromStr};
use types::ticker_symbol::TickerSymbol;

const INCOME_KEY: &str = "akhsakov.tax.income";
const BIRTH_KEY: &str = "akhsakov.tax.birth";

fn baht(v: Decimal) -> String {
    fmt_money(v, "฿", 2)
}

async fn load(key: &str) -> String {
    document::eval(&format!("try {{ return localStorage.getItem({key:?}) || ''; }} catch (e) {{ return ''; }}"))
        .join::<String>()
        .await
        .unwrap_or_default()
}

fn store(key: &str, value: &str) {
    document::eval(&format!("try {{ localStorage.setItem({key:?}, {value:?}); }} catch (e) {{}}"));
}

/// Baht per US dollar on each date (Yahoo's daily USD/THB closes).
async fn baht_rates(dates: Vec<String>) -> HashMap<String, Decimal> {
    let fetched = futures::future::join_all(dates.into_iter().map(|d| async move {
        let rate = api::quote::quote::get_fx_rate("THB".into(), d.clone()).await.ok();
        (d, rate.filter(|r| *r > Decimal::ZERO).map(|usd_per_thb| Decimal::ONE / usd_per_thb))
    }))
    .await;
    fetched.into_iter().filter_map(|(d, r)| Some((d, r?))).collect()
}

#[component]
pub fn ThaiTaxCard(transactions: Vec<Transaction>) -> Element {
    let assets = use_assets();
    let wrappers: HashMap<TickerSymbol, TaxWrapper> = assets
        .read()
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| Some((a.ticker, a.wrapper?)))
        .collect();
    let dates = dates_needing_rates(&transactions);
    let rates = use_resource(use_reactive!(|dates| baht_rates(dates)));
    let mut income = use_signal(String::new);
    let mut birth = use_signal(String::new);
    let mut year = use_signal(|| None::<i32>);
    let mut copied = use_signal(|| None::<bool>);
    use_hook(move || {
        spawn(async move {
            income.set(load(INCOME_KEY).await);
            birth.set(load(BIRTH_KEY).await);
        });
    });

    let Some(rates) = rates.read().clone() else {
        return rsx! {
            Card { title: tr("Thai tax"), p { class: "text-sm text-ctp-subtext0", {tr("Getting baht exchange rates…")} } }
        };
    };
    let missing = dates.iter().filter(|d| !rates.contains_key(*d)).count();
    let in_thb = in_baht(&transactions, |d| rates.get(d).copied());
    let years = tax_years(&in_thb, &wrappers);
    let income_value = Decimal::from_str(income().trim().replace(',', "").as_str()).ok().filter(|v| *v > Decimal::ZERO);
    let birth_year = birth().trim().parse::<i32>().ok().filter(|y| (1900..2100).contains(y));
    let early = early_sales(&transactions, &wrappers, birth_year);

    if years.is_empty() {
        return rsx! {
            Card { title: tr("Thai tax"), subtitle: tr("Dividends, withholding and SSF / RMF in baht, per tax year").to_string(),
                p { class: "text-sm text-ctp-subtext0", {tr("No dividends, sales or tax-saving fund purchases yet.")} }
            }
        };
    }
    let chosen = year().filter(|y| years.iter().any(|t| t.year == *y)).unwrap_or(years[0].year);
    let y = years.iter().find(|t| t.year == chosen).cloned().unwrap_or_default();
    let room = income_value.map(|i| fund_room(i, &y));
    let text = report_text(&y, income_value);
    let text_copy = text.clone();

    rsx! {
        Card {
            title: tr("Thai tax"),
            subtitle: tr("Dividends, withholding and SSF / RMF in baht, per tax year").to_string(),
            actions: rsx! {
                div { class: "flex flex-wrap items-center gap-2",
                    select {
                        class: "rounded-full border border-ctp-surface1 bg-ctp-crust/40 px-3 py-1 text-sm text-ctp-text cursor-pointer [&_option]:bg-ctp-mantle",
                        "aria-label": "Tax year",
                        onchange: move |e| year.set(e.value().parse().ok()),
                        for t in years.iter() {
                            option { key: "{t.year}", value: "{t.year}", selected: t.year == chosen, "{t.year}" }
                        }
                    }
                    ActionButton {
                        label: match copied() { Some(true) => tr("Copied"), Some(false) => tr("Copy failed"), None => tr("Copy") },
                        tone: ButtonTone::Quiet,
                        onclick: move |_| {
                            let t = text_copy.clone();
                            spawn(async move { copied.set(Some(copy_to_clipboard(&t).await)) });
                        },
                    }
                    ActionButton {
                        label: tr("⇩ Report"),
                        tone: ButtonTone::Quiet,
                        onclick: move |_| download(&format!("thai-tax-{chosen}.txt"), "text/plain", &text),
                    }
                }
            },
            div { class: "grid gap-5",
                div {
                    h3 { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-ctp-subtext0", {tr("Thai dividends")} }
                    div { class: "grid grid-cols-2 gap-3 sm:grid-cols-3",
                        MetricTile { label: tr("Before tax"), value: baht(y.thai_dividends) }
                        MetricTile { label: tr("Withheld (10%)"), value: baht(y.thai_withheld) }
                        MetricTile {
                            label: tr("Tax credit if declared"),
                            value: baht(y.dividend_credit()),
                            hint: tr("Estimate for companies paying 20% corporate tax").to_string(),
                        }
                    }
                }
                div {
                    h3 { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-ctp-subtext0", {tr("Foreign income")} }
                    div { class: "grid grid-cols-2 gap-3 sm:grid-cols-4",
                        MetricTile { label: tr("Dividends before tax"), value: baht(y.foreign_dividends) }
                        MetricTile { label: tr("Foreign tax withheld"), value: baht(y.foreign_withheld) }
                        MetricTile { label: tr("Realized gains"), value: baht(y.foreign_gains) }
                        MetricTile {
                            label: tr("Taxable if brought in"),
                            value: baht(y.foreign_income()),
                            tone: "text-ctp-peach",
                            hint: tr("Since 2024, foreign income is taxed when it's brought into Thailand").to_string(),
                        }
                    }
                }
                p { class: "text-sm text-ctp-subtext0",
                    {trf("Gains on Thai shares and funds: {} (tax-free for individuals)", &[&baht(y.thai_gains)])}
                }
                div {
                    div { class: "mb-2 flex flex-wrap items-center justify-between gap-3",
                        h3 { class: "text-xs font-semibold uppercase tracking-wide text-ctp-subtext0", {tr("SSF / RMF / Thai ESG")} }
                        div { class: "flex flex-wrap items-center gap-3 text-xs text-ctp-subtext0",
                            label { class: "flex items-center gap-2",
                                {tr("Yearly income ฿")}
                                input {
                                    class: "w-28 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                                    inputmode: "decimal",
                                    value: "{income}",
                                    oninput: move |e| {
                                        store(INCOME_KEY, &e.value());
                                        income.set(e.value());
                                    },
                                }
                            }
                            label { class: "flex items-center gap-2",
                                {tr("Born (year)")}
                                input {
                                    class: "w-20 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                                    inputmode: "numeric",
                                    placeholder: "1990",
                                    value: "{birth}",
                                    oninput: move |e| {
                                        store(BIRTH_KEY, &e.value());
                                        birth.set(e.value());
                                    },
                                }
                            }
                        }
                    }
                    if wrappers.is_empty() {
                        p { class: "text-sm text-ctp-subtext0", {tr("Mark funds as SSF, RMF or Thai ESG under Funds, gold & other assets (Overview tab) to track them here.")} }
                    } else if let Some(room) = room {
                        div { class: "flex flex-col gap-2",
                            for r in room {
                                {
                                    let used = if r.limit > Decimal::ZERO { (r.bought / r.limit * Decimal::ONE_HUNDRED).min(Decimal::ONE_HUNDRED) } else { Decimal::ZERO };
                                    rsx! {
                                        div { key: "{r.wrapper.key()}",
                                            div { class: "flex items-baseline justify-between gap-2 text-sm",
                                                span { class: "font-semibold text-ctp-text", "{r.wrapper}" }
                                                span { class: "text-xs text-ctp-subtext0",
                                                    {trf("{} bought · {} left of {}", &[&baht(r.bought), &baht(r.room), &baht(r.limit)])}
                                                }
                                            }
                                            div { class: "mt-1 h-2 overflow-hidden rounded-full bg-ctp-surface0",
                                                div { class: "h-full rounded-full bg-gradient-to-r from-ctp-mauve to-ctp-sky", style: "width:{used}%;" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        p { class: "text-sm text-ctp-subtext0", {tr("Enter your yearly income to see how much you can still deduct.")} }
                        for w in TaxWrapper::ALL {
                            if y.bought(w) > Decimal::ZERO {
                                p { key: "{w.key()}", class: "text-sm text-ctp-text", "{w}: {baht(y.bought(w))}" }
                            }
                        }
                    }
                }
                if !early.is_empty() {
                    div { class: "rounded-2xl bg-ctp-red/10 px-4 py-3 text-sm text-ctp-red",
                        p { class: "font-semibold", {tr("Sold before the holding period ended — the tax deduction may have to be paid back:")} }
                        ul { class: "mt-1 list-disc pl-5 text-xs",
                            for (i, e) in early.into_iter().enumerate() {
                                li { key: "{i}", "{e.ticker} ({e.wrapper}) · {crate::format::fmt_shares(e.shares)} · {e.bought} → {e.sold}" }
                            }
                        }
                    }
                }
                if missing > 0 {
                    p { class: "text-xs text-ctp-peach", {trf("{} dates had no baht rate and were left out.", &[&missing])} }
                }
                p { class: "text-xs text-ctp-overlay1",
                    {tr("Dividends: amount before tax, with the tax withheld in the fee field. Foreign amounts use each day's USD/THB rate. For information only — check with the Revenue Department.")}
                }
            }
        }
    }
}
