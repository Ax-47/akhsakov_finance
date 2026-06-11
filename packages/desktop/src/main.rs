#[cfg(feature = "server")]
use api::quote_services_setup;
use dioxus::prelude::*;

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
    #[cfg(not(feature = "server"))]
    dioxus::fullstack::set_server_url("http://127.0.0.1:8080");
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
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
