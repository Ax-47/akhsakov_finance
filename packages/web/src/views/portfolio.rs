use dioxus::prelude::*;

/// Portfolio detail page — stat cards, allocation donut, holdings table.
#[component]
pub fn Portfolio() -> Element {
    rsx! { ui::Dashboard {} }
}
