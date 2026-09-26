use crate::Route;
use dioxus::prelude::*;
use ui::{
    BacktestIcon, CalendarIcon, DashboardIcon, EconomyIcon, MarketIcon, PortfolioIcon, ScreenerIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LINK, NAV_LINK_ACTIVE,
};

/// Full-window shell: sidebar on the left, scrollable page on the right.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        Sidebar {
            links: rsx! {
                Link { to: Route::Home {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    DashboardIcon {}
                    {ui::i18n::tr("Dashboard")}
                }
                Link { to: Route::Portfolio {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    PortfolioIcon {}
                    {ui::i18n::tr("Portfolio")}
                }
                Link { to: Route::Market {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    MarketIcon {}
                    {ui::i18n::tr("Markets")}
                }
                Link { to: Route::Screener {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    ScreenerIcon {}
                    {ui::i18n::tr("Screener")}
                }
                Link { to: Route::Calendar {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    CalendarIcon {}
                    {ui::i18n::tr("Calendar")}
                }
                Link { to: Route::Economy {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    EconomyIcon {}
                    {ui::i18n::tr("Economy")}
                }
                Link { to: Route::Backtest {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    BacktestIcon {}
                    {ui::i18n::tr("Backtest")}
                }
                Link { to: Route::Watchlist {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    WatchlistIcon {}
                    {ui::i18n::tr("Watchlist")}
                }
                Link { to: Route::Settings {}, class: NAV_LINK, active_class: NAV_LINK_ACTIVE,
                    SettingsIcon {}
                    {ui::i18n::tr("Settings")}
                }
            },
            Outlet::<Route> {}
        }
    }
}
