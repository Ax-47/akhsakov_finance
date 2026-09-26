//! Card surface and small controls shared by the portfolio page.

use crate::i18n::tr;
use dioxus::prelude::*;

/// Rounded, hairline-bordered, slightly translucent surface.
/// `min-w-0`: inside grids and flex rows a card must be allowed to shrink,
/// so wide tables scroll within it instead of widening the page.
pub const CARD: &str = "min-w-0 rounded-3xl border border-ctp-surface0/70 bg-ctp-mantle/60 \
                        transition-colors hover:border-ctp-surface1/80";

#[component]
pub fn Card(
    title: String,
    #[props(default)] subtitle: Option<String>,
    /// Controls shown at the right of the header.
    #[props(default)]
    actions: Option<Element>,
    /// Skip body padding, for tables that run edge to edge.
    #[props(default)]
    flush: bool,
    children: Element,
) -> Element {
    rsx! {
        section { class: CARD,
            header { class: "flex flex-wrap items-center justify-between gap-3 px-6 pt-5 pb-4",
                div {
                    h2 { class: "text-base font-semibold text-ctp-text", "{title}" }
                    if let Some(subtitle) = subtitle {
                        p { class: "text-xs text-ctp-subtext0 mt-0.5", "{subtitle}" }
                    }
                }
                if let Some(actions) = actions {
                    {actions}
                }
            }
            div { class: if !flush { "px-6 pb-6" }, {children} }
        }
    }
}

/// Pill-shaped segmented control container; fill it with [`ToggleButton`]s.
#[component]
pub fn Segmented(children: Element) -> Element {
    rsx! {
        div { class: "inline-flex gap-0.5 rounded-full border border-ctp-surface0 bg-ctp-crust/40 p-0.5",
            {children}
        }
    }
}

#[component]
pub fn ToggleButton(label: String, active: bool, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            class: if active {
                "min-h-8 px-3.5 py-1.5 rounded-full text-sm font-semibold bg-ctp-surface0 text-ctp-text cursor-pointer transition-colors"
            } else {
                "min-h-8 px-3.5 py-1.5 rounded-full text-sm font-medium text-ctp-subtext0 hover:text-ctp-text cursor-pointer transition-colors"
            },
            aria_pressed: active,
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}

/// Small labelled number with an optional one-line explanation.
#[component]
pub fn MetricTile(
    label: String,
    value: String,
    #[props(default)] hint: String,
    #[props(default = "text-ctp-text")] tone: &'static str,
) -> Element {
    rsx! {
        div { class: "rounded-2xl border border-ctp-surface0/70 bg-ctp-base/50 px-4 py-3",
            div { class: "text-xs text-ctp-subtext0", "{label}" }
            div { class: "mt-1 text-xl font-semibold tabular-nums {tone}", "{value}" }
            if !hint.is_empty() {
                div { class: "mt-0.5 text-xs text-ctp-overlay1 leading-snug", "{hint}" }
            }
        }
    }
}

