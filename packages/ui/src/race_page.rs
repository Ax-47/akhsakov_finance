//! AI races: AI paper portfolios trade the same frozen prices in
//! synchronized rounds, with equal starting cash and a leaderboard. Races
//! are something you watch, so they have a page of their own rather than a
//! card in Settings (docs/UI.md).

use crate::i18n::{tr, trf};
use crate::{
    components::{
        card::{ActionButton, Card, Field, MetricTile, INPUT},
        charts::{GrowthChart, Series},
        color_schema::{CHART_COLORS_HEX, CHART_NEUTRAL_HEX},
    },
    errors::error_message,
    files::download,
    format::fmt_usd,
    page::{GhostButton, Page},
};
use dioxus::prelude::*;
use dtos::{
    ai_models::{AiRace, AiRaceRun, AiRaceStatus, AiRunStatus, NewAiRace},
    ai_portfolio::AiPortfolioInfo,
};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Refreshes the races every second while one is running.
fn poll_races(mut races: Signal<Vec<AiRace>>, mut error: Signal<Option<String>>) {
    spawn(async move {
        for _ in 0..86_400 {
            crate::notify::poll_delay(1_000).await;
            match api::get_ai_races().await {
                Ok(value) => {
                    let active = value.iter().any(|race| race.status == AiRaceStatus::Running);
                    races.set(value);
                    if !active {
                        return;
                    }
                }
                Err(e) => {
                    error.set(Some(error_message(e)));
                    return;
                }
            }
        }
    });
}

fn status_label(status: AiRaceStatus) -> &'static str {
    tr(status.label())
}

fn money(value: f64) -> String {
    fmt_usd(Decimal::try_from(value).unwrap_or_default(), 2)
}

#[component]
pub fn RacePage() -> Element {
    let mut portfolios = use_signal(Vec::<AiPortfolioInfo>::new);
    let mut races = use_signal(Vec::<AiRace>::new);
    let mut loaded = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let busy = use_signal(|| false);

    use_hook(move || {
        spawn(async move {
            if let Ok(value) = api::get_ai_portfolios().await {
                portfolios.set(value);
            }
            match api::get_ai_races().await {
                Ok(value) => {
                    let active = value.iter().any(|race| race.status == AiRaceStatus::Running);
                    races.set(value);
                    if active {
                        poll_races(races, error);
                    }
                }
                Err(e) => error.set(Some(error_message(e))),
            }
            loaded.set(true);
        });
    });

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("AI races")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0",
                    {tr("Let your AI portfolios compete with the same paper money and the same prices, round by round, and see which strategy does best.")}
                }
            }
            div { class: "mt-10 grid gap-5 motion-safe:animate-rise",
                if let Some(message) = error() {
                    p { class: "text-sm text-ctp-red break-words", "{message}" }
                }
                if !loaded() {
                    p { class: "text-sm text-ctp-subtext0", {tr("Loading…")} }
                } else {
                    for race in races() {
                        RaceCard { key: "{race.id}", race: race.clone(), races, error, busy }
                    }
                    NewRace { portfolios: portfolios(), races, error, busy }
                }
            }
        }
    }
}

