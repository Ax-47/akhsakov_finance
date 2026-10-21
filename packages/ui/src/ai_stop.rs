//! The emergency stop for all AI trading, shown where AI portfolios are run.

use crate::errors::error_message;
use crate::i18n::{tr, trf};
use dioxus::prelude::*;
use dtos::ai_models::{AiNotifyLevel, AiStopState};

/// Shows whether AI trading is stopped and turns the stop on (after a
/// second tap to confirm) or off.
#[component]
pub fn AiStopSwitch() -> Element {
    let mut state = use_signal(|| None::<AiStopState>);
    let mut armed = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    use_hook(move || {
        spawn(async move {
            match api::get_ai_stop().await {
                Ok(value) => state.set(Some(value)),
                Err(e) => error.set(Some(error_message(e))),
            }
        });
    });
    let set = move |active: bool| {
        spawn(async move {
            busy.set(true);
            match api::set_ai_stop(active).await {
                Ok(value) => {
                    state.set(Some(value));
                    error.set(None);
                }
                Err(e) => error.set(Some(error_message(e))),
            }
            armed.set(false);
            busy.set(false);
        });
    };
    let Some(current) = state() else {
        return rsx! {
            if let Some(message) = error() {
                p { class: "text-sm text-ctp-red break-words", "{message}" }
            }
        };
    };
    let last = current.history.first().cloned();
    rsx! {
        div {
            class: if current.active {
                "flex flex-wrap items-center justify-between gap-3 rounded-xl border border-ctp-red/60 bg-ctp-red/10 p-4"
            } else {
                "flex flex-wrap items-center justify-between gap-3 rounded-xl border border-ctp-surface0 p-4"
            },
            div { class: "min-w-0",
                div { class: if current.active { "font-semibold text-ctp-red" } else { "font-medium text-ctp-text" },
                    if current.active { {tr("AI trading is stopped")} } else { {tr("Emergency stop")} }
                }
                p { class: "text-xs text-ctp-subtext0",
                    if current.active {
                        {tr("No AI run, race round or AI order can go ahead until you turn the stop off. Races stay paused; resume them yourself.")}
                    } else {
                        {tr("Ends every AI run now, pauses running races and refuses AI orders, from the app's models and from MCP clients alike.")}
                    }
                }
                if let Some(event) = last {
                    p { class: "text-xs text-ctp-overlay1",
                        if event.active {
                            {trf("Turned on by {} at {}", &[&event.actor, &event.at])}
                        } else {
                            {trf("Turned off by {} at {}", &[&event.actor, &event.at])}
                        }
                    }
                }
                if let Some(message) = error() {
                    p { class: "text-xs text-ctp-red break-words", "{message}" }
                }
            }
            if current.active {
                button {
                    class: "min-h-10 rounded-full px-5 py-2 text-sm font-semibold cursor-pointer bg-ctp-surface0 text-ctp-text hover:bg-ctp-surface1 disabled:cursor-wait disabled:opacity-60",
                    disabled: busy(),
                    onclick: move |_| set(false),
                    {tr("Turn the stop off")}
                }
            } else {
                button {
                    class: "min-h-10 rounded-full px-5 py-2 text-sm font-semibold cursor-pointer bg-ctp-red text-ctp-crust hover:brightness-110 disabled:cursor-wait disabled:opacity-60",
                    disabled: busy(),
                    onclick: move |_| if armed() { set(true) } else { armed.set(true) },
                    onmouseleave: move |_| armed.set(false),
                    onblur: move |_| armed.set(false),
                    if armed() { {tr("Stop all AI trading?")} } else { {tr("Stop all AI")} }
                }
            }
        }
    }
}

/// Chooses which AI events reach the user's notifications.
#[component]
pub fn AiNotifySetting() -> Element {
    let mut level = use_signal(|| None::<AiNotifyLevel>);
    let mut error = use_signal(|| None::<String>);
    use_hook(move || {
        spawn(async move {
            match api::get_ai_notify_level().await {
                Ok(value) => level.set(Some(value)),
                Err(e) => error.set(Some(error_message(e))),
            }
        });
    });
    let Some(current) = level() else {
        return rsx! {};
    };
    rsx! {
        crate::components::card::Field { label: tr("Notify me about AI runs and races"),
            select {
                class: crate::components::card::INPUT,
                onchange: move |e| {
                    let chosen = match e.value().as_str() {
                        "off" => AiNotifyLevel::Off,
                        "all" => AiNotifyLevel::All,
                        _ => AiNotifyLevel::Failures,
                    };
                    spawn(async move {
                        match api::set_ai_notify_level(chosen).await {
                            Ok(value) => level.set(Some(value)),
                            Err(e) => error.set(Some(error_message(e))),
                        }
                    });
                },
                option { value: "failures", selected: current == AiNotifyLevel::Failures, {tr("Only when something fails")} }
                option { value: "all", selected: current == AiNotifyLevel::All, {tr("Failures, trades and race results")} }
                option { value: "off", selected: current == AiNotifyLevel::Off, {tr("Never")} }
            }
            if let Some(message) = error() {
                p { class: "text-xs text-ctp-red break-words", "{message}" }
            }
        }
    }
}
