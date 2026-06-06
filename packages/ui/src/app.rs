use dioxus::prelude::*;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Injects the shared Catppuccin-Mocha Tailwind stylesheet and renders children.
/// Use this in any platform's `App` root to get Tailwind styles globally.
#[component]
pub fn App(children: Element) -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        {children}
    }
}