/// The form that sets up a race.
#[component]
fn NewRace(
    portfolios: Vec<AiPortfolioInfo>,
    races: Signal<Vec<AiRace>>,
    error: Signal<Option<String>>,
    busy: Signal<bool>,
) -> Element {
    let mut races = races;
    let mut error = error;
    let mut busy = busy;
    let mut selected = use_signal(Vec::<Uuid>::new);
    let mut name = use_signal(|| tr("AI race").to_string());
    let mut capital = use_signal(|| "10000".to_string());
    let mut rounds = use_signal(|| "10".to_string());
    let mut frequency = use_signal(|| "60".to_string());
    let mut deadline = use_signal(|| "120".to_string());

    let create = move |_| async move {
        let parsed = (
            capital().parse::<f64>(),
            rounds().parse::<u32>(),
            frequency().parse::<u32>(),
            deadline().parse::<u32>(),
        );
        let (Ok(starting_capital), Ok(rounds_count), Ok(trading_frequency_minutes), Ok(round_timeout_seconds)) = parsed else {
            return error.set(Some(tr("Enter valid race settings.").to_string()));
        };
        busy.set(true);
        let input = NewAiRace {
            name: name(),
            contestant_ids: selected(),
            starting_capital,
            rounds: rounds_count,
            trading_frequency_minutes,
            round_timeout_seconds,
        };
        match api::create_ai_race(input).await {
            Ok(race) => {
                races.write().insert(0, race);
                selected.write().clear();
                error.set(None);
            }
            Err(e) => error.set(Some(error_message(e))),
        }
        busy.set(false);
    };

    rsx! {
        Card {
            title: tr("New race"),
            subtitle: tr("Pick at least two AI portfolios. Each starts with the same cash.").to_string(),
            div { class: "grid gap-4 text-sm",
                div { class: "grid gap-3 md:grid-cols-2",
                    Field { label: tr("Race name"), input { class: INPUT, value: name, oninput: move |e| name.set(e.value()) } }
                    Field { label: tr("Starting cash per portfolio (USD)"), input { class: INPUT, r#type: "number", min: "1", value: capital, oninput: move |e| capital.set(e.value()) } }
                    Field { label: tr("Rounds"), hint: tr("Up to 365.").to_string(), input { class: INPUT, r#type: "number", min: "1", max: "365", value: rounds, oninput: move |e| rounds.set(e.value()) } }
                    Field { label: tr("Minutes between rounds"), input { class: INPUT, r#type: "number", min: "1", value: frequency, oninput: move |e| frequency.set(e.value()) } }
                    Field { label: tr("Time each AI gets per round (seconds)"), hint: tr("An AI that runs out of time counts as a failed run for that round.").to_string(),
                        input { class: INPUT, r#type: "number", min: "15", max: "600", value: deadline, oninput: move |e| deadline.set(e.value()) }
                    }
                }
                div {
                    div { class: "mb-2 font-medium text-ctp-text", {tr("Contestants")} }
                    div { class: "grid gap-2 sm:grid-cols-2",
                        for portfolio in portfolios.clone() {
                            label { key: "{portfolio.portfolio_id}", class: "flex min-h-10 items-center gap-2 rounded-lg border border-ctp-surface0 px-3 py-2 cursor-pointer",
                                input {
                                    r#type: "checkbox",
                                    checked: selected().contains(&portfolio.portfolio_id),
                                    onchange: move |event| {
                                        if event.checked() {
                                            if !selected().contains(&portfolio.portfolio_id) {
                                                selected.write().push(portfolio.portfolio_id);
                                            }
                                        } else {
                                            selected.write().retain(|id| *id != portfolio.portfolio_id);
                                        }
                                    }
                                }
                                span { class: "truncate", "{portfolio.name}" }
                            }
                        }
                    }
                    if portfolios.len() < 2 {
                        p { class: "mt-2 text-xs text-ctp-peach", {tr("Create and configure at least two AI portfolios first.")} }
                    }
                }
                p { class: "text-xs text-ctp-overlay1", {tr("Starting a race resets every selected paper portfolio to the same cash balance. During running or paused races, model and strategy changes are locked.")} }
                div { class: "flex justify-end",
                    ActionButton { label: if busy() { tr("Creating…") } else { tr("Create race") }, disabled: busy() || selected().len() < 2, onclick: create }
                }
            }
        }
    }
}

/// One race: controls, a summary once it's over, value by round, the
/// leaderboard, and what each contestant did.
#[component]
fn RaceCard(
    race: AiRace,
    races: Signal<Vec<AiRace>>,
    error: Signal<Option<String>>,
    busy: Signal<bool>,
) -> Element {
    let mut races = races;
    let mut error = error;
    let mut busy = busy;
    let mut open = use_signal(|| None::<Uuid>);
    let id = race.id;

    let control = move |action: &'static str| {
        spawn(async move {
            busy.set(true);
            let result = match action {
                "start" => api::start_ai_race(id).await,
                "pause" => api::pause_ai_race(id).await,
                "resume" => api::resume_ai_race(id).await,
                _ => api::stop_ai_race(id).await,
            };
            match result {
                Ok(updated) => {
                    if let Some(r) = races.write().iter_mut().find(|r| r.id == id) {
                        *r = updated;
                    }
                    error.set(None);
                    if action == "start" || action == "resume" {
                        poll_races(races, error);
                    }
                }
                Err(e) => error.set(Some(error_message(e))),
            }
            busy.set(false);
        });
    };

    let active = matches!(race.status, AiRaceStatus::Running | AiRaceStatus::Paused);
    let subtitle = trf(
        "{} · {} of {} rounds · every {} min",
        &[
            &status_label(race.status),
            &race.completed_rounds,
            &race.rounds,
            &race.trading_frequency_minutes,
        ],
    );
    let has_mcp = race.contestants.iter().any(|c| c.mcp);

    rsx! {
        Card {
            title: race.name.clone(),
            subtitle,
            actions: rsx! {
                div { class: "flex flex-wrap gap-2",
                    if race.status == AiRaceStatus::Draft {
                        ActionButton { label: tr("Start"), disabled: busy(), onclick: move |_| control("start") }
                    }
                    if race.status == AiRaceStatus::Running {
                        GhostButton { label: tr("Pause"), onclick: move |_| control("pause") }
                    }
                    if race.status == AiRaceStatus::Paused {
                        ActionButton { label: tr("Resume"), disabled: busy(), onclick: move |_| control("resume") }
                    }
                    if active {
                        GhostButton { label: tr("Stop"), onclick: move |_| control("stop") }
                    }
                    GhostButton { label: tr("⇩ Results"), onclick: move |_| async move {
                        match api::download_ai_race_results(id).await {
                            Ok(csv) => download(&format!("ai-race-{id}.csv"), "text/csv", &csv),
                            Err(e) => error.set(Some(error_message(e))),
                        }
                    } }
                }
            },
            div { class: "grid gap-5",
                RaceSummary { race: race.clone() }
                if race.completed_rounds > 0 {
                    RaceChart { race: race.clone() }
                }
                Leaderboard { race: race.clone(), open }
                if let Some(portfolio_id) = open() {
                    ContestantRuns {
                        key: "{portfolio_id}",
                        race_id: id,
                        portfolio_id,
                        name: race.contestants.iter().find(|c| c.portfolio_id == portfolio_id).map(|c| c.name.clone()).unwrap_or_default(),
                        on_close: move |_| open.set(None),
                    }
                }
                details {
                    summary { class: "cursor-pointer text-xs text-ctp-subtext0", {tr("Race history")} }
                    div { class: "mt-2 grid gap-1 text-xs text-ctp-overlay1",
                        for event in race.audit.clone() {
                            div { key: "{event.sequence}", class: "break-words", "{event.at} · {event.detail}" }
                        }
                    }
                }
                p { class: "text-xs text-ctp-overlay1",
                    {tr("Every AI gets the same deadline each round; a slow or busy provider counts as a failed run. The next round waits for the current one to close.")}
                }
                if has_mcp {
                    p { class: "text-xs text-ctp-overlay1",
                        {tr("Portfolios traded by an outside AI app (MCP) can only trade while their round is open, so have that app check get_my_portfolio often. A round without any call counts as a failed run.")}
                    }
                }
            }
        }
    }
}

/// Winner and headline numbers, once a race is over; the leader while it runs.
#[component]
fn RaceSummary(race: AiRace) -> Element {
    let Some(leader) = race.leaderboard.first().filter(|_| race.completed_rounds > 0) else {
        return rsx! {};
    };
    let over = race.winner().is_some();
    let failed: u32 = race.leaderboard.iter().map(|r| r.failed_model_runs).sum();
    let bench = race.benchmark_return_pct();
    let beat = bench.map(|b| leader.total_return_pct - b);
    fn tone(v: f64) -> &'static str {
        if v >= 0.0 { "text-ctp-green" } else { "text-ctp-red" }
    }
    let leader_label = if over { tr("Winner") } else { tr("Leading") }.to_string();
    let leader_name = leader.name.clone();
    let leader_hint = format!("{:+.2}%", leader.total_return_pct);
    let leader_tone = tone(leader.total_return_pct);
    let bench_value = bench.map(|b| format!("{b:+.2}%")).unwrap_or_else(|| "—".into());
    let bench_hint = race
        .benchmark
        .as_ref()
        .map(|b| b.ticker.clone())
        .unwrap_or_else(|| tr("Not recorded for this race").to_string());
    let bench_tone = bench.map(tone).unwrap_or("text-ctp-text");
    let beat_value = beat.map(|b| format!("{b:+.2} pt")).unwrap_or_else(|| "—".into());
    let beat_tone = beat.map(tone).unwrap_or("text-ctp-text");
    let failed_hint = trf("{} of {} rounds", &[&race.completed_rounds, &race.rounds]);
    let failed_tone = if failed > 0 { "text-ctp-peach" } else { "text-ctp-text" };
    rsx! {
        div { class: "grid grid-cols-2 gap-3 lg:grid-cols-4",
            MetricTile { label: leader_label, value: leader_name, hint: leader_hint, tone: leader_tone }
            MetricTile { label: tr("Benchmark").to_string(), value: bench_value, hint: bench_hint, tone: bench_tone }
            MetricTile { label: tr("Leader vs benchmark").to_string(), value: beat_value, tone: beat_tone }
            MetricTile { label: tr("Failed runs").to_string(), value: failed.to_string(), hint: failed_hint, tone: failed_tone }
        }
    }
}

/// Each contestant's value by round, with the benchmark for comparison.
#[component]
fn RaceChart(race: AiRace) -> Element {
    let rounds = race.completed_rounds;
    let labels: Vec<String> = (0..=rounds)
        .map(|r| if r == 0 { tr("Start").to_string() } else { trf("Round {}", &[&r]) })
        .collect();
    let line = |points: Vec<(u32, f64)>| -> Vec<Option<Decimal>> {
        let mut values = vec![None; rounds as usize + 1];
        for (round, value) in points {
            if let Some(slot) = values.get_mut(round as usize) {
                *slot = Decimal::try_from(value).ok();
            }
        }
        values
    };
    let mut series: Vec<Series> = race
        .contestants
        .iter()
        .enumerate()
        .map(|(i, c)| Series {
            name: c.name.clone(),
            color: CHART_COLORS_HEX[i % CHART_COLORS_HEX.len()].to_string(),
            values: line(race.values_of(c.portfolio_id)),
        })
        .collect();
    if let Some(b) = &race.benchmark {
        series.push(Series {
            name: trf("{} (benchmark)", &[&b.ticker]),
            color: CHART_NEUTRAL_HEX.to_string(),
            values: line(b.points.clone()),
        });
    }
    rsx! {
        div {
            div { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-ctp-subtext0", {tr("Value by round")} }
            GrowthChart { chart_dates: labels, series, height: Decimal::from(240) }
        }
    }
}

#[component]
fn Leaderboard(race: AiRace, open: Signal<Option<Uuid>>) -> Element {
    let mut open = open;
    rsx! {
        div {
            div { class: "mb-2 text-xs text-ctp-overlay1", {tr("Tap a contestant to see what it did each round.")} }
            div { class: "overflow-x-auto",
                table { class: "w-full min-w-[720px] text-left text-xs",
                    thead { tr { class: "text-ctp-overlay1",
                        th { class: "p-2", "#" }
                        th { class: "p-2", {tr("Contestant")} }
                        th { class: "p-2 text-right", {tr("Return")} }
                        th { class: "p-2 text-right", {tr("Drawdown")} }
                        th { class: "p-2 text-right", {tr("Volatility")} }
                        th { class: "p-2 text-right", {tr("Risk-adjusted")} }
                        th { class: "p-2 text-right", {tr("Fees")} }
                        th { class: "p-2 text-right", {tr("Turnover")} }
                        th { class: "p-2 text-right", {tr("Cash")} }
                        th { class: "p-2 text-right", {tr("Failed runs")} }
                    } }
                    tbody {
                        for row in race.leaderboard.clone() {
                            tr {
                                key: "{row.portfolio_id}",
                                class: if open() == Some(row.portfolio_id) { "border-t border-ctp-surface0/60 bg-ctp-surface0/40 cursor-pointer" } else { "border-t border-ctp-surface0/60 cursor-pointer hover:bg-ctp-surface0/30" },
                                onclick: move |_| {
                                    let id = row.portfolio_id;
                                    open.set(if open() == Some(id) { None } else { Some(id) });
                                },
                                td { class: "p-2 tabular-nums", "{row.rank}" }
                                td { class: "p-2 font-medium text-ctp-text",
                                    "{row.name}"
                                    if race.contestants.iter().any(|c| c.portfolio_id == row.portfolio_id && c.mcp) {
                                        span { class: "ml-2 rounded bg-ctp-surface0 px-1.5 py-0.5 text-[10px] text-ctp-subtext0", "MCP" }
                                    }
                                }
                                td { class: if row.total_return_pct >= 0.0 { "p-2 text-right tabular-nums text-ctp-green" } else { "p-2 text-right tabular-nums text-ctp-red" },
                                    "{row.total_return_pct:+.2}%"
                                }
                                td { class: "p-2 text-right tabular-nums", "{row.max_drawdown_pct:.2}%" }
                                td { class: "p-2 text-right tabular-nums", "{row.volatility_pct:.2}%" }
                                td { class: "p-2 text-right tabular-nums", "{row.risk_adjusted_return:.2}" }
                                td { class: "p-2 text-right tabular-nums", {money(row.fees)} }
                                td { class: "p-2 text-right tabular-nums", {money(row.turnover)} }
                                td { class: "p-2 text-right tabular-nums", "{row.cash_allocation_pct:.1}%" }
                                td { class: if row.failed_model_runs > 0 { "p-2 text-right tabular-nums text-ctp-peach" } else { "p-2 text-right tabular-nums" }, "{row.failed_model_runs}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What one contestant did in each round: its summary, and every tool call
/// with the reason it gave.
#[component]
fn ContestantRuns(race_id: Uuid, portfolio_id: Uuid, name: String, on_close: EventHandler<()>) -> Element {
    let runs = use_resource(move || async move { api::get_ai_race_runs(race_id, portfolio_id).await });
    rsx! {
        div { class: "rounded-xl border border-ctp-surface0 p-4",
            div { class: "mb-3 flex items-center justify-between gap-3",
                div { class: "font-medium text-ctp-text", {trf("{} round by round", &[&name])} }
                GhostButton { label: tr("Close"), onclick: move |_| on_close.call(()) }
            }
            match &*runs.read() {
                None => rsx! { p { class: "text-xs text-ctp-subtext0", {tr("Loading…")} } },
                Some(Err(e)) => rsx! { p { class: "text-xs text-ctp-red break-words", {error_message(e.clone())} } },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-ctp-subtext0", {tr("No rounds yet.")} }
                },
                Some(Ok(list)) => rsx! {
                    div { class: "grid gap-2",
                        for item in list.clone() {
                            RoundRun { key: "{item.run.id}", item }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn RoundRun(item: AiRaceRun) -> Element {
    let run = &item.run;
    let (label, tone) = match run.status {
        AiRunStatus::Running => (tr("Running"), "text-ctp-sky"),
        AiRunStatus::Completed => (tr("Done"), "text-ctp-green"),
        AiRunStatus::Failed => (tr("Failed"), "text-ctp-red"),
    };
    let trades = run.events.iter().filter(|e| e.tool == "place_order" && e.success).count();
    rsx! {
        details { class: "rounded-lg border border-ctp-surface0/70 px-3 py-2 text-xs",
            summary { class: "flex min-h-8 cursor-pointer flex-wrap items-center gap-x-3 gap-y-1",
                span { class: "font-medium text-ctp-text", {trf("Round {}", &[&item.round])} }
                span { class: tone, "{label}" }
                span { class: "text-ctp-subtext0", {trf("{} trades · {} tool calls", &[&trades, &run.events.len()])} }
            }
            div { class: "mt-2 grid gap-2",
                if let Some(summary) = &run.final_response {
                    p { class: "whitespace-pre-wrap break-words text-ctp-subtext1", "{summary}" }
                }
                if let Some(error) = &run.error {
                    p { class: "break-words text-ctp-red", "{error}" }
                }
                for event in run.events.clone() {
                    div { key: "{event.sequence}", class: "rounded bg-ctp-surface0/40 px-2 py-1",
                        div { class: if event.success { "font-mono text-ctp-text" } else { "font-mono text-ctp-red" }, "{event.tool}" }
                        if let Some(reason) = serde_json::from_str::<serde_json::Value>(&event.arguments).ok().and_then(|v| v["reason"].as_str().map(str::to_string)) {
                            div { class: "text-ctp-subtext0 break-words", "“{reason}”" }
                        }
                        if !event.success {
                            div { class: "text-ctp-red break-words", "{event.detail}" }
                        }
                    }
                }
            }
        }
    }
}
