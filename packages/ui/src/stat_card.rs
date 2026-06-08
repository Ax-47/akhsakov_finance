use dioxus::prelude::*;

/// A summary stat card displaying an icon, label, and value.
#[component]
pub fn StatCard(label: String, value: String, icon: String) -> Element {
    rsx! {
        div { class: "flex flex-col gap-2 rounded-xl bg-ctp-surface0 p-5 shadow-sm",
            div { class: "flex items-center gap-2",
                span { class: "text-2xl leading-none", "{icon}" }
                span { class: "text-xs font-semibold uppercase tracking-widest text-ctp-subtext0",
                    "{label}"
                }
            }
            span { class: "text-2xl font-bold text-ctp-text", "{value}" }
        }
    }
}
