//! Shared UI components for the workspace.

mod hero;
pub use hero::Hero;

mod navbar;
pub use navbar::Navbar;

mod echo;
pub use echo::Echo;

mod app;
pub use app::App;

mod pie_chart;
pub use pie_chart::PieChart;
pub use pie_chart::CHART_COLORS;

mod stat_card;
pub use stat_card::StatCard;

mod dashboard;
pub use dashboard::Dashboard;

mod home;
pub use home::Home;
