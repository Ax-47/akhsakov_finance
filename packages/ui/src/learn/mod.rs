//! Learn: short lessons on analysing a stock and understanding risk. Each
//! has calculators, links to where the idea shows up in the app, and one
//! question to check it landed. Lessons answered correctly are ticked on
//! this device.

mod lessons;
mod math;
mod tools;

use crate::{
    components::card::{Card, CARD},
    i18n::{tr, trf},
    page::Page,
};
use dioxus::prelude::*;
use lessons::{find, Block, Go, Lesson, Track, LESSONS};
use tools::ToolCard;

const DONE_KEY: &str = "akhsakov.learn.done";

/// Slugs of lessons whose question was answered correctly.
static DONE: GlobalSignal<Vec<&'static str>> = Signal::global(Vec::new);
static DONE_LOADED: GlobalSignal<bool> = Signal::global(|| false);

/// Loads saved progress once (after hydration: it lives in localStorage).
fn use_progress() {
    use_future(|| async {
        if *DONE_LOADED.peek() {
            return;
        }
        let saved = document::eval(&format!(
            "try {{ return localStorage.getItem('{DONE_KEY}') || ''; }} catch (e) {{ return ''; }}"
        ))
        .join::<String>()
        .await
        .unwrap_or_default();
        *DONE_LOADED.write() = true;
        let mut done = DONE.write();
        for slug in saved.split(',').filter_map(|s| find(s).map(|i| LESSONS[i].slug)) {
            if !done.contains(&slug) {
                done.push(slug);
            }
        }
    });
}

fn mark_done(slug: &'static str) {
    if DONE.peek().contains(&slug) {
        return;
    }
    DONE.write().push(slug);
    let text = DONE.peek().join(",");
    document::eval(&format!(
        "try {{ localStorage.setItem('{DONE_KEY}', {text:?}); }} catch (e) {{}}"
    ));
}

const TITLE: &str = "text-3xl sm:text-4xl font-bold tracking-tight pb-1 \
                     bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent";

// ─── Index ────────────────────────────────────────────────────────────────────

/// Route target for `/learn`: every lesson, by track, with progress.
#[component]
pub fn LearnPage() -> Element {
    use_progress();
    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: TITLE, {tr("Learn")} }
                p { class: "mt-2 max-w-2xl text-sm text-ctp-subtext0",
                    {tr("Short lessons on analysing stocks and understanding risk, each with calculators and a quick question at the end.")}
                }
            }
            for track in Track::ALL {
                TrackSection { key: "{track:?}", track }
            }
            Disclaimer {}
        }
    }
}

