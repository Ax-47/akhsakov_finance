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
    #[route("/market")]
    Market {},
    #[route("/watchlist")]
    Watchlist {},
    #[route("/settings")]
    Settings {},
    #[route("/stock/:ticker")]
    Stock { ticker: String },
}

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::fullstack::set_server_url("http://127.0.0.1:8080");
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        let router = api::with_services(dioxus::server::router(App));
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

#[component]
fn Settings() -> Element {
    rsx! {
        ui::SettingsPage {}
    }
}

#[component]
fn Market() -> Element {
    rsx! {
        ui::MarketPage {}
    }
}

#[component]
fn Watchlist() -> Element {
    rsx! {
        ui::WatchlistPage {}
    }
}

#[component]
fn Stock(ticker: String) -> Element {
    rsx! {
        ui::StockPage { ticker }
    }
}
