pub const CHART_COLOR_CLASSES: &[&str] = &[
    "bg-ctp-mauve",
    "bg-ctp-blue",
    "bg-ctp-green",
    "bg-ctp-peach",
    "bg-ctp-red",
    "bg-ctp-sky",
    "bg-ctp-teal",
    "bg-ctp-yellow",
    "bg-ctp-lavender",
    "bg-ctp-pink",
    "bg-ctp-sapphire",
    "bg-ctp-rosewater",
];
/// Catppuccin Mocha hex values, index-aligned with `CHART_COLOR_CLASSES`.
/// ECharts cannot resolve CSS variables at paint time, so charts use these.
pub const CHART_COLORS_HEX: &[&str] = &[
    "#cba6f7", // mauve
    "#89b4fa", // blue
    "#a6e3a1", // green
    "#fab387", // peach
    "#f38ba8", // red
    "#89dceb", // sky
    "#94e2d5", // teal
    "#f9e2af", // yellow
    "#b4befe", // lavender
    "#f5c2e7", // pink
    "#74c7ec", // sapphire
    "#f5e0dc", // rosewater
];
