//! Your thesis on each holding of each portfolio: why you own it, what
//! would make you sell, a target, conviction, a review date and a journal.
//! AI assistants can read these and add to them through the connector
//! (Settings), so they're memory for both of you.

use crate::i18n::{tr, trf};
use crate::{
    app::DataRefresh,
    components::card::{ActionButton, ButtonTone, Card, Chevron, Field, Segmented, ToggleButton, INPUT},
    format::fmt_usd,
    hooks::scoped_data,
    page::GhostButton,
};
use dioxus::prelude::*;
use dtos::{
    compute_positions,
    portfolio::GetDashBoardResponse,
    thesis::{Author, Thesis, ThesisDraft, ThesisStatus},
};
use rust_decimal::Decimal;
use std::{collections::HashMap, str::FromStr};
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

const BADGE: &str = "rounded-full px-2 py-0.5 text-xs font-medium";

/// Bumped after a change, to reload the theses on the page.
#[derive(Clone, Copy)]
struct ThesisReload(Signal<u32>);

impl ThesisReload {
    fn bump(mut self) {
        self.0 += 1;
    }
}

/// Every thesis; reloads after changes made on this page.
fn use_theses() -> Resource<Result<Vec<Thesis>, String>> {
    let refresh = use_context::<DataRefresh>();
    let reload = use_context_provider(|| ThesisReload(Signal::new(0)));
    use_resource(move || async move {
        let _ = (refresh.0(), reload.0());
        api::get_theses().await.map_err(error_text)
    })
}

/// Today as YYYY-MM-DD, on this device ("" until known).
fn use_today() -> String {
    let today = use_resource(|| async { crate::notify::today().await.unwrap_or_default() });
    let today = today.read().clone().unwrap_or_default();
    today
}

fn error_text(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    }
}

/// Portfolio tab: a section per portfolio in view, a row per holding.
#[component]
pub fn ThesisTab(portfolio: Option<Uuid>) -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let theses = use_theses();
    let today = use_today();
    let connected = use_resource(|| async { api::get_connector_key().await.ok().flatten().is_some() });

    let sections: Vec<(Uuid, String, Vec<TickerSymbol>)> = {
        let data = data.read();
        data.portfolios
            .iter()
            .filter(|p| portfolio.is_none_or(|id| id == p.id))
            .map(|p| {
                // From the trades alone: price moves don't change what's held.
                let mut held = compute_positions(&scoped_data(&data, Some(&p.id.to_string())), &HashMap::new());
                held.sort_by_key(|h| std::cmp::Reverse(h.cost_basis()));
                (p.id, p.name.clone(), held.into_iter().map(|h| h.ticker).collect())
            })
            .collect()
    };

    let body = match &*theses.read() {
        None => rsx! {
            Card { title: tr("Theses"), p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} } }
        },
        Some(Err(e)) => rsx! {
            Card { title: tr("Theses"), p { class: "text-sm text-ctp-red", "{e}" } }
        },
        Some(Ok(all)) => rsx! {
            for (id, name, held) in sections {
                PortfolioTheses {
                    key: "{id}",
                    portfolio: id,
                    name,
                    held,
                    theses: all.iter().filter(|t| t.portfolio_id == id).cloned().collect::<Vec<_>>(),
                    today: today.clone(),
                }
            }
        },
    };
    rsx! {
        div { class: "flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60 px-4 py-3 text-sm",
            span { class: "text-ctp-subtext0",
                if connected.read().unwrap_or(false) {
                    {tr("An AI assistant is connected: it reads these before discussing a holding and can add what it learns to the journal.")}
                } else {
                    {tr("Write down why you own each stock. Connect an AI assistant and it can remember these too.")}
                }
            }
            if !connected.read().unwrap_or(true) {
                GhostButton { label: tr("Connect an AI assistant"), onclick: move |_| { navigator().push("/settings"); } }
            }
        }
        {body}
    }
}

