use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn Navbar() -> Element {
    rsx! {
        div {
            class: "shell",

            nav {
                class: "sidebar",
                div {
                    class: "sidebar-logo",
                    div { class: "sidebar-logo-name", "◈ Akhsakov" }
                    div { class: "sidebar-logo-sub",  "Finance" }
                }
                NavLink { to: Route::Dashboard {},    icon: "⬡", label: "Dashboard" }
                // NavLink { to: Route::Holdings {},     icon: "◈", label: "Holdings" }
                // NavLink { to: Route::Transactions {}, icon: "↕", label: "Transactions" }
                // NavLink { to: Route::Charts {},       icon: "⌁", label: "Charts" }
                // NavLink { to: Route::Analysis {},     icon: "⌬", label: "Analysis" }
                // NavLink { to: Route::Thesis {},       icon: "⊡", label: "Thesis" }
            }
            div {
                class: "main-content",
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
            class: "nav-link",
            active_class: "nav-link active",
            span { class: "nav-icon", "{icon}" }
            "{label}"
        }
    }
}