#[component]
fn TrackSection(track: Track) -> Element {
    let lessons: Vec<(usize, &'static Lesson)> = track.lessons().collect();
    let done = lessons
        .iter()
        .filter(|(_, l)| DONE.read().contains(&l.slug))
        .count();
    let share = done as f64 / lessons.len().max(1) as f64 * 100.0;
    rsx! {
        section { class: "mt-10 motion-safe:animate-rise",
            div { class: "flex flex-wrap items-end justify-between gap-3",
                div {
                    h2 { class: "text-xl font-semibold text-ctp-text", "{track.title().get()}" }
                    p { class: "mt-1 text-sm text-ctp-subtext0", "{track.blurb().get()}" }
                }
                span { class: "text-xs tabular-nums text-ctp-subtext0",
                    {trf("{} of {} done", &[&done, &lessons.len()])}
                }
            }
            div { class: "mt-3 h-1 rounded-full bg-ctp-surface0/70",
                div { class: "h-full rounded-full bg-ctp-green transition-[width]", style: "width:{share:.0}%;" }
            }
            div { class: "mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-3",
                for (n, (index, _)) in lessons.iter().enumerate() {
                    LessonCard { key: "{index}", index: *index, number: n + 1 }
                }
            }
        }
    }
}

#[component]
fn LessonCard(index: usize, number: usize) -> Element {
    let lesson = &LESSONS[index];
    let done = DONE.read().contains(&lesson.slug);
    rsx! {
        Link {
            to: format!("/learn/{}", lesson.slug),
            class: "{CARD} group flex flex-col p-5",
            div { class: "flex items-center justify-between gap-2 text-xs text-ctp-subtext0",
                span { {trf("Lesson {} · {} min", &[&number, &lesson.minutes])} }
                if done {
                    span { class: "rounded-full bg-ctp-green/15 px-2 py-0.5 font-semibold text-ctp-green", {tr("✓ Done")} }
                } else if !lesson.tools.is_empty() {
                    span { class: "rounded-full bg-ctp-mauve/15 px-2 py-0.5 font-semibold text-ctp-mauve", {tr("Calculator")} }
                }
            }
            h3 { class: "mt-2 font-semibold text-ctp-text transition-colors group-hover:text-ctp-mauve", "{lesson.title.get()}" }
            p { class: "mt-1 text-sm leading-relaxed text-ctp-subtext0", "{lesson.summary.get()}" }
        }
    }
}

#[component]
fn Disclaimer() -> Element {
    rsx! {
        p { class: "mt-10 text-xs text-ctp-overlay1",
            {tr("For learning only, not investment advice. Past numbers don't promise future results.")}
        }
    }
}

// ─── Lesson ───────────────────────────────────────────────────────────────────

/// Route target for `/learn/:slug`.
#[component]
pub fn LessonPage(slug: String) -> Element {
    use_progress();
    match find(&slug) {
        // Keyed, so moving to the next lesson starts it fresh (top of the
        // page, quiz unanswered) instead of reusing this one's state.
        Some(index) => rsx! {
            for index in [index] {
                LessonView { key: "{index}", index }
            }
        },
        None => rsx! {
            Page {
                p { class: "text-sm text-ctp-subtext0", {tr("There's no such lesson.")} }
                Link { to: "/learn", class: "mt-3 inline-block text-sm text-ctp-mauve hover:underline", {tr("← All lessons")} }
            }
        },
    }
}

#[component]
fn LessonView(index: usize) -> Element {
    let lesson = &LESSONS[index];
    let siblings: Vec<usize> = lesson.track.lessons().map(|(i, _)| i).collect();
    let position = siblings.iter().position(|i| *i == index).unwrap_or(0);
    // Tracks follow each other in `LESSONS`, so this runs on from one
    // track's last lesson into the next track.
    let previous = index.checked_sub(1);
    let next = Some(index + 1).filter(|i| *i < LESSONS.len());

    rsx! {
        Page {
            div { class: "mx-auto max-w-3xl",
                Link { to: "/learn", class: "text-sm text-ctp-subtext0 transition-colors hover:text-ctp-text", {tr("← All lessons")} }
                header { class: "mt-4 motion-safe:animate-rise",
                    div { class: "text-xs text-ctp-subtext0",
                        "{lesson.track.title().get()} · "
                        {trf("Lesson {} of {} · {} min read", &[&(position + 1), &siblings.len(), &lesson.minutes])}
                    }
                    h1 { class: "mt-1 {TITLE}", "{lesson.title.get()}" }
                    p { class: "mt-2 text-ctp-subtext0", "{lesson.summary.get()}" }
                }
                article { class: "mt-8 flex flex-col gap-5 motion-safe:animate-rise",
                    for i in 0..lesson.body.len() {
                        BlockView { key: "{i}", index, block: i }
                    }
                }
                div { class: "mt-8 grid gap-5",
                    for tool in lesson.tools.iter().copied() {
                        ToolCard { key: "{tool:?}", tool }
                    }
                    SeeInApp { index }
                    QuizCard { index }
                }
                nav { class: "mt-8 grid gap-3 sm:grid-cols-2",
                    if let Some(i) = previous {
                        NeighbourLink { index: i, label: tr("← Previous") }
                    } else {
                        div {}
                    }
                    if let Some(i) = next {
                        NeighbourLink { index: i, label: tr("Next →"), right: true }
                    }
                }
                Disclaimer {}
            }
        }
    }
}

#[component]
fn BlockView(index: usize, block: usize) -> Element {
    let text = "text-[15px] leading-7 text-ctp-subtext1";
    match &LESSONS[index].body[block] {
        Block::P(p) => rsx! { p { class: text, "{p.get()}" } },
        Block::H(h) => rsx! { h2 { class: "pt-2 text-lg font-semibold text-ctp-text", "{h.get()}" } },
        Block::List(items) => rsx! {
            ul { class: "flex flex-col gap-2.5",
                for (i, item) in items.iter().enumerate() {
                    li { key: "{i}", class: "flex gap-3 {text}",
                        span { class: "mt-2.5 h-1.5 w-1.5 shrink-0 rounded-full bg-ctp-mauve" }
                        span { "{item.get()}" }
                    }
                }
            }
        },
        Block::Terms(terms) => rsx! {
            dl { class: "overflow-hidden rounded-2xl border border-ctp-surface0/70 bg-ctp-mantle/60",
                for (i, (term, meaning)) in terms.iter().enumerate() {
                    div { key: "{i}", class: "grid gap-1 border-t border-ctp-surface0/60 px-5 py-3.5 first:border-t-0 sm:grid-cols-[11rem_1fr] sm:gap-5",
                        dt { class: "text-sm font-semibold text-ctp-text", "{term.get()}" }
                        dd { class: "text-sm leading-6 text-ctp-subtext1", "{meaning.get()}" }
                    }
                }
            }
        },
        Block::Formula(formula, note) => rsx! {
            div { class: "rounded-2xl border border-ctp-mauve/30 bg-ctp-mauve/5 px-5 py-4",
                div { class: "font-mono text-sm font-semibold text-ctp-mauve", "{formula.get()}" }
                p { class: "mt-2 text-sm leading-6 text-ctp-subtext1", "{note.get()}" }
            }
        },
        Block::Tip(tip) => rsx! {
            Callout { label: tr("Key point"), text: tip.get(), style: "border-ctp-green bg-ctp-green/10", label_style: "text-ctp-green" }
        },
        Block::Warn(warn) => rsx! {
            Callout { label: tr("Watch out"), text: warn.get(), style: "border-ctp-peach bg-ctp-peach/10", label_style: "text-ctp-peach" }
        },
    }
}

#[component]
fn Callout(label: String, text: String, style: &'static str, label_style: &'static str) -> Element {
    rsx! {
        div { class: "rounded-r-2xl border-l-4 px-5 py-4 {style}",
            div { class: "text-xs font-semibold uppercase tracking-wide {label_style}", "{label}" }
            p { class: "mt-1 text-sm leading-6 text-ctp-text", "{text}" }
        }
    }
}

/// Links to where the lesson's idea shows up in the app.
#[component]
fn SeeInApp(index: usize) -> Element {
    let link = "flex items-center justify-between gap-3 rounded-2xl border border-ctp-surface0/70 bg-ctp-base/50 \
                px-4 py-3 text-left text-sm text-ctp-subtext1 cursor-pointer transition-colors \
                hover:border-ctp-mauve hover:text-ctp-text";
    rsx! {
        Card { title: tr("See it in the app"), subtitle: tr("Where this shows up with real numbers").to_string(),
            div { class: "grid gap-2",
                for (i, (label, go)) in LESSONS[index].see.iter().enumerate() {
                    match *go {
                        Go::Page(path) => rsx! {
                            Link { key: "{i}", to: path, class: link, span { "{label.get()}" } span { "→" } }
                        },
                        Go::Portfolio(tab) => rsx! {
                            button { key: "{i}", class: link, onclick: move |_| crate::dashboard::open_tab(tab),
                                span { "{label.get()}" } span { "→" }
                            }
                        },
                        Go::Stock(ticker, tab) => rsx! {
                            button { key: "{i}", class: link, onclick: move |_| crate::stock_page::open_tab(ticker, tab),
                                span { "{label.get()} · {ticker}" } span { "→" }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn QuizCard(index: usize) -> Element {
    let lesson = &LESSONS[index];
    let quiz = &lesson.quiz;
    let mut picked = use_signal(|| None::<usize>);
    let done = DONE.read().contains(&lesson.slug);
    let right = picked() == Some(quiz.answer);
    let subtitle = if done { tr("✓ Done") } else { tr("Pick an answer") };
    let option_class = move |i: usize| {
        let state = match picked() {
            Some(p) if p == i && i == quiz.answer => "border-ctp-green bg-ctp-green/10 text-ctp-text",
            Some(p) if p == i => "border-ctp-red bg-ctp-red/10 text-ctp-text",
            _ => "border-ctp-surface0/70 bg-ctp-base/50 text-ctp-subtext1 hover:border-ctp-mauve hover:text-ctp-text",
        };
        format!("rounded-2xl border px-4 py-3 text-left text-sm cursor-pointer transition-colors {state}")
    };

    rsx! {
        Card {
            title: tr("Check yourself"),
            subtitle: subtitle.to_string(),
            p { class: "text-[15px] leading-7 text-ctp-text", "{quiz.question.get()}" }
            div { class: "mt-4 grid gap-2", role: "radiogroup",
                for (i, option) in quiz.options.iter().enumerate() {
                    button {
                        key: "{i}",
                        role: "radio",
                        "aria-checked": picked() == Some(i),
                        class: option_class(i),
                        onclick: move |_| {
                            picked.set(Some(i));
                            if i == quiz.answer {
                                mark_done(lesson.slug);
                            }
                        },
                        "{option.get()}"
                    }
                }
            }
            if right {
                div { class: "mt-4 rounded-2xl bg-ctp-green/10 px-4 py-3", role: "status",
                    div { class: "text-sm font-semibold text-ctp-green", {tr("Correct!")} }
                    p { class: "mt-1 text-sm leading-6 text-ctp-text", "{quiz.why.get()}" }
                }
            } else if picked().is_some() {
                p { class: "mt-4 text-sm text-ctp-red", role: "status", {tr("Not quite. Try another answer.")} }
            }
        }
    }
}

#[component]
fn NeighbourLink(index: usize, label: String, #[props(default)] right: bool) -> Element {
    let lesson = &LESSONS[index];
    rsx! {
        Link {
            to: format!("/learn/{}", lesson.slug),
            class: if right { "{CARD} block p-4 text-right sm:col-start-2" } else { "{CARD} block p-4" },
            div { class: "text-xs text-ctp-subtext0", "{label}" }
            div { class: "mt-1 text-sm font-semibold text-ctp-text", "{lesson.title.get()}" }
        }
    }
}
