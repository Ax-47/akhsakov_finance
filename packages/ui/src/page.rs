//! Page shell and hero shared by the dashboard and portfolio pages.

use crate::{
    format::{fmt_signed, fmt_usd},
    LiveNumber,
};
use dioxus::prelude::*;
use rust_decimal::Decimal;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Mocha background with soft colour glows and a centred content column.
#[component]
pub fn Page(children: Element) -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        div { class: "mocha relative min-h-screen overflow-hidden bg-ctp-base text-ctp-text",
            div { class: "pointer-events-none absolute inset-x-0 top-0 h-[28rem] overflow-hidden", aria_hidden: "true",
                div { class: "absolute -top-32 -left-24 h-80 w-80 rounded-full bg-ctp-mauve/20 blur-3xl" }
                div { class: "absolute -top-20 left-1/3 h-72 w-72 rounded-full bg-ctp-pink/15 blur-3xl" }
                div { class: "absolute -top-28 right-0 h-80 w-80 rounded-full bg-ctp-sky/15 blur-3xl" }
            }
            main { class: "relative mx-auto max-w-6xl px-4 sm:px-8 py-10 sm:py-14", {children} }
        }
    }
}

/// Live status, gradient title, big live total, today's move and a row of
/// [`HeroStat`]s passed as children.
#[component]
pub fn PageHero(
    title: String,
    loaded: bool,
    total_value: Decimal,
    day_change: Decimal,
    day_pct: Decimal,
    /// Buttons shown top-right.
    #[props(default)]
    actions: Option<Element>,
    children: Element,
) -> Element {
    let up = day_change >= Decimal::ZERO;
    rsx! {
        header { class: "motion-safe:animate-rise",
            div { class: "flex items-center justify-between gap-4 mb-4",
                div { class: "flex items-center gap-2 text-xs text-ctp-overlay1",
                    if loaded {
                        span { class: "relative flex h-2 w-2",
                            span { class: "absolute inline-flex h-full w-full rounded-full bg-ctp-green opacity-60 motion-safe:animate-ping" }
                            span { class: "relative inline-flex h-2 w-2 rounded-full bg-ctp-green" }
                        }
                        "Live prices"
                    } else {
                        span { class: "h-2 w-2 rounded-full bg-ctp-overlay0" }
                        "Fetching prices…"
                    }
                }
                if let Some(actions) = actions {
                    {actions}
                }
            }
            h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 \
                         bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                "{title}"
            }
            div { class: "mt-4 text-5xl sm:text-6xl font-semibold tracking-tight tabular-nums text-ctp-text",
                LiveNumber { value: total_value, text: fmt_usd(total_value, 2) }
            }
            div { class: "mt-5 flex flex-wrap items-center gap-x-6 gap-y-3 text-sm",
                span {
                    class: if up {
                        "inline-flex items-center gap-1.5 rounded-full px-3 py-1 font-semibold bg-ctp-green/15 text-ctp-green"
                    } else {
                        "inline-flex items-center gap-1.5 rounded-full px-3 py-1 font-semibold bg-ctp-red/15 text-ctp-red"
                    },
                    if up { "▲" } else { "▼" }
                    " {fmt_signed(day_change, 2)} ({day_pct:+.2}%)"
                    span { class: "font-normal opacity-70", "today" }
                }
                {children}
            }
        }
    }
}

#[component]
pub fn HeroStat(
    label: String,
    value: String,
    #[props(default = "text-ctp-text")] color: &'static str,
) -> Element {
    rsx! {
        span { class: "flex items-baseline gap-2",
            span { class: "text-ctp-overlay1", "{label}" }
            span { class: "font-medium tabular-nums {color}", "{value}" }
        }
    }
}

/// Pill-shaped secondary button for page headers.
#[component]
pub fn GhostButton(label: String, #[props(default)] onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            class: "rounded-full border border-ctp-surface1 px-3.5 py-1.5 text-xs font-medium text-ctp-subtext1 \
                    cursor-pointer transition-colors hover:border-ctp-mauve hover:text-ctp-text",
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}
