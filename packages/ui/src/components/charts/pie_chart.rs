use std::sync::atomic::{AtomicUsize, Ordering};

use dioxus::prelude::*;
use rust_decimal::Decimal;

/// CSS-variable colour references used for legend dots in the dashboard.
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
/// Hard-coded Catppuccin Mocha hex values for ECharts `itemStyle.color`.
/// ECharts cannot resolve CSS variables at paint time, so we provide the
/// actual hex colours that correspond 1-to-1 with `CHART_COLORS`.
const CHART_COLORS_HEX: &[&str] = &[
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

/// Global counter so every `PieChart` instance gets a unique DOM id.
static CHART_CTR: AtomicUsize = AtomicUsize::new(0);

// ─── PieChart ─────────────────────────────────────────────────────────────────

/// Renders an ECharts donut chart.
/// `data` is a list of `(label, value)` pairs; values are normalised
/// internally so they don't need to sum to any particular total.
#[component]
pub fn PieChart(data: Vec<(String, Decimal)>, size: Decimal) -> Element {
    if data.is_empty() {
        return rsx! {
            div { style: "display:flex;align-items:center;justify-content:center;height:{size}px;color:var(--overlay0);font-size:.8rem;",
                "No data"
            }
        };
    }

    // Stable, unique DOM id for this component instance.
    let chart_id = use_memo(|| format!("echart-pie-{}", CHART_CTR.fetch_add(1, Ordering::Relaxed)));
    let id = chart_id.read().clone();

    // Build the ECharts `series.data` array as a JS literal.
    let series_data: String = data
        .iter()
        .enumerate()
        .map(|(i, (name, val))| {
            let hex = CHART_COLORS_HEX[i % CHART_COLORS_HEX.len()];
            // Escape any double-quotes in the name (unlikely but safe).
            let safe_name = name.replace('"', "\\\"");
            format!(r#"{{name:"{safe_name}",value:{val:.4},itemStyle:{{color:"{hex}"}}}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");

    // Full ECharts initialisation script.
    // The function is idempotent: disposes any previous chart instance first.
    let init_script = format!(
        r#"
(function() {{
    function initChart() {{
        var el = document.getElementById('{id}');
        if (!el) return;
        if (el.__echart) {{ el.__echart.dispose(); el.__echart = null; }}

        var chart = echarts.init(el, null, {{ renderer: 'canvas', locale: 'EN' }});
        el.__echart = chart;

        chart.setOption({{
            backgroundColor: 'transparent',
            tooltip: {{
                trigger: 'item',
                formatter: function(p) {{
                    return '<span style="color:' + p.color + ';margin-right:4px;">●</span>'
                         + '<b>' + p.name + '</b><br/>'
                         + p.percent.toFixed(1) + '%';
                }},
                backgroundColor: '#313244',
                borderColor: '#585b70',
                textStyle: {{ color: '#cdd6f4', fontSize: 12 }},
                extraCssText: 'border-radius:8px;padding:8px 12px;',
            }},
            series: [{{
                type: 'pie',
                radius: ['42%', '72%'],
                center: ['50%', '50%'],
                data: [{series_data}],
                label: {{ show: false }},
                labelLine: {{ show: false }},
                emphasis: {{
                    scale: true,
                    scaleSize: 8,
                    label: {{
                        show: true,
                        fontSize: 11,
                        fontWeight: 'bold',
                        color: '#cdd6f4',
                        formatter: '{{b}}\n{{d}}%',
                    }},
                }},
                animationType: 'scale',
                animationEasing: 'elasticOut',
                animationDuration: 600,
            }}],
        }});

        window.addEventListener('resize', function() {{
            if (el.__echart) el.__echart.resize();
        }});
    }}

    if (typeof echarts !== 'undefined') {{
        initChart();
    }} else {{
        var s = document.createElement('script');
        s.src = 'https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js';
        s.onload = initChart;
        document.head.appendChild(s);
    }}
}})();
"#
    );

    // Run (or re-run) the init script after every render where data changed.
    // Dioxus calls this closure with the latest captured `init_script` value.
    use_effect(move || {
        let script = init_script.clone();
        spawn(async move {
            document::eval(&script).await.ok();
        });
    });

    rsx! {
        div { id: "{id}", style: "width:100%;height:{size}px;" }
    }
}
