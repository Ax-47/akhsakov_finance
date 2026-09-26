use dioxus::prelude::*;
use ui::{
    BacktestIcon, CalendarIcon, DashboardIcon, EconomyIcon, MarketIcon, PortfolioIcon, ScreenerIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LINK, NAV_LINK_ACTIVE,
};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Shell)]
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

/// Backend the phone talks to. Set `AKHSAKOV_SERVER_URL` when building,
/// e.g. `http://192.168.1.20:8080` (a phone can't reach your computer's
/// 127.0.0.1; the Android emulator uses `http://10.0.2.2:8080`).
#[cfg(not(feature = "server"))]
const SERVER_URL: &str = match option_env!("AKHSAKOV_SERVER_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8080",
};

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::fullstack::set_server_url(SERVER_URL);
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
    #[cfg(feature = "server")]
    dioxus::serve(|| async move { Ok(api::with_services(dioxus::server::router(App))) });
}

#[component]
fn App() -> Element {
    rsx! {
        ui::App { Router::<Route> {} }
    }
}

/// Shared shell; on a phone the sidebar collapses to a top bar.
#[component]
fn Shell() -> Element {
    rsx! {
        Sidebar {
            links: rsx! {
                Link { to: Route::Home {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, DashboardIcon {} {ui::i18n::tr("Dashboard")} }
                Link { to: Route::Portfolio {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, PortfolioIcon {} {ui::i18n::tr("Portfolio")} }
                Link { to: Route::Market {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, MarketIcon {} {ui::i18n::tr("Markets")} }
                Link { to: Route::Screener {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, ScreenerIcon {} {ui::i18n::tr("Screener")} }
                Link { to: Route::Calendar {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, CalendarIcon {} {ui::i18n::tr("Calendar")} }
                Link { to: Route::Economy {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, EconomyIcon {} {ui::i18n::tr("Economy")} }
                Link { to: Route::Backtest {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, BacktestIcon {} {ui::i18n::tr("Backtest")} }
                Link { to: Route::Watchlist {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, WatchlistIcon {} {ui::i18n::tr("Watchlist")} }
                Link { to: Route::Settings {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE, SettingsIcon {} {ui::i18n::tr("Settings")} }
            },
            Outlet::<Route> {}
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

#[component]
fn Market() -> Element {
    rsx! { ui::MarketPage {} }
}

#[component]
fn Screener() -> Element {
    rsx! { ui::ScreenerPage {} }
}

#[component]
fn Calendar() -> Element {
    rsx! { ui::CalendarPage {} }
}

#[component]
fn Economy() -> Element {
    rsx! { ui::EconomyPage {} }
}

#[component]
fn Backtest() -> Element {
    rsx! { ui::BacktestPage {} }
}

#[component]
fn Watchlist() -> Element {
    rsx! { ui::WatchlistPage {} }
}

#[component]
fn Settings() -> Element {
    rsx! { ui::SettingsPage {} }
}

#[component]
fn Stock(ticker: String) -> Element {
    rsx! { ui::StockPage { ticker } }
}
