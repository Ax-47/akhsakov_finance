use crate::Route;
use dioxus::prelude::*;
use ui::{
    DashboardIcon, MarketIcon, PortfolioIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LINK, NAV_LINK_ACTIVE,
};

/// Full-window shell: sidebar on the left, scrollable page on the right.
#[component]
pub fn Navbar() -> Element {
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
