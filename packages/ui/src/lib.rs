//! Shared UI components for the workspace.

mod backtest_page;
pub use backtest_page::BacktestPage;
mod calendar_page;
pub use calendar_page::CalendarPage;
mod economy_page;
mod income_tab;
pub use economy_page::EconomyPage;
mod market_page;
pub use market_page::MarketPage;
mod screener_page;
pub use screener_page::ScreenerPage;
mod stock_table;
mod settings_page;
pub use settings_page::SettingsPage;
mod sidebar;
mod stock_page;
mod stock_research;
mod watchlist_page;
pub use sidebar::{
    BacktestIcon, CalendarIcon, DashboardIcon, EconomyIcon, MarketIcon, PortfolioIcon, ScreenerIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LINK, NAV_LINK_ACTIVE,
};
pub use stock_page::StockPage;
pub use watchlist_page::WatchlistPage;

mod app;
pub mod i18n;
mod theme;
mod auth;
pub use app::App;

mod alerts;
mod components;
mod editors;
mod files;
mod format;
mod notify;
mod page;
mod plan_tab;
mod search;

mod dashboard;
pub use dashboard::Dashboard;

mod home;
pub use home::Home;

mod hooks;
pub use hooks::use_price_stream;

mod live_number;
pub use live_number::{LiveNumber, MotionStyles};
