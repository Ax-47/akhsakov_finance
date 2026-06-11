use dioxus::prelude::*;
use dtos::transaction::AppData;

use ui::Navbar;

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

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use api::quote_services_setup;
        use dioxus::server::axum::Extension;
        let router = dioxus::server::router(App).layer(Extension(quote_services_setup()));
        Ok(router)
    });
}

#[component]
fn App() -> Element {
    rsx! {
        ui::App { Router::<Route> {} }
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        ui::Home {}
    }
}

#[component]
fn Portfolio() -> Element {
    rsx! {
        ui::Dashboard {}
    }
}