/// Themed number field with its own up / down chevrons. Native spinners
/// can't be styled consistently (WebKitGTK ignores most CSS for them), so
/// this uses a text input plus buttons. Arrow keys step too. An empty
/// `value` steps from `placeholder`.
#[component]
pub fn Stepper(
    value: String,
    step: f64,
    aria_label: String,
    on_change: EventHandler<String>,
    #[props(default)] placeholder: String,
    #[props(default = 2)] decimals: usize,
    /// Text inside the pill before the number, e.g. "Risk-free".
    #[props(default)]
    label: String,
    /// Text after the number, e.g. "%".
    #[props(default)]
    suffix: String,
    #[props(default = "w-12")] width: &'static str,
) -> Element {
    let base = value
        .trim()
        .parse::<f64>()
        .or_else(|_| placeholder.trim().parse::<f64>())
        .unwrap_or(0.0);
    let up = format!("{:.*}", decimals, base + step);
    let down = format!("{:.*}", decimals, base - step);
    let (key_up, key_down) = (up.clone(), down.clone());

    rsx! {
        div { class: "inline-flex items-center gap-1.5 rounded-full border border-ctp-surface0 bg-ctp-crust/40 \
                      pl-3 pr-1 py-0.5 text-xs text-ctp-subtext0 transition-colors \
                      hover:border-ctp-surface1 focus-within:border-ctp-mauve",
            if !label.is_empty() {
                span { "{label}" }
            }
            input {
                r#type: "text",
                inputmode: "decimal",
                "aria-label": "{aria_label}",
                placeholder: "{placeholder}",
                value: "{value}",
                class: "{width} bg-transparent text-right text-sm tabular-nums text-ctp-text \
                        placeholder:text-ctp-subtext0 outline-none",
                oninput: move |e| on_change.call(e.value()),
                onkeydown: move |e| match e.key() {
                    Key::ArrowUp => {
                        e.prevent_default();
                        on_change.call(key_up.clone());
                    }
                    Key::ArrowDown => {
                        e.prevent_default();
                        on_change.call(key_down.clone());
                    }
                    _ => {}
                },
            }
            if !suffix.is_empty() {
                span { "{suffix}" }
            }
            div { class: "flex flex-col",
                StepButton { up: true, onclick: move |_| on_change.call(up.clone()) }
                StepButton { up: false, onclick: move |_| on_change.call(down.clone()) }
            }
        }
    }
}

#[component]
fn StepButton(up: bool, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            r#type: "button",
            tabindex: "-1",
            "aria-label": if up { tr("Increase") } else { tr("Decrease") },
            class: "flex h-3 w-5 items-center justify-center rounded-full cursor-pointer \
                    text-ctp-subtext0 transition-colors hover:bg-ctp-surface0 hover:text-ctp-mauve \
                    active:text-ctp-pink",
            onclick: move |e| onclick.call(e),
            svg {
                class: if up { "h-1.5 w-2.5" } else { "h-1.5 w-2.5 rotate-180" },
                view_box: "0 0 10 6",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.6",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M1 5l4-4 4 4" }
            }
        }
    }
}

/// Row in a dropdown menu.
#[component]
pub fn MenuItem(
    label: String,
    selected: bool,
    taken: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if selected {
                "flex w-full items-center justify-between rounded-xl px-2.5 py-1.5 text-sm bg-ctp-surface0 text-ctp-text cursor-pointer"
            } else if taken {
                "flex w-full items-center justify-between rounded-xl px-2.5 py-1.5 text-sm text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0/60"
            } else {
                "flex w-full items-center justify-between rounded-xl px-2.5 py-1.5 text-sm text-ctp-subtext1 cursor-pointer hover:bg-ctp-surface0/60 hover:text-ctp-text"
            },
            onclick: move |e| onclick.call(e),
            "{label}"
            if selected {
                span { class: "text-ctp-mauve", "✓" }
            } else if taken {
                span { class: "text-xs", "shown" }
            }
        }
    }
}

/// Down chevron that flips when `open`.
#[component]
pub fn Chevron(open: bool) -> Element {
    rsx! {
        svg {
            class: if open { "h-3 w-3 text-ctp-subtext0 rotate-180 transition-transform" } else { "h-3 w-3 text-ctp-subtext0 transition-transform" },
            view_box: "0 0 12 8",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.6",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M1 1.5l5 5 5-5" }
        }
    }
}

