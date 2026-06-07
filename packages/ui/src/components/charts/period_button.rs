use dioxus::prelude::*;

#[component]
pub fn PeriodButton(label: String, mut active_period: Signal<String>) -> Element {
    let is_active = active_period() == label;
    rsx! {
        button {
            class: "px-3 py-1.5 rounded text-xs font-medium transition-colors cursor-pointer",
            style: if is_active {
                "background:var(--blue);color:var(--mantle);font-weight:700;border:none;"
            } else {
                "background:transparent;color:var(--subtext0);border:none;"
            },
            onclick: move |_| active_period.set(label.clone()),
            "{label}"
        }
    }
}
