use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use dioxus::prelude::*;
use dtos::{
    position::{compute_positions, portfolio_summary, Position},
    transaction::{AppData, TransactionType},
};

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
static GROWTH_CTR: AtomicUsize = AtomicUsize::new(0);

// ─── Home ─────────────────────────────────────────────────────────────────────

#[component]
pub fn Home() -> Element {
    let data = use_context::<Signal<AppData>>();

    let mut price_res = use_resource(move || {
        let tickers: Vec<String> = {
            let mut set = std::collections::HashSet::new();
            for tx in data().transactions.iter() {
                set.insert(tx.ticker.clone());
            }
            set.into_iter().collect()
        };
        async move { api::get_live_prices(tickers).await }
    });

    let mut selected_period = use_signal(|| "6M".to_string());
    let mut active_tab = use_signal(|| "My Portfolios".to_string());

    // ── Derived state ─────────────────────────────────────────────────────────
    let prices = price_res().and_then(|r| r.ok()).unwrap_or_default();
    let loaded = !prices.is_empty();
    let positions = compute_positions(&data(), &prices);
    let (total_value, total_cost, total_pnl, day_change) = portfolio_summary(&positions);
    let realized = compute_realized_pnl(&data());

    let pnl_pct = if total_cost > 0.0 {
        total_pnl / total_cost * 100.0
    } else {
        0.0
    };
    let day_pct = if total_value > 0.0 {
        day_change / total_value * 100.0
    } else {
        0.0
    };
    let pos_count = positions.len();

    let (chart_dates, chart_values) = growth_path(&selected_period(), pnl_pct);
    let chart_positive = total_pnl >= 0.0;
    let (analysis_text, bullets, movers) =
        build_analysis(&positions, day_change, day_pct, total_pnl, pnl_pct);

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }

        div { class: "mocha min-h-screen bg-ctp-base text-ctp-text",

            // ── Header strip ──────────────────────────────────────────────────
            div { class: "bg-ctp-mantle px-6 pt-5 pb-6 border-b border-ctp-surface0",
                div { class: "flex items-center justify-between mb-3",
                    span { class: "text-xs text-ctp-subtext0 font-medium tracking-wide",
                        "All Portfolio Holdings"
                    }
                    button {
                        class: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold text-ctp-text border border-ctp-surface2 hover:bg-ctp-surface0 transition-colors cursor-pointer",
                        style: "background:transparent;",
                        "＋  New Portfolio"
                    }
                }

                // Large total value
                div { class: "flex items-baseline gap-3 mb-5",
                    span { class: "text-4xl font-bold text-ctp-text tabular-nums",
                        "{fmt_usd(total_value, 2)}"
                    }
                    if loaded && !positions.is_empty() {
                        span {
                            style: "background:color-mix(in srgb,var(--green) 15%,transparent);color:var(--green);border:1px solid color-mix(in srgb,var(--green) 30%,transparent);padding:.15rem .45rem;border-radius:.3rem;font-size:.68rem;font-weight:700;letter-spacing:.04em;",
                            "● Live"
                        }
                    }
                }

                // Stats row
                div { class: "flex items-start",
                    StatItem {
                        label:   "Cash Holdings".to_string(),
                        value:   "--".to_string(),
                        sub:     "".to_string(),
                        neutral: true,
                    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem {
                        label:   "Day Change".to_string(),
                        value:   format!("{:+.2}", day_change),
                        sub:     format!("({:+.2}%)", day_pct),
                        neutral: !loaded,
                    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem {
                        label:   "Unrealized Gain/Loss".to_string(),
                        value:   format!("{:+.2}", total_pnl),
                        sub:     format!("({:+.2}%)", pnl_pct),
                        neutral: !loaded,
                    }
                    div { class: "w-px bg-ctp-surface1 self-stretch mx-6" }
                    StatItem {
                        label:   "Realized Gain/Loss".to_string(),
                        value:   format!("{:.2}", realized),
                        sub:     "(0.00%)".to_string(),
                        neutral: true,
                    }
                }
            }

            // ── Chart section ─────────────────────────────────────────────────
            div { class: "border-b border-ctp-surface0",
                div { class: "flex items-center justify-between px-6 pt-4 pb-3",
                    div { class: "flex items-center gap-0.5",
                        for p in ["1D", "5D", "1M", "6M", "YTD", "1Y", "All"] {
                            PeriodButton { label: p.to_string(), active_period: selected_period }
                        }
                    }
                    div {
                        class: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-ctp-surface1 text-xs text-ctp-subtext1 cursor-default select-none",
                        "Holdings Growth "
                        span { class: "text-ctp-overlay0", "▾" }
                    }
                }
                GrowthChart {
                    labels:      chart_dates,
                    values:      chart_values,
                    is_positive: chart_positive,
                    height:      220.0,
                }
            }

            // ── Portfolio / Holdings table ──────────────────────────────────────
            div { class: "px-6 pb-10",
                div { class: "flex border-b border-ctp-surface1",
                    TabButton { label: "My Portfolios".to_string(), active_tab }
                    TabButton { label: "My Holdings".to_string(),   active_tab }
                }

                if active_tab() == "My Portfolios" {
                    table { class: "w-full text-xs mt-4",
                        thead {
                            tr { class: "text-ctp-overlay0 border-b border-ctp-surface1",
                                th { class: "py-2 w-6" }
                                th { class: "py-2 pr-6 text-left font-semibold uppercase tracking-wider", "Portfolio Name" }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Symbols" }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                                    div { "Cost Basis" }
                                    div { class: "font-normal normal-case tracking-normal text-ctp-overlay0", "Includes cash" }
                                }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                                    div { "Market Value" }
                                    div { class: "font-normal normal-case tracking-normal text-ctp-overlay0", "Includes cash" }
                                }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Day Change" }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                                    div { "Unrealized" }
                                    div { "Gain/Loss" }
                                }
                                th { class: "py-2 text-right font-semibold uppercase tracking-wider",
                                    div { "Realized" }
                                    div { "Gain/Loss" }
                                }
                            }
                        }
                        tbody {
                            tr { class: "border-b border-ctp-surface1 hover:bg-ctp-surface0 transition-colors",
                                td { class: "py-4 pr-2 text-ctp-overlay0 select-none", "⠿" }
                                td { class: "py-4 pr-6",
                                    div { class: "flex items-center gap-2",
                                        span { "🗂" }
                                        span { class: "font-medium", "My Portfolio" }
                                    }
                                }
                                td { class: "py-4 pr-6 text-right tabular-nums text-ctp-subtext1", "{pos_count}" }
                                td { class: "py-4 pr-6 text-right tabular-nums", "${total_cost:.2}" }
                                td { class: "py-4 pr-6 text-right tabular-nums font-medium",
                                    if loaded && total_value > 0.0 { "${total_value:.2}" } else { "--" }
                                }
                                td { class: "py-4 pr-6 text-right tabular-nums",
                                    style: if day_change >= 0.0 { "color:var(--green)" } else { "color:var(--red)" },
                                    if loaded {
                                        div { "{day_change:+.2}" }
                                        div { class: "text-xs", "{day_pct:+.2}%" }
                                    } else { "--" }
                                }
                                td { class: "py-4 pr-6 text-right tabular-nums",
                                    style: if total_pnl >= 0.0 { "color:var(--green)" } else { "color:var(--red)" },
                                    div { "{total_pnl:+.2}" }
                                    div { class: "text-xs", "{pnl_pct:+.2}%" }
                                }
                                td { class: "py-4 text-right tabular-nums",
                                    style: if realized >= 0.0 { "color:var(--green)" } else { "color:var(--red)" },
                                    if realized.abs() > 0.01 { "${realized:.2}" } else { "--" }
                                }
                            }
                        }
                    }
                } else {
                    table { class: "w-full text-xs mt-4",
                        thead {
                            tr { class: "text-ctp-overlay0 border-b border-ctp-surface1",
                                th { class: "py-2 pr-6 text-left  font-semibold uppercase tracking-wider", "Ticker"   }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Shares"   }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Avg Cost" }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Price"    }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "Value"    }
                                th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider", "P&L"      }
                                th { class: "py-2 text-right      font-semibold uppercase tracking-wider", "Day"      }
                            }
                        }
                        tbody {
                            for pos in &positions {
                                tr {
                                    key: "{pos.ticker}",
                                    class: "border-b border-ctp-surface1 hover:bg-ctp-surface0 transition-colors",
                                    td { class: "py-3 pr-6",
                                        span { class: "font-bold text-ctp-blue tracking-wide", "{pos.ticker}" }
                                    }
                                    td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0", "{pos.shares:.4}" }
                                    td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0", "${pos.avg_cost:.2}" }
                                    td { class: "py-3 pr-6 text-right tabular-nums",
                                        if pos.current_price > 0.0 { "${pos.current_price:.2}" } else { "—" }
                                    }
                                    td { class: "py-3 pr-6 text-right tabular-nums font-medium",
                                        if pos.current_price > 0.0 { "${pos.market_value():.2}" } else { "—" }
                                    }
                                    td {
                                        class: if pos.unrealized_pnl() >= 0.0 {
                                            "py-3 pr-6 text-right tabular-nums text-ctp-green"
                                        } else {
                                            "py-3 pr-6 text-right tabular-nums text-ctp-red"
                                        },
                                        if pos.current_price > 0.0 {
                                            if pos.unrealized_pnl() >= 0.0 {
                                                "+{pos.unrealized_pnl():.2} (+{pos.unrealized_pnl_pct():.1}%)"
                                            } else {
                                                "{pos.unrealized_pnl():.2} ({pos.unrealized_pnl_pct():.1}%)"
                                            }
                                        } else { "—" }
                                    }
                                    td {
                                        class: if pos.daily_change_pct >= 0.0 {
                                            "py-3 text-right tabular-nums text-ctp-green"
                                        } else {
                                            "py-3 text-right tabular-nums text-ctp-red"
                                        },
                                        if pos.current_price > 0.0 {
                                            if pos.daily_change_pct >= 0.0 {
                                                "▲ {pos.daily_change_pct:.2}%"
                                            } else {
                                                "▼ {pos.daily_change_pct.abs():.2}%"
                                            }
                                        } else { "—" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Sub-components ───────────────────────────────────────────────────────────

#[component]
fn StatItem(label: String, value: String, sub: String, neutral: bool) -> Element {
    let positive = value.starts_with('+');
    let color_style = if neutral || value == "--" {
        "color:var(--subtext1);"
    } else if positive {
        "color:var(--green);"
    } else {
        "color:var(--red);"
    };
    rsx! {
        div { class: "flex flex-col",
            div { class: "text-xs text-ctp-subtext0 mb-1", "{label}" }
            div { class: "text-sm font-semibold tabular-nums", style: "{color_style}", "{value}" }
            if !sub.is_empty() {
                div { class: "text-xs tabular-nums", style: "{color_style}", "{sub}" }
            }
        }
    }
}

#[component]
fn PeriodButton(label: String, mut active_period: Signal<String>) -> Element {
    let is_active = active_period() == label;
    rsx! {
        button {
            class: "px-3 py-1.5 rounded text-xs font-medium transition-colors cursor-pointer",
            style: if is_active {
                "background:var(--blue);color:var(--mantle);font-weight:700;border:none;"
            } else {
                "background:transparent;color:var(--subtext0);border:none;"
            },
            onclick: move |_| active_period.set(label.clone()),
            "{label}"
        }
    }
}

#[component]
fn TabButton(label: String, mut active_tab: Signal<String>) -> Element {
    let is_active = active_tab() == label;
    rsx! {
        button {
            class: "px-5 py-3 text-sm font-medium transition-colors cursor-pointer",
            style: if is_active {
                "border-bottom:2px solid var(--blue);color:var(--text);background:transparent;"
            } else {
                "border-bottom:2px solid transparent;color:var(--subtext0);background:transparent;"
            },
            onclick: move |_| active_tab.set(label.clone()),
            "{label}"
        }
    }
}

#[component]
fn GrowthChart(labels: Vec<String>, values: Vec<f64>, is_positive: bool, height: f64) -> Element {
    let chart_id = use_memo(|| {
        format!(
            "echart-growth-{}",
            GROWTH_CTR.fetch_add(1, Ordering::Relaxed)
        )
    });
    let id = chart_id.read().clone();

    let dates_json = labels
        .iter()
        .map(|d| format!("\"{}\"", d.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    let values_json = values
        .iter()
        .map(|v| format!("{v:.4}"))
        .collect::<Vec<_>>()
        .join(",");

    let line_color = if is_positive { "#89b4fa" } else { "#f38ba8" };
    let area_color = if is_positive {
        "rgba(137,180,250,0.20)"
    } else {
        "rgba(243,139,168,0.20)"
    };

    let script = format!(
        r#"
(function() {{
    function init() {{
        var el = document.getElementById('{id}');
        if (!el) return;
        if (el.__chart) {{ el.__chart.dispose(); el.__chart = null; }}
        var c = echarts.init(el, null, {{renderer:'canvas'}});
        el.__chart = c;
        c.setOption({{
            backgroundColor: 'transparent',
            grid: {{ left:'10px', right:'60px', top:'10px', bottom:'36px', containLabel:true }},
            tooltip: {{
                trigger: 'axis',
                axisPointer: {{ type:'line', lineStyle:{{ color:'#585b70', type:'dashed' }} }},
                formatter: function(p) {{
                    var v = p[0].value;
                    var col = v >= 0 ? '#a6e3a1' : '#f38ba8';
                    return '<span style="font-size:11px;color:#a6adc8">' + p[0].name + '</span><br/>'
                         + '<span style="color:'+col+';font-weight:700;font-size:13px">'
                         + (v >= 0 ? '+' : '') + v.toFixed(2) + '%</span>';
                }},
                backgroundColor:'#1e1e2e', borderColor:'#313244',
                textStyle:{{ color:'#cdd6f4' }},
                extraCssText:'border-radius:8px;padding:8px 12px;',
            }},
            xAxis: {{
                type:'category', boundaryGap:false,
                data:[{dates_json}],
                axisLine:{{ lineStyle:{{ color:'#313244' }} }},
                axisTick:{{ show:false }},
                axisLabel:{{ color:'#6c7086', fontSize:11, interval:'auto' }},
            }},
            yAxis: {{
                type:'value', position:'right',
                axisLabel:{{ color:'#6c7086', fontSize:11,
                    formatter: function(v) {{ return v.toFixed(1)+'%'; }} }},
                splitLine:{{ lineStyle:{{ color:'#181825', type:'dashed' }} }},
                axisLine:{{ show:false }}, axisTick:{{ show:false }},
            }},
            series:[{{
                type:'line', data:[{values_json}],
                smooth:0.3, symbol:'none',
                lineStyle:{{ color:'{line_color}', width:2 }},
                areaStyle:{{
                    color:{{ type:'linear', x:0, y:0, x2:0, y2:1,
                        colorStops:[
                            {{ offset:0, color:'{area_color}' }},
                            {{ offset:1, color:'rgba(0,0,0,0)' }}
                        ]
                    }}
                }},
                markLine:{{
                    silent:true, symbol:['none','none'],
                    lineStyle:{{ color:'#45475a', type:'dashed', width:1 }},
                    label:{{ show:false }},
                    data:[{{ yAxis:0 }}],
                }},
            }}],
        }});
        window.addEventListener('resize', function() {{ if(el.__chart) el.__chart.resize(); }});
    }}
    if (typeof echarts !== 'undefined') {{ init(); }}
    else {{
        var s = document.createElement('script');
        s.src = 'https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js';
        s.onload = init; document.head.appendChild(s);
    }}
}})();
"#
    );

    use_effect(move || {
        let s = script.clone();
        spawn(async move {
            document::eval(&s).await.ok();
        });
    });

    rsx! {
        div { id: "{id}", style: "width:100%;height:{height}px;" }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn compute_realized_pnl(data: &AppData) -> f64 {
    let mut book: HashMap<String, (f64, f64)> = HashMap::new();
    let mut realized = 0.0;
    for tx in &data.transactions {
        match tx.transaction_type {
            TransactionType::Buy => {
                let e = book.entry(tx.ticker.clone()).or_default();
                e.0 += tx.shares * tx.price + tx.fees;
                e.1 += tx.shares;
            }
            TransactionType::Sell => {
                if let Some((cost, shares)) = book.get_mut(&tx.ticker) {
                    if *shares > 1e-9 {
                        let avg = *cost / *shares;
                        realized += tx.shares * (tx.price - avg) - tx.fees;
                        *cost -= tx.shares * avg;
                        *shares -= tx.shares;
                    }
                }
            }
            TransactionType::Dividend => {
                realized += tx.price;
            }
        }
    }
    realized
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

fn build_analysis(
    positions: &[Position],
    day_change: f64,
    day_pct: f64,
    total_pnl: f64,
    pnl_pct: f64,
) -> (String, Vec<String>, Vec<String>) {
    let dir = if day_change >= 0.0 {
        "gained"
    } else {
        "declined"
    };
    let trend = if total_pnl >= 0.0 { "up" } else { "down" };

    let mut sorted: Vec<(&Position, f64)> = positions
        .iter()
        .filter(|p| p.current_price > 0.0)
        .map(|p| (p, p.daily_change_pct))
        .collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let losers: Vec<String> = sorted
        .iter()
        .filter(|(_, pct)| *pct < -0.5)
        .take(2)
        .map(|(p, pct)| format!("{} ({:+.1}%)", p.ticker, pct))
        .collect();

    let detail = if !losers.is_empty() {
        format!(", driven by {}", losers.join(", "))
    } else {
        String::new()
    };

    let text = format!(
        "Your portfolio {} {:.2}%{detail}. Overall holdings are {trend} {:.2}% ({:+.2}) on your total investment.",
        dir, day_pct.abs(), pnl_pct.abs(), total_pnl
    );

    let bullets = vec![
        format!("Day change: {:+.2} ({:+.2}%)", day_change, day_pct),
        format!("All-time P&L: {:+.2} ({:+.2}%)", total_pnl, pnl_pct),
        format!(
            "{} active positions, {} transactions",
            positions.len(),
            positions.len()
        ),
    ];

    let movers: Vec<String> = sorted
        .iter()
        .filter(|(_, pct)| pct.abs() > 0.5)
        .map(|(p, _)| p.ticker.clone())
        .take(4)
        .collect();

    (text, bullets, movers)
}

fn fmt_usd(value: f64, decimals: usize) -> String {
    let neg = value < 0.0;
    let abs = value.abs();
    let whole = abs as u64;
    let s: String = {
        let raw = whole.to_string();
        let mut out = String::new();
        for (i, c) in raw.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out.chars().rev().collect()
    };
    let sign = if neg { "-" } else { "" };
    if decimals == 0 {
        format!("{sign}${s}")
    } else {
        let frac = ((abs - whole as f64) * 10f64.powi(decimals as i32)).round() as u64;
        format!("{sign}${s}.{frac:0>width$}", width = decimals)
    }
}
