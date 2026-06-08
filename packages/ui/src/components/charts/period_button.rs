use dioxus::prelude::*;

#[component]
pub fn PeriodButton(label: String, mut active_period: Signal<String>) -> Element {
    let label_for_memo = label.clone();
    let is_active = use_memo(move || active_period() == label_for_memo);
    rsx! {
        button {
            class: if is_active() {
                "px-3 py-1.5 rounded text-xs font-bold transition-colors cursor-pointer bg-ctp-blue text-ctp-mantle"
            } else {
                "px-3 py-1.5 rounded text-xs font-medium transition-colors cursor-pointer bg-transparent text-ctp-subtext0 hover:text-ctp-text"
            },
            onclick: move |_| active_period.set(label.clone()),
            "{label}"
        }
    }
}