#[component]
fn PortfolioTheses(
    portfolio: Uuid,
    name: String,
    held: Vec<TickerSymbol>,
    theses: Vec<Thesis>,
    today: String,
) -> Element {
    let find = |t: &TickerSymbol| theses.iter().find(|x| x.ticker == *t).cloned();
    let written = held
        .iter()
        .filter(|t| find(t).is_some_and(|x| !x.draft.thesis.is_empty()))
        .count();
    let due = theses.iter().filter(|t| t.review_due(&today)).count();
    let sold: Vec<Thesis> = theses.iter().filter(|t| !held.contains(&t.ticker)).cloned().collect();
    let mut subtitle = trf("{} of {} holdings have a thesis", &[&written, &held.len()]);
    if due > 0 {
        subtitle = format!("{subtitle} · {}", trf("{} due for review", &[&due]));
    }
    rsx! {
        Card { title: name, subtitle, flush: true,
            div { class: "divide-y divide-ctp-surface0/60 border-t border-ctp-surface0/60",
                for t in held.iter().cloned() {
                    ThesisRow { key: "{t}", portfolio, thesis: find(&t), ticker: t.clone(), today: today.clone() }
                }
                for th in sold {
                    ThesisRow {
                        key: "sold-{th.ticker}",
                        portfolio,
                        ticker: th.ticker.clone(),
                        thesis: Some(th),
                        today: today.clone(),
                        held: false,
                    }
                }
            }
            if held.is_empty() && theses.is_empty() {
                p { class: "px-6 py-5 text-sm text-ctp-subtext0", {tr("Nothing held in this portfolio yet.")} }
            }
        }
    }
}

/// Stock page: your thesis on this stock in each portfolio that holds it
/// or has one. Nothing if neither.
#[component]
pub(crate) fn StockTheses(ticker: TickerSymbol) -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let theses = use_theses();
    let today = use_today();
    let Some(Ok(all)) = &*theses.read() else {
        return rsx! {};
    };
    let rows: Vec<(Uuid, String, bool, Option<Thesis>)> = data
        .read()
        .portfolios
        .iter()
        .filter_map(|p| {
            let held = p.assets.iter().any(|a| a.ticker_symbol == ticker);
            let thesis = all.iter().find(|t| t.portfolio_id == p.id && t.ticker == ticker).cloned();
            (held || thesis.is_some()).then(|| (p.id, p.name.clone(), held, thesis))
        })
        .collect();
    if rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        Card {
            title: tr("Your thesis"),
            subtitle: tr("One per portfolio. Connected AI assistants can read these and add to the journal.").to_string(),
            flush: true,
            div { class: "divide-y divide-ctp-surface0/60 border-t border-ctp-surface0/60",
                for (id, name, held, thesis) in rows {
                    ThesisRow { key: "{id}", portfolio: id, ticker: ticker.clone(), thesis, today: today.clone(), held, label: name }
                }
            }
        }
    }
}

/// One holding: a summary line that opens the editor.
#[component]
fn ThesisRow(
    portfolio: Uuid,
    ticker: TickerSymbol,
    thesis: Option<Thesis>,
    today: String,
    #[props(default = true)] held: bool,
    /// Shown instead of the ticker (the portfolio's name on the stock page).
    #[props(default)]
    label: Option<String>,
) -> Element {
    let mut open = use_signal(|| false);
    let draft = thesis.as_ref().map(|t| t.draft.clone()).unwrap_or_default();
    let due = thesis.as_ref().is_some_and(|t| t.review_due(&today));
    let by_ai = thesis.as_ref().is_some_and(|t| t.updated_by == Author::Ai || t.log.first().is_some_and(|e| e.author == Author::Ai));
    let summary = draft.thesis.lines().next().unwrap_or_default().to_string();
    let label = label.unwrap_or_else(|| ticker.to_string());
    rsx! {
        div {
            button {
                class: "flex w-full items-center gap-4 px-6 py-3.5 text-left cursor-pointer transition-colors hover:bg-ctp-surface0/30",
                aria_expanded: open(),
                onclick: move |_| open.toggle(),
                span { class: "w-24 shrink-0 truncate font-semibold text-ctp-text", "{label}" }
                span { class: "block min-w-0 flex-1",
                    span { class: "flex flex-wrap items-center gap-1.5",
                        if thesis.is_some() {
                            StatusBadge { status: draft.status }
                        }
                        if let Some(c) = draft.conviction {
                            Conviction { value: c }
                        }
                        if let Some(p) = draft.target_price {
                            span { class: "text-xs tabular-nums text-ctp-subtext0", {trf("Target {}", &[&fmt_usd(p, 2)])} }
                        }
                        if due {
                            span { class: "{BADGE} bg-ctp-peach/15 text-ctp-peach", {tr("Review due")} }
                        }
                        if !held {
                            span { class: "{BADGE} bg-ctp-surface0 text-ctp-subtext0", {tr("Not held")} }
                        }
                        if by_ai {
                            span { class: "{BADGE} bg-ctp-mauve/15 text-ctp-mauve", title: tr("Last change was by AI"), "AI" }
                        }
                    }
                    if summary.is_empty() {
                        span { class: "mt-0.5 block truncate text-sm text-ctp-overlay1", {tr("No thesis yet. Why do you own it?")} }
                    } else {
                        span { class: "mt-0.5 block truncate text-sm text-ctp-subtext1", "{summary}" }
                    }
                }
                Chevron { open: open() }
            }
            if open() {
                ThesisEditor { portfolio, ticker, thesis }
            }
        }
    }
}

