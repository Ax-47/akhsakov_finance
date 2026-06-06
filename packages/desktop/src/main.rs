use dioxus::prelude::*;
use dtos::transaction::AppData;

use crate::views::Navbar;

mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/portfolio")]
    Portfolio {},
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut app_data: Signal<AppData> = use_signal(AppData::default);
    use_context_provider(|| app_data);

    let _ = use_resource(move || async move {
        match api::get_transactions().await {
            Ok(data) => *app_data.write() = data,
            Err(e) => eprintln!("[desktop] Failed to load transactions: {e}"),
        }
    });

    rsx! {
        ui::App {
            Router::<Route> {}
        }
    }
}

#[component]
fn Home() -> Element {
    rsx! { ui::Home {} }
}

#[component]
fn Portfolio() -> Element {
    rsx! { ui::Dashboard {} }
}
