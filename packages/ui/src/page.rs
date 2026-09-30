//! Page shell and hero shared by the dashboard and portfolio pages.

use crate::i18n::tr;
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
    // A newly opened page starts at the top.
    use_hook(|| {
        document::eval("window.scrollTo(0, 0); document.querySelector('[data-scroll-root]')?.scrollTo(0, 0);");
    });
    // Navigation asked for by a dialog, which can't use the router itself.
    let crate::app::PendingNav(mut pending) = use_context::<crate::app::PendingNav>();
    use_effect(move || {
        if let Some(path) = pending() {
            pending.set(None);
            navigator().push(path);
        }
    });
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        div { class: "{crate::theme::theme_class()} relative min-h-screen overflow-hidden bg-ctp-base text-ctp-text",
            // Soft colour glows as plain gradients: blur filters are costly
            // to repaint (notably in the desktop webview).
            div {
                class: "decor pointer-events-none absolute inset-x-0 top-0 h-[28rem]",
                aria_hidden: "true",
                style: "background:\
                    radial-gradient(22rem 16rem at 8% 0%, color-mix(in oklab, var(--catppuccin-color-mauve) 18%, transparent), transparent 70%),\
                    radial-gradient(20rem 14rem at 45% 0%, color-mix(in oklab, var(--catppuccin-color-pink) 13%, transparent), transparent 70%),\
                    radial-gradient(22rem 16rem at 95% 0%, color-mix(in oklab, var(--catppuccin-color-sky) 13%, transparent), transparent 70%);",
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
    /// A label shown beside the title, e.g. for an AI-managed portfolio.
    #[props(default)]
    badge: Option<String>,
    /// A cover picture (URL or `data:` URL) shown as a banner above the
    /// title.
    #[props(default)]
    cover: Option<String>,
    children: Element,
) -> Element {
    let up = day_change >= Decimal::ZERO;
    rsx! {
        header { class: "motion-safe:animate-rise",
            if let Some(cover) = cover {
                div {
                    class: "relative -mx-4 mb-6 h-36 overflow-hidden bg-ctp-surface0 bg-cover bg-center sm:mx-0 sm:h-52 sm:rounded-3xl",
                    style: "background-image: url('{cover}');",
                    role: "img",
                    "aria-label": tr("Cover picture"),
                    // Fades into the page so the header below reads on any picture.
                    div { class: "absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-ctp-base/80 to-transparent" }
                }
            }
            // On phones the status gets its own line and the buttons wrap
            // under it, instead of squeezing the status into a narrow column.
            div { class: "mb-4 flex flex-col items-start gap-3 sm:flex-row sm:items-center sm:justify-between sm:gap-4",
                div { class: "flex items-center gap-2 text-xs text-ctp-subtext0",
                    if loaded {
                        span { class: "relative flex h-2 w-2",
                            span { class: "absolute inline-flex h-full w-full rounded-full bg-ctp-green opacity-60 motion-safe:animate-[ping_1s_cubic-bezier(0,0,0.2,1)_3]" }
                            span { class: "relative inline-flex h-2 w-2 rounded-full bg-ctp-green" }
                        }
                        {tr("Live prices")}
                    } else {
                        span { class: "h-2 w-2 rounded-full bg-ctp-overlay0" }
                        {tr("Fetching prices…")}
                    }
                }
                if let Some(actions) = actions {
                    {actions}
                }
            }
            div { class: "flex flex-wrap items-center gap-3",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 \
                             bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    "{title}"
                }
                if let Some(badge) = badge {
                    AiBadge { label: badge }
                }
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
                    span { class: "font-normal opacity-70", {tr("today")} }
                }
                {children}
            }
        }
    }
}

/// Marks a portfolio managed by an AI assistant.
#[component]
pub fn AiBadge(#[props(default = "AI".to_string())] label: String) -> Element {
    rsx! {
        span {
            class: "inline-flex shrink-0 items-center gap-1 rounded-full bg-ctp-mauve/15 px-2 py-0.5 text-xs font-medium text-ctp-mauve",
            title: tr("An AI assistant manages this portfolio with paper money"),
            "🤖 {label}"
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
            span { class: "text-ctp-subtext0", "{label}" }
            span { class: "font-medium tabular-nums {color}", "{value}" }
        }
    }
}

/// Pill-shaped secondary button for page headers.
#[component]
///
/// `primary` marks the page's main action (filled) so it stands out from
/// the secondary ones.
pub fn GhostButton(
    label: String,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    #[props(default)] primary: bool,
) -> Element {
    let style = if primary {
        "border-ctp-mauve bg-ctp-mauve text-ctp-crust font-semibold hover:brightness-110"
    } else {
        "border-ctp-surface1 text-ctp-subtext1 font-medium hover:border-ctp-mauve hover:text-ctp-text"
    };
    rsx! {
        button {
            class: "min-h-9 rounded-full border px-4 py-1.5 text-sm cursor-pointer transition-colors {style}",
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}
