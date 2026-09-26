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
    #[route("/screener")]
    Screener {},
    #[route("/calendar")]
    Calendar {},
    #[route("/economy")]
    Economy {},
    #[route("/backtest")]
    Backtest {},
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
    #[cfg(all(feature = "desktop", not(feature = "server")))]
    {
        // On Wayland, WebKitGTK runs through XWayland without GPU buffer
        // sharing: stable, but slower. The native path is faster yet can
        // tear or go blank while the window is dragged on some setups, so
        // it's opt-in with AKHSAKOV_FAST_RENDERING=1.
        let fast = std::env::var("AKHSAKOV_FAST_RENDERING").is_ok_and(|v| v == "1");
        dioxus::LaunchBuilder::new()
            .with_cfg(dioxus::desktop::Config::new().with_disable_dma_buf_on_wayland(!fast))
            .launch(App);
    }
    #[cfg(not(any(feature = "desktop", feature = "server")))]
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
fn Screener() -> Element {
    rsx! {
        ui::ScreenerPage {}
    }
}

#[component]
fn Calendar() -> Element {
    rsx! {
        ui::CalendarPage {}
    }
}

#[component]
fn Economy() -> Element {
    rsx! {
        ui::EconomyPage {}
    }
}

#[component]
fn Backtest() -> Element {
    rsx! {
        ui::BacktestPage {}
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
