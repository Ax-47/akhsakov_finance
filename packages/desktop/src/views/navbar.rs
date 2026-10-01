use crate::{views::ScrollBench, Route};
use dioxus::prelude::*;
use ui::{
    BacktestIcon, RaceIcon, CalendarIcon, DashboardIcon, NavSection, EconomyIcon, LearnIcon, MarketIcon, PortfolioIcon, ScreenerIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LABEL, NAV_LINK, NAV_LINK_ACTIVE,
};

/// Full-window shell: sidebar on the left, scrollable page on the right.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        KeepLinksInApp {}
        ScrollBench {}
        Sidebar {
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

/// Safety net for in-app links. When a click on an `<a>` isn't taken by the
/// router (the event didn't reach `Link`'s handler), Dioxus desktop hands the
/// href to the system browser, which opens `/portfolio` as
/// file:///portfolio ("File not found"). In-app paths are routed here
/// instead; other links still open in the browser.
#[component]
fn KeepLinksInApp() -> Element {
    #[cfg(all(feature = "desktop", not(feature = "server")))]
    use_future(|| async {
        let mut channel = document::eval(
            r#"const i = window.interpreter;
               const open = i.handleClickNavigate;
               i.handleClickNavigate = function (event, target) {
                   const a = target.closest("a");
                   const href = a && a.getAttribute("href");
                   if (href && href.startsWith("/") && !href.startsWith("//")) {
                       event.preventDefault();
                       try { dioxus.send(href); return; } catch (_) { i.handleClickNavigate = open; }
                   }
                   return open.call(this, event, target);
               };
               await new Promise(() => {});"#,
        );
        while let Ok(href) = channel.recv::<String>().await {
            if let Ok(route) = href.parse::<Route>() {
                navigator().push(route);
            }
        }
    });
    rsx! {}
}
