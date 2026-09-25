use dioxus::prelude::*;
#[component]
pub fn SectionHeader(
    title: String,
    subtitle: String,
    live: bool,
    on_refresh: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "flex items-start justify-between mb-6 gap-4 ak-rise",
            div {
                div { class: "flex items-center gap-3 mb-1",
                    div {
                        class: "w-1 h-7 rounded-[2px] shrink-0 bg-gradient-to-b from-ctp-blue to-ctp-mauve"
                    }

                    h1 {
                        class: "text-2xl font-bold ak-gradient-text",
                        "{title}"
                    }
                }

                div {
                    class: "text-xs text-ctp-subtext0 flex items-center gap-2 pl-4",

                    "{subtitle}"

                    if live {
                        span { class: "ak-live", "Live" }
                    }
                }
            }

            button {
                class: "ak-btn mt-1",
                onclick: move |_| on_refresh.call(()),
                "↻ Refresh"
            }
        }
    }
}
