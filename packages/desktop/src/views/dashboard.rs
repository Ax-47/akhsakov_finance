use dioxus::prelude::*;

use dtos::asset::get_asset_response::GetAssetResponse;
use dtos::portfolio::GetPortfolioResponse;
use types::asset_class::AssetClass;

// ─── Dashboard ────────────────────────────────────────────────────────────────

#[component]
pub fn Dashboard() -> Element {
    let portfolios = use_context::<Signal<Vec<GetPortfolioResponse>>>();
    let mut selected_id = use_signal(|| portfolios.read().first().map(|p| p.id.to_string()));

    let selected_portfolio: Option<GetPortfolioResponse> = use_memo(move || {
        let id = selected_id.read().clone()?;
        portfolios
            .read()
            .iter()
            .find(|p| p.id.to_string() == id)
            .cloned()
    })
    .read()
    .clone();

    // Placeholder historical series — replace with a real price-history fetch.
    let sp_for_series = selected_portfolio.clone();
    let growth_series: Vec<(String, f64)> = use_memo(move || match sp_for_series.as_ref() {
        None => vec![],
        Some(p) => {
            let base = portfolio_total_cost(p).max(1_000.0);
            let factors = [
                1.0, 1.04, 0.98, 1.06, 1.09, 1.07, 1.13, 1.11, 1.18, 1.15, 1.22, 1.20_f64,
            ];
            factors
                .iter()
                .enumerate()
                .map(|(i, f)| (format!("2025-{:02}", i + 1), base * f))
                .collect()
        }
    })
    .read()
    .clone();

    rsx! {
        div { class: "dashboard",

            // ── Left: portfolio list ──────────────────────────────────────────
            aside { class: "portfolio-sidebar",
                div { class: "sidebar-title", "Portfolios" }
                div { class: "portfolio-list",
                    for p in portfolios.read().iter() {
                        {
                            let pid = p.id.to_string();
                            let is_active = selected_id.read().as_deref() == Some(&pid);
                            let total = portfolio_total_cost(p);
                            let name = p.name.clone();
                            let sym = portfolio_currency_symbol(p);
                            let n = p.assets.len();
                            rsx! {
                                button {
                                    key: "{pid}",
                                    class: if is_active { "portfolio-card active" } else { "portfolio-card" },
                                    onclick: move |_| selected_id.set(Some(pid.clone())),
                                    div { class: "pc-row",
                                        div { class: "pc-name", "{name}" }
                                        div { class: "pc-currency badge", "{sym}" }
                                    }
                                    div { class: "pc-row pc-meta",
                                        span { class: "pc-positions", "{n} positions" }
                                        span { class: "pc-cost", "{sym}{total:.2}" }
                                    }
                                    // Mini allocation bar — one segment per asset
                                    div { class: "pc-bar",
                                        for (i, asset) in p.assets.iter().enumerate() {
                                            {
                                                let w = asset_cost_basis(asset) / total.max(1.0) * 100.0;
                                                rsx! {
                                                    div {
                                                        key: "{asset.ticker_symbol}",
                                                        class: "pc-bar-seg",
                                                        style: "width:{w:.1}%;background:var(--c{i});",
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if portfolios.read().is_empty() {
                        div { class: "empty-portfolios",
                            div { class: "ep-icon", "◈" }
                            div { "No portfolios yet" }
                        }
                    }
                }
            }

            // ── Right: detail + chart ─────────────────────────────────────────
            div { class: "dashboard-main",
                {match selected_portfolio.as_ref() {
                    None => rsx! {
                        div { class: "no-selection",
                            div { class: "ns-icon", "◈" }
                            div { "Select a portfolio" }
                        }
                    },
                    Some(p) => {
                        let total = portfolio_total_cost(p);
                        let cash  = portfolio_cash_value(p);
                        let sym   = portfolio_currency_symbol(p).to_string();
                        let n_assets = p.assets.len();
                        let name  = p.name.clone();

                        rsx! {
                            // ── Header ──────────────────────────────────────
                            div { class: "dm-header",
                                div { class: "dm-name", "{name}" }
                                div { class: "dm-stats",
                                    StatPill { label: "Invested", value: format!("{sym}{total:.2}") }
                                    StatPill { label: "Cash",     value: format!("{sym}{cash:.2}") }
                                    StatPill { label: "Positions", value: n_assets.to_string() }
                                }
                            }

                            // ── ECharts Growth Chart ─────────────────────────
                            GrowthChart {
                                series: growth_series.clone(),
                                currency_symbol: sym.clone(),
                            }

                            // ── Positions table ──────────────────────────────
                            div { class: "positions-table",
                                div { class: "pt-header",
                                    span { "Ticker" }
                                    span { "Class" }
                                    span { "Qty" }
                                    span { "Avg Cost" }
                                    span { "Cost Basis" }
                                }
                                for asset in p.assets.iter() {
                                    div {
                                        key: "{asset.ticker_symbol}",
                                        class: "pt-row",
                                        div { class: "ticker-label", "{asset.ticker_symbol}" }
                                        div { class: "badge", "{asset.asset_class}" }
                                        div { class: "num", "{asset.quantity.value():.4}" }
                                        div { class: "num", "{sym}{asset.cost.amount():.2}" }
                                        div { class: "num", "{sym}{asset_cost_basis(asset):.2}" }
                                    }
                                }
                                if p.assets.is_empty() {
                                    div { class: "pt-empty", "No assets in this portfolio" }
                                }
                            }
                        }
                    }
                }}
            }
        }
    }
}

// ─── GrowthChart ─────────────────────────────────────────────────────────────
// Renders an Apache ECharts line chart by injecting a JS snippet via eval.

#[component]
fn GrowthChart(series: Vec<(String, f64)>, currency_symbol: String) -> Element {
    let chart_id = "echart-growth";

    let dates_json = series
        .iter()
        .map(|(d, _)| format!("\"{}\"", d))
        .collect::<Vec<_>>()
        .join(",");

    let values_json = series
        .iter()
        .map(|(_, v)| format!("{:.2}", v))
        .collect::<Vec<_>>()
        .join(",");

    let sym = currency_symbol.clone();

    let init_script = format!(
        r#"
(function() {{
    function init() {{
        var el = document.getElementById('{chart_id}');
        if (!el) return;
        if (window.__echarts_growth) {{
            window.__echarts_growth.dispose();
        }}
        var chart = echarts.init(el, 'dark');
        window.__echarts_growth = chart;

        chart.setOption({{
            backgroundColor: 'transparent',
            tooltip: {{
                trigger: 'axis',
                formatter: function(p) {{
                    return p[0].name + '<br/><b>{sym}' +
                        p[0].value.toLocaleString('en-US', {{minimumFractionDigits:2}}) + '</b>';
                }},
                backgroundColor: '#1e2130',
                borderColor: '#2a2f45',
                textStyle: {{ color: '#cdd6f4', fontFamily: 'IBM Plex Mono' }},
            }},
            grid: {{ left: 60, right: 20, top: 20, bottom: 40 }},
            xAxis: {{
                type: 'category',
                data: [{dates_json}],
                axisLine:  {{ lineStyle: {{ color: '#313244' }} }},
                axisTick:  {{ show: false }},
                axisLabel: {{ color: '#6c7086', fontFamily: 'IBM Plex Mono', fontSize: 11 }},
            }},
            yAxis: {{
                type: 'value',
                axisLabel: {{
                    color: '#6c7086',
                    fontFamily: 'IBM Plex Mono',
                    fontSize: 11,
                    formatter: function(v) {{ return '{sym}' + v.toLocaleString(); }},
                }},
                splitLine: {{ lineStyle: {{ color: '#1e2130' }} }},
            }},
            series: [{{
                type: 'line',
                data: [{values_json}],
                smooth: true,
                symbol: 'none',
                lineStyle: {{ color: '#89b4fa', width: 2 }},
                areaStyle: {{
                    color: {{
                        type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                        colorStops: [
                            {{ offset: 0, color: 'rgba(137,180,250,0.25)' }},
                            {{ offset: 1, color: 'rgba(137,180,250,0.02)' }},
                        ],
                    }},
                }},
            }}],
        }});

        window.addEventListener('resize', function() {{ chart.resize(); }});
    }}

    if (typeof echarts !== 'undefined') {{
        init();
    }} else {{
        var s = document.createElement('script');
        s.src = 'https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js';
        s.onload = init;
        document.head.appendChild(s);
    }}
}})();
"#
    );

    // Re-init the chart whenever the series data changes.
    use_effect(move || {
        let script = init_script.clone();
        spawn(async move {
            // Yield briefly so the div is in the DOM before we call into JS.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            document::eval(&script).await.ok();
        });
    });

    rsx! {
        div { class: "chart-section",
            div { class: "chart-label", "Portfolio Growth" }
            div {
                id: "{chart_id}",
                class: "echart-container",
            }
        }
    }
}

// ─── StatPill ─────────────────────────────────────────────────────────────────

#[component]
fn StatPill(label: String, value: String) -> Element {
    rsx! {
        div { class: "stat-pill",
            div { class: "sp-label", "{label}" }
            div { class: "sp-value", "{value}" }
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Total cost basis of all assets in a portfolio (sum of qty × cost/share).
fn portfolio_total_cost(p: &GetPortfolioResponse) -> f64 {
    p.assets.iter().map(asset_cost_basis).sum()
}

/// Cost basis of a single asset position (qty × cost/share).
fn asset_cost_basis(a: &GetAssetResponse) -> f64 {
    (a.quantity * a.cost)
        .amount()
        .to_string()
        .parse::<f64>()
        .unwrap_or(0.0)
}

/// Currency symbol derived from the portfolio's first asset (defaults to "$").
fn portfolio_currency_symbol(p: &GetPortfolioResponse) -> &'static str {
    p.assets
        .first()
        .map(|a| a.cost.currency().symbol())
        .unwrap_or("$")
}

/// Total value held in Cash-class assets.
fn portfolio_cash_value(p: &GetPortfolioResponse) -> f64 {
    p.assets
        .iter()
        .filter(|a| a.asset_class == AssetClass::Cash)
        .map(asset_cost_basis)
        .sum()
}
