use dioxus::prelude::*;

use ui::{
    DashboardIcon, MarketIcon, PortfolioIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LINK, NAV_LINK_ACTIVE,
};

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

const FAVICON: Asset = asset!("/assets/favicon.ico");

fn main() {
    // Browser: hydrate the server-rendered page and run the app.
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        let router = api::with_services(dioxus::server::router(App));
        Ok(router)
    });
}

/// Full-window shell: sidebar on the left, scrollable page on the right.
#[component]
fn Navbar() -> Element {
    rsx! {
        Sidebar {
            links: rsx! {
                Link { to: Route::Home {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    DashboardIcon {}
                    "Dashboard"
                }
                Link { to: Route::Portfolio {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    PortfolioIcon {}
                    "Portfolio"
                }
                Link { to: Route::Market {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    MarketIcon {}
                    "Markets"
                }
                Link { to: Route::Watchlist {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    WatchlistIcon {}
                    "Watchlist"
                }
                Link { to: Route::Settings {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    SettingsIcon {}
                    "Settings"
                }
            },
            Outlet::<Route> {}
        }
    }
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
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
