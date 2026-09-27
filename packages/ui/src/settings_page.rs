//! Settings: display currency, analysis assumptions, and data export.

use crate::i18n::{tr, trf};
use crate::{
    app::{AppSettings, DataRefresh, PortfolioScope},
    components::card::{ActionButton, Card, Field, Segmented, Stepper, ToggleButton, INPUT},
    editors::{Dialog, Dialogs},
    files::{print_report, ExportButtons},
    format::fmt_usd,
    hooks::use_portfolio_memo,
    page::{AiBadge, GhostButton, Page},
};
use dioxus::prelude::*;
use dtos::{
    ai_models::{AiRun, AiRunStatus, ModelProfile, TraderConfig},
    ai_portfolio::{AiPortfolioInfo, DEFAULT_STARTING_CASH},
    csv_export::{holdings_csv, transactions_csv},
    portfolio::GetDashBoardResponse,
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
                crate::server_address::ServerCard {}
                crate::auth::SecurityCard {}
                NotificationSettings {}
                ConnectorCard {}
                ModelProfilesCard {}
                AiPortfolioCard {}
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
    let mut key = use_signal(|| None::<String>);
    let mut loaded = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut address = use_signal(String::new);
    let mut confirm = use_signal(|| None::<&'static str>);
    let mut example = use_signal(|| McpExample::Generic);
    let example_chip = |on: bool| if on {
        "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3 py-1.5 text-xs font-medium text-ctp-text cursor-pointer"
    } else {
        "rounded-full border border-ctp-surface0 px-3 py-1.5 text-xs text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
    };
    use_future(move || async move {
        match api::get_connector_key().await {
            Ok(k) => key.set(k),
            Err(e) => error.set(Some(e.to_string())),
        }
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
    let new_key = move |_| async move {
        confirm.set(None);
        match api::new_connector_key().await {
            Ok(k) => {
                key.set(Some(k));
                error.set(None);
            }
            Err(e) => error.set(Some(message(e))),
        }
    };
    let turn_off = move |_| async move {
        confirm.set(None);
        match api::turn_off_connector().await {
            Ok(()) => key.set(None),
            Err(e) => error.set(Some(message(e))),
        }
    };
    let base = address().trim().trim_end_matches('/').to_string();
    rsx! {
        Card {
            title: tr("Connect an AI assistant through MCP"),
            subtitle: tr("One provider-neutral MCP endpoint works with any compatible client. Pick a client below only to see its setup example.").to_string(),
            if !loaded() {
                p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
            } else if let Some(k) = key() {
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
                    div { class: "flex flex-wrap gap-2",
                        for choice in McpExample::ALL {
                            button {
                                key: "{choice:?}",
                                class: example_chip(example() == choice),
                                onclick: move |_| example.set(choice),
                                {choice.label()}
                            }
                        }
                    }
                    match example() {
                        McpExample::Generic => rsx! {
                            ConnectorRow { label: tr("Streamable HTTP endpoint"), hint: tr("Use this URL with an Authorization: Bearer header."), shown: format!("{base}/mcp  ·  Bearer {}", masked(&k)), copy: format!("{base}/mcp\nAuthorization: Bearer {k}") }
                        },
                        McpExample::Claude => rsx! {
                            ConnectorRow { label: tr("Claude Code"), hint: tr("Run this in a terminal."), shown: format!("claude mcp add --transport http akhsakov {base}/mcp --header \"Authorization: Bearer {}\"", masked(&k)), copy: format!("claude mcp add --transport http akhsakov {base}/mcp --header \"Authorization: Bearer {k}\"") }
                            ConnectorRow { label: tr("Claude web and desktop apps"), hint: tr("Add a custom remote connector. The key is included in the URL for clients that cannot set headers."), shown: format!("{base}/mcp/{}", masked(&k)), copy: format!("{base}/mcp/{k}") }
                        },
                        McpExample::Codex => rsx! {
                            ConnectorRow { label: tr("Codex config.toml"), hint: tr("Add this to ~/.codex/config.toml, then restart Codex."), shown: format!("[mcp_servers.akhsakov]\nurl = \"{base}/mcp\"\nhttp_headers = {{ Authorization = \"Bearer {}\" }}", masked(&k)), copy: format!("[mcp_servers.akhsakov]\nurl = \"{base}/mcp\"\nhttp_headers = {{ Authorization = \"Bearer {k}\" }}") }
                        },
                        McpExample::Agy => rsx! {
                            ConnectorRow { label: tr("Antigravity / agy MCP config"), hint: tr("Put this server under mcpServers in .agents/mcp_config.json or the global MCP config."), shown: format!("\"akhsakov\": {{ \"serverUrl\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {}\" }} }}", masked(&k)), copy: format!("\"akhsakov\": {{ \"serverUrl\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {k}\" }} }}") }
                        },
                        McpExample::Editors => rsx! {
                            ConnectorRow { label: tr("Cursor / VS Code MCP JSON"), hint: tr("Add this entry beneath mcpServers (Cursor) or servers (VS Code)."), shown: format!("\"akhsakov\": {{ \"type\": \"http\", \"url\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {}\" }} }}", masked(&k)), copy: format!("\"akhsakov\": {{ \"type\": \"http\", \"url\": \"{base}/mcp\", \"headers\": {{ \"Authorization\": \"Bearer {k}\" }} }}") }
                        },
                        McpExample::Other => rsx! {
                            ConnectorRow { label: tr("URL-only fallback"), hint: tr("For compatible clients that accept a URL but cannot set an Authorization header."), shown: format!("{base}/mcp/{}", masked(&k)), copy: format!("{base}/mcp/{k}") }
                        },
                    }
                    p { class: "text-xs text-ctp-peach",
                        {tr("The address holds your key: anyone who has it can read your portfolios and change your theses. Keep it private; make a new key if it leaks.")}
                    }
                    div { class: "flex flex-wrap items-center gap-2",
                        match confirm() {
                            Some("new") => rsx! {
                                span { class: "text-xs text-ctp-peach", {tr("Connected MCP clients will need the new key. Continue?")} }
                                GhostButton { label: tr("Make a new key"), onclick: new_key }
                                GhostButton { label: tr("Cancel"), onclick: move |_| confirm.set(None) }
                            },
                            Some(_) => rsx! {
                                span { class: "text-xs text-ctp-peach", {tr("Connected MCP clients will lose access. Continue?")} }
                                GhostButton { label: tr("Turn off"), onclick: turn_off }
                                GhostButton { label: tr("Cancel"), onclick: move |_| confirm.set(None) }
                            },
                            None => rsx! {
                                GhostButton { label: tr("New key"), onclick: move |_| confirm.set(Some("new")) }
                                GhostButton { label: tr("Turn off"), onclick: move |_| confirm.set(Some("off")) }
                            },
                        }
                    }
                }
            } else {
                div { class: "flex flex-wrap items-center justify-between gap-3 text-sm",
                    p { class: "max-w-xl text-ctp-subtext0",
                        {tr("AI assistants connect through MCP with a private key. Until you turn it on, nothing outside the app can reach your data this way.")}
                    }
                    ActionButton { label: tr("Turn on"), onclick: new_key }
                }
            }
            if let Some(e) = error() {
                p { class: "mt-3 text-sm text-ctp-red", "{e}" }
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
    };
    let save = move |_| async move {
        let key = (!api_key().trim().is_empty()).then(|| api_key());
        match api::save_model_profile(editing(), name(), base_url(), model(), key).await {
            Ok(saved) => {
                status.set(Some(Ok(format!("Saved {}", saved.name))));
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
                            }
                            div { class: "flex flex-wrap gap-2",
                                GhostButton {
                                    label: tr("Test"),
                                    onclick: move |_| async move {
                                        status.set(Some(Ok("Testing connection…".into())));
                                        match api::test_model_profile(profile.id).await {
                                            Ok(()) => status.set(Some(Ok("Connection and tool calling work.".into()))),
                                            Err(e) => status.set(Some(Err(server_message(e)))),
                                        }
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
                                    },
                                }
                                GhostButton {
                                    label: tr("Delete"),
                                    onclick: move |_| async move {
                                        match api::delete_model_profile(profile.id).await {
                                            Ok(()) => { status.set(None); reload(); }
                                            Err(e) => status.set(Some(Err(server_message(e)))),
                                        }
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

/// Portfolios an AI assistant manages through MCP or a configured model,
/// own paper money. Your other portfolios stay read-only to it.
#[component]
fn AiPortfolioCard() -> Element {
    let refresh = use_context::<DataRefresh>();
    let profiles = use_context::<Signal<Vec<ModelProfile>>>();
    let mut infos = use_signal(Vec::<AiPortfolioInfo>::new);
    let mut loaded = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut name = use_signal(String::new);
    let mut amount = use_signal(|| DEFAULT_STARTING_CASH.to_string());
    let reload = move || {
        spawn(async move {
            match api::get_ai_portfolios().await {
                Ok(list) => infos.set(list),
                Err(e) => error.set(Some(e.to_string())),
            }
            loaded.set(true);
        });
    };
    use_hook(reload);
    let start = move |_| async move {
        let cash = match parse_amount(&amount()) {
            Ok(c) => c,
            Err(e) => return error.set(Some(e)),
        };
        match api::start_ai_portfolio(name(), cash).await {
            Ok(_) => {
                error.set(None);
                name.set(String::new());
                amount.set(DEFAULT_STARTING_CASH.to_string());
                reload();
                refresh.reload();
            }
            Err(e) => error.set(Some(server_message(e))),
        }
    };
    let list = infos();
    rsx! {
        Card {
            title: tr("AI paper portfolios"),
            subtitle: tr("Give an AI assistant paper money to manage through MCP or a configured model connection. It cannot trade in your real portfolios.").to_string(),
            if !loaded() {
                p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
            } else {
                div { class: "grid gap-4 text-sm",
                    for info in list.iter().cloned() {
                        AiPortfolioRow {
                            key: "{info.portfolio_id}",
                            info,
                            profiles: profiles(),
                            on_change: move |_| {
                                reload();
                                refresh.reload();
                            },
                        }
                    }
                    if !list.is_empty() {
                        p { class: "text-xs text-ctp-subtext0",
                            {tr("Use an MCP client or Run AI trader below. Paper trades use the latest price with no fee and journal their reason.")}
                        }
                    }
                    div { class: if list.is_empty() { "grid gap-3" } else { "grid gap-3 border-t border-ctp-surface0/60 pt-4" },
                        div { class: "flex flex-wrap items-end gap-3",
                            Field { label: tr("Name"),
                                input {
                                    class: "{INPUT} max-w-sm",
                                    placeholder: "AI",
                                    value: "{name}",
                                    oninput: move |e| name.set(e.value()),
                                }
                            }
                            Field { label: tr("Starting cash (USD)"),
                                input {
                                    class: "{INPUT} max-w-sm",
                                    inputmode: "decimal",
                                    value: "{amount}",
                                    oninput: move |e| amount.set(e.value()),
                                }
                            }
                        }
                        p { class: "text-xs text-ctp-overlay1",
                            {tr("Paper money only: nothing is bought at a real broker.")}
                        }
                        div { class: "flex justify-end",
                            ActionButton {
                                label: if list.is_empty() { tr("Create an AI portfolio") } else { tr("Add another AI portfolio") },
                                onclick: start,
                            }
                        }
                    }
                }
            }
            if let Some(e) = error() {
                p { class: "mt-3 text-sm text-ctp-red", "{e}" }
            }
        }
    }
}

/// One AI paper portfolio: funding, model strategy, runs and audit history.
#[component]
fn AiPortfolioRow(info: AiPortfolioInfo, profiles: Vec<ModelProfile>, on_change: EventHandler<()>) -> Element {
    let PortfolioScope(mut scope) = use_context::<PortfolioScope>();
    let refresh = use_context::<DataRefresh>();
    let mut amount = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut confirm_stop = use_signal(|| false);
    let mut trader = use_signal(|| TraderConfig::new(info.portfolio_id));
    let mut runs = use_signal(Vec::<AiRun>::new);
    let mut trader_loaded = use_signal(|| false);
    let mut running = use_signal(|| false);
    let id = info.portfolio_id;
    let load_trader = move || {
        spawn(async move {
            match api::get_trader_config(id).await {
                Ok(value) => trader.set(value),
                Err(e) => error.set(Some(server_message(e))),
            }
            if let Ok(value) = api::get_ai_trader_runs(id).await {
                running.set(value.first().is_some_and(|r| r.status == AiRunStatus::Running));
                runs.set(value);
            }
            trader_loaded.set(true);
        });
    };
    use_hook(load_trader);
    let fund = move |_| async move {
        let cash = match parse_amount(&amount()) {
            Ok(c) => c,
            Err(e) => return error.set(Some(e)),
        };
        match api::fund_ai_portfolio(id, cash).await {
            Ok(_) => {
                error.set(None);
                amount.set(String::new());
                on_change.call(());
            }
            Err(e) => error.set(Some(server_message(e))),
        }
    };
    let stop = move |_| async move {
        confirm_stop.set(false);
        match api::stop_ai_portfolio(id).await {
            Ok(()) => on_change.call(()),
            Err(e) => error.set(Some(server_message(e))),
        }
    };
    let save_trader = move |_| async move {
        match api::save_trader_config(trader()).await {
            Ok(saved) => {
                trader.set(saved);
                error.set(None);
            }
            Err(e) => error.set(Some(server_message(e))),
        }
    };
    let run_trader = move |_| async move {
        match api::save_trader_config(trader()).await {
            Err(e) => error.set(Some(server_message(e))),
            Ok(saved) => {
                trader.set(saved);
                match api::start_ai_trader_run(id).await {
                    Err(e) => error.set(Some(server_message(e))),
                    Ok(run) => {
                        running.set(true);
                        error.set(None);
                        runs.write().insert(0, run.clone());
                        let run_id = run.id;
                        spawn(async move {
                            for _ in 0..120 {
                                let _ = document::eval("await new Promise(resolve => setTimeout(resolve, 1000)); return 'ok';")
                                    .join::<String>().await;
                                match api::get_ai_trader_run(run_id).await {
                                    Ok(updated) => {
                                        if let Some(item) = runs.write().iter_mut().find(|r| r.id == run_id) {
                                            *item = updated.clone();
                                        }
                                        if updated.status != AiRunStatus::Running {
                                            running.set(false);
                                            refresh.reload();
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        error.set(Some(server_message(e)));
                                        running.set(false);
                                        break;
                                    }
                                }
                            }
                        });
                    }
                }
            }
        }
    };
    rsx! {
        div { class: "grid gap-3 rounded-xl border border-ctp-surface0 p-4",
            div { class: "flex flex-wrap items-center justify-between gap-3",
                div {
                    div { class: "flex items-center gap-2 font-medium text-ctp-text", "{info.name}" AiBadge {} }
                    div { class: "text-xs text-ctp-subtext0",
                        {trf("Given {} · cash {} · {} trades", &[&fmt_usd(info.funded, 2), &fmt_usd(info.cash, 2), &info.trades])}
                    }
                }
                GhostButton {
                    label: tr("Open portfolio"),
                    onclick: move |_| {
                        scope.set(Some(id.to_string()));
                        navigator().push("/portfolio");
                    },
                }
            }
            div { class: "flex flex-wrap items-center gap-2",
                input {
                    class: "{INPUT} max-w-sm",
                    inputmode: "decimal",
                    placeholder: tr("Add funds (USD)"),
                    value: "{amount}",
                    oninput: move |e| amount.set(e.value()),
                }
                GhostButton { label: tr("Add funds"), onclick: fund }
                if confirm_stop() {
                    span { class: "text-xs text-ctp-peach", {tr("AI trading will stop; the portfolio and its history stay. Continue?")} }
                    GhostButton { label: tr("Stop"), onclick: stop }
                    GhostButton { label: tr("Cancel"), onclick: move |_| confirm_stop.set(false) }
                } else {
                    GhostButton { label: tr("Stop AI trading"), onclick: move |_| confirm_stop.set(true) }
                }
            }
            if trader_loaded() {
                div { class: "grid gap-3 border-t border-ctp-surface0 pt-3",
                    div { class: "grid gap-3 md:grid-cols-2",
                        Field { label: tr("Model connection"),
                            select {
                                class: INPUT,
                                value: trader().profile_id.map(|id| id.to_string()).unwrap_or_default(),
                                onchange: move |e| {
                                    trader.write().profile_id = Uuid::parse_str(&e.value()).ok();
                                },
                                option { value: "", {tr("Choose a connection")} }
                                for profile in profiles.iter() {
                                    option { key: "{profile.id}", value: "{profile.id}", "{profile.name} · {profile.model}" }
                                }
                            }
                        }
                        Field { label: tr("Trading strategy"),
                            textarea {
                                class: "{INPUT} min-h-24",
                                value: trader().strategy,
                                oninput: move |e| trader.write().strategy = e.value(),
                            }
                        }
                    }
                    p { class: "text-xs text-ctp-peach", {tr("Run AI trader executes paper orders immediately. It can access only this AI portfolio.")} }
                    div { class: "flex justify-end gap-2",
                        GhostButton { label: tr("Save strategy"), onclick: save_trader }
                        ActionButton { label: if running() { tr("Running…") } else { tr("Run AI trader") }, disabled: running() || trader().profile_id.is_none(), onclick: run_trader }
                    }
                    for run in runs().into_iter().take(5) {
                        div { key: "{run.id}", class: "rounded-xl bg-ctp-crust/30 p-3 text-xs",
                            div { class: "flex flex-wrap justify-between gap-2",
                                span { class: "font-medium text-ctp-text", "{run.status.label()} · {run.model}" }
                                span { class: "text-ctp-overlay1", "{run.started_at}" }
                            }
                            if let Some(text) = run.final_response { p { class: "mt-2 whitespace-pre-wrap text-ctp-subtext0", "{text}" } }
                            if let Some(message) = run.error { p { class: "mt-2 text-ctp-red", "{message}" } }
                            for event in run.events {
                                div { class: "mt-2 border-t border-ctp-surface0 pt-2 text-ctp-subtext0",
                                    "#{event.sequence} {event.tool} · "
                                    if event.success { "ok" } else { "failed" }
                                    code { class: "mt-1 block break-all text-ctp-overlay1", "{event.arguments}" }
                                    if event.detail != "Completed" { p { class: "mt-1 break-all", "{event.detail}" } }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(e) = error() {
                p { class: "text-sm text-ctp-red", "{e}" }
            }
        }
    }
}

/// A positive dollar amount typed by hand ("1,000", "$500").
fn parse_amount(text: &str) -> Result<Decimal, String> {
    Decimal::from_str(text.trim().trim_start_matches('$').replace(',', "").as_str())
        .ok()
        .filter(|d| *d > Decimal::ZERO)
        .ok_or_else(|| tr("Enter an amount more than zero").to_string())
}

fn server_message(e: ServerFnError) -> String {
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
                    onclick: move |_| {
                        let text = copy.clone();
                        spawn(async move { copied.set(Some(crate::files::copy_to_clipboard(&text).await)) });
                    },
                }
            }
        }
    }
}
