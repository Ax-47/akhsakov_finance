use dioxus::prelude::*;
use std::f64::consts::PI;

/// Catppuccin Mocha accent colors for chart series
pub const CHART_COLORS: &[&str] = &[
    "var(--mauve)",
    "var(--blue)",
    "var(--green)",
    "var(--peach)",
    "var(--red)",
    "var(--sky)",
    "var(--teal)",
    "var(--yellow)",
    "var(--lavender)",
    "var(--pink)",
    "var(--sapphire)",
    "var(--rosewater)",
];

/// Builds an SVG path for a donut sector.
fn donut_sector(cx: f64, cy: f64, r_out: f64, r_in: f64, start: f64, end: f64) -> String {
    // Clamp the arc so a "full circle" (100 %) still renders correctly
    let end_clamped = if (end - start).abs() >= 2.0 * PI {
        start + 2.0 * PI - 0.001
    } else {
        end
    };

    let x1 = cx + r_out * start.cos();
    let y1 = cy + r_out * start.sin();
    let x2 = cx + r_out * end_clamped.cos();
    let y2 = cy + r_out * end_clamped.sin();
    let ix1 = cx + r_in * start.cos();
    let iy1 = cy + r_in * start.sin();
    let ix2 = cx + r_in * end_clamped.cos();
    let iy2 = cy + r_in * end_clamped.sin();

    let large = if (end_clamped - start) > PI { 1 } else { 0 };

    format!(
        "M {x1:.2} {y1:.2} A {r_out} {r_out} 0 {large} 1 {x2:.2} {y2:.2} \
         L {ix2:.2} {iy2:.2} A {r_in} {r_in} 0 {large} 0 {ix1:.2} {iy1:.2} Z"
    )
}

/// A donut chart.
/// `data` is a list of (label, percentage 0..100) pairs.
#[component]
pub fn PieChart(data: Vec<(String, f64)>, size: f64) -> Element {
    if data.is_empty() {
        return rsx! { div { class: "empty-state", "No data" } };
    }

    let cx = size / 2.0;
    let cy = size / 2.0;
    let r_out = size * 0.42;
    let r_in = size * 0.26;

    // Build sectors
    let total: f64 = data.iter().map(|(_, v)| v).sum();
    let mut start_angle = -PI / 2.0; // Start at top (12 o'clock)

    let sectors: Vec<(String, String, String, f64)> = data // (path, color, label, pct)
        .iter()
        .enumerate()
        .map(|(i, (label, pct))| {
            let fraction = pct / total.max(1.0);
            let sweep = fraction * 2.0 * PI;
            let end_angle = start_angle + sweep;
            let path = donut_sector(cx, cy, r_out, r_in, start_angle, end_angle);
            let color = CHART_COLORS[i % CHART_COLORS.len()].to_string();
            let result = (path, color, label.clone(), *pct);
            start_angle = end_angle;
            result
        })
        .collect();

    rsx! {
        svg {
            width: "{size}",
            height: "{size}",
            "viewBox": "0 0 {size} {size}",
            style: "width: 100%; max-width: {size}px; display: block; margin: 0 auto;",

            for (path, color, _label, _pct) in &sectors {
                path {
                    key: "{_label}",
                    d: "{path}",
                    fill: "{color}",
                    stroke: "var(--mantle)",
                    "strokeWidth": "2",
                }
            }

            // Center label
            text {
                x: "{cx}",
                y: "{cy - 6.0}",
                "textAnchor": "middle",
                "dominantBaseline": "middle",
                "fontSize": "11",
                fill: "var(--subtext0)",
                "Allocation"
            }
        }
    }
}
