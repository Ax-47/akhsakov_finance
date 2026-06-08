use dioxus::prelude::*;
use dtos::portfolio::GetDashBoardResponse;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Injects the shared Catppuccin-Mocha Tailwind stylesheet and renders children.
/// Use this in any platform's `App` root to get Tailwind styles globally.
#[component]
pub fn App(children: Element) -> Element {
    let mut app_data: Signal<GetDashBoardResponse> = use_signal(GetDashBoardResponse::default);
    use_context_provider(|| app_data);
    let _ = use_resource(move || async move {
        match api::get_dashboard().await {
            Ok(data) => *app_data.write() = data,
            Err(e) => eprintln!("[desktop] Failed to load dashboard data: {e}"),
        }
    });

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
         {children}
    }
}
