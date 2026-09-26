//! Side-by-side comparison of stocks on valuation, profitability, growth,
//! financial health, income and analyst measures.

use crate::i18n::tr;
use crate::components::{
    card::{Card, Chevron, MenuItem},
    charts::bars::{AxisLine, BarChart, BarSeries, DualLineChart, Unit},
    color_schema::CHART_COLORS_HEX,
};
use crate::format::or_dash;
use dioxus::prelude::*;
use dtos::fundamentals::{
    current_valuation, ttm, yoy_growth, PeriodFinancials, StockFundamentals, ValuationPoint,
};
use types::ticker_symbol::TickerSymbol;

const MAX_STOCKS: usize = 6;

/// Which direction is good for a measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Better {
    Higher,
    Lower,
    /// Neither (e.g. market cap): no best / worst highlight.
    Neutral,
}

pub struct Measure {
    pub group: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub better: Better,
    pub get: fn(&StockFundamentals) -> Option<f64>,
}

fn safe_div(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    Some(a? / b.filter(|b| *b != 0.0)?)
}

/// Every comparable measure, in display order.
pub fn measures() -> Vec<Measure> {
    use Better::*;
    vec![
        Measure {
            group: "Valuation",
            label: tr("Market cap"),
            unit: Unit::Money,
            better: Neutral,
            get: |f| current_valuation(&f.stats, &f.quarterly).market_cap,
        },
        Measure {
            group: "Valuation",
            label: tr("Trailing P/E"),
            unit: Unit::Number,
            better: Lower,
            get: |f| current_valuation(&f.stats, &f.quarterly).pe_ttm,
        },
        Measure {
            group: "Valuation",
            label: tr("Forward P/E"),
            unit: Unit::Number,
            better: Lower,
            get: |f| f.stats.forward_pe,
        },
        Measure {
            group: "Valuation",
            label: tr("Price / sales"),
            unit: Unit::Number,
            better: Lower,
            get: |f| current_valuation(&f.stats, &f.quarterly).price_to_sales,
        },
        Measure {
            group: "Valuation",
            label: tr("Price / book"),
            unit: Unit::Number,
            better: Lower,
            get: |f| current_valuation(&f.stats, &f.quarterly).price_to_book,
        },
        Measure {
            group: "Valuation",
            label: tr("EV / revenue"),
            unit: Unit::Number,
            better: Lower,
            get: |f| current_valuation(&f.stats, &f.quarterly).ev_to_revenue,
        },
        Measure {
            group: "Profitability",
            label: tr("Gross margin"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| {
                safe_div(
                    ttm(&f.quarterly, |p| p.gross_profit),
                    ttm(&f.quarterly, |p| p.revenue),
                )
            },
        },
        Measure {
            group: "Profitability",
            label: tr("Operating margin"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| {
                safe_div(
                    ttm(&f.quarterly, |p| p.operating_income),
                    ttm(&f.quarterly, |p| p.revenue),
                )
            },
        },
        Measure {
            group: "Profitability",
            label: tr("Net margin"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| {
                safe_div(
                    ttm(&f.quarterly, |p| p.net_income),
                    ttm(&f.quarterly, |p| p.revenue),
                )
            },
        },
        Measure {
            group: "Profitability",
            label: tr("Return on equity"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| {
                safe_div(
                    ttm(&f.quarterly, |p| p.net_income),
                    f.quarterly.first()?.total_equity,
                )
            },
        },
        Measure {
            group: "Growth",
            label: tr("Revenue growth (YoY)"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| yoy_growth(&f.quarterly, |p| p.revenue),
        },
        Measure {
            group: "Growth",
            label: tr("Earnings growth (YoY)"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| yoy_growth(&f.quarterly, |p| p.net_income),
        },
        Measure {
            group: "Financial health",
            label: tr("Debt / equity"),
            unit: Unit::Number,
            better: Lower,
            get: |f| f.quarterly.first()?.debt_to_equity(),
        },
        Measure {
            group: "Financial health",
            label: tr("Current ratio"),
            unit: Unit::Number,
            better: Higher,
            get: |f| f.quarterly.first()?.current_ratio(),
        },
        Measure {
            group: "Financial health",
            label: tr("Free cash flow margin"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| {
                safe_div(
                    ttm(&f.quarterly, |p| p.free_cash_flow),
                    ttm(&f.quarterly, |p| p.revenue),
                )
            },
        },
        Measure {
            group: "Income",
            label: tr("Dividend yield"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| f.stats.dividend_yield,
        },
        Measure {
            group: "Analysts",
            label: tr("Upside to target"),
            unit: Unit::Percent,
            better: Higher,
            get: |f| {
                Some(f.analysts.as_ref()?.target_mean? / f.stats.price.filter(|p| *p > 0.0)? - 1.0)
            },
        },
        Measure {
            group: "Analysts",
            label: tr("Rating (1 buy – 5 sell)"),
            unit: Unit::Number,
            better: Lower,
            get: |f| f.analysts.as_ref()?.mean,
        },
    ]
}

/// Indices of the best and worst values, when at least two are present.
pub fn best_worst(values: &[Option<f64>], better: Better) -> (Option<usize>, Option<usize>) {
    let present: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| Some((i, (*v)?)))
        .collect();
    if present.len() < 2 || better == Better::Neutral {
        return (None, None);
    }
    let max = present
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|p| p.0);
    let min = present
        .iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|p| p.0);
    match better {
        Better::Higher => (max, min),
        _ => (min, max),
    }
}

// ─── Measure vs measure (one stock over time) ─────────────────────────────────

/// A measure with a value at each quarter end.
pub struct TimeMeasure {
    pub group: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub get: fn(&PeriodFinancials, Option<&ValuationPoint>) -> Option<f64>,
}

pub fn time_measures() -> Vec<TimeMeasure> {
    let m = |group, label, unit, get| TimeMeasure {
        group,
        label,
        unit,
        get,
    };
    vec![
        m("Income statement", SALES, Unit::Money, |p, _| p.revenue),
        m("Income statement", "Gross profit", Unit::Money, |p, _| {
            p.gross_profit
        }),
        m(
            "Income statement",
            "Operating income",
            Unit::Money,
            |p, _| p.operating_income,
        ),
        m("Income statement", "Net income", Unit::Money, |p, _| {
            p.net_income
        }),
        m("Margins", "Gross margin", Unit::Percent, |p, _| {
            p.gross_margin()
        }),
        m("Margins", "Operating margin", Unit::Percent, |p, _| {
            p.operating_margin()
        }),
        m("Margins", "Net margin", Unit::Percent, |p, _| {
            p.net_margin()
        }),
        m("Cash flow", "Operating cash flow", Unit::Money, |p, _| {
            p.operating_cashflow
        }),
        m("Cash flow", "Free cash flow", Unit::Money, |p, _| {
            p.free_cash_flow
        }),
        m("Balance sheet", "Cash", Unit::Money, |p, _| p.cash),
        m("Balance sheet", DEBT, Unit::Money, |p, _| p.long_term_debt),
        m("Balance sheet", "Total liabilities", Unit::Money, |p, _| {
            p.total_liabilities
        }),
        m(
            "Balance sheet",
            "Shareholders' equity",
            Unit::Money,
            |p, _| p.total_equity,
        ),
        m("Balance sheet", "Debt / equity", Unit::Number, |p, _| {
            p.debt_to_equity()
        }),
        m("Balance sheet", "Current ratio", Unit::Number, |p, _| {
            p.current_ratio()
        }),
        m("Valuation", "Share price", Unit::Money, |_, v| {
            Some(v?.price)
        }),
        m("Valuation", "Market cap", Unit::Money, |_, v| v?.market_cap),
        m("Valuation", "Enterprise value", Unit::Money, |_, v| {
            v?.enterprise_value
        }),
        m("Valuation", "Trailing P/E", Unit::Number, |_, v| v?.pe_ttm),
        m("Valuation", "Price / sales", Unit::Number, |_, v| {
            v?.price_to_sales
        }),
        m("Valuation", "Price / book", Unit::Number, |_, v| {
            v?.price_to_book
        }),
        m("Valuation", "EV / revenue", Unit::Number, |_, v| {
            v?.ev_to_revenue
        }),
    ]
}

const SALES: &str = "Revenue (sales)";
const DEBT: &str = "Long-term debt";

/// Common pairs offered as one-click shortcuts.
const PRESETS: [(&str, &str, &str); 5] = [
    ("Sales vs debt", SALES, DEBT),
    ("Cash vs debt", "Cash", DEBT),
    ("Sales vs profit", SALES, "Net income"),
    ("Sales vs price", SALES, "Share price"),
    ("Profit vs P/E", "Net income", "Trailing P/E"),
];

/// Pearson correlation over the points where both series have a value.
pub fn pearson(a: &[Option<f64>], b: &[Option<f64>]) -> Option<f64> {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .filter_map(|(x, y)| Some(((*x)?, (*y)?)))
        .collect();
    if pairs.len() < 3 {
        return None;
    }
    let n = pairs.len() as f64;
    let (mx, my) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let cov: f64 = pairs.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
    let vx: f64 = pairs.iter().map(|(x, _)| (x - mx).powi(2)).sum();
    let vy: f64 = pairs.iter().map(|(_, y)| (y - my).powi(2)).sum();
    let den = (vx * vy).sqrt();
    (den > 0.0).then(|| (cov / den).clamp(-1.0, 1.0))
}

fn describe_correlation(a: &str, b: &str, r: f64, n: usize) -> String {
    let strength = match r.abs() {
        v if v >= 0.7 => "strongly",
        v if v >= 0.4 => "somewhat",
        _ => "barely",
    };
    let direction = if r >= 0.0 {
        "move together"
    } else {
        "move in opposite directions"
    };
    if r.abs() < 0.4 {
        format!(
            "Over these {n} quarters, {a} and {b} are {strength} related (correlation {r:+.2})."
        )
    } else {
        format!(
            "Over these {n} quarters, {a} and {b} {strength} {direction} (correlation {r:+.2})."
        )
    }
}

/// Two measures of one stock over its recent quarters, on one chart.
#[component]
pub fn MeasureVsMeasure(fundamentals: StockFundamentals) -> Element {
    let all = time_measures();
    let find = |label: &str| all.iter().position(|m| m.label == label).unwrap_or(0);
    let presets: Vec<(&str, usize, usize)> = PRESETS
        .iter()
        .map(|(name, x, y)| (*name, find(x), find(y)))
        .collect();
    let mut a = use_signal(|| find(SALES));
    let mut b = use_signal(|| find("Trailing P/E"));

    // Last 8 quarters, oldest first, each with its valuation point.
    let quarters: Vec<&PeriodFinancials> = fundamentals.quarterly.iter().take(8).rev().collect();
    let valuation_at = |q: &PeriodFinancials| {
        fundamentals
            .valuation
            .iter()
            .find(|v| Some(&v.date) == q.end_date.as_ref())
    };
    let labels: Vec<String> = quarters.iter().map(|q| quarter_label(q)).collect();
    let series = |m: &TimeMeasure| -> Vec<Option<f64>> {
        quarters
            .iter()
            .map(|q| (m.get)(q, valuation_at(q)))
            .collect()
    };

    let (ma, mb) = (&all[a().min(all.len() - 1)], &all[b().min(all.len() - 1)]);
    let (va, vb) = (series(ma), series(mb));
    let both = va
        .iter()
        .zip(&vb)
        .filter(|(x, y)| x.is_some() && y.is_some())
        .count();
    let reading = pearson(&va, &vb).map(|r| describe_correlation(ma.label, mb.label, r, both));
    let change = |v: &[Option<f64>]| {
        let first = v.iter().flatten().next().copied()?;
        let last = v.iter().flatten().last().copied()?;
        (first != 0.0).then(|| (last / first.abs() - first.signum()) * 100.0)
    };

    let options: Vec<(&'static str, &'static str)> =
        all.iter().map(|m| (m.group, m.label)).collect();
    let latest = |v: &[Option<f64>]| v.iter().flatten().last().copied();

    rsx! {
        Card {
            title: tr("Measure vs measure"),
            subtitle: format!("Two measures of this stock over its last {} quarters", labels.len()),
            // Quick picks
            div { class: "flex flex-wrap gap-1.5",
                for (name, x, y) in presets {
                    button {
                        key: "{name}",
                        class: if (a(), b()) == (x, y) {
                            "rounded-full bg-ctp-mauve/15 px-3 py-1 text-xs font-medium text-ctp-mauve cursor-pointer"
                        } else {
                            "rounded-full px-3 py-1 text-xs text-ctp-overlay1 cursor-pointer transition-colors hover:bg-ctp-surface0/60 hover:text-ctp-text"
                        },
                        onclick: move |_| {
                            a.set(x);
                            b.set(y);
                        },
                        "{name}"
                    }
                }
            }
            // Pickers
            div { class: "mt-4 flex flex-wrap items-center gap-2",
                MeasurePicker { options: options.clone(), value: a(), color: LEFT_HEX, onchange: move |i| a.set(i) }
                button {
                    class: "flex h-8 w-8 items-center justify-center rounded-full text-ctp-overlay1 cursor-pointer \
                            transition-colors hover:bg-ctp-surface0 hover:text-ctp-text",
                    title: tr("Swap"),
                    "aria-label": "Swap measures",
                    onclick: move |_| {
                        let (x, y) = (a(), b());
                        a.set(y);
                        b.set(x);
                    },
                    "⇄"
                }
                MeasurePicker { options: options.clone(), value: b(), color: RIGHT_HEX, onchange: move |i| b.set(i) }
            }
            // Summary
            div { class: "mt-5 grid grid-cols-2 divide-x divide-ctp-surface0/70 rounded-2xl border border-ctp-surface0/70 bg-ctp-base/40",
                MeasureSummary { label: ma.label, color: LEFT_HEX, value: or_dash(latest(&va), |v| ma.unit.format(v)), change: change(&va) }
                MeasureSummary { label: mb.label, color: RIGHT_HEX, value: or_dash(latest(&vb), |v| mb.unit.format(v)), change: change(&vb) }
            }
            div { class: "mt-5",
                DualLineChart {
                    labels: labels.clone(),
                    left: AxisLine { name: ma.label.into(), color: LEFT_HEX, unit: ma.unit, values: va.clone() },
                    right: AxisLine { name: mb.label.into(), color: RIGHT_HEX, unit: mb.unit, values: vb.clone() },
                }
            }
            p { class: "mt-3 rounded-xl bg-ctp-base/40 px-3 py-2 text-sm text-ctp-subtext0",
                {reading.unwrap_or_else(|| "Not enough overlapping quarters to say how these relate.".into())}
            }
        }
    }
}

const LEFT_HEX: &str = "var(--catppuccin-color-mauve)"; // mauve
const RIGHT_HEX: &str = "var(--catppuccin-color-sky)"; // sky

#[component]
fn MeasureSummary(
    label: &'static str,
    color: &'static str,
    value: String,
    change: Option<f64>,
) -> Element {
    rsx! {
        div { class: "px-4 py-3",
            div { class: "flex items-center gap-2 text-xs text-ctp-overlay1",
                span { class: "h-2 w-2 rounded-full", style: "background:{color};" }
                "{label}"
            }
            div { class: "mt-1 flex flex-wrap items-baseline gap-2",
                span { class: "text-2xl font-semibold tabular-nums text-ctp-text", "{value}" }
                if let Some(c) = change {
                    span {
                        class: if c >= 0.0 { "rounded-full bg-ctp-green/15 px-2 py-0.5 text-xs font-semibold tabular-nums text-ctp-green" } else { "rounded-full bg-ctp-red/15 px-2 py-0.5 text-xs font-semibold tabular-nums text-ctp-red" },
                        "{c:+.1}%"
                    }
                    span { class: "text-xs text-ctp-overlay0", "over the period" }
                }
            }
        }
    }
}

/// Themed dropdown (native `<select>` can't be styled in WebKitGTK).
#[component]
fn MeasurePicker(
    options: Vec<(&'static str, &'static str)>,
    value: usize,
    color: &'static str,
    onchange: EventHandler<usize>,
) -> Element {
    let mut open = use_signal(|| false);
    let current = options.get(value).map_or("", |o| o.1);
    rsx! {
        div { class: "relative",
            button {
                class: if open() {
                    "inline-flex items-center gap-2 rounded-full border border-ctp-mauve bg-ctp-crust/40 py-1.5 pl-3 pr-2.5 text-sm cursor-pointer"
                } else {
                    "inline-flex items-center gap-2 rounded-full border border-ctp-surface1 bg-ctp-crust/40 py-1.5 pl-3 pr-2.5 text-sm cursor-pointer transition-colors hover:border-ctp-surface2"
                },
                aria_expanded: open(),
                onclick: move |_| open.toggle(),
                span { class: "h-2.5 w-2.5 rounded-full", style: "background:{color};" }
                span { class: "font-medium text-ctp-text", "{current}" }
                Chevron { open: open() }
            }
            if open() {
                div { class: "fixed inset-0 z-20", onclick: move |_| open.set(false) }
                div { class: "absolute left-0 top-full z-30 mt-2 max-h-80 w-60 overflow-y-auto rounded-2xl border border-ctp-surface0 \
                              bg-ctp-mantle p-1.5 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                    for (group, items) in group_measures(&options) {
                        div { key: "{group}",
                            div { class: "px-2.5 pt-2 pb-1 text-xs text-ctp-overlay0", "{group}" }
                            for (i, label) in items {
                                MenuItem {
                                    key: "{i}",
                                    label,
                                    selected: i == value,
                                    taken: false,
                                    onclick: move |_| {
                                        onchange.call(i);
                                        open.set(false);
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Measures grouped by section, keeping their original indices.
fn group_measures(
    measures: &[(&'static str, &'static str)],
) -> Vec<(&'static str, Vec<(usize, &'static str)>)> {
    let mut groups: Vec<(&'static str, Vec<(usize, &'static str)>)> = Vec::new();
    for (i, (group, label)) in measures.iter().enumerate() {
        match groups.last_mut() {
            Some((g, items)) if g == group => items.push((i, label)),
            _ => groups.push((group, vec![(i, label)])),
        }
    }
    groups
}

fn quarter_label(q: &PeriodFinancials) -> String {
    match &q.end_date {
        Some(d) if d.len() >= 7 => {
            let month: u32 = d[5..7].parse().unwrap_or(12);
            format!("Q{} '{}", month.div_ceil(3), &d[2..4])
        }
        _ => q.period.clone(),
    }
}

// ─── UI ───────────────────────────────────────────────────────────────────────

/// `ticker` is always first; `peers` pre-fills the other columns.
#[component]
pub fn CompareMeasures(ticker: TickerSymbol, peers: Vec<TickerSymbol>) -> Element {
    let mut tickers = use_signal(move || {
        let mut list = vec![ticker.clone()];
        list.extend(peers.into_iter().filter(|p| *p != ticker).take(3));
        list
    });
    let mut custom = use_signal(String::new);
    let mut selected = use_signal(|| 1usize); // Trailing P/E

    let data = use_resource(move || {
        let list = tickers();
        async move {
            let fetched =
                futures::future::join_all(list.iter().cloned().map(api::get_fundamentals)).await;
            list.into_iter()
                .zip(fetched.into_iter().map(Result::ok))
                .collect::<Vec<_>>()
        }
    });

    let all = measures();
    let rows = data.read().clone();
    let loaded: Vec<(TickerSymbol, Option<StockFundamentals>)> = rows.unwrap_or_default();
    let names: Vec<String> = tickers.read().iter().map(|t| t.to_string()).collect();
    let values_of = |m: &Measure| -> Vec<Option<f64>> {
        tickers
            .read()
            .iter()
            .map(|t| {
                loaded
                    .iter()
                    .find(|(lt, _)| lt == t)
                    .and_then(|(_, f)| (m.get)(f.as_ref()?))
            })
            .collect()
    };
    let chosen = &all[selected().min(all.len() - 1)];
    let chosen_values = values_of(chosen);

    rsx! {
        div { class: "grid gap-5 motion-safe:animate-rise",
            Card {
                title: tr("Compare measures"),
                subtitle: tr("Best in each row is green, worst is red · click a row to chart it").to_string(),
                actions: rsx! {
                    if tickers.read().len() < MAX_STOCKS {
                        form {
                            onsubmit: move |e| {
                                e.prevent_default();
                                if let Ok(t) = TickerSymbol::new(&custom()) {
                                    if !tickers.read().contains(&t) {
                                        tickers.write().push(t);
                                    }
                                    custom.set(String::new());
                                }
                            },
                            input {
                                class: "w-48 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm \
                                        text-ctp-text placeholder:text-ctp-overlay0 outline-none focus:border-ctp-mauve",
                                placeholder: tr("Add a ticker ↵"),
                                value: "{custom}",
                                oninput: move |e| custom.set(e.value()),
                            }
                        }
                    }
                },
                div { class: "flex flex-wrap gap-2",
                    for (i, t) in tickers().into_iter().enumerate() {
                        span {
                            key: "{t}",
                            class: "inline-flex items-center gap-2 rounded-full border border-ctp-surface1 bg-ctp-crust/40 py-1 pl-3 pr-1.5 text-sm",
                            span { class: "h-2.5 w-2.5 rounded-full", style: "background:{CHART_COLORS_HEX[i % CHART_COLORS_HEX.len()]};" }
                            span { class: "font-semibold text-ctp-text", "{t}" }
                            if i > 0 {
                                button {
                                    class: "flex h-5 w-5 items-center justify-center rounded-full text-ctp-overlay1 cursor-pointer \
                                            transition-colors hover:bg-ctp-surface0 hover:text-ctp-red",
                                    "aria-label": "Remove {t}",
                                    onclick: move |_| {
                                        tickers.write().remove(i);
                                    },
                                    "×"
                                }
                            } else {
                                span { class: "pr-1.5 text-xs text-ctp-overlay0", "this stock" }
                            }
                        }
                    }
                }
            }

            if data.read().is_none() {
                Card { title: tr("Measures"), p { class: "py-12 text-center text-sm text-ctp-overlay1", "Loading fundamentals for {names.len()} stocks…" } }
            } else {
                Card { title: "{chosen.label}", subtitle: better_hint(chosen.better).to_string(),
                    BarChart {
                        labels: names.clone(),
                        series: vec![BarSeries { name: chosen.label.to_string(), color: "var(--catppuccin-color-mauve)", values: chosen_values }],
                        unit: chosen.unit,
                    }
                }
                Card { title: tr("Measures"), flush: true,
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm whitespace-nowrap",
                            thead {
                                tr { class: "text-xs text-ctp-overlay1",
                                    th { class: "pl-6 pr-4 py-2.5 text-left font-medium", "" }
                                    for (i, name) in names.iter().enumerate() {
                                        th { class: "px-4 py-2.5 text-right font-medium",
                                            span { class: "inline-flex items-center gap-1.5",
                                                span { class: "h-2 w-2 rounded-full", style: "background:{CHART_COLORS_HEX[i % CHART_COLORS_HEX.len()]};" }
                                                "{name}"
                                            }
                                        }
                                    }
                                }
                            }
                            tbody {
                                for (mi, m) in all.iter().enumerate() {
                                    MeasureRow {
                                        key: "{m.label}",
                                        group: (mi == 0 || all[mi - 1].group != m.group).then_some(m.group),
                                        columns: names.len(),
                                        label: m.label,
                                        hint: better_hint(m.better),
                                        values: values_of(m),
                                        better: m.better,
                                        unit: m.unit,
                                        active: selected() == mi,
                                        onclick: move |_| selected.set(mi),
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

fn better_hint(better: Better) -> &'static str {
    match better {
        Better::Higher => tr("Higher is better"),
        Better::Lower => tr("Lower is better"),
        Better::Neutral => tr("Size, not better or worse"),
    }
}

#[component]
fn MeasureRow(
    /// Group heading to show above this row (first row of a group).
    group: Option<&'static str>,
    columns: usize,
    label: &'static str,
    hint: &'static str,
    values: Vec<Option<f64>>,
    better: Better,
    unit: Unit,
    active: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let (best, worst) = best_worst(&values, better);
    rsx! {
        if let Some(group) = group {
            tr {
                td { class: "pl-6 pt-4 pb-1 text-xs font-semibold text-ctp-mauve", colspan: "{columns + 1}", "{group}" }
            }
        }
        tr {
            class: if active {
                "border-t border-ctp-surface0/60 bg-ctp-surface0/40 cursor-pointer"
            } else {
                "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors cursor-pointer"
            },
            title: "{hint}",
            onclick: move |e| onclick.call(e),
            td { class: "pl-6 pr-4 py-2.5",
                span { class: "text-ctp-subtext1", "{label}" }
                span { class: "ml-2 text-[0.7rem] text-ctp-overlay0",
                    match better {
                        Better::Higher => "↑",
                        Better::Lower => "↓",
                        Better::Neutral => "",
                    }
                }
            }
            for (i, v) in values.iter().enumerate() {
                td { class: "px-4 py-2.5 text-right",
                    span {
                        class: if best == Some(i) {
                            "rounded-full bg-ctp-green/15 px-2 py-0.5 font-semibold tabular-nums text-ctp-green"
                        } else if worst == Some(i) {
                            "rounded-full bg-ctp-red/10 px-2 py-0.5 tabular-nums text-ctp-red"
                        } else {
                            "tabular-nums text-ctp-subtext0"
                        },
                        {or_dash(*v, |v| unit.format(v))}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtos::fundamentals::{KeyStats, PeriodFinancials};

    #[test]
    fn presets_name_real_measures() {
        let labels: Vec<&str> = time_measures().iter().map(|m| m.label).collect();
        for (name, x, y) in PRESETS {
            assert!(labels.contains(&x) && labels.contains(&y), "preset {name}");
        }
    }

    #[test]
    fn pearson_and_reading() {
        let up = [Some(1.0), Some(2.0), None, Some(3.0), Some(4.0)];
        let down = [Some(8.0), Some(6.0), Some(5.0), Some(4.0), Some(2.0)];
        assert!((pearson(&up, &up).unwrap() - 1.0).abs() < 1e-9);
        assert!(pearson(&up, &down).unwrap() < -0.95);
        assert_eq!(pearson(&up[..2], &down[..2]), None);
        let text = describe_correlation("Revenue", "P/E", -0.8, 8);
        assert!(text.contains("strongly move in opposite directions"));
    }

    #[test]
    fn best_and_worst_follow_direction() {
        let v = [Some(20.0), None, Some(10.0), Some(30.0)];
        assert_eq!(best_worst(&v, Better::Lower), (Some(2), Some(3)));
        assert_eq!(best_worst(&v, Better::Higher), (Some(3), Some(2)));
        assert_eq!(best_worst(&v, Better::Neutral), (None, None));
        assert_eq!(best_worst(&[Some(1.0), None], Better::Higher), (None, None));
    }

    #[test]
    fn measures_read_fundamentals() {
        let quarter = PeriodFinancials {
            revenue: Some(100.0),
            gross_profit: Some(60.0),
            net_income: Some(20.0),
            total_equity: Some(400.0),
            ..Default::default()
        };
        let f = StockFundamentals {
            stats: KeyStats {
                price: Some(50.0),
                forward_pe: Some(18.0),
                ..Default::default()
            },
            quarterly: vec![quarter; 4],
            ..Default::default()
        };
        let get = |label: &str| {
            (measures()
                .into_iter()
                .find(|m| m.label == label)
                .unwrap()
                .get)(&f)
        };
        assert_eq!(get("Gross margin"), Some(0.6));
        assert_eq!(get("Net margin"), Some(0.2));
        assert_eq!(get("Return on equity"), Some(0.2)); // 80 / 400
        assert_eq!(get("Forward P/E"), Some(18.0));
        assert_eq!(get("Upside to target"), None);
    }
}
