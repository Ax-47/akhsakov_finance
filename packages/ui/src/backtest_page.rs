//! Backtest: how a mix of stocks would have done, as a lump sum and/or with
//! a monthly contribution (dollar-cost averaging), against a benchmark.

use crate::i18n::tr;
use crate::{
    app::AppSettings,
    components::{
        card::{ActionButton, Card, Field, MetricTile, Segmented, ToggleButton, INPUT},
        charts::{GrowthChart, Series},
    },
    format::{display_currency, fmt_usd},
    hooks::use_portfolio,
    page::{GhostButton, Page},
};
use api::quote::quote::get_charts;
use dioxus::prelude::*;
use dtos::insights::{backtest, BacktestPlan, BacktestResult};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::BTreeMap;
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

#[derive(Clone, PartialEq)]
struct Asset {
    ticker: String,
    /// Percent.
    weight: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Years {
    Three,
    Five,
    Ten,
    Max,
}

impl Years {
    fn label(self) -> &'static str {
        match self {
            Self::Three => "3 years",
            Self::Five => "5 years",
            Self::Ten => "10 years",
            Self::Max => tr("Max"),
        }
    }
    fn range(self) -> Range {
        match self {
            Self::Three => Range::Y5,
            Self::Five => Range::Y5,
            Self::Ten => Range::Y10,
            Self::Max => Range::Max,
        }
    }
    /// Months kept from the end of the fetched history.
    fn months(self) -> Option<usize> {
        match self {
            Self::Three => Some(37),
            Self::Five => Some(61),
            Self::Ten => Some(121),
            Self::Max => None,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Outcome {
    mix: BacktestResult,
    benchmark: BacktestResult,
    benchmark_name: String,
    /// First month every asset had a price; earlier months are partial.
    note: Option<String>,
}

#[component]
pub fn BacktestPage() -> Element {
    let settings = use_context::<AppSettings>();
    let allocation = use_portfolio(None).allocation;
    let mut assets = use_signal(|| {
        vec![
            Asset { ticker: "VOO".into(), weight: "60".into() },
            Asset { ticker: "QQQ".into(), weight: "40".into() },
        ]
    });
    let (symbol, rate) = display_currency();
    let mut initial = use_signal(move || format!("{:.0}", 10_000.0 * rate));
    let mut monthly = use_signal(move || format!("{:.0}", 500.0 * rate));
    let mut years = use_signal(|| Years::Five);
    let mut rebalance = use_signal(|| true);
    let mut outcome = use_signal(|| None::<Result<Outcome, String>>);
    let mut running = use_signal(|| false);

    let weights_total: f64 = assets.read().iter().filter_map(|a| a.weight.parse::<f64>().ok()).sum();

    let run = move |_| async move {
        running.set(true);
        let result = simulate(
            assets(),
            &initial(),
            &monthly(),
            years(),
            rebalance(),
            settings.benchmark(),
        )
        .await;
        outcome.set(Some(result));
        running.set(false);
    };

    let has_holdings = !allocation.is_empty();
    let run_label = if running() { tr("Running…") } else { tr("Run backtest") };
    let use_mine = move |_| {
        let mine: Vec<Asset> = allocation
            .iter()
            .map(|(t, w)| Asset { ticker: t.to_string(), weight: format!("{:.1}", w.to_f64().unwrap_or(0.0)) })
            .collect();
        if !mine.is_empty() {
            assets.set(mine);
        }
    };

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("Backtest")}
                }
                p { class: "mt-2 text-sm text-ctp-overlay1",
                    {tr("How a mix of stocks or funds would have done, investing a lump sum and a monthly amount. Uses monthly prices; dividends aren't reinvested.")}
                }
            }

