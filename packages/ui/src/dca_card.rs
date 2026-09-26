//! Plan tab "Monthly plans" card: buy a fixed amount each month (DCA).
//! The server reminds you on the day; here the purchase is recorded in one
//! click at today's price.

use crate::i18n::{tr, trf};
use crate::{
    app::DataRefresh,
    components::card::{ActionButton, ButtonTone, Card, Field, Modal, INPUT},
    notify::today,
};
use dioxus::prelude::*;
use dtos::{dca::DcaPlan, portfolio::GetDashBoardResponse};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

fn message(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message,
        other => other.to_string(),
    }
}

fn num(s: &str) -> Option<Decimal> {
    Decimal::from_str(&s.trim().replace(',', "")).ok()
}

#[component]
pub fn DcaCard(portfolio: Option<Uuid>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let plans = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_dca_plans().await.unwrap_or_default()
    });
    let date = use_resource(today);
    let today = date.read().clone().flatten().unwrap_or_default();
    let mut editing = use_signal(|| None::<Option<DcaPlan>>);
    let list: Vec<DcaPlan> = plans
        .read()
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| portfolio.is_none_or(|id| p.portfolio_id == id))
        .collect();

    rsx! {
        Card {
            title: tr("Monthly plans (DCA)"),
            subtitle: tr("Invest a fixed amount each month; you get a reminder on the day").to_string(),
            actions: rsx! {
                ActionButton { label: tr("＋ Plan"), tone: ButtonTone::Quiet, onclick: move |_| editing.set(Some(None)) }
            },
            if list.is_empty() {
                p { class: "text-sm text-ctp-subtext0", {tr("e.g. ฿3,000 of a fund or $500 of VOO on the 5th of every month. Reminders arrive in the app and on your push channels (Settings).")} }
            }
            div { class: "flex flex-col gap-2",
                for p in list {
                    PlanRow { key: "{p.id}", plan: p.clone(), today: today.clone(), on_edit: move |p| editing.set(Some(Some(p))) }
                }
            }
        }
        if let Some(existing) = editing() {
            PlanDialog { existing, portfolio, on_close: move |_| editing.set(None) }
        }
    }
}

