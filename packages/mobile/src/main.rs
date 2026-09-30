use dioxus::prelude::*;
use ui::{
    BacktestIcon, RaceIcon, CalendarIcon, DashboardIcon, NavSection, EconomyIcon, LearnIcon, MarketIcon, PortfolioIcon, ScreenerIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LABEL, NAV_LINK, NAV_LINK_ACTIVE, TAB_LINK, TAB_LINK_ACTIVE,
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
    #[route("/learn")]
    Learn {},
    #[route("/learn/:slug")]
    Lesson { slug: String },
    #[route("/backtest")]
    Backtest {},
    #[route("/race")]
    Race {},
    #[route("/watchlist")]
    Watchlist {},
    #[route("/settings")]
    Settings {},
    #[route("/stock/:ticker")]
    Stock { ticker: String },
}

/// Backend the phone talks to, when set at build time with
/// `AKHSAKOV_SERVER_URL`, e.g. `http://192.168.1.20:8080` (a phone can't
/// reach your computer's 127.0.0.1; the Android emulator uses
/// `http://10.0.2.2:8080`). Without it the app asks on first launch.
#[cfg(not(feature = "server"))]
const SERVER_URL: Option<&str> = option_env!("AKHSAKOV_SERVER_URL");

fn main() {
    #[cfg(not(feature = "server"))]
    match SERVER_URL {
        Some(url) => dioxus::fullstack::set_server_url(url),
        None => ui::server_address::ask_on_device(),
    }
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
            // Phones: the main pages as a bottom tab bar, the rest under "More".
            tabs: rsx! {
                Link { to: Route::Home {}, class: TAB_LINK, active_class: TAB_LINK_ACTIVE,
                    DashboardIcon {}
                    {ui::i18n::tr("Dashboard")}
                }
                Link { to: Route::Portfolio {}, class: TAB_LINK, active_class: TAB_LINK_ACTIVE,
                    PortfolioIcon {}
                    {ui::i18n::tr("Portfolio")}
                }
                Link { to: Route::Watchlist {}, class: TAB_LINK, active_class: TAB_LINK_ACTIVE,
                    WatchlistIcon {}
                    {ui::i18n::tr("Watchlist")}
                }
                Link { to: Route::Market {}, class: TAB_LINK, active_class: TAB_LINK_ACTIVE,
                    MarketIcon {}
                    {ui::i18n::tr("Markets")}
                }
            },
            links: rsx! {
                NavSection { label: ui::i18n::tr("Your money") }
                Link { to: Route::Home {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    DashboardIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Dashboard")} }
                }
                Link { to: Route::Portfolio {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    PortfolioIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Portfolio")} }
                }
                Link { to: Route::Watchlist {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    WatchlistIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Watchlist")} }
                }
                NavSection { label: ui::i18n::tr("Research") }
                Link { to: Route::Market {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    MarketIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Markets")} }
                }
                Link { to: Route::Screener {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    ScreenerIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Screener")} }
                }
                Link { to: Route::Calendar {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    CalendarIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Calendar")} }
                }
                Link { to: Route::Economy {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    EconomyIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Economy")} }
                }
                Link { to: Route::Learn {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    LearnIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Learn")} }
                }
                NavSection { label: ui::i18n::tr("Tools") }
                Link { to: Route::Backtest {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    BacktestIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Backtest")} }
                }
                Link { to: Route::Race {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    RaceIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("AI races")} }
                }
                Link { to: Route::Settings {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    SettingsIcon {}
                    span { class: NAV_LABEL, {ui::i18n::tr("Settings")} }
                }
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
fn Learn() -> Element {
    rsx! {
        ui::LearnPage {}
    }
}

#[component]
fn Lesson(slug: String) -> Element {
    rsx! {
        ui::LessonPage { slug }
    }
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

#[component]
fn Race() -> Element {
    rsx! { ui::RacePage {} }
}