            div { class: "mt-10 grid gap-5 lg:grid-cols-[1fr_320px] motion-safe:animate-rise",
                Card {
                    title: tr("Mix"),
                    subtitle: format!("Weights add up to {weights_total:.0}%{}", if (weights_total - 100.0).abs() > 0.5 { " — they'll be scaled to 100%" } else { "" }),
                    actions: rsx! {
                        div { class: "flex gap-2",
                            if has_holdings {
                                GhostButton { label: tr("Use my portfolio"), onclick: use_mine }
                            }
                            GhostButton { label: tr("＋ Add"), onclick: move |_| assets.write().push(Asset { ticker: String::new(), weight: "10".into() }) }
                        }
                    },
                    div { class: "grid gap-2",
                        for (i, a) in assets().into_iter().enumerate() {
                            div { key: "{i}", class: "flex items-center gap-2",
                                input {
                                    class: "{INPUT} flex-1 uppercase",
                                    placeholder: tr("Ticker, e.g. VOO"),
                                    value: "{a.ticker}",
                                    oninput: move |e| assets.write()[i].ticker = e.value(),
                                }
                                input {
                                    class: "{INPUT} w-24 text-right tabular-nums",
                                    inputmode: "decimal",
                                    value: "{a.weight}",
                                    oninput: move |e| assets.write()[i].weight = e.value(),
                                }
                                span { class: "text-sm text-ctp-overlay1", "%" }
                                button {
                                    class: "rounded-full px-2 py-1 text-ctp-overlay1 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                                    title: tr("Remove"),
                                    onclick: move |_| { assets.write().remove(i); },
                                    "×"
                                }
                            }
                        }
                    }
                }
                Card { title: tr("Plan"),
                    div { class: "grid gap-4",
                        Field { label: "Start with ({symbol.trim()})",
                            input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{initial}", oninput: move |e| initial.set(e.value()) }
                        }
                        Field { label: "Then every month ({symbol.trim()})",
                            input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{monthly}", oninput: move |e| monthly.set(e.value()) }
                        }
                        Field { label: tr("Over"),
                            Segmented {
                                for y in [Years::Three, Years::Five, Years::Ten, Years::Max] {
                                    ToggleButton { key: "{y.label()}", label: y.label(), active: years() == y, onclick: move |_| years.set(y) }
                                }
                            }
                        }
                        label { class: "flex items-center gap-2 text-sm text-ctp-subtext1 cursor-pointer",
                            input { r#type: "checkbox", checked: rebalance(), onchange: move |e| rebalance.set(e.checked()) }
                            {tr("Rebalance to the weights once a year")}
                        }
                        ActionButton { label: run_label, disabled: running(), onclick: run }
                    }
                }
            }

            match outcome() {
                None => rsx! {},
                Some(Err(message)) => rsx! {
                    div { class: "mt-5", Card { title: tr("Result"), p { class: "text-sm text-ctp-red", "{message}" } } }
                },
                Some(Ok(o)) => rsx! { Results { outcome: o } },
            }
        }
    }
}

#[component]
fn Results(outcome: Outcome) -> Element {
    let Outcome { mix, benchmark, benchmark_name, note } = outcome;
    let money = |v: f64| fmt_usd(Decimal::try_from(v).unwrap_or_default(), 0);
    let pct = |v: Option<f64>| v.map_or("—".into(), |v| format!("{:+.2}%", v * 100.0));
    let gain = mix.final_value - mix.invested;
    let vs = mix.final_value - benchmark.final_value;
    let labels: Vec<String> = mix.points.iter().map(|p| p.0.get(..7).unwrap_or_default().to_string()).collect();
    let growth = |r: &BacktestResult| -> Vec<Option<Decimal>> {
        r.points
            .iter()
            .map(|(_, v, inv)| (*inv > 0.0).then(|| Decimal::try_from((v / inv - 1.0) * 100.0).ok()).flatten())
            .collect()
    };
    let gain_tone = if gain >= 0.0 { "text-ctp-green" } else { "text-ctp-red" };
    let vs_tone = if vs >= 0.0 { "text-ctp-green" } else { "text-ctp-red" };
    let vs_sign = if vs >= 0.0 { "+" } else { "" };
    let series = vec![
        Series { name: "Your mix".into(), color: "var(--catppuccin-color-mauve)".into(), values: growth(&mix) },
        Series { name: benchmark_name.clone(), color: "var(--catppuccin-color-sky)".into(), values: growth(&benchmark) },
    ];
    rsx! {
        div { class: "mt-5 grid grid-cols-2 gap-3 lg:grid-cols-4 motion-safe:animate-rise",
            MetricTile {
                label: tr("Would be worth"),
                value: money(mix.final_value),
                hint: format!("You'd have put in {}", money(mix.invested)),
                tone: gain_tone,
            }
            MetricTile {
                label: tr("Yearly return"),
                value: pct(mix.irr),
                hint: format!("Growth rate {} a year, ignoring timing", pct(mix.cagr)),
            }
            MetricTile {
                label: tr("Worst fall"),
                value: format!("{:.1}%", mix.max_drawdown * 100.0),
                hint: tr("Largest drop from a peak along the way"),
                tone: "text-ctp-red",
            }
            MetricTile {
                label: format!("vs {benchmark_name}"),
                value: format!("{vs_sign}{}", money(vs)),
                hint: format!("{} would be worth {}", benchmark_name, money(benchmark.final_value)),
                tone: vs_tone,
            }
        }
        div { class: "mt-5 motion-safe:animate-rise",
            Card { title: tr("Return on money put in"), subtitle: note.unwrap_or_else(|| "Value ÷ amount invested so far".into()),
                document::Script { src: asset!("/assets/js/growth_chart.js") }
                GrowthChart { chart_dates: labels, series, height: Decimal::from(280) }
            }
        }
    }
}