#[component]
fn PlanRow(plan: DcaPlan, today: String, on_edit: EventHandler<DcaPlan>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let portfolio_name = data
        .read()
        .portfolios
        .iter()
        .find(|p| p.id == plan.portfolio_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    // Open while recording: (price, shares, fee) as typed.
    let mut recording = use_signal(|| None::<(String, String, String)>);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let due = !today.is_empty() && plan.is_due(&today);
    let next = plan.next_due(&today);

    let p = plan.clone();
    let start_recording = move |_| {
        let p = p.clone();
        async move {
            error.set(None);
            let price = api::quote::quote::get_native_quote(p.ticker.clone())
                .await
                .ok()
                .map(|q| q.current_price)
                .filter(|v| *v > Decimal::ZERO);
            let shares = price.and_then(|v| p.shares_at(v));
            recording.set(Some((
                price.map(|v| v.round_dp(4).normalize().to_string()).unwrap_or_default(),
                shares.map(|s| s.normalize().to_string()).unwrap_or_default(),
                String::new(),
            )));
        }
    };
    let p = plan.clone();
    let today_for_save = today.clone();
    let save = move |_| {
        let (p, date) = (p.clone(), today_for_save.clone());
        async move {
            let Some((price, shares, fee)) = recording() else { return };
            let (Some(price), Some(shares)) = (num(&price), num(&shares)) else {
                return error.set(Some(tr("Enter the price and the number of units").into()));
            };
            let fee = if fee.trim().is_empty() { Some(Decimal::ZERO) } else { num(&fee) };
            let Some(fee) = fee else {
                return error.set(Some(tr("Enter a number for the fee").into()));
            };
            busy.set(true);
            match api::record_dca(p.id, date, shares, price, fee).await {
                Ok(()) => {
                    recording.set(None);
                    refresh.reload();
                }
                Err(e) => error.set(Some(message(e))),
            }
            busy.set(false);
        }
    };
    let p = plan.clone();
    let toggle = move |_| {
        let p = p.clone();
        async move {
            if api::save_dca_plan(DcaPlan { active: !p.active, ..p }).await.is_ok() {
                refresh.reload();
            }
        }
    };
    let edit = plan.clone();
    let id = plan.id;

    rsx! {
        div { class: "group rounded-xl bg-ctp-base/40 px-3 py-2 text-sm",
            div { class: "flex flex-wrap items-center gap-x-3 gap-y-1",
                span { class: "w-20 font-semibold text-ctp-text", "{plan.ticker}" }
                span { class: "tabular-nums text-ctp-text", "{plan.amount.normalize()} {plan.currency}" }
                span { class: "text-xs text-ctp-subtext0", {trf("day {} each month · {}", &[&plan.day, &portfolio_name])} }
                span { class: "ml-auto flex items-center gap-2",
                    if !plan.active {
                        span { class: "rounded-full bg-ctp-surface0 px-2 py-0.5 text-xs text-ctp-subtext1", {tr("Paused")} }
                    } else if due {
                        span { class: "rounded-full bg-ctp-peach/15 px-2 py-0.5 text-xs font-semibold text-ctp-peach", {tr("Due")} }
                    } else if let Some(n) = &next {
                        span { class: "text-xs text-ctp-subtext0", {trf("next {}", &[n])} }
                    }
                    if plan.active && recording().is_none() {
                        ActionButton { label: tr("Record buy"), tone: if due { ButtonTone::default() } else { ButtonTone::Quiet }, onclick: start_recording }
                    }
                    span { class: "flex gap-1 opacity-40 transition-opacity group-hover:opacity-100 focus-within:opacity-100",
                        button {
                            class: "rounded-full px-2 py-0.5 text-xs text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                            onclick: toggle,
                            if plan.active { {tr("Pause")} } else { {tr("Resume")} }
                        }
                        button {
                            class: "rounded-full px-2 py-0.5 text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                            "aria-label": "Edit plan",
                            onclick: move |_| on_edit.call(edit.clone()),
                            "✎"
                        }
                        button {
                            class: "rounded-full px-2 py-0.5 text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                            "aria-label": "Delete plan",
                            onclick: move |_| async move {
                                if api::delete_dca_plan(id).await.is_ok() {
                                    refresh.reload();
                                }
                            },
                            "🗑"
                        }
                    }
                }
            }
            if let Some((price, shares, fee)) = recording() {
                form {
                    class: "mt-2 flex flex-wrap items-end gap-2",
                    onsubmit: move |e| e.prevent_default(),
                    label { class: "grid gap-1 text-xs text-ctp-subtext0",
                        {trf("Price ({})", &[&plan.currency])}
                        input {
                            class: "w-28 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                            inputmode: "decimal",
                            value: "{price}",
                            oninput: {
                                let amount = plan.amount;
                                move |e: FormEvent| {
                                    let v = e.value();
                                    let units = num(&v).filter(|p| *p > Decimal::ZERO).map(|p| (amount / p).round_dp(6).normalize().to_string());
                                    recording.with_mut(|r| if let Some(r) = r {
                                        r.0 = v;
                                        if let Some(u) = units {
                                            r.1 = u;
                                        }
                                    });
                                }
                            },
                        }
                    }
                    label { class: "grid gap-1 text-xs text-ctp-subtext0",
                        {tr("Units")}
                        input {
                            class: "w-28 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                            inputmode: "decimal",
                            value: "{shares}",
                            oninput: move |e| recording.with_mut(|r| if let Some(r) = r { r.1 = e.value() }),
                        }
                    }
                    label { class: "grid gap-1 text-xs text-ctp-subtext0",
                        {tr("Fee")}
                        input {
                            class: "w-20 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                            inputmode: "decimal",
                            placeholder: "0",
                            value: "{fee}",
                            oninput: move |e| recording.with_mut(|r| if let Some(r) = r { r.2 = e.value() }),
                        }
                    }
                    ActionButton { label: tr("Save purchase"), disabled: busy(), onclick: save }
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| recording.set(None) }
                }
            }
            if let Some(e) = error() {
                p { class: "mt-1 text-xs text-ctp-red", "{e}" }
            }
        }
    }
}

