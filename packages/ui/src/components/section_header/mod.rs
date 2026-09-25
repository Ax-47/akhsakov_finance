use dioxus::prelude::*;
#[component]
pub fn SectionHeader(
    title: String,
    subtitle: String,
    live: bool,
    on_refresh: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "flex items-start justify-between mb-6 gap-4",
            div {
                div { class: "flex items-center gap-3 mb-1",
                    div {
                        class: "w-1 h-7 rounded-[2px] shrink-0 bg-gradient-to-b from-ctp-blue to-ctp-mauve"
                    }

                    h1 {
                        class: "text-2xl font-bold text-ctp-text",
                        "{title}"
                    }
                }

                div {
                    class: "text-xs text-ctp-subtext0 flex items-center gap-2 pl-4",

                    "{subtitle}"

                    if live {
                        span {
                            class: "
                                bg-ctp-green/20
                                text-ctp-green
                                border border-ctp-green/35
                                px-2 py-0.5
                                rounded
                                text-[0.68rem]
                                font-bold
                                tracking-[0.04em]
                            ",
                            "● Live"
                        }
                    }
                }
            }

            button {
                class: "
                    flex items-center gap-1.5
                    px-3 py-2
                    mt-1
                    rounded-lg
                    text-xs font-semibold
                    cursor-pointer
                    border border-ctp-surface2
                    text-ctp-subtext1
                    hover:bg-ctp-surface1
                    hover:text-ctp-text
                    transition-colors
                ",
                onclick: move |_| on_refresh.call(()),
                "↻ Refresh"
            }
        }
    }
}