/// Fetches monthly prices, aligns them by month and runs the plan for the
/// mix and the benchmark.
async fn simulate(
    assets: Vec<Asset>,
    initial: &str,
    monthly: &str,
    years: Years,
    rebalance: bool,
    benchmark: TickerSymbol,
) -> Result<Outcome, String> {
    let (_, rate) = display_currency();
    let amount = |s: &str| s.trim().replace(',', "").parse::<f64>().ok().filter(|v| *v >= 0.0);
    let (Some(initial), Some(monthly)) = (amount(initial), amount(monthly)) else {
        return Err("Enter amounts of zero or more".into());
    };
    if initial + monthly <= 0.0 {
        return Err("Invest something at the start or each month".into());
    }
    // Amounts are typed in the display currency; prices are USD.
    let (initial, monthly) = (initial / rate, monthly / rate);
    let mut picked: Vec<(TickerSymbol, f64)> = Vec::new();
    for a in &assets {
        if a.ticker.trim().is_empty() {
            continue;
        }
        let t = TickerSymbol::new(&a.ticker).map_err(|_| format!("“{}” isn't a ticker", a.ticker))?;
        let w = a.weight.trim().parse::<f64>().ok().filter(|w| *w > 0.0).ok_or(format!("Enter a weight for {t}"))?;
        picked.push((t, w));
    }
    if picked.is_empty() {
        return Err("Add at least one ticker".into());
    }

    let mut symbols: Vec<TickerSymbol> = picked.iter().map(|(t, _)| t.clone()).collect();
    symbols.push(benchmark.clone());
    let charts = get_charts(symbols, years.range(), Interval::M1, false)
        .await
        .map_err(|e| e.to_string())?;
    let monthly_closes = |t: &TickerSymbol| -> BTreeMap<String, f64> {
        charts
            .get(t)
            .map(|candles: &Vec<Candle>| {
                candles
                    .iter()
                    .filter_map(|c| Some((c.ts.date_naive().format("%Y-%m-01").to_string(), c.close.to_f64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let series: Vec<BTreeMap<String, f64>> = picked.iter().map(|(t, _)| monthly_closes(t)).collect();
    if let Some((t, _)) = picked.iter().zip(&series).find(|(_, s)| s.is_empty()).map(|(p, _)| p) {
        return Err(format!("No price history for {t}"));
    }
    let bench = monthly_closes(&benchmark);

    let mut dates: Vec<String> = series.iter().flat_map(|s| s.keys().cloned()).collect();
    dates.sort();
    dates.dedup();
    if let Some(keep) = years.months() {
        dates = dates.split_off(dates.len().saturating_sub(keep));
    }
    let prices: Vec<Vec<Option<f64>>> = series
        .iter()
        .map(|s| dates.iter().map(|d| s.get(d).copied()).collect())
        .collect();
    let late = picked
        .iter()
        .zip(&prices)
        .filter(|(_, p)| p.first().is_some_and(Option::is_none))
        .map(|((t, _), p)| {
            let first = p.iter().position(Option::is_some).and_then(|i| dates.get(i)).cloned().unwrap_or_default();
            format!("{t} from {}", first.get(..7).unwrap_or_default())
        })
        .collect::<Vec<_>>();

    let plan = BacktestPlan {
        weights: picked.iter().map(|(_, w)| *w).collect(),
        initial,
        contribution: monthly,
        rebalance_yearly: rebalance,
    };
    let mix = backtest(&dates, &prices, &plan);
    let bench_prices = vec![dates.iter().map(|d| bench.get(d).copied()).collect()];
    let benchmark_result = backtest(
        &dates,
        &bench_prices,
        &BacktestPlan { weights: vec![1.0], ..plan },
    );
    if mix.points.is_empty() {
        return Err("Not enough price history".into());
    }
    Ok(Outcome {
        mix,
        benchmark: benchmark_result,
        benchmark_name: if benchmark.as_str() == "^GSPC" { "S&P 500".into() } else { benchmark.to_string() },
        note: (!late.is_empty()).then(|| format!("Not listed yet at the start: {}. Until then the others take their share.", late.join(", "))),
    })
}
