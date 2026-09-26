//! Settings: display currency, analysis assumptions, and data export.

use crate::i18n::tr;
use crate::{
    app::{AppSettings, DataRefresh},
    components::card::{ActionButton, Card, Field, Segmented, Stepper, ToggleButton, INPUT},
    editors::{Dialog, Dialogs},
    files::{print_report, ExportButtons},
    hooks::{use_portfolio, PortfolioState},
    page::{GhostButton, Page},
};
use dioxus::prelude::*;
use dtos::{
    csv_export::{holdings_csv, transactions_csv},
    portfolio::GetDashBoardResponse,
    settings::{Settings, CURRENCIES},
};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;

const BENCHMARKS: [(&str, &str); 3] = [
    ("^GSPC", "S&P 500"),
    ("^NDX", "Nasdaq 100"),
    ("^DJI", "Dow Jones"),
];

#[component]
pub fn SettingsPage() -> Element {
    let AppSettings(saved) = use_context::<AppSettings>();
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let PortfolioState { positions, .. } = use_portfolio(None);

    let mut draft = use_signal(move || saved.peek().clone());
    let mut risk_free = use_signal(move || saved.peek().risk_free.normalize().to_string());
    let mut assumed = use_signal(move || saved.peek().assumed_return.normalize().to_string());
    let mut custom_benchmark = use_signal(String::new);
    let mut status = use_signal(|| None::<Result<(), String>>);
    let mut switching = use_signal(|| false);
    let mut currency_error = use_signal(|| None::<String>);

    let save = move |_| async move {
        let mut s: Settings = draft();
        let (Ok(rf), Ok(ar)) = (
            Decimal::from_str(risk_free().trim()),
            Decimal::from_str(assumed().trim()),
        ) else {
            return status.set(Some(Err("Enter numbers for the rates".into())));
        };
        s.risk_free = rf;
        s.assumed_return = ar;
        // The currency is saved as soon as it's picked.
        s.currency = saved.peek().currency.clone();
        match api::save_settings(s).await {
            Ok(()) => {
                status.set(Some(Ok(())));
                refresh.reload();
            }
            Err(ServerFnError::ServerError { message, .. }) => status.set(Some(Err(message))),
            Err(e) => status.set(Some(Err(e.to_string()))),
        }
    };

    let names: Vec<(uuid::Uuid, String)> = data
        .read()
        .portfolios
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    let tx_csv = transactions_csv(&data.read().transactions, |t| {
        names
            .iter()
            .find(|(id, _)| *id == t.portfolio_id)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    });
    let pos_csv = holdings_csv(&positions);
    let benchmark = draft.read().benchmark.clone();
    let is_preset = BENCHMARKS.iter().any(|(t, _)| *t == benchmark.as_str());

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("Settings")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0", {tr("How amounts are shown, and the assumptions behind the analysis.")} }
            }
            div { class: "mt-10 grid gap-5 motion-safe:animate-rise",
                Card { title: tr("Display currency"), subtitle: tr("Applies instantly, converted at today's rate. What you enter and store stays in USD.").to_string(),
                    div { class: "flex flex-wrap gap-2",
                        for (code, symbol) in CURRENCIES {
                            button {
                                key: "{code}",
                                class: if saved.read().currency == code {
                                    "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3.5 py-1.5 text-sm font-medium text-ctp-text cursor-pointer"
                                } else {
                                    "rounded-full border border-ctp-surface0 px-3.5 py-1.5 text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
                                },
                                disabled: switching(),
                                onclick: move |_| async move {
                                    switching.set(true);
                                    currency_error.set(AppSettings(saved).change_currency(code).await.err());
                                    switching.set(false);
                                },
                                "{symbol.trim()} {code}"
                            }
                        }
                    }
                    if let Some(message) = currency_error() {
                        p { class: "mt-3 text-sm text-ctp-red", "{message}" }
                    }
                }
                Card { title: tr("Analysis"),
                    div { class: "grid gap-5 md:grid-cols-3",
                        div {
                            div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Risk-free rate")} }
                            Stepper { aria_label: "Risk-free rate", value: risk_free(), step: 0.25, suffix: "%", width: "w-14", on_change: move |v| risk_free.set(v) }
                            p { class: "mt-2 text-xs text-ctp-overlay1", {tr("Used for Sharpe, alpha and CAPM. Roughly the 3-month T-bill yield.")} }
                        }
                        div {
                            div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Benchmark")} }
                            Segmented {
                                for (ticker, name) in BENCHMARKS {
                                    ToggleButton {
                                        key: "{ticker}",
                                        label: name,
                                        active: benchmark.as_str() == ticker,
                                        onclick: move |_| {
                                            if let Ok(t) = TickerSymbol::new(ticker) {
                                                draft.write().benchmark = t;
                                            }
                                        },
                                    }
                                }
                            }
                            form {
                                class: "mt-2",
                                onsubmit: move |e| {
                                    e.prevent_default();
                                    if let Ok(t) = TickerSymbol::new(&custom_benchmark()) {
                                        draft.write().benchmark = t;
                                    }
                                },
                                input {
                                    class: "w-full rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm uppercase text-ctp-text placeholder:normal-case placeholder:text-ctp-overlay1 outline-none focus:border-ctp-mauve",
                                    placeholder: if is_preset { "…or any ticker, e.g. VT ↵".to_string() } else { format!("Using {benchmark}") },
                                    value: "{custom_benchmark}",
                                    oninput: move |e| custom_benchmark.set(e.value()),
                                }
                            }
                            p { class: "mt-2 text-xs text-ctp-overlay1", {tr("What your portfolio is compared against in Performance and Risk.")} }
                        }
                        div {
                            div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Assumed yearly return")} }
                            Stepper { aria_label: "Assumed return", value: assumed(), step: 0.5, suffix: "%", width: "w-14", on_change: move |v| assumed.set(v) }
                            p { class: "mt-2 text-xs text-ctp-overlay1", {tr("Used to project your goals.")} }
                        }
                    }
                }
                div { class: "flex items-center justify-end gap-3",
                    if let Some(result) = status() {
                        span { class: if result.is_ok() { "text-sm text-ctp-green" } else { "text-sm text-ctp-red" },
                            {result.as_ref().err().cloned().unwrap_or_else(|| "Saved.".into())}
                        }
                    }
                    ActionButton { label: tr("Save settings"), onclick: save }
                }
                Appearance {}
                crate::auth::SecurityCard {}
                NotificationSettings {}
                Card { title: tr("Your data"),
                    div { class: "grid gap-4 text-sm",
                        DataRow { label: tr("Transactions"), hint: tr("Every trade, dividend and cash movement. Re-importable."),
                            ExportButtons { filename: "akhsakov-transactions.csv", csv: tx_csv }
                        }
                        DataRow { label: tr("Holdings"), hint: tr("Current positions at live prices."),
                            ExportButtons { filename: "akhsakov-holdings.csv", csv: pos_csv }
                        }
                        DataRow { label: tr("Import"), hint: tr("Add transactions from a broker's CSV export."),
                            GhostButton { label: tr("Import CSV"), onclick: move |_| dialogs.open(Dialog::Import(None)) }
                        }
                        DataRow { label: tr("Full backup"), hint: tr("Everything — portfolios, trades, watchlists, notes, settings and accounts — in one file."),
                            BackupButtons {}
                        }
                        DataRow { label: tr("Report"), hint: tr("Print the current page, or save it as a PDF from the print dialog."),
                            GhostButton { label: tr("Print / PDF"), onclick: move |_| print_report() }
                        }
                        p { class: "text-xs text-ctp-overlay1",
                            {tr("Your data is stored in akhsakov_finance.db where the app runs (set AKHSAKOV_DB to keep it elsewhere). Point the phone app at the same server to see the same data everywhere.")}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DataRow(label: String, hint: String, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-wrap items-center justify-between gap-3 border-t border-ctp-surface0/60 pt-4 first:border-t-0 first:pt-0",
            div {
                div { class: "font-medium text-ctp-text", "{label}" }
                div { class: "text-xs text-ctp-subtext0", "{hint}" }
            }
            {children}
        }
    }
}

/// Where fired alerts are pushed: ntfy (phone app, no account), Telegram or
/// any webhook (Slack, Discord, …).
#[component]
fn NotificationSettings() -> Element {
    let mut channels = use_signal(dtos::notifications::Channels::default);
    let mut loaded = use_signal(|| false);
    let mut status = use_signal(|| None::<Result<String, String>>);
    let mut tests = use_signal(Vec::<(String, Option<String>)>::new);
    use_future(move || async move {
        if let Ok(c) = api::get_channels().await {
            channels.set(c);
        }
        loaded.set(true);
    });
    let message = |e: ServerFnError| match e {
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    };
    let save = move |_| async move {
        status.set(Some(match api::save_channels(channels()).await {
            Ok(()) => Ok("Saved.".into()),
            Err(e) => Err(message(e)),
        }));
    };
    let test = move |_| async move {
        tests.set(vec![]);
        match api::test_channels(channels()).await {
            Ok(results) => {
                status.set(None);
                tests.set(results);
            }
            Err(e) => status.set(Some(Err(message(e)))),
        }
    };
    let c = channels();
    let field = |label: &'static str, hint: &'static str, value: String, placeholder: &'static str, set: fn(&mut dtos::notifications::Channels, String)| {
        rsx! {
            Field { label, hint,
                input {
                    class: INPUT,
                    placeholder,
                    value: "{value}",
                    oninput: move |e| { channels.with_mut(|c| set(c, e.value())); status.set(None); },
                }
            }
        }
    };
    rsx! {
        Card {
            title: tr("Alert notifications"),
            subtitle: tr("Alerts are checked on the server every minute, even when the app is closed. Get them on your phone or in chat.").to_string(),
            if !loaded() {
                p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
            } else {
                div { class: "grid gap-4 md:grid-cols-2",
                    {field("ntfy topic", "Install the ntfy app and subscribe to this topic. Pick something hard to guess.", c.ntfy_topic.clone(), "e.g. akhsakov-7f3k2", |c, v| c.ntfy_topic = v)}
                    {field("ntfy server", "Leave as is unless you host your own.", c.ntfy_server.clone(), "https://ntfy.sh", |c, v| c.ntfy_server = v)}
                    {field("Telegram bot token", "From @BotFather.", c.telegram_token.clone(), "123456:ABC…", |c, v| c.telegram_token = v)}
                    {field("Telegram chat id", "Message your bot, then look it up with @userinfobot.", c.telegram_chat_id.clone(), "e.g. 123456789", |c, v| c.telegram_chat_id = v)}
                    div { class: "md:col-span-2",
                        {field("Webhook URL", "Slack or Discord incoming webhook, or any URL that accepts JSON.", c.webhook_url.clone(), "https://hooks.slack.com/…", |c, v| c.webhook_url = v)}
                    }
                }
                div { class: "mt-4 flex flex-wrap items-center gap-3",
                    ActionButton { label: tr("Save"), onclick: save }
                    GhostButton { label: tr("Send a test"), onclick: test }
                    match status() {
                        Some(Ok(m)) => rsx! { span { class: "text-sm text-ctp-green", "{m}" } },
                        Some(Err(m)) => rsx! { span { class: "text-sm text-ctp-red", "{m}" } },
                        None => rsx! {},
                    }
                    for (name, error) in tests() {
                        span {
                            key: "{name}",
                            class: if error.is_none() { "text-sm text-ctp-green" } else { "text-sm text-ctp-red" },
                            {match &error { None => format!("{name}: sent ✓"), Some(e) => format!("{name}: {e}") }}
                        }
                    }
                }
                p { class: "mt-3 text-xs text-ctp-overlay1",
                    {tr("Tokens are stored in your database file, on the machine the app runs on.")}
                }
            }
        }
    }
}

/// Download a backup file, or restore one (replacing everything).
#[component]
fn BackupButtons() -> Element {
    let mut busy = use_signal(|| false);
    // A picked file waiting for confirmation: (name, base64).
    let mut pending = use_signal(|| None::<(String, String)>);
    let mut note = use_signal(|| None::<Result<String, String>>);
    let download = move |_| async move {
        busy.set(true);
        match api::download_backup().await {
            Ok(data) => {
                let today = crate::notify::today().await.unwrap_or_default();
                crate::files::download_base64(&format!("akhsakov-backup-{today}.db"), "application/vnd.sqlite3", &data);
                note.set(None);
            }
            Err(e) => note.set(Some(Err(e.to_string()))),
        }
        busy.set(false);
    };
    let restore = move |_| async move {
        let Some((_, data)) = pending() else { return };
        busy.set(true);
        match api::restore_backup(data).await {
            Ok(()) => {
                pending.set(None);
                note.set(Some(Ok("Restored. Reloading…".into())));
                document::eval("setTimeout(() => location.reload(), 800)");
            }
            Err(ServerFnError::ServerError { message, .. }) => note.set(Some(Err(message))),
            Err(e) => note.set(Some(Err(e.to_string()))),
        }
        busy.set(false);
    };
    rsx! {
        div { class: "flex flex-wrap items-center justify-end gap-2",
            if let Some((name, _)) = pending() {
                span { class: "text-xs text-ctp-peach", "Replace all your data with {name}?" }
                GhostButton { label: tr("Yes, restore"), onclick: restore }
                GhostButton { label: tr("Cancel"), onclick: move |_| pending.set(None) }
            } else {
                GhostButton { label: if busy() { "…" } else { tr("⇩ Download") }, onclick: download }
                label { class: "rounded-full border border-ctp-surface1 px-3.5 py-1.5 text-xs font-medium text-ctp-subtext1 cursor-pointer transition-colors hover:border-ctp-mauve hover:text-ctp-text",
                    {tr("Restore…")}
                    input {
                        class: "hidden",
                        r#type: "file",
                        accept: ".db,.sqlite,application/vnd.sqlite3,application/octet-stream",
                        onchange: move |e| async move {
                            use base64::Engine;
                            if let Some(file) = e.files().into_iter().next() {
                                match file.read_bytes().await {
                                    Ok(bytes) => pending.set(Some((file.name(), base64::engine::general_purpose::STANDARD.encode(bytes)))),
                                    Err(_) => note.set(Some(Err("Couldn't read that file".into()))),
                                }
                            }
                        },
                    }
                }
            }
            match note() {
                Some(Ok(m)) => rsx! { span { class: "text-xs text-ctp-green", "{m}" } },
                Some(Err(m)) => rsx! { span { class: "text-xs text-ctp-red", "{m}" } },
                None => rsx! {},
            }
        }
    }
}

/// Theme and language, remembered on this device.
#[component]
fn Appearance() -> Element {
    use crate::{i18n::{self, Lang}, theme::{self, Theme}};
    let chip = |on: bool| {
        if on {
            "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3.5 py-1.5 text-sm font-medium text-ctp-text cursor-pointer"
        } else {
            "rounded-full border border-ctp-surface0 px-3.5 py-1.5 text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
        }
    };
    rsx! {
        Card { title: tr("Appearance"), subtitle: tr("Saved on this device.").to_string(),
            div { class: "grid gap-5 md:grid-cols-2",
                div {
                    div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Theme")} }
                    div { class: "flex flex-wrap gap-2",
                        for th in Theme::ALL {
                            button { key: "{th:?}", class: chip(theme::current() == th), onclick: move |_| theme::set_theme(th), {tr(th.label())} }
                        }
                    }
                }
                div {
                    div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Language")} }
                    div { class: "flex flex-wrap gap-2",
                        for lang in Lang::ALL {
                            button { key: "{lang:?}", class: chip(i18n::current() == lang), onclick: move |_| i18n::set_language(lang), "{lang.label()}" }
                        }
                    }
                }
            }
        }
    }
}
