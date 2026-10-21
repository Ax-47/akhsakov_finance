//! Settings: display currency, analysis assumptions, and data export.

use crate::i18n::{tr, trf};
use crate::{
    app::{AppSettings, DataRefresh},
    components::card::{ActionButton, Card, Field, Segmented, Stepper, ToggleButton, INPUT},
    editors::{Dialog, Dialogs},
    files::{print_report, ExportButtons},
    hooks::use_portfolio_memo,
    page::{GhostButton, Page},
};
use dioxus::prelude::*;
use dtos::{
    ai_models::{
        ApiStyle, ModelProfile, ModelProfileSettings, DEFAULT_MODEL_TIMEOUT_SECONDS,
    },
    csv_export::{holdings_csv, transactions_csv},
    portfolio::GetDashBoardResponse,
    mcp::{McpAccessPreset, McpAuditEvent, McpConnection, McpPortfolioScope},
    settings::{Settings, BENCHMARKS, CURRENCIES},
};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;


#[component]
pub fn SettingsPage() -> Element {
    let AppSettings(saved) = use_context::<AppSettings>();
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    // Peeked only when exporting, so prices moving don't re-render the page.
    let portfolio = use_portfolio_memo(None);
    let model_profiles = use_signal(Vec::<ModelProfile>::new);
    use_context_provider(|| model_profiles);

    let mut draft = use_signal(move || saved.peek().clone());
    let mut risk_free = use_signal(move || saved.peek().risk_free.normalize().to_string());
    let mut assumed = use_signal(move || saved.peek().assumed_return.normalize().to_string());
    let mut custom_benchmark = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let toasts = use_context::<crate::notify::Toasts>();
    let mut switching = use_signal(|| false);
    let mut currency_error = use_signal(|| None::<String>);

    let save = move |_| async move {
        let mut s: Settings = draft();
        let (Ok(rf), Ok(ar)) = (
            Decimal::from_str(risk_free().trim()),
            Decimal::from_str(assumed().trim()),
        ) else {
            return error.set(Some(tr("Enter numbers for the rates").into()));
        };
        s.risk_free = rf;
        s.assumed_return = ar;
        // The currency is saved as soon as it's picked.
        s.currency = saved.peek().currency.clone();
        match api::save_settings(s).await {
            Ok(()) => {
                error.set(None);
                toasts.show(tr("Settings saved"), "");
                refresh.reload();
            }
            Err(ServerFnError::ServerError { message, .. }) => error.set(Some(message)),
            Err(e) => error.set(Some(e.to_string())),
        }
    };

    let tx_csv = move || {
        let data = data.peek();
        transactions_csv(&data.transactions, |t| {
            data.portfolios
                .iter()
                .find(|p| p.id == t.portfolio_id)
                .map(|p| p.name.clone())
                .unwrap_or_default()
        })
    };
    let pos_csv = move || holdings_csv(&portfolio.peek().positions);
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
                            Stepper { aria_label: tr("Risk-free rate"), value: risk_free(), step: 0.25, suffix: "%", width: "w-14", on_change: move |v| risk_free.set(v) }
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
                                    placeholder: if is_preset { tr("…or any ticker, e.g. ^SET50.BK ↵").to_string() } else { crate::i18n::trf("Using {}", &[&benchmark]) },
                                    value: "{custom_benchmark}",
                                    oninput: move |e| custom_benchmark.set(e.value()),
                                }
                            }
                            p { class: "mt-2 text-xs text-ctp-overlay1", {tr("What your portfolio is compared against in Performance and Risk.")} }
                            if dtos::settings::is_price_index(benchmark.as_str()) {
                                p { class: "mt-1 text-xs text-ctp-peach", {tr("An index level leaves out dividends, while your holdings' prices include them, so you'll look better than you are. A fund tracking it (e.g. SPY) is a fairer yardstick.")} }
                            }
                        }
                        div {
                            div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Assumed yearly return")} }
                            Stepper { aria_label: tr("Assumed return"), value: assumed(), step: 0.5, suffix: "%", width: "w-14", on_change: move |v| assumed.set(v) }
                            p { class: "mt-2 text-xs text-ctp-overlay1", {tr("Used to project your goals.")} }
                        }
                    }
                }
                div { class: "flex items-center justify-end gap-3",
                    if let Some(message) = error() {
                        span { class: "text-sm text-ctp-red", "{message}" }
                    }
                    ActionButton { label: tr("Save settings"), onclick: save }
                }
                Appearance {}
                crate::server_address::ServerCard {}
                crate::auth::SecurityCard {}
                NotificationSettings {}
                ConnectorCard {}
                ModelProfilesCard {}
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
                            {tr("Your data is stored in akhsakov_finance.db on the computer that runs the server (set AKHSAKOV_DB to keep it elsewhere). Point the phone app at the same server to see the same data everywhere.")}
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
    let toasts = use_context::<crate::notify::Toasts>();
    let save = move |_| async move {
        match api::save_channels(channels()).await {
            Ok(()) => {
                status.set(None);
                toasts.show(tr("Notification settings saved"), "");
            }
            Err(e) => status.set(Some(Err(message(e)))),
        }
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
            Field { label: tr(label), hint: tr(hint),
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
                            {match &error { None => trf("{}: sent ✓", &[&name]), Some(e) => format!("{name}: {e}") }}
                        }
                    }
                }
                div { class: "mt-4", crate::alerts::SystemNotificationsToggle {} }
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
                note.set(Some(Ok(tr("Restored. Reloading…").into())));
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
    use crate::{i18n::{self, Lang}, perf::{self, Effects}, theme::{self, Theme}};
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
                div { class: "md:col-span-2",
                    div { class: "mb-2 text-xs text-ctp-subtext0", {tr("Effects")} }
                    div { class: "flex flex-wrap gap-2",
                        for e in Effects::ALL {
                            button { key: "{e:?}", class: chip(perf::current() == e), onclick: move |_| perf::set_effects(e), {tr(e.label())} }
                        }
                    }
                    p { class: "mt-2 text-xs text-ctp-overlay1",
                        {tr("Lite turns off animations and rolling numbers, and updates live prices every 3 seconds instead of every second. For older or slower computers.")}
                        " "
                        if perf::current() == Effects::Auto {
                            if perf::low_end() {
                                {tr("Auto is using Lite on this device.")}
                            } else {
                                {tr("Auto is using full effects on this device.")}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum McpExample {
    Generic,
    Claude,
    Codex,
    Agy,
    Editors,
    Other,
}

impl McpExample {
    const ALL: [Self; 6] = [Self::Generic, Self::Claude, Self::Codex, Self::Agy, Self::Editors, Self::Other];

    fn label(self) -> &'static str {
        match self {
            Self::Generic => "Generic MCP",
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Agy => "Antigravity / agy",
            Self::Editors => "Cursor / VS Code",
            Self::Other => "Other",
        }
    }
}

/// Provider-neutral MCP access. Named clients below are setup examples only.
#[component]
fn ConnectorCard() -> Element {
    let mut connections = use_signal(Vec::<McpConnection>::new);
    let mut scopes = use_signal(Vec::<McpPortfolioScope>::new);
    let mut loaded = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut address = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut preset = use_signal(|| McpAccessPreset::ReadOnly);
    let mut selected = use_signal(Vec::<Uuid>::new);
    let mut editing = use_signal(|| None::<Uuid>);
    let mut editing_enabled = use_signal(|| true);
    let mut secret = use_signal(|| None::<String>);
    let mut audit = use_signal(Vec::<McpAuditEvent>::new);
    let mut audit_for = use_signal(|| None::<Uuid>);
    let mut example = use_signal(|| McpExample::Generic);
    let example_chip = |on: bool| if on {
        "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3 py-1.5 text-xs font-medium text-ctp-text cursor-pointer"
    } else {
        "rounded-full border border-ctp-surface0 px-3 py-1.5 text-xs text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
    };
    use_future(move || async move {
        match api::get_mcp_connections().await {
            Ok(value) => connections.set(value),
            Err(e) => error.set(Some(e.to_string())),
        }
        if let Ok(value) = api::get_mcp_portfolio_scopes().await { scopes.set(value); }
        loaded.set(true);
        // The desktop app talks to its own server; in a browser, the page's
        // own address is the server.
        let server = dioxus::fullstack::get_server_url();
        address.set(if server.is_empty() {
            document::eval("return window.location.origin;").join::<String>().await.unwrap_or_default()
        } else {
            server.to_string()
        });
    });
    let message = |e: ServerFnError| match e {
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    };
    let save = move |_| async move {
        let result = match editing() {
            Some(id) => api::update_mcp_connection(id, name(), preset(), editing_enabled(), selected()).await.map(|_| None),
            None => api::create_mcp_connection(name(), preset(), selected()).await.map(Some),
        };
        match result {
            Ok(created) => {
                if let Some(created)=created { secret.set(Some(created.token)); }
                if let Ok(value)=api::get_mcp_connections().await { connections.set(value); }
                name.set(String::new()); selected.set(vec![]); editing.set(None);
                error.set(None);
            }
            Err(e) => error.set(Some(message(e))),
        }
    };
    let base = address().trim().trim_end_matches('/').to_string();
    rsx! {
        Card {
            title: tr("MCP connections"),
            subtitle: tr("Give each assistant its own revocable credential, permissions, and portfolio access.").to_string(),
            if !loaded() {
                p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
            } else {
                div { class: "grid gap-4 text-sm",
                    Field {
                        label: tr("Address the MCP client uses"),
                        hint: tr("Cloud clients need an HTTPS address they can reach. Clients on this computer can use the local address."),
                        input {
                            class: INPUT,
                            value: "{address}",
                            oninput: move |e| address.set(e.value()),
                        }
                    }
                    for connection in connections() {
                        div { key: "{connection.id}", class: "rounded-xl border border-ctp-surface0 p-4 grid gap-2",
                            div { class: "flex flex-wrap items-center justify-between gap-2",
                                div {
                                    p { class: "font-medium text-ctp-text", "{connection.name}" }
                                    p { class: "text-xs text-ctp-subtext0", {trf("{} · {} · {} portfolios · last used {}", &[&if connection.enabled { tr("enabled") } else { tr("disabled") }, &connection.preset.key(), &connection.portfolio_ids.len(), &connection.last_used_at.as_deref().map_or(tr("never").to_string(), crate::format::fmt_when)])} }
                                }
                                div { class: "flex flex-wrap gap-2",
                                    GhostButton { label: tr("Edit"), onclick: { let c=connection.clone(); move |_| { editing.set(Some(c.id));editing_enabled.set(c.enabled);name.set(c.name.clone());preset.set(c.preset);selected.set(c.portfolio_ids.clone()); } } }
                                    GhostButton { label: if connection.enabled { tr("Disable") } else { tr("Enable") }, onclick: { let c=connection.clone(); move |_| { let c=c.clone(); async move { let _=api::update_mcp_connection(c.id,c.name,c.preset,!c.enabled,c.portfolio_ids).await; if let Ok(v)=api::get_mcp_connections().await{connections.set(v);} } } } }
                                    crate::page::ConfirmButton {
                                        label: tr("Rotate secret"),
                                        warning: tr("The old secret stops working at once; apps using it need the new one. Continue?"),
                                        onconfirm: { let id=connection.id; move |_| async move { match api::rotate_mcp_connection(id).await { Ok(v)=>secret.set(Some(v.token)),Err(e)=>error.set(Some(message(e))) } } },
                                    }
                                    GhostButton { label: tr("Audit"), onclick: { let id=connection.id; move |_| async move { if let Ok(v)=api::get_mcp_audit_events(id).await {audit.set(v);audit_for.set(Some(id));} } } }
                                    crate::page::ConfirmButton {
                                        label: tr("Delete"),
                                        warning: tr("Apps using this connection lose access at once. Continue?"),
                                        onconfirm: { let id=connection.id; move |_| async move {
                                            if let Err(e)=api::delete_mcp_connection(id).await { error.set(Some(message(e))); }
                                            if let Ok(v)=api::get_mcp_connections().await { connections.set(v); }
                                        } },
                                    }
                                }
                            }
                        }
                    }
                    div { class: "rounded-xl border border-ctp-surface0 p-4 grid gap-3",
                        p { class: "font-medium", if editing().is_some() { {tr("Edit connection")} } else { {tr("New connection")} } }
                        Field { label: tr("Name"), input { class: INPUT, value:"{name}", oninput:move|e|name.set(e.value()) } }
                        div { class:"flex flex-wrap gap-2", for (value,label) in [(McpAccessPreset::ReadOnly,"Read only"),(McpAccessPreset::ThesisEditor,"Thesis editor"),(McpAccessPreset::Trader,"Trader")] { button { class:example_chip(preset()==value),onclick:move |_|preset.set(value),{tr(label)} } } }
                        p { class: "text-xs text-ctp-overlay1", {tr(preset_hint(preset()))} }
                        div { class: "grid gap-2",
                            div { class: "flex flex-wrap items-end justify-between gap-2",
                                div {
                                    p { class: "text-sm font-medium text-ctp-text", {tr("Portfolio access")} }
                                    p { class: "text-xs text-ctp-subtext0", {trf("Choose one or more portfolios · {} selected", &[&selected().len()])} }
                                }
                                div { class: "flex gap-2",
                                    button {
                                        r#type: "button",
                                        class: "text-xs text-ctp-mauve hover:underline",
                                        onclick: move |_| selected.set(scopes().into_iter().map(|portfolio| portfolio.id).collect()),
                                        {tr("Select all")}
                                    }
                                    button {
                                        r#type: "button",
                                        class: "text-xs text-ctp-subtext0 hover:text-ctp-text hover:underline",
                                        disabled: selected().is_empty(),
                                        onclick: move |_| selected.set(Vec::new()),
                                        {tr("Clear")}
                                    }
                                }
                            }
                            div { class: "grid gap-2 sm:grid-cols-2",
                                for portfolio in scopes() {
                                    label {
                                        key: "{portfolio.id}",
                                        class: if selected().contains(&portfolio.id) {
                                            "flex cursor-pointer items-center gap-3 rounded-lg border border-ctp-mauve bg-ctp-mauve/10 px-3 py-2"
                                        } else {
                                            "flex cursor-pointer items-center gap-3 rounded-lg border border-ctp-surface0 px-3 py-2 hover:border-ctp-surface1"
                                        },
                                        input {
                                            r#type: "checkbox",
                                            checked: selected().contains(&portfolio.id),
                                            onchange: move |event| {
                                                if event.checked() {
                                                    if !selected().contains(&portfolio.id) {
                                                        selected.write().push(portfolio.id);
                                                    }
                                                } else {
                                                    selected.write().retain(|id| *id != portfolio.id);
                                                }
                                            }
                                        }
                                        span { class: "min-w-0 flex-1 truncate", "{portfolio.name}" }
                                        if portfolio.ai {
                                            span { class: "shrink-0 text-xs text-ctp-mauve", {tr("AI paper")} }
                                        }
                                    }
                                }
                            }
                        }
                        div { class:"flex gap-2", ActionButton { label:if editing().is_some(){tr("Save")}else{tr("Create connection")},onclick:save } if editing().is_some(){GhostButton{label:tr("Cancel"),onclick:move |_|{editing.set(None);name.set(String::new());selected.set(vec![])}}} }
                    }
                    if let Some(k)=secret() { div { class:"rounded-xl border border-ctp-peach/50 p-4 grid gap-3", p { class:"font-medium text-ctp-peach",{tr("Copy this secret now. It will not be shown again.")} } div { class:"flex flex-wrap gap-2", for choice in McpExample::ALL { button { class:example_chip(example()==choice),onclick:move |_|example.set(choice),"{choice.label()}" } } } match example() { McpExample::Generic=>rsx!{ConnectorRow{label:tr("Bearer token"),hint:tr("Use this URL and Authorization header."),shown:format!("{base}/mcp · Bearer {}",masked(&k)),copy:format!("{base}/mcp\nAuthorization: Bearer {k}")}}, McpExample::Claude=>rsx!{ConnectorRow{label:"Claude Code",hint:tr("Run in a terminal."),shown:format!("claude mcp add --transport http akhsakov {base}/mcp --header \"Authorization: Bearer {}\"",masked(&k)),copy:format!("claude mcp add --transport http akhsakov {base}/mcp --header \"Authorization: Bearer {k}\"")}}, McpExample::Codex=>rsx!{ConnectorRow{label:"Codex config.toml",hint:tr("Restart Codex after editing."),shown:format!("[mcp_servers.akhsakov]\nurl = \"{base}/mcp\"\nhttp_headers = {{ Authorization = \"Bearer {}\" }}",masked(&k)),copy:format!("[mcp_servers.akhsakov]\nurl = \"{base}/mcp\"\nhttp_headers = {{ Authorization = \"Bearer {k}\" }}")}}, McpExample::Agy=>rsx!{ConnectorRow{label:"Antigravity / agy MCP config",hint:tr("Put this beneath mcpServers."),shown:format!("\"akhsakov\": {{ \"serverUrl\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {}\" }} }}",masked(&k)),copy:format!("\"akhsakov\": {{ \"serverUrl\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {k}\" }} }}")}}, McpExample::Editors=>rsx!{ConnectorRow{label:"Cursor / VS Code MCP JSON",hint:tr("Add this beneath mcpServers or servers."),shown:format!("\"akhsakov\": {{ \"type\": \"http\", \"url\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {}\" }} }}",masked(&k)),copy:format!("\"akhsakov\": {{ \"type\": \"http\", \"url\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {k}\" }} }}")}}, McpExample::Other=>rsx!{ConnectorRow{label:tr("URL token"),hint:tr("For clients that cannot set headers."),shown:format!("{base}/mcp/{}",masked(&k)),copy:format!("{base}/mcp/{k}")}} } GhostButton { label:tr("I saved it"),onclick:move |_|secret.set(None) } } }
                    if audit_for().is_some() {
                        div { class:"rounded-xl border border-ctp-surface0 p-4 grid gap-2",
                            p { class:"font-medium",{tr("Latest audit events")} }
                            if audit().is_empty() { p { class:"text-xs text-ctp-subtext0",{tr("No events yet.")} } }
                            for event in audit() {
                                p { key:"{event.id}", class:"text-xs text-ctp-subtext0", {format!("{} · {} {} · {}",crate::format::fmt_when(&event.at),event.method,event.tool.as_deref().unwrap_or(""),if event.success{tr("success")}else{event.error_category.as_deref().unwrap_or(tr("error"))})} }
                            }
                        }
                    }
                }
            }
            if let Some(e) = error() {
                p { class: "mt-3 text-sm text-ctp-red break-words", "{e}" }
            }
        }
    }
}

/// Reusable, provider-neutral OpenAI-compatible connections.
#[component]
fn ModelProfilesCard() -> Element {
    let mut profiles = use_context::<Signal<Vec<ModelProfile>>>();
    let mut loaded = use_signal(|| false);
    let mut editing = use_signal(|| None::<Uuid>);
    let mut name = use_signal(String::new);
    let mut base_url = use_signal(String::new);
    let mut model = use_signal(String::new);
    let mut api_key = use_signal(String::new);
    let toasts = use_context::<crate::notify::Toasts>();
    let mut timeout = use_signal(|| DEFAULT_MODEL_TIMEOUT_SECONDS.to_string());
    let mut max_tokens = use_signal(String::new);
    let mut key_header = use_signal(String::new);
    let mut headers = use_signal(String::new);
    let mut saved_headers = use_signal(Vec::<String>::new);
    let mut remove_headers = use_signal(|| false);
    let mut input_price = use_signal(String::new);
    let mut output_price = use_signal(String::new);
    let mut api_style = use_signal(ApiStyle::default);
    let mut stream = use_signal(|| false);
    let mut status = use_signal(|| None::<Result<String, String>>);
    let reload = move || {
        spawn(async move {
            match api::get_model_profiles().await {
                Ok(list) => profiles.set(list),
                Err(e) => status.set(Some(Err(server_message(e)))),
            }
            loaded.set(true);
        });
    };
    use_hook(reload);
    let mut clear = move || {
        editing.set(None);
        name.set(String::new());
        base_url.set(String::new());
        model.set(String::new());
        api_key.set(String::new());
        timeout.set(DEFAULT_MODEL_TIMEOUT_SECONDS.to_string());
        max_tokens.set(String::new());
        key_header.set(String::new());
        headers.set(String::new());
        saved_headers.set(Vec::new());
        remove_headers.set(false);
        input_price.set(String::new());
        output_price.set(String::new());
        api_style.set(ApiStyle::default());
        stream.set(false);
    };
    let save = move |_| async move {
        let key = (!api_key().trim().is_empty()).then(|| api_key());
        let settings = match profile_settings(&timeout(), &max_tokens(), &key_header(), &headers(), remove_headers(), &input_price(), &output_price()) {
            Ok(settings) => ModelProfileSettings { api_style: api_style(), stream: stream(), ..settings },
            Err(message) => return status.set(Some(Err(message))),
        };
        match api::save_model_profile(editing(), name(), base_url(), model(), key, settings).await {
            Ok(saved) => {
                status.set(None);
                toasts.show(crate::i18n::trf("Saved {}", &[&saved.name]), "");
                clear();
                reload();
            }
            Err(e) => status.set(Some(Err(server_message(e)))),
        }
    };
    rsx! {
        Card {
            title: tr("Model connections"),
            subtitle: tr("Reusable OpenAI-compatible API connections. The app stores API keys separately and never shows them again.").to_string(),
            if !loaded() {
                p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
            } else {
                div { class: "grid gap-4",
                    for profile in profiles() {
                        div { key: "{profile.id}", class: "flex flex-wrap items-center justify-between gap-3 rounded-xl border border-ctp-surface0 p-4",
                            div {
                                div { class: "font-medium text-ctp-text", "{profile.name}" }
                                div { class: "text-xs text-ctp-subtext0", "{profile.model} · {profile.base_url}" }
                                div { class: "text-xs text-ctp-overlay1", if profile.has_key { "API key saved" } else { "API key required" } }
                                div { class: "text-xs text-ctp-overlay1", "{profile_summary(&profile)}" }
                                if let Some(caps) = &profile.capabilities {
                                    div { class: if caps.tool_calls { "text-xs text-ctp-subtext0" } else { "text-xs text-ctp-red" }, "{capability_summary(caps)}" }
                                }
                            }
                            div { class: "flex flex-wrap gap-2",
                                GhostButton {
                                    label: tr("Test"),
                                    onclick: move |_| async move {
                                        status.set(Some(Ok("Testing connection…".into())));
                                        match api::test_model_profile(profile.id).await {
                                            Ok(caps) if caps.tool_calls => status.set(Some(Ok(tr("Connection and tool calling work.").to_string()))),
                                            Ok(_) => status.set(Some(Err(tr("The endpoint answered, but this model didn't call a tool, so it can't run an AI trader.").to_string()))),
                                            Err(e) => status.set(Some(Err(server_message(e)))),
                                        }
                                        reload();
                                    },
                                }
                                GhostButton {
                                    label: tr("Edit"),
                                    onclick: move |_| {
                                        editing.set(Some(profile.id));
                                        name.set(profile.name.clone());
                                        base_url.set(profile.base_url.clone());
                                        model.set(profile.model.clone());
                                        api_key.set(String::new());
                                        timeout.set(profile.timeout_seconds.to_string());
                                        max_tokens.set(profile.max_tokens.map(|n| n.to_string()).unwrap_or_default());
                                        key_header.set(profile.key_header.clone());
                                        headers.set(String::new());
                                        saved_headers.set(profile.extra_header_names.clone());
                                        remove_headers.set(false);
                                        input_price.set(profile.input_price_per_million.map(|v| v.to_string()).unwrap_or_default());
                                        output_price.set(profile.output_price_per_million.map(|v| v.to_string()).unwrap_or_default());
                                        api_style.set(profile.api_style);
                                        stream.set(profile.stream);
                                    },
                                }
                                crate::components::card::DeleteButton {
                                    title: tr("Delete connection"),
                                    onconfirm: move |_| {
                                        spawn(async move {
                                            match api::delete_model_profile(profile.id).await {
                                                Ok(()) => { status.set(None); reload(); }
                                                Err(e) => status.set(Some(Err(server_message(e)))),
                                            }
                                        });
                                    },
                                }
                            }
                        }
                    }
                    div { class: "grid gap-3 border-t border-ctp-surface0/60 pt-4 md:grid-cols-2",
                        Field { label: tr("Connection name"), input { class: INPUT, value: "{name}", oninput: move |e| name.set(e.value()) } }
                        Field { label: tr("Model identifier"), input { class: INPUT, placeholder: "provider/model-name", value: "{model}", oninput: move |e| model.set(e.value()) } }
                        Field { label: tr("API base URL"), input { class: INPUT, placeholder: "https://api.example.com/v1", value: "{base_url}", oninput: move |e| base_url.set(e.value()) } }
                        Field { label: if editing().is_some() { tr("New API key (leave blank to keep current)") } else { tr("API key") }, input { class: INPUT, r#type: "password", value: "{api_key}", oninput: move |e| api_key.set(e.value()) } }
                        Field { label: tr("Timeout per answer (seconds)"), input { class: INPUT, r#type: "number", min: "1", max: "600", value: "{timeout}", oninput: move |e| timeout.set(e.value()) } }
                        Field { label: tr("Max tokens per answer"), input { class: INPUT, r#type: "number", min: "1", placeholder: tr("Provider default"), value: "{max_tokens}", oninput: move |e| max_tokens.set(e.value()) } }
                        Field { label: tr("API style"),
                            select {
                                class: INPUT,
                                onchange: move |e| api_style.set(if e.value() == "responses" { ApiStyle::Responses } else { ApiStyle::ChatCompletions }),
                                option { value: "chat_completions", selected: api_style() == ApiStyle::ChatCompletions, {tr("Chat Completions (most providers)")} }
                                option { value: "responses", selected: api_style() == ApiStyle::Responses, {tr("Responses")} }
                            }
                        }
                        Field { label: tr("API key header"), input { class: INPUT, placeholder: "Authorization: Bearer", value: "{key_header}", oninput: move |e| key_header.set(e.value()) } }
                        Field { label: tr("Price per million input tokens (USD)"), input { class: INPUT, r#type: "number", min: "0", step: "any", placeholder: tr("Unknown"), value: "{input_price}", oninput: move |e| input_price.set(e.value()) } }
                        Field { label: tr("Price per million output tokens (USD)"), input { class: INPUT, r#type: "number", min: "0", step: "any", placeholder: tr("Unknown"), value: "{output_price}", oninput: move |e| output_price.set(e.value()) } }
                        label { class: "flex items-center gap-2 text-sm text-ctp-text md:col-span-2",
                            input { r#type: "checkbox", checked: stream(), onchange: move |e| stream.set(e.checked()) }
                            {tr("Stream replies, to watch the answer as it's written. Turn off if the provider doesn't support it.")}
                        }
                        Field { label: tr("Extra headers (one per line, Name: value)"),
                            textarea {
                                class: "{INPUT} min-h-24 font-mono",
                                placeholder: "OpenAI-Organization: org-…",
                                value: "{headers}",
                                oninput: move |e| headers.set(e.value()),
                            }
                            if !saved_headers().is_empty() {
                                p { class: "text-xs text-ctp-subtext0", {trf("Saved headers: {}. Leave blank to keep them.", &[&saved_headers().join(", ")])} }
                                label { class: "flex items-center gap-2 text-xs text-ctp-subtext0",
                                    input { r#type: "checkbox", checked: remove_headers(), onchange: move |e| remove_headers.set(e.checked()) }
                                    {tr("Remove saved headers")}
                                }
                            }
                        }
                    }
                    div { class: "flex justify-end gap-2",
                        if editing().is_some() { GhostButton { label: tr("Cancel"), onclick: move |_| clear() } }
                        ActionButton { label: if editing().is_some() { tr("Save changes") } else { tr("Add connection") }, onclick: save }
                    }
                    match status() {
                        Some(Ok(message)) => rsx! { p { class: "text-sm text-ctp-green", "{message}" } },
                        Some(Err(message)) => rsx! { p { class: "text-sm text-ctp-red", "{message}" } },
                        None => rsx! {},
                    }
                }
            }
        }
    }
}

/// Reads the settings form. Blank headers keep the saved ones unless
/// `remove` is ticked.
fn profile_settings(
    timeout: &str,
    max_tokens: &str,
    key_header: &str,
    headers: &str,
    remove: bool,
    input_price: &str,
    output_price: &str,
) -> Result<ModelProfileSettings, String> {
    let price = |value: &str| match value.trim() {
        "" => Ok(None),
        value => value
            .parse::<f64>()
            .map(Some)
            .map_err(|_| tr("Enter prices as numbers, or leave them blank.").to_string()),
    };
    let input_price_per_million = price(input_price)?;
    let output_price_per_million = price(output_price)?;
    let timeout_seconds = timeout
        .trim()
        .parse()
        .map_err(|_| tr("Enter the timeout in whole seconds.").to_string())?;
    let max_tokens = match max_tokens.trim() {
        "" => None,
        value => Some(
            value
                .parse()
                .map_err(|_| tr("Enter max tokens as a whole number, or leave it blank.").to_string())?,
        ),
    };
    let lines: Vec<&str> = headers.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let extra_headers = if !lines.is_empty() {
        let parsed = lines
            .iter()
            .map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_string(), value.trim().to_string()))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| tr("Write each header as Name: value.").to_string())?;
        Some(parsed)
    } else if remove {
        Some(Vec::new())
    } else {
        None
    };
    Ok(ModelProfileSettings {
        timeout_seconds,
        max_tokens,
        key_header: key_header.trim().to_string(),
        extra_headers,
        input_price_per_million,
        output_price_per_million,
        api_style: ApiStyle::default(),
        stream: false,
    })
}

/// What the model did when its connection was tested, in one line.
fn capability_summary(caps: &dtos::ai_models::ModelCapabilities) -> String {
    let mark = |ok: bool| if ok { "✓" } else { "✗" };
    [
        (tr("Tool calls"), caps.tool_calls),
        (tr("JSON arguments"), caps.json_arguments),
        (tr("Several calls at once"), caps.parallel_tool_calls),
        (tr("Forced tool choice"), caps.tool_choice),
    ]
    .iter()
    .map(|(label, ok)| format!("{label} {}", mark(*ok)))
    .collect::<Vec<_>>()
    .join(" · ")
}

/// One line on a saved profile's request settings.
fn profile_summary(profile: &ModelProfile) -> String {
    let mut parts = vec![trf("Timeout {} s", &[&profile.timeout_seconds])];
    if profile.api_style == ApiStyle::Responses {
        parts.push(tr("Responses API").to_string());
    }
    if profile.stream {
        parts.push(tr("Streams replies").to_string());
    }
    if let Some(max) = profile.max_tokens {
        parts.push(trf("Max {} tokens", &[&max]));
    }
    if !profile.key_header.is_empty() {
        parts.push(trf("Key in {}", &[&profile.key_header]));
    }
    if !profile.extra_header_names.is_empty() {
        parts.push(trf("Headers: {}", &[&profile.extra_header_names.join(", ")]));
    }
    if let (Some(input), Some(output)) = (profile.input_price_per_million, profile.output_price_per_million) {
        parts.push(trf("${} / ${} per million tokens", &[&input, &output]));
    }
    parts.join(" · ")
}

/// What an MCP connection with `preset` may do, in the user's words.
fn preset_hint(preset: McpAccessPreset) -> &'static str {
    match preset {
        McpAccessPreset::ReadOnly => "Can read the chosen portfolios, theses, transactions, goals, watchlists and market data. Changes nothing.",
        McpAccessPreset::ThesisEditor => "Can also write theses and notes, add stocks to watchlists and create alerts, when you ask.",
        McpAccessPreset::Trader => "Can also trade with paper money in the chosen AI portfolios.",
    }
}

pub(crate) fn server_message(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    }
}

/// The key with all but its last four characters hidden.
fn masked(key: &str) -> String {
    format!("••••{}", &key[key.len().saturating_sub(4)..])
}

/// One way to connect: what to paste where, with a copy button.
#[component]
fn ConnectorRow(label: String, hint: String, shown: String, copy: String) -> Element {
    let mut copied = use_signal(|| None::<bool>);
    rsx! {
        div { class: "border-t border-ctp-surface0/60 pt-4",
            div { class: "font-medium text-ctp-text", "{label}" }
            div { class: "text-xs text-ctp-subtext0", "{hint}" }
            div { class: "mt-2 flex items-start gap-2",
                code { class: "min-w-0 flex-1 break-all rounded-xl border border-ctp-surface0 bg-ctp-crust/40 px-3 py-2 font-mono text-xs text-ctp-subtext1",
                    "{shown}"
                }
                GhostButton {
                    label: match copied() {
                        Some(true) => tr("Copied ✓"),
                        Some(false) => tr("Couldn't copy"),
                        None => tr("Copy"),
                    },
                    onclick: {
                        let copy = copy.clone();
                        move |_| {
                            let text = copy.clone();
                            spawn(async move { copied.set(Some(crate::files::copy_to_clipboard(&text).await)) });
                        }
                    },
                }
            }
            // The row shows a masked secret, so when copying fails give the
            // full text to copy by hand.
            if copied() == Some(false) {
                div { class: "mt-2 grid gap-1",
                    div { class: "text-xs text-ctp-peach", {tr("Select the text below and copy it yourself (Ctrl+C).")} }
                    textarea {
                        class: "w-full resize-none rounded-xl border border-ctp-surface0 bg-ctp-crust/40 px-3 py-2 font-mono text-xs text-ctp-subtext1",
                        readonly: true,
                        rows: copy.lines().count().max(2) as i64,
                        value: "{copy}",
                        onmounted: move |e| async move { let _ = e.set_focus(true).await; },
                        onfocus: move |_| { document::eval("document.activeElement?.select?.()"); },
                    }
                }
            }
        }
    }
}
