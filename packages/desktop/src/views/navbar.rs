use crate::Route;
use dioxus::prelude::*;

/// Full-window shell: slim sidebar on the left, scrollable content on the right.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        div { class: "mocha flex h-screen overflow-hidden bg-ctp-base",

            nav { class: "w-52 flex-shrink-0 bg-ctp-mantle border-r border-ctp-surface0 flex flex-col py-5",
                div { class: "px-5 mb-6",
                    div { class: "text-base font-bold text-ctp-text", "◈ Akhsakov" }
                    div { class: "text-xs text-ctp-subtext0 mt-0.5", "Finance" }
                }
                NavLink { to: Route::Home {},      icon: "⬡".to_string(), label: "Dashboard".to_string() }
                NavLink { to: Route::Portfolio {}, icon: "◈".to_string(), label: "Portfolio".to_string() }
            }

            div { class: "flex-1 overflow-auto",
                Outlet::<Route> {}
            }
        }
    }
}

#[component]
fn NavLink(to: Route, icon: String, label: String) -> Element {
    rsx! {
        Link {
            to,
            class: "flex items-center gap-2.5 px-3 py-2.5 mx-3 rounded-lg text-sm text-ctp-subtext1 hover:bg-ctp-surface0 hover:text-ctp-text transition-colors",
            active_class: "bg-ctp-surface0 text-ctp-text",
            span { class: "text-sm opacity-70", "{icon}" }
            "{label}"
        }
    }
}