#[component]
fn StatusBadge(status: ThesisStatus) -> Element {
    let color = match status {
        ThesisStatus::OnTrack => "bg-ctp-green/15 text-ctp-green",
        ThesisStatus::AtRisk => "bg-ctp-yellow/15 text-ctp-yellow",
        ThesisStatus::Broken => "bg-ctp-red/15 text-ctp-red",
    };
    rsx! {
        span { class: "{BADGE} {color}", {tr(status.label())} }
    }
}

/// e.g. ●●●○○
#[component]
fn Conviction(value: u8) -> Element {
    let n = value.min(5) as usize;
    rsx! {
        span { class: "text-xs tracking-wider", title: trf("Conviction {} of 5", &[&value]),
            span { class: "text-ctp-mauve", {"●".repeat(n)} }
            span { class: "text-ctp-surface2", {"●".repeat(5 - n)} }
        }
    }
}

#[component]
fn ThesisEditor(portfolio: Uuid, ticker: TickerSymbol, thesis: Option<Thesis>) -> Element {
    let reload = use_context::<ThesisReload>();
    let saved = thesis.as_ref().map(|t| t.draft.clone()).unwrap_or_default();
    let mut text = use_signal(|| saved.thesis.clone());
    let mut exit_if = use_signal(|| saved.exit_if.clone());
    let mut target = use_signal(|| saved.target_price.map(|p| p.normalize().to_string()).unwrap_or_default());
    let mut conviction = use_signal(|| saved.conviction);
    let mut review_on = use_signal(|| saved.review_on.clone().unwrap_or_default());
    let mut status = use_signal(|| saved.status);
    let mut note = use_signal(|| None::<Result<&'static str, String>>);
    let mut confirm_delete = use_signal(|| false);

    let t = ticker.clone();
    let save = move |_| {
        let t = t.clone();
        async move {
            let target_price = match target().trim() {
                "" => None,
                s => match Decimal::from_str(s) {
                    Ok(p) => Some(p),
                    Err(_) => return note.set(Some(Err(tr("Enter a number for the target price").into()))),
                },
            };
            let draft = ThesisDraft {
                thesis: text(),
                exit_if: exit_if(),
                target_price,
                conviction: conviction(),
                review_on: Some(review_on()).filter(|d| !d.is_empty()),
                status: status(),
            };
            match api::save_thesis(portfolio, t, draft).await {
                Ok(()) => {
                    note.set(Some(Ok(tr("Saved"))));
                    reload.bump();
                }
                Err(e) => note.set(Some(Err(error_text(e)))),
            }
        }
    };
    let t = ticker.clone();
    let delete = move |_| {
        let t = t.clone();
        async move {
            match api::delete_thesis(portfolio, t).await {
                Ok(()) => reload.bump(),
                Err(e) => note.set(Some(Err(error_text(e)))),
            }
            confirm_delete.set(false);
        }
    };

    rsx! {
        div { class: "grid gap-4 border-t border-ctp-surface0/40 bg-ctp-crust/20 px-6 py-5",
            Field { label: tr("Why you own it"),
                textarea {
                    class: "{INPUT} min-h-28 resize-y",
                    placeholder: tr("e.g. The leader in AI chips; data-centre sales keep growing faster than the market expects."),
                    value: "{text}",
                    oninput: move |e| { text.set(e.value()); note.set(None); },
                }
            }
            Field { label: tr("Sell if…"), hint: tr("What would prove you wrong. Decide it now, while you're calm."),
                textarea {
                    class: "{INPUT} min-h-16 resize-y",
                    placeholder: tr("e.g. Two quarters of falling data-centre revenue, or it goes above 35% of the portfolio."),
                    value: "{exit_if}",
                    oninput: move |e| { exit_if.set(e.value()); note.set(None); },
                }
            }
            div { class: "grid gap-4 sm:grid-cols-3",
                Field { label: tr("Target price ($)"),
                    input {
                        class: "{INPUT} tabular-nums",
                        inputmode: "decimal",
                        placeholder: "250",
                        value: "{target}",
                        oninput: move |e| { target.set(e.value()); note.set(None); },
                    }
                }
                Field { label: tr("Review on"),
                    input {
                        class: INPUT,
                        r#type: "date",
                        value: "{review_on}",
                        oninput: move |e| { review_on.set(e.value()); note.set(None); },
                    }
                }
                div {
                    span { class: "mb-1.5 block text-xs text-ctp-subtext0", {tr("Conviction")} }
                    div { class: "flex gap-1",
                        for n in 1..=5u8 {
                            button {
                                key: "{n}",
                                class: if conviction().is_some_and(|c| n <= c) {
                                    "h-9 w-9 rounded-full bg-ctp-mauve/20 text-sm font-semibold text-ctp-mauve cursor-pointer"
                                } else {
                                    "h-9 w-9 rounded-full border border-ctp-surface0 text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1"
                                },
                                aria_pressed: conviction() == Some(n),
                                onclick: move |_| {
                                    conviction.set(if conviction() == Some(n) { None } else { Some(n) });
                                    note.set(None);
                                },
                                "{n}"
                            }
                        }
                    }
                }
            }
            div {
                span { class: "mb-1.5 block text-xs text-ctp-subtext0", {tr("How it's going")} }
                Segmented {
                    for s in ThesisStatus::ALL {
                        ToggleButton {
                            key: "{s:?}",
                            label: tr(s.label()),
                            active: status() == s,
                            onclick: move |_| { status.set(s); note.set(None); },
                        }
                    }
                }
            }
            div { class: "flex flex-wrap items-center gap-3",
                ActionButton { label: tr("Save"), onclick: save }
                if thesis.is_some() {
                    if confirm_delete() {
                        span { class: "text-sm text-ctp-peach", {tr("Delete this thesis and its journal?")} }
                        ActionButton { label: tr("Delete"), tone: ButtonTone::Danger, onclick: delete }
                        ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| confirm_delete.set(false) }
                    } else {
                        ActionButton { label: tr("Delete thesis"), tone: ButtonTone::Quiet, onclick: move |_| confirm_delete.set(true) }
                    }
                }
                match note() {
                    Some(Ok(m)) => rsx! { span { class: "text-sm text-ctp-green", "{m}" } },
                    Some(Err(m)) => rsx! { span { class: "text-sm text-ctp-red", "{m}" } },
                    None => rsx! {},
                }
            }
            Journal { portfolio, ticker, thesis }
        }
    }
}