/// Themed dropdown for one choice among `(value, label)` options; native
/// selects can't be styled consistently. `prefix` is shown dimmed before
/// the current label, e.g. "Sort".
#[component]
pub fn Select(
    options: Vec<(String, String)>,
    value: String,
    onchange: EventHandler<String>,
    #[props(default)] prefix: String,
) -> Element {
    let mut open = use_signal(|| false);
    let current = options
        .iter()
        .find(|(v, _)| *v == value)
        .map_or(value.clone(), |(_, label)| label.clone());
    rsx! {
        div { class: "relative",
            button {
                r#type: "button",
                class: if open() {
                    "inline-flex w-full items-center justify-between gap-2 rounded-xl border border-ctp-mauve bg-ctp-crust/40 px-3 py-2 text-sm cursor-pointer"
                } else {
                    "inline-flex w-full items-center justify-between gap-2 rounded-xl border border-ctp-surface0 bg-ctp-crust/40 px-3 py-2 text-sm cursor-pointer transition-colors hover:border-ctp-surface1"
                },
                aria_expanded: open(),
                onclick: move |_| open.toggle(),
                span { class: "truncate",
                    if !prefix.is_empty() {
                        span { class: "text-ctp-subtext0", "{prefix} " }
                    }
                    span { class: "font-medium text-ctp-text", "{current}" }
                }
                Chevron { open: open() }
            }
            if open() {
                div { class: "fixed inset-0 z-20", onclick: move |_| open.set(false) }
                div { class: "absolute left-0 top-full z-30 mt-2 max-h-72 min-w-full w-max overflow-y-auto rounded-2xl border border-ctp-surface0 \
                              bg-ctp-mantle p-1.5 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                    for (v, label) in options.iter().cloned() {
                        MenuItem {
                            key: "{v}",
                            label,
                            selected: v == value,
                            taken: false,
                            onclick: move |_| {
                                onchange.call(v.clone());
                                open.set(false);
                            },
                        }
                    }
                }
            }
        }
    }
}

/// Dimmed overlay with a centred panel; click outside or × to close.
#[component]
pub fn Modal(title: String, on_close: EventHandler<()>, children: Element) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-ctp-crust/70 p-4 backdrop-blur-sm",
            onclick: move |_| on_close.call(()),
            div {
                class: "w-full max-w-lg rounded-3xl border border-ctp-surface0 bg-ctp-mantle p-6 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                role: "dialog",
                onclick: move |e| e.stop_propagation(),
                div { class: "mb-5 flex items-center justify-between",
                    h2 { class: "text-lg font-semibold text-ctp-text", "{title}" }
                    button {
                        class: "flex h-8 w-8 items-center justify-center rounded-full text-ctp-subtext0 cursor-pointer transition-colors hover:bg-ctp-surface0 hover:text-ctp-text",
                        "aria-label": "Close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                {children}
            }
        }
    }
}

/// Text input styling shared by forms.
pub const INPUT: &str = "w-full rounded-xl border border-ctp-surface0 bg-ctp-crust/40 px-3 py-2 text-sm text-ctp-text \
                         placeholder:text-ctp-overlay1 outline-none transition-colors focus:border-ctp-mauve";

/// Labelled form row.
#[component]
pub fn Field(label: String, #[props(default)] hint: String, children: Element) -> Element {
    rsx! {
        label { class: "block",
            span { class: "mb-1.5 block text-xs text-ctp-subtext0", "{label}" }
            {children}
            if !hint.is_empty() {
                span { class: "mt-1 block text-xs text-ctp-overlay1", "{hint}" }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonTone {
    #[default]
    Primary,
    Danger,
    Quiet,
}

#[component]
pub fn ActionButton(
    label: String,
    onclick: EventHandler<MouseEvent>,
    #[props(default)] tone: ButtonTone,
    #[props(default)] disabled: bool,
) -> Element {
    let style = match tone {
        ButtonTone::Primary => "bg-ctp-mauve text-ctp-crust hover:brightness-110",
        ButtonTone::Danger => "bg-ctp-red text-ctp-crust hover:brightness-110",
        ButtonTone::Quiet => "text-ctp-subtext1 hover:bg-ctp-surface0 hover:text-ctp-text",
    };
    rsx! {
        button {
            class: "min-h-10 rounded-full px-5 py-2 text-sm font-semibold transition cursor-pointer disabled:cursor-wait disabled:opacity-60 {style}",
            disabled,
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}
