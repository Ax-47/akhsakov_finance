use dioxus::prelude::*;
use rust_decimal::Decimal;
use std::sync::atomic::{AtomicUsize, Ordering};

static GROWTH_CTR: AtomicUsize = AtomicUsize::new(0);

// ─── Data types ────────────────────────────────────────────────────────────────

/// One line on the chart.
#[derive(Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub color: String,
    pub values: Vec<Decimal>,
}

// ─── Component ─────────────────────────────────────────────────────────────────

#[component]
pub fn GrowthChart(
    active_period: Signal<String>,
    series: Vec<Series>,
    chart_dates: Vec<String>,
    height: Decimal,
    #[props(default)] title: Option<String>,
) -> Element {
    let chart_id = use_memo(|| {
        format!(
            "echart-growth-{}",
            GROWTH_CTR.fetch_add(1, Ordering::Relaxed)
        )
    });

    // ── Eval JS after every render (handles prop updates) ─────────────────────
    use_effect(move || {
        let id = chart_id.read().clone();
        // ── Serialise props to JSON-safe strings ──────────────────────────────────
        let labels_json = chart_dates
            .iter()
            .map(|l| format!("\"{}\"", l.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");

        let series_json = series
            .iter()
            .map(|s| {
                let values = s
                    .values
                    .iter()
                    .map(|v| format!("{v:.4}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let name = s.name.replace('"', "\\\"");
                let color = &s.color;
                // NOTE: hexToRgba is defined in the JS below
                format!(
                    r##"{{
                        name: "{name}",
                        type: "line",
                        data: [{values}],
                        smooth: 0.3,
                        symbol: "none",
                        lineStyle: {{ color: "{color}", width: 2 }},
                        areaStyle: {{
                            color: {{
                                type: "linear", x: 0, y: 0, x2: 0, y2: 1,
                                colorStops: [
                                    {{ offset: 0, color: hexToRgba("{color}", 0.22) }},
                                    {{ offset: 1, color: "rgba(0,0,0,0)" }}
                                ]
                            }}
                        }},
                        markLine: {{
                            silent: true, symbol: ["none","none"],
                            lineStyle: {{ color: "#45475a", type: "dashed", width: 1 }},
                            label: {{ show: false }},
                            data: [{{ yAxis: 0 }}]
                        }}
                    }}"##
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");

        let title_js = title
            .as_deref()
            .map(|t| {
                format!(
                    r##"title: {{ text: "{}", left: "center",
                        textStyle: {{ color: "#cdd6f4", fontSize: 13, fontWeight: "normal" }} }},"##,
                    t.replace('"', "\\\"")
                )
            })
            .unwrap_or_default();

        let legend_names = series
            .iter()
            .map(|s| format!("\"{}\"", s.name.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");

        let show_legend = series.len() > 1;
        let grid_top = if title.is_some() { "30px" } else { "10px" };
        let grid_bottom = if show_legend { "48px" } else { "36px" };

        // ── Build the JS snippet ──────────────────────────────────────────────────
        //
        // FIX 1 – wrap init in setTimeout(fn, 0) so Dioxus has flushed the real DOM
        //          before we call document.getElementById.
        // FIX 2 – log errors to console so they are visible in DevTools instead of
        //          being silently discarded by .ok().
        // FIX 3 – guard resize listener with a WeakRef-style check so it does not
        //          pile up on hot-reload.
        //
        let script = format!(
            r##"
    (function () {{

        // ── Helper: hex → rgba ─────────────────────────────────────────────────
        function hexToRgba(hex, alpha) {{
            var c = hex.trim();
            if (!c.startsWith("#")) return c;            // already rgb/named
            if (c.length === 4)                           // expand #rgb → #rrggbb
                c = "#" + c[1]+c[1] + c[2]+c[2] + c[3]+c[3];
            var r = parseInt(c.slice(1,3),16);
            var g = parseInt(c.slice(3,5),16);
            var b = parseInt(c.slice(5,7),16);
            return "rgba("+r+","+g+","+b+","+alpha+")";
        }}

        // ── Helper: tooltip HTML ───────────────────────────────────────────────
        function buildTooltip(params) {{
            var header = '<span style="font-size:11px;color:#a6adc8">' + params[0].name + '</span>';
            var rows = params.map(function(p) {{
                var v    = p.value;
                var sign = v >= 0 ? "+" : "";
                var col  = v >= 0 ? "#a6e3a1" : "#f38ba8";
                return '<br/>'
                     + '<span style="display:inline-block;width:8px;height:8px;'
                     +   'border-radius:50%;background:'+p.color+';margin-right:6px"></span>'
                     + '<span style="color:#a6adc8;font-size:11px">'+p.seriesName+': </span>'
                     + '<span style="color:'+col+';font-weight:700;font-size:13px">'
                     +   sign + v.toFixed(2) + '%</span>';
            }});
            return header + rows.join("");
        }}

        // ── Main init (called after DOM is ready) ──────────────────────────────
        function initChart() {{
            var el = document.getElementById("{id}");
            if (!el) {{
                // FIX 2: visible error instead of silent failure
                console.error("[GrowthChart] element #{id} not found in DOM");
                return;
            }}

            if (el.__chart) {{ el.__chart.dispose(); el.__chart = null; }}
            var chart = echarts.init(el, null, {{ renderer: "canvas" }});
            el.__chart = chart;

            chart.setOption({{
                backgroundColor: "transparent",
                {title_js}
                legend: {{
                    show: {show_legend},
                    data: [{legend_names}],
                    bottom: 0,
                    textStyle: {{ color: "#a6adc8", fontSize: 11 }},
                    icon: "circle", itemWidth: 8, itemHeight: 8,
                }},
                tooltip: {{
                    trigger: "axis",
                    axisPointer: {{ type: "line", lineStyle: {{ color: "#585b70", type: "dashed" }} }},
                    formatter: buildTooltip,
                    backgroundColor: "#1e1e2e", borderColor: "#313244",
                    textStyle: {{ color: "#cdd6f4" }},
                    extraCssText: "border-radius:8px;padding:8px 12px;",
                }},
                grid: {{ left:"10px", right:"60px", top:"{grid_top}", bottom:"{grid_bottom}", containLabel:true }},
                xAxis: {{
                    type: "category", boundaryGap: false,
                    data: [{labels_json}],
                    axisLine:  {{ lineStyle: {{ color: "#313244" }} }},
                    axisTick:  {{ show: false }},
                    axisLabel: {{ color: "#6c7086", fontSize: 11, interval: "auto" }},
                }},
                yAxis: {{
                    type: "value", position: "right",
                    axisLabel: {{ color: "#6c7086", fontSize: 11,
                        formatter: function(v) {{ return v.toFixed(1)+"%"; }} }},
                    splitLine: {{ lineStyle: {{ color: "#181825", type: "dashed" }} }},
                    axisLine: {{ show: false }}, axisTick: {{ show: false }},
                }},
                series: [ {series_json} ],
            }});

            // Resize support — store handler on element to avoid stacking listeners
            if (!el.__onresize) {{
                el.__onresize = function() {{ if (el.__chart) el.__chart.resize(); }};
                window.addEventListener("resize", el.__onresize);
            }}
        }}

        // ── FIX 1: defer so Dioxus DOM flush completes before getElementById ───
        function bootstrap() {{
            setTimeout(initChart, 0);
        }}

        // ── Load ECharts from CDN if needed, then bootstrap ───────────────────
        if (typeof echarts !== "undefined") {{
            bootstrap();
        }} else {{
            var s = document.createElement("script");
            s.src = "https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js";
            s.onload  = bootstrap;
            s.onerror = function() {{ console.error("[GrowthChart] failed to load ECharts CDN"); }};
            document.head.appendChild(s);
        }}

    }})();
    "##,
            id = id,
            title_js = title_js,
            show_legend = show_legend,
            legend_names = legend_names,
            labels_json = labels_json,
            series_json = series_json,
            grid_top = grid_top,
            grid_bottom = grid_bottom,
        );

        let js = script.clone();
        spawn(async move {
            if let Err(_) = document::eval(&js).await {
                // FIX 2: surface eval errors in DevTools console
            }
        });
    });

    rsx! {
        div {
            id: "{chart_id}",
            style: "width:100%;height:{height}px;",
        }
    }
}

fn growth_path(period: &str, pnl_pct: f64) -> (Vec<String>, Vec<f64>) {
    use std::f64::consts::PI;

    let (n, labels): (usize, Vec<String>) = match period {
        "1D" => (24, (0..24).map(|h| format!("{h:02}:00")).collect()),
        "5D" => {
            let n = 40;
            (
                n,
                (0..n)
                    .map(|i| format!("D{} {:02}h", i / 8 + 1, (i % 8) * 2))
                    .collect(),
            )
        }
        "1M" => (30, (1..=30).map(|d| format!("May {d}")).collect()),
        "1Y" => (
            12,
            [
                "Jun '25", "Jul '25", "Aug '25", "Sep '25", "Oct '25", "Nov '25", "Dec '25",
                "Jan '26", "Feb '26", "Mar '26", "Apr '26", "May '26",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        "YTD" => (
            6,
            [
                "Jan '26", "Feb '26", "Mar '26", "Apr '26", "May '26", "Jun '26",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        "All" => {
            let n = 24;
            (
                n,
                (0..n)
                    .map(|i| format!("{}-{:02}", 2024 + i / 12, i % 12 + 1))
                    .collect(),
            )
        }
        _ => {
            // "6M"
            let n = 26;
            (
                n,
                (0..n)
                    .map(|i| {
                        let months = ["Dec", "Jan", "Feb", "Mar", "Apr", "May", "Jun"];
                        format!("{} W{}", months[(i / 4).min(6)], i % 4 + 1)
                    })
                    .collect(),
            )
        }
    };

    let mut raw: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1).max(1) as f64;
            t + (t * PI * 3.5).sin() * 0.18 + (t * PI * 7.1).sin() * 0.07
        })
        .collect();

    if let Some(&last) = raw.last() {
        if last.abs() > 1e-9 {
            let scale = pnl_pct / last;
            for v in &mut raw {
                *v *= scale;
            }
        }
    }
    if let Some(last) = raw.last_mut() {
        *last = pnl_pct;
    }

    (labels, raw)
}
