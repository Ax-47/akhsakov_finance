//! AI traders: paper portfolios an AI invests for you, with their runs,
//! schedules and memory. Connections to models are set up in Settings.

use crate::i18n::{tr, trf};
use crate::{
    app::{DataRefresh, PortfolioScope},
    components::card::{ActionButton, Card, Field, INPUT},
    format::fmt_usd,
    page::{AiBadge, GhostButton, Page},
    settings_page::server_message,
};
use dioxus::prelude::*;
use dtos::{
    ai_models::{AiRun, AiRunStatus, MarketHours, ModelProfile, TraderConfig, TraderMemory, TraderSchedule},
    ai_portfolio::{AiPortfolioInfo, DEFAULT_STARTING_CASH},
    mcp::{McpAccessPreset, McpConnection},
};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

/// ISO weekday numbers and their short names, Monday first.
const WEEKDAYS: [(u8, &str); 7] = [
    (1, "Mon"),
    (2, "Tue"),
    (3, "Wed"),
    (4, "Thu"),
    (5, "Fri"),
    (6, "Sat"),
    (7, "Sun"),
];

/// The page: the AI portfolios and what they've done.
#[component]
pub fn AiPage() -> Element {
    // Model connections, for choosing which one trades each portfolio.
    let mut profiles = use_signal(Vec::<ModelProfile>::new);
    use_context_provider(|| profiles);
    use_hook(move || {
        spawn(async move {
            if let Ok(list) = api::get_model_profiles().await {
                profiles.set(list);
            }
        })
    });
    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("AI traders")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0",
                    {tr("Let an AI invest paper money for you and see how it does. Connect a model in Settings first.")}
                }
            }
            div { class: "mt-10 grid gap-5 motion-safe:animate-rise",
                AiPortfolioCard {}
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
    let mut connections = use_signal(Vec::<McpConnection>::new);
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
            if let Ok(list) = api::get_mcp_connections().await {
                connections.set(list);
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
            crate::ai_stop::AiStopSwitch {}
            crate::ai_stop::AiNotifySetting {}
            crate::ai_usage::AiUsageTable {}
            if !loaded() {
                p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
            } else {
                div { class: "grid gap-4 text-sm",
                    for info in list.iter().cloned() {
                        AiPortfolioRow {
                            key: "{info.portfolio_id}",
                            info,
                            profiles: profiles(),
                            connections: connections(),
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
                p { class: "mt-3 text-sm text-ctp-red break-words", "{e}" }
            }
        }
    }
}

/// One AI paper portfolio: funding, model strategy, runs and audit history.
fn poll_ai_run(
    run_id: Uuid,
    portfolio_id: Uuid,
    mut runs: Signal<Vec<AiRun>>,
    mut running: Signal<bool>,
    mut memory: Signal<TraderMemory>,
    mut error: Signal<Option<String>>,
    refresh: DataRefresh,
) {
    spawn(async move {
        for _ in 0..150 {
            crate::notify::poll_delay(1_000).await;
            match api::get_ai_trader_run(run_id).await {
                Ok(updated) => {
                    if let Some(item) = runs.write().iter_mut().find(|run| run.id == run_id) {
                        *item = updated.clone();
                    }
                    if updated.status != AiRunStatus::Running {
                        running.set(false);
                        if let Ok(saved) = api::get_trader_memory(portfolio_id).await {
                            memory.set(saved);
                        }
                        refresh.reload();
                        return;
                    }
                }
                Err(e) => {
                    error.set(Some(server_message(e)));
                    running.set(false);
                    return;
                }
            }
        }
        running.set(false);
    });
}

#[component]
fn AiPortfolioRow(info: AiPortfolioInfo, profiles: Vec<ModelProfile>, connections: Vec<McpConnection>, on_change: EventHandler<()>) -> Element {
    let PortfolioScope(mut scope) = use_context::<PortfolioScope>();
    let refresh = use_context::<DataRefresh>();
    let mut amount = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut confirm_stop = use_signal(|| false);
    let mut trader = use_signal(|| TraderConfig::new(info.portfolio_id));
    let mut memory = use_signal(|| TraderMemory::empty(info.portfolio_id));
    let mut runs = use_signal(Vec::<AiRun>::new);
    let mut trader_loaded = use_signal(|| false);
    let mut memory_loaded = use_signal(|| false);
    let mut running = use_signal(|| false);
    let mut confirm_clear_memory = use_signal(|| false);
    let mut schedule = use_signal(|| TraderSchedule::new(info.portfolio_id));
    let toasts = use_context::<crate::notify::Toasts>();
    let id = info.portfolio_id;
    // MCP connections that could drive this portfolio.
    let drivers: Vec<McpConnection> = connections
        .into_iter()
        .filter(|c| c.enabled && c.preset == McpAccessPreset::Trader && c.portfolio_ids.contains(&id))
        .collect();
    let mcp_driven = trader().mcp_connection_id.is_some();
    let load_trader = move || {
        spawn(async move {
            match api::get_trader_config(id).await {
                Ok(value) => trader.set(value),
                Err(e) => error.set(Some(server_message(e))),
            }
            if let Ok(value) = api::get_ai_trader_runs(id).await {
                let active = value
                    .iter()
                    .find(|run| run.status == AiRunStatus::Running)
                    .map(|run| run.id);
                running.set(active.is_some());
                runs.set(value);
                if let Some(run_id) = active {
                    poll_ai_run(run_id, id, runs, running, memory, error, refresh);
                }
            }
            match api::get_trader_memory(id).await {
                Ok(value) => memory.set(value),
                Err(e) => error.set(Some(server_message(e))),
            }
            if let Ok(value) = api::get_trader_schedule(id).await {
                schedule.set(value);
            }
            memory_loaded.set(true);
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
    let save_schedule = move |_| async move {
        match api::save_trader_schedule(schedule()).await {
            Ok(saved) => {
                schedule.set(saved);
                toasts.show(tr("Schedule saved."), "");
                error.set(None);
            }
            Err(e) => error.set(Some(server_message(e))),
        }
    };
    let save_memory = move |_| async move {
        match api::save_trader_config(trader()).await {
            Ok(saved) => trader.set(saved),
            Err(e) => return error.set(Some(server_message(e))),
        }
        let draft = memory();
        match api::save_trader_memory(
            id,
            draft.decision_summary,
            draft.unresolved_questions,
        )
        .await
        {
            Ok(saved) => {
                memory.set(saved);
                error.set(None);
            }
            Err(e) => error.set(Some(server_message(e))),
        }
    };
    let clear_memory = move |_| async move {
        confirm_clear_memory.set(false);
        match api::clear_trader_memory(id).await {
            Ok(cleared) => {
                memory.set(cleared);
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
                        poll_ai_run(run.id, id, runs, running, memory, error, refresh);
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
                                onchange: move |e| {
                                    let value = e.value();
                                    let (kind, raw) = value.split_once(':').unwrap_or_default();
                                    let chosen = Uuid::parse_str(raw).ok();
                                    let mut config = trader.write();
                                    config.profile_id = chosen.filter(|_| kind == "openai");
                                    config.mcp_connection_id = chosen.filter(|_| kind == "mcp");
                                },
                                option { value: "", selected: trader().profile_id.is_none() && !mcp_driven, {tr("Choose a connection")} }
                                if !profiles.is_empty() {
                                    optgroup { label: tr("OpenAI-compatible API"),
                                        for profile in profiles.iter() {
                                            option {
                                                key: "{profile.id}",
                                                value: "openai:{profile.id}",
                                                selected: trader().profile_id == Some(profile.id),
                                                "{profile.name} · {profile.model}"
                                            }
                                        }
                                    }
                                }
                                if !drivers.is_empty() {
                                    optgroup { label: tr("MCP connection"),
                                        for connection in drivers.iter() {
                                            option {
                                                key: "{connection.id}",
                                                value: "mcp:{connection.id}",
                                                selected: trader().mcp_connection_id == Some(connection.id),
                                                "{connection.name} · MCP"
                                            }
                                        }
                                    }
                                }
                            }
                            p { class: "mt-1 text-xs text-ctp-overlay1",
                                {tr("MCP connections appear here when they are enabled, use the Trader preset, and can access this portfolio.")}
                            }
                        }
                        Field { label: tr("Trading strategy"),
                            textarea {
                                class: "{INPUT} min-h-40",
                                value: trader().strategy,
                                oninput: move |e| trader.write().strategy = e.value(),
                            }
                        }
                    }
                    div { class: "grid gap-3 md:grid-cols-3",
                        Field { label: tr("Horizon (years)"),
                            input {
                                class: INPUT,
                                r#type: "number",
                                min: "1",
                                max: "50",
                                placeholder: tr("None"),
                                value: trader().horizon_years.map(|v| v.to_string()).unwrap_or_default(),
                                oninput: move |e| trader.write().horizon_years = e.value().trim().parse().ok(),
                            }
                        }
                        Field { label: tr("Largest drawdown accepted (%)"),
                            input {
                                class: INPUT,
                                r#type: "number",
                                min: "1",
                                max: "100",
                                placeholder: tr("None"),
                                value: trader().max_drawdown_pct.map(|v| v.to_string()).unwrap_or_default(),
                                oninput: move |e| trader.write().max_drawdown_pct = e.value().trim().parse().ok(),
                            }
                        }
                        Field { label: tr("Benchmark to beat"),
                            input {
                                class: INPUT,
                                placeholder: "SPY",
                                value: trader().benchmark,
                                oninput: move |e| trader.write().benchmark = e.value(),
                            }
                        }
                    }
                    p { class: "text-xs text-ctp-overlay1",
                        {tr("The trader gets this mandate and the portfolio's goals at the start of every run. Set goals on the portfolio's Plan tab.")}
                    }
                    div { class: "grid gap-3 md:grid-cols-2",
                        Field { label: tr("Cost limit per run (USD)"),
                            input {
                                class: INPUT,
                                r#type: "number",
                                min: "0",
                                step: "any",
                                placeholder: tr("None"),
                                value: trader().max_cost_per_run.map(|v| v.to_string()).unwrap_or_default(),
                                oninput: move |e| trader.write().max_cost_per_run = e.value().trim().parse().ok(),
                            }
                        }
                        Field { label: tr("Cost limit per month (USD)"),
                            input {
                                class: INPUT,
                                r#type: "number",
                                min: "0",
                                step: "any",
                                placeholder: tr("None"),
                                value: trader().max_cost_per_month.map(|v| v.to_string()).unwrap_or_default(),
                                oninput: move |e| trader.write().max_cost_per_month = e.value().trim().parse().ok(),
                            }
                        }
                    }
                    div { class: "grid gap-3 md:grid-cols-2",
                        Field { label: tr("Memory size (characters)"),
                            input {
                                class: INPUT,
                                r#type: "number",
                                min: "1000",
                                max: "50000",
                                value: "{trader().memory_char_limit}",
                                oninput: move |e| {
                                    if let Ok(value) = e.value().parse() {
                                        trader.write().memory_char_limit = value;
                                    }
                                },
                            }
                        }
                        Field { label: tr("Model context window (tokens)"),
                            input {
                                class: INPUT,
                                r#type: "number",
                                min: "8192",
                                max: "1000000",
                                step: "1024",
                                value: "{trader().context_token_limit}",
                                oninput: move |e| {
                                    if let Ok(value) = e.value().parse() {
                                        trader.write().context_token_limit = value;
                                    }
                                },
                            }
                        }
                    }
                    p { class: "text-xs text-ctp-overlay1",
                        {tr("Older tool turns are compacted automatically before the configured context window is exceeded.")}
                    }
                    if memory_loaded() {
                        div { class: "grid gap-3 rounded-xl border border-ctp-surface0/70 bg-ctp-crust/20 p-3",
                            div { class: "flex flex-wrap items-center justify-between gap-2",
                                div {
                                    div { class: "font-medium text-ctp-text", {tr("Persistent trader memory")} }
                                    p { class: "text-xs text-ctp-overlay1",
                                        {trf("{} of {} characters used", &[&memory().used_chars(), &trader().memory_char_limit])}
                                    }
                                }
                                if let Some(updated) = memory().updated_at {
                                    span { class: "text-xs text-ctp-overlay1", {trf("Updated {}", &[&crate::format::fmt_when(&updated)])} }
                                }
                            }
                            Field { label: tr("Decision summary"),
                                textarea {
                                    class: "{INPUT} min-h-28",
                                    placeholder: tr("Important decisions, constraints, and reasons retained across runs"),
                                    value: memory().decision_summary,
                                    oninput: move |e| memory.write().decision_summary = e.value(),
                                }
                            }
                            Field { label: tr("Unresolved questions"),
                                textarea {
                                    class: "{INPUT} min-h-20",
                                    placeholder: tr("Evidence or questions the next run should revisit"),
                                    value: memory().unresolved_questions,
                                    oninput: move |e| memory.write().unresolved_questions = e.value(),
                                }
                            }
                            div { class: "flex flex-wrap justify-end gap-2",
                                if confirm_clear_memory() {
                                    span { class: "self-center text-xs text-ctp-peach", {tr("Clear all saved trader memory?")} }
                                    GhostButton { label: tr("Clear memory"), onclick: clear_memory }
                                    GhostButton { label: tr("Cancel"), onclick: move |_| confirm_clear_memory.set(false) }
                                } else {
                                    GhostButton { label: tr("Clear memory"), onclick: move |_| confirm_clear_memory.set(true) }
                                }
                                ActionButton { label: tr("Save memory"), onclick: save_memory }
                            }
                        }
                    }
                    if mcp_driven {
                        p { class: "text-xs text-ctp-peach", {tr("This portfolio trades when its MCP client connects. In a race, the client must act within each round's window.")} }
                    } else {
                        p { class: "text-xs text-ctp-peach", {tr("Run AI trader executes paper orders immediately. It can access only this AI portfolio.")} }
                    }
                    div { class: "flex justify-end gap-2",
                        GhostButton { label: tr("Save strategy"), onclick: save_trader }
                        if !mcp_driven {
                            ActionButton { label: if running() { tr("Run in progress…") } else { tr("Run AI trader") }, disabled: running() || trader().profile_id.is_none(), onclick: run_trader }
                        }
                    }
                    if !mcp_driven {
                        div { class: "grid gap-3 rounded-xl border border-ctp-surface0/70 p-3",
                            label { class: "flex items-center gap-2 font-medium text-ctp-text",
                                input {
                                    r#type: "checkbox",
                                    checked: schedule().enabled,
                                    onchange: move |e| {
                                        schedule.write().enabled = e.checked();
                                    },
                                }
                                {tr("Run on a schedule")}
                            }
                            div { class: "grid gap-3 md:grid-cols-2",
                                Field { label: tr("Every (hours)"),
                                    input {
                                        class: INPUT,
                                        r#type: "number",
                                        min: "1",
                                        max: "168",
                                        value: "{schedule().every_hours}",
                                        oninput: move |e| {
                                            if let Ok(value) = e.value().parse() {
                                                schedule.write().every_hours = value;
                                            }
                                        },
                                    }
                                }
                                Field { label: tr("When"),
                                    select {
                                        class: INPUT,
                                        onchange: move |e| {
                                            schedule.write().market_hours = match e.value().as_str() {
                                                "us" => MarketHours::Us,
                                                "thai" => MarketHours::Thai,
                                                _ => MarketHours::Any,
                                            };
                                        },
                                        option { value: "any", selected: schedule().market_hours == MarketHours::Any, {tr("Any time")} }
                                        option { value: "us", selected: schedule().market_hours == MarketHours::Us, {tr("While the US market is open")} }
                                        option { value: "thai", selected: schedule().market_hours == MarketHours::Thai, {tr("While the Thai market is open")} }
                                    }
                                }
                            }
                            div { class: "flex flex-wrap gap-1",
                                for (day, name) in WEEKDAYS {
                                    button {
                                        key: "{day}",
                                        r#type: "button",
                                        class: if schedule().weekdays.contains(&day) {
                                            "rounded-full border border-ctp-mauve bg-ctp-mauve/10 px-3 py-1 text-xs text-ctp-text cursor-pointer"
                                        } else {
                                            "rounded-full border border-ctp-surface0 px-3 py-1 text-xs text-ctp-subtext0 cursor-pointer"
                                        },
                                        onclick: move |_| {
                                            let mut s = schedule.write();
                                            if let Some(i) = s.weekdays.iter().position(|d| *d == day) {
                                                s.weekdays.remove(i);
                                            } else {
                                                s.weekdays.push(day);
                                            }
                                        },
                                        {tr(name)}
                                    }
                                }
                            }
                            if let Some(next) = schedule().next_run_at.filter(|_| schedule().enabled) {
                                p { class: "text-xs text-ctp-subtext0", {trf("Next run: {}", &[&crate::format::fmt_when(&next)])} }
                            }
                            if let Some(last) = schedule().last_run_at {
                                p { class: "text-xs text-ctp-overlay1", {trf("Last run: {}", &[&crate::format::fmt_when(&last)])} }
                            }
                            if let Some(note) = schedule().last_note {
                                p { class: "text-xs text-ctp-peach break-words", "{note}" }
                            }
                            div { class: "flex items-center justify-end gap-2",
                                GhostButton { label: tr("Save schedule"), onclick: save_schedule }
                            }
                        }
                    }
                    for run in runs().into_iter().take(5) {
                        div { key: "{run.id}", class: "min-w-0 rounded-xl bg-ctp-crust/30 p-3 text-xs",
                            div { class: "flex flex-wrap justify-between gap-2",
                                span { class: "font-medium text-ctp-text", "{run.status.label()}" }
                                span { class: "text-ctp-overlay1", {crate::format::fmt_when(&run.started_at)} }
                            }
                            if let Some(tokens) = run.total_tokens {
                                p { class: "mt-1 text-ctp-overlay1",
                                    {trf("{} tokens", &[&tokens])}
                                    if let Some(cost) = run.cost_usd { " · ≈ {crate::ai_usage::cost_label(cost)}" }
                                }
                            }
                            if let Some(live) = run.live_text.clone() { p { class: "mt-2 break-words whitespace-pre-wrap text-ctp-sky", "{live}" } }
                            if let Some(text) = run.final_response { p { class: "mt-2 break-words whitespace-pre-wrap text-ctp-subtext0", "{text}" } }
                            if let Some(message) = run.error { p { class: "mt-2 break-words text-ctp-red", "{message}" } }
                        }
                    }
                }
            }
            if let Some(e) = error() {
                p { class: "text-sm text-ctp-red break-words", "{e}" }
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
