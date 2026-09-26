//! Shared UI components for the workspace.

mod market_page;
pub use market_page::MarketPage;
mod settings_page;
pub use settings_page::SettingsPage;
mod sidebar;
mod stock_page;
mod stock_research;
mod watchlist_page;
pub use sidebar::{
    DashboardIcon, MarketIcon, PortfolioIcon, SettingsIcon, Sidebar, WatchlistIcon, NAV_LINK, NAV_LINK_ACTIVE,
};
pub use stock_page::StockPage;
pub use watchlist_page::WatchlistPage;

mod app;
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