/// New plan (`existing: None`) or edit one.
#[component]
fn PlanDialog(existing: Option<DcaPlan>, portfolio: Option<Uuid>, on_close: EventHandler<()>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let choices: Vec<(Uuid, String)> = data.read().portfolios.iter().map(|p| (p.id, p.name.clone())).collect();
    let first = portfolio.or_else(|| choices.first().map(|p| p.0));
    let e = existing.clone();
    let mut target = use_signal(move || e.as_ref().map(|p| p.portfolio_id).or(first));
    let e = existing.clone();
    let mut ticker = use_signal(move || e.as_ref().map(|p| p.ticker.to_string()).unwrap_or_default());
    let e = existing.clone();
    let mut amount = use_signal(move || e.as_ref().map(|p| p.amount.normalize().to_string()).unwrap_or_default());
    let e = existing.clone();
    let mut currency = use_signal(move || e.as_ref().map(|p| p.currency.clone()).unwrap_or_else(|| "USD".into()));
    let e = existing.clone();
    let mut day = use_signal(move || e.as_ref().map(|p| p.day.to_string()).unwrap_or_else(|| "1".into()));
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let is_new = existing.is_none();

    let submit = move |_| {
        let existing = existing.clone();
        async move {
            let Some(portfolio_id) = target() else {
                return error.set(Some(tr("Choose a portfolio").into()));
            };
            let Ok(t) = TickerSymbol::new(&ticker()) else {
                return error.set(Some(tr("Enter a ticker, e.g. NVDA").into()));
            };
            let Some(a) = num(&amount()) else {
                return error.set(Some(tr("Enter the amount").into()));
            };
            let Ok(d) = day().trim().parse::<u32>() else {
                return error.set(Some(tr("Pick a day from 1 to 28").into()));
            };
            let start = match &existing {
                Some(p) => p.start.clone(),
                None => today().await.unwrap_or_default(),
            };
            let plan = DcaPlan {
                id: existing.as_ref().map_or_else(Uuid::new_v4, |p| p.id),
                portfolio_id,
                ticker: t,
                amount: a,
                currency: currency(),
                day: d,
                active: existing.as_ref().is_none_or(|p| p.active),
                start,
                last_done: existing.as_ref().and_then(|p| p.last_done.clone()),
            };
            busy.set(true);
            match api::save_dca_plan(plan).await {
                Ok(()) => {
                    refresh.reload();
                    on_close.call(());
                }
                Err(e) => {
                    busy.set(false);
                    error.set(Some(message(e)));
                }
            }
        }
    };

    rsx! {
        Modal { title: if is_new { tr("New monthly plan") } else { tr("Edit monthly plan") }, on_close,
            form { class: "grid gap-4", onsubmit: move |e| e.prevent_default(),
                div { class: "grid grid-cols-2 gap-3",
                    Field { label: tr("Portfolio"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| target.set(Uuid::parse_str(&e.value()).ok()),
                            for (pid, name) in choices.iter().cloned() {
                                option { value: "{pid}", selected: target() == Some(pid), "{name}" }
                            }
                        }
                    }
                    Field { label: tr("Ticker"),
                        input {
                            class: "{INPUT} uppercase",
                            placeholder: "e.g. VOO",
                            value: "{ticker}",
                            oninput: move |e| ticker.set(e.value()),
                            // The plan is in the asset's own currency.
                            onchange: move |e| async move {
                                let Ok(t) = TickerSymbol::new(&e.value()) else { return };
                                if let Ok(q) = api::quote::quote::get_native_quote(t).await {
                                    currency.set(q.currency);
                                }
                            },
                        }
                    }
                    Field { label: trf("Amount each month ({})", &[&currency()]),
                        input { class: INPUT, inputmode: "decimal", value: "{amount}", oninput: move |e| amount.set(e.value()) }
                    }
                    Field { label: tr("Day of the month (1–28)"),
                        input { class: INPUT, inputmode: "numeric", value: "{day}", oninput: move |e| day.set(e.value()) }
                    }
                }
                if let Some(e) = error() {
                    p { class: "rounded-xl bg-ctp-red/10 px-3 py-2 text-sm text-ctp-red", "{e}" }
                }
                div { class: "flex justify-end gap-2",
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                    ActionButton { label: tr("Save"), disabled: busy(), onclick: submit }
                }
            }
        }
    }
}