/// Dated notes on the thesis, newest first, by you or by AI.
#[component]
fn Journal(portfolio: Uuid, ticker: TickerSymbol, thesis: Option<Thesis>) -> Element {
    let reload = use_context::<ThesisReload>();
    let mut entry = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let log = thesis.map(|t| t.log).unwrap_or_default();
    let add = move |ticker: TickerSymbol| {
        if entry().trim().is_empty() {
            return;
        }
        spawn(async move {
            match api::add_thesis_entry(portfolio, ticker, entry()).await {
                Ok(_) => {
                    entry.set(String::new());
                    error.set(None);
                    reload.bump();
                }
                Err(e) => error.set(Some(error_text(e))),
            }
        });
    };
    let (t1, t2) = (ticker.clone(), ticker.clone());
    rsx! {
        div { class: "border-t border-ctp-surface0/40 pt-4",
            div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Journal")} }
            div { class: "flex gap-2",
                input {
                    class: INPUT,
                    placeholder: tr("What happened, and what it means for the thesis ↵"),
                    value: "{entry}",
                    oninput: move |e| entry.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            add(t1.clone());
                        }
                    },
                }
                ActionButton { label: tr("Add"), tone: ButtonTone::Quiet, onclick: move |_| add(t2.clone()) }
            }
            if let Some(e) = error() {
                p { class: "mt-2 text-sm text-ctp-red", "{e}" }
            }
            if !log.is_empty() {
                ul { class: "mt-3 grid gap-2.5",
                    for e in log {
                        li { key: "{e.id}", class: "group flex items-start gap-3 text-sm",
                            span { class: "w-20 shrink-0 pt-0.5 text-xs tabular-nums text-ctp-overlay1", title: "{e.created_at} UTC",
                                {e.created_at.get(..10).unwrap_or(&e.created_at).to_string()}
                            }
                            if e.author == Author::Ai {
                                span { class: "{BADGE} shrink-0 bg-ctp-mauve/15 text-ctp-mauve", "AI" }
                            } else {
                                span { class: "{BADGE} shrink-0 bg-ctp-surface0 text-ctp-subtext0", {tr("You")} }
                            }
                            span { class: "min-w-0 flex-1 whitespace-pre-line text-ctp-subtext1", "{e.text}" }
                            button {
                                class: "rounded-full px-2 py-0.5 text-ctp-subtext0 opacity-40 cursor-pointer transition-opacity group-hover:opacity-100 focus-visible:opacity-100 hover:bg-ctp-surface0 hover:text-ctp-red",
                                "aria-label": tr("Delete entry"),
                                onclick: move |_| async move {
                                    match api::delete_thesis_entry(e.id).await {
                                        Ok(()) => reload.bump(),
                                        Err(err) => error.set(Some(error_text(err))),
                                    }
                                },
                                "×"
                            }
                        }
                    }
                }
            }
        }
    }
}
