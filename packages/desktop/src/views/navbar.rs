use crate::Route;
use dioxus::prelude::*;

/// Full-window shell: glass sidebar on the left, scrollable content on the right.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        div { class: "mocha ak-shell",

            nav { class: "ak-nav ak-glass ak-glass-strong",
                div { class: "ak-brand",
                    div { class: "ak-brand-title ak-gradient-text", "◈ Akhsakov" }
                    div { class: "ak-brand-sub", "Finance 🌸" }
                }
                NavLink {
                    to: Route::Home {},
                    icon: "⬡".to_string(),
                    label: "Dashboard".to_string(),
                }
                NavLink {
                    to: Route::Portfolio {},
                    icon: "◈".to_string(),
                    label: "Portfolio".to_string(),
                }

                div { class: "ak-nav-footer",
                    ui::Mascot { message: "Hi hi! Poke me for tips~ 🐾", size: 76, stacked: true }
                }
            }

            div { class: "ak-main", Outlet::<Route> {} }
        }
    }
}

#[component]
fn NavLink(to: Route, icon: String, label: String) -> Element {
    rsx! {
        Link { to, class: "ak-nav-link", active_class: "active",
            span { class: "ak-nav-icon", "{icon}" }
            "{label}"
        }
    }
}
