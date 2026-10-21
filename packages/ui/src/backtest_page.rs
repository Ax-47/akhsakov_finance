//! Backtest: how a mix of stocks would have done, as a lump sum and/or with
//! a monthly contribution (dollar-cost averaging), against a benchmark.

use crate::i18n::tr;
use crate::{
    app::AppSettings,
    components::{
        card::{ActionButton, Card, Field, MetricTile, Segmented, ToggleButton, INPUT},
        charts::{GrowthChart, Series},
    },
    files::ExportButtons,
    format::{display_currency, fmt_usd},
    hooks::use_portfolio_memo,
    page::{GhostButton, Page},
};
use api::quote::quote::get_charts;
use dioxus::prelude::*;
use dtos::{
    csv_export::backtest_csv,
    insights::{backtest, BacktestPlan, BacktestResult, Fees, Rebalance},
};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::BTreeMap;
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

#[derive(Clone, PartialEq)]
struct Asset {
    ticker: String,
    /// Percent.
    weight: String,
}

/// The fee fields as typed: percentages, and a per-trade amount in the
/// display currency.
#[derive(Clone, PartialEq)]
struct FeeInputs {
    rate: String,
    per_trade: String,
    yearly: String,
}

impl FeeInputs {
    /// `rate` converts display-currency amounts to USD.
    fn parse(&self, rate: f64) -> Result<Fees, String> {
        let number = |s: &str| {
            let s = s.trim();
            if s.is_empty() {
                Some(0.0)
            } else {
                s.replace(',', "").parse::<f64>().ok().filter(|v| (0.0..=1e9).contains(v))
            }
        };
        let percent = |s: &str| number(s).filter(|v| *v <= 10.0).map(|v| v / 100.0);
        match (percent(&self.rate), number(&self.per_trade), percent(&self.yearly)) {
            (Some(r), Some(flat), Some(yearly)) => Ok(Fees { rate: r, per_trade: flat / rate, yearly }),
            _ => Err(tr("Fees are zero or more, and percentages at most 10%").into()),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum RebalanceChoice {
    Never,
    Yearly,
    Quarterly,
    Drift,
}

impl RebalanceChoice {
    const ALL: [Self; 4] = [Self::Never, Self::Yearly, Self::Quarterly, Self::Drift];

    fn label(self) -> &'static str {
        match self {
            Self::Never => tr("Off"),
            Self::Yearly => tr("Yearly"),
            Self::Quarterly => tr("Quarterly"),
            Self::Drift => tr("When off target"),
        }
    }

    /// `band` is the drift allowed, in percentage points, as typed.
    fn plan(self, band: &str) -> Result<Rebalance, String> {
        Ok(match self {
            Self::Never => Rebalance::Never,
            Self::Yearly => Rebalance::Every(12),
            Self::Quarterly => Rebalance::Every(3),
            Self::Drift => {
                let band = band.trim().parse::<f64>().ok().filter(|b| *b > 0.0 && *b < 100.0);
                Rebalance::Drift(band.ok_or_else(|| tr("Enter how far off target to allow, between 0 and 100 points").to_string())? / 100.0)
            }
        })
    }
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

/// How many mixes can be compared at once.
const MAX_MIXES: usize = 3;

/// Line colours for the mixes, in order; the benchmark is sky.
const MIX_COLORS: [&str; MAX_MIXES] = [
    "var(--catppuccin-color-mauve)",
    "var(--catppuccin-color-peach)",
    "var(--catppuccin-color-green)",
];

fn mix_name(i: usize) -> String {
    crate::i18n::trf("Mix {}", &[&(i + 1)])
}

#[derive(Clone, PartialEq)]
struct Outcome {
    /// Each mix's name and result, in the order they were set up.
    mixes: Vec<(String, BacktestResult)>,
    benchmark: BacktestResult,
    benchmark_name: String,
    /// First month every asset had a price; earlier months are partial.
    note: Option<String>,
}

#[component]
pub fn BacktestPage() -> Element {
    let settings = use_context::<AppSettings>();
    // Only whether you hold anything re-renders the page; the weights are
    // read when "use mine" is pressed, not on every price update.
    let portfolio = use_portfolio_memo(None);
    let has_holdings = use_memo(move || !portfolio.read().allocation.is_empty());
    let mut mixes = use_signal(|| {
        vec![vec![
            Asset { ticker: "VOO".into(), weight: "60".into() },
            Asset { ticker: "QQQ".into(), weight: "40".into() },
        ]]
    });
    // The mix being edited.
    let mut active = use_signal(|| 0usize);
    let assets = use_memo(move || mixes.read().get(active()).cloned().unwrap_or_default());
    let mut edit = move |change: &dyn Fn(&mut Vec<Asset>)| {
        let at = active();
        if let Some(mix) = mixes.write().get_mut(at) {
            change(mix);
        }
    };
    let (symbol, rate) = display_currency();
    let mut initial = use_signal(move || format!("{:.0}", 10_000.0 * rate));
    let mut monthly = use_signal(move || format!("{:.0}", 500.0 * rate));
    let mut years = use_signal(|| Years::Five);
    let mut rebalance = use_signal(|| RebalanceChoice::Yearly);
    let mut drift = use_signal(|| "5".to_string());
    let mut fees = use_signal(|| FeeInputs { rate: "0".into(), per_trade: "0".into(), yearly: "0".into() });
    let mut outcome = use_signal(|| None::<Result<Outcome, String>>);
    let mut running = use_signal(|| false);

    let weights_total: f64 = assets.read().iter().filter_map(|a| a.weight.parse::<f64>().ok()).sum();

    let run = move |_| async move {
        running.set(true);
        let result = match rebalance().plan(&drift()) {
            Ok(rebalance) => simulate(
                mixes(),
                &initial(),
                &monthly(),
                years(),
                rebalance,
                fees(),
                settings.benchmark(),
            )
            .await,
            Err(message) => Err(message),
        };
        outcome.set(Some(result));
        running.set(false);
    };

    let run_label = if running() { tr("Running…") } else { tr("Run backtest") };
    let use_mine = move |_| {
        let mine: Vec<Asset> = portfolio
            .peek()
            .allocation
            .iter()
            .map(|(t, w)| Asset { ticker: t.to_string(), weight: format!("{:.1}", w.to_f64().unwrap_or(0.0)) })
            .collect();
        if !mine.is_empty() {
            edit(&|mix| *mix = mine.clone());
        }
    };
    let mix_count = mixes.read().len();

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    {tr("Backtest")}
                }
                p { class: "mt-2 text-sm text-ctp-subtext0",
                    {tr("How a mix of stocks or funds would have done, investing a lump sum and a monthly amount. Uses monthly prices with dividends reinvested (an index benchmark such as ^GSPC leaves them out).")}
                }
            }

            div { class: "mt-10 grid gap-5 lg:grid-cols-[1fr_360px] motion-safe:animate-rise",
                Card {
                    title: tr("Mix"),
                    subtitle: {let base = crate::i18n::trf("Weights add up to {}%", &[&format!("{weights_total:.0}")]); if (weights_total - 100.0).abs() > 0.5 { format!("{base} — {}", tr("they'll be scaled to 100%")) } else { base }},
                    actions: rsx! {
                        div { class: "flex gap-2",
                            if has_holdings() {
                                GhostButton { label: tr("Use my portfolio"), onclick: use_mine }
                            }
                            GhostButton { label: tr("＋ Add"), onclick: move |_| edit(&|mix| mix.push(Asset { ticker: String::new(), weight: "10".into() })) }
                        }
                    },
                    if mix_count > 1 {
                        div { class: "mb-3 flex flex-wrap items-center justify-between gap-2",
                            Segmented {
                                for k in 0..mix_count {
                                    ToggleButton { key: "{k}", label: mix_name(k), active: active() == k, onclick: move |_| active.set(k) }
                                }
                            }
                            GhostButton {
                                label: tr("Remove this mix"),
                                onclick: move |_| {
                                    let at = active();
                                    mixes.write().remove(at);
                                    active.set(at.saturating_sub(1));
                                },
                            }
                        }
                    }
                    div { class: "grid gap-2",
                        for (i, a) in assets().into_iter().enumerate() {
                            div { key: "{i}", class: "flex items-center gap-2",
                                div { class: "min-w-0 flex-1",
                                    input {
                                        class: "{INPUT} uppercase",
                                        placeholder: tr("Ticker, e.g. VOO"),
                                        value: "{a.ticker}",
                                        oninput: move |e| edit(&|mix| mix[i].ticker = e.value()),
                                    }
                                }
                                // INPUT is w-full, so the width lives on a wrapper.
                                div { class: "w-20 shrink-0",
                                    input {
                                        class: "{INPUT} text-right tabular-nums",
                                        inputmode: "decimal",
                                        value: "{a.weight}",
                                        oninput: move |e| edit(&|mix| mix[i].weight = e.value()),
                                    }
                                }
                                span { class: "text-sm text-ctp-subtext0", "%" }
                                button {
                                    class: "rounded-full px-2 py-1 text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-red",
                                    title: tr("Remove"),
                                    onclick: move |_| edit(&|mix| { mix.remove(i); }),
                                    "×"
                                }
                            }
                        }
                    }
                    if mix_count < MAX_MIXES {
                        div { class: "mt-3",
                            GhostButton {
                                label: tr("＋ Compare another mix"),
                                onclick: move |_| {
                                    let copy = assets();
                                    mixes.write().push(copy);
                                    active.set(mix_count);
                                },
                            }
                        }
                    }
                }
                Card { title: tr("Plan"),
                    // One column that can't grow past the card, so wide choices
                    // scroll instead of pushing the fields out.
                    div { class: "grid grid-cols-1 gap-4",
                        Field { label: format!("{} ({})", tr("Start with"), symbol.trim()),
                            input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{initial}", oninput: move |e| initial.set(e.value()) }
                        }
                        Field { label: format!("{} ({})", tr("Then every month"), symbol.trim()),
                            input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{monthly}", oninput: move |e| monthly.set(e.value()) }
                        }
                        Field { label: tr("Over"),
                            Segmented {
                                for y in [Years::Three, Years::Five, Years::Ten, Years::Max] {
                                    ToggleButton { key: "{y.label()}", label: y.label(), active: years() == y, onclick: move |_| years.set(y) }
                                }
                            }
                        }
                        Field { label: tr("Rebalance to the weights"),
                            Segmented {
                                for choice in RebalanceChoice::ALL {
                                    ToggleButton { key: "{choice.label()}", label: choice.label(), active: rebalance() == choice, onclick: move |_| rebalance.set(choice) }
                                }
                            }
                        }
                        if rebalance() == RebalanceChoice::Drift {
                            Field { label: tr("When a weight is off by (percentage points)"),
                                input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{drift}", oninput: move |e| drift.set(e.value()) }
                            }
                        }
                        details {
                            summary { class: "cursor-pointer text-sm text-ctp-subtext1", {tr("Fees")} }
                            div { class: "mt-3 grid gap-3",
                                Field { label: tr("Trading fee (% of each trade)"),
                                    input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{fees.read().rate}", oninput: move |e| fees.write().rate = e.value() }
                                }
                                Field { label: format!("{} ({})", tr("Fee per trade"), symbol.trim()),
                                    input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{fees.read().per_trade}", oninput: move |e| fees.write().per_trade = e.value() }
                                }
                                Field { label: tr("Yearly fee (% of value)"), hint: tr("Fund prices already include their own expense ratio; use this for anything on top, such as an advisory or platform fee."),
                                    input { class: "{INPUT} tabular-nums", inputmode: "decimal", value: "{fees.read().yearly}", oninput: move |e| fees.write().yearly = e.value() }
                                }
                                p { class: "text-xs text-ctp-overlay1", {tr("The benchmark pays the same fees.")} }
                            }
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
                Some(Ok(o)) if o.mixes.len() == 1 => rsx! { Results { outcome: o } },
                Some(Ok(o)) => rsx! { Comparison { outcome: o } },
            }
        }
    }
}

#[component]
fn Results(outcome: Outcome) -> Element {
    let Outcome { mixes, benchmark, benchmark_name, note } = outcome;
    let mix = mixes.into_iter().next().map(|(_, r)| r).unwrap_or_default();
    let export = vec![(tr("Your mix").to_string(), mix.clone()), (benchmark_name.clone(), benchmark.clone())];
    let risk_free = use_context::<AppSettings>().0.read().risk_free.to_f64().unwrap_or(0.0) / 100.0;
    let sharpe = mix.sharpe(risk_free);
    let bench_sharpe = benchmark.sharpe(risk_free);
    let year = |y: &Option<(String, f64)>| y.as_ref().map_or("—".into(), |(y, r)| format!("{:+.1}% ({y})", r * 100.0));
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
        Series { name: tr("Your mix").into(), color: "var(--catppuccin-color-mauve)".into(), values: growth(&mix) },
        Series { name: benchmark_name.clone(), color: "var(--catppuccin-color-sky)".into(), values: growth(&benchmark) },
    ];
    rsx! {
        div { class: "mt-5 grid grid-cols-2 gap-3 lg:grid-cols-4 motion-safe:animate-rise",
            MetricTile {
                label: tr("Would be worth"),
                value: money(mix.final_value),
                hint: crate::i18n::trf("You'd have put in {}", &[&money(mix.invested)]),
                tone: gain_tone,
            }
            MetricTile {
                label: tr("Yearly return"),
                value: pct(mix.irr),
                hint: crate::i18n::trf("Growth rate {} a year, ignoring timing", &[&pct(mix.cagr)]),
            }
            MetricTile {
                label: tr("Worst fall"),
                value: format!("{:.1}%", mix.max_drawdown * 100.0),
                hint: match (mix.max_drawdown < 0.0, mix.recovery_months) {
                    (false, _) => tr("Largest drop from a peak along the way").to_string(),
                    (true, Some(months)) => crate::i18n::trf("Back to its peak after {} months", &[&months]),
                    (true, None) => tr("Not back to its peak yet").to_string(),
                },
                tone: "text-ctp-red",
            }
            MetricTile {
                label: crate::i18n::trf("vs {}", &[&benchmark_name]),
                value: format!("{vs_sign}{}", money(vs)),
                hint: crate::i18n::trf("{} would be worth {}", &[&benchmark_name, &money(benchmark.final_value)]),
                tone: vs_tone,
            }
        }
        div { class: "mt-3 grid grid-cols-2 gap-3 lg:grid-cols-4 motion-safe:animate-rise",
            MetricTile {
                label: tr("Ups and downs"),
                value: mix.volatility.map_or("—".into(), |v| format!("{:.1}%", v * 100.0)),
                hint: tr("Volatility: how far a year typically swings"),
            }
            MetricTile {
                label: tr("Sharpe ratio"),
                value: sharpe.map_or("—".into(), |v| format!("{v:.2}")),
                hint: crate::i18n::trf("Return per unit of risk · {} {}", &[&benchmark_name, &bench_sharpe.map_or("—".into(), |v| format!("{v:.2}"))]),
            }
            MetricTile {
                label: tr("Best year"),
                value: year(&mix.best_year),
                hint: tr("Full calendar years only"),
                tone: "text-ctp-green",
            }
            MetricTile {
                label: tr("Worst year"),
                value: year(&mix.worst_year),
                hint: tr("Full calendar years only"),
                tone: "text-ctp-red",
            }
        }
        p { class: "mt-2 text-xs text-ctp-overlay1",
            if mix.fees_paid > 0.0 {
                {crate::i18n::trf("Prices include dividends, reinvested. Fees paid: {}; taxes aren't counted.", &[&money(mix.fees_paid)])}
            } else {
                {tr("Prices include dividends, reinvested; fees and taxes aren't counted.")}
            }
            if mix.rebalances > 0 {
                " "
                {crate::i18n::trf("Rebalanced {} times.", &[&mix.rebalances])}
            }
        }
        div { class: "mt-5 motion-safe:animate-rise",
            Card { title: tr("Return on money put in"), subtitle: note.unwrap_or_else(|| tr("Value ÷ amount invested so far").into()),
                actions: rsx! { ExportButtons { filename: "akhsakov-backtest.csv", csv: move || backtest_csv(&export) } },
                document::Script { src: asset!("/assets/js/growth_chart.js") }
                GrowthChart { chart_dates: labels, series, height: Decimal::from(280) }
            }
        }
    }
}

/// Several mixes side by side: one line each on the chart and a column each
/// in a table of the same measures.
#[component]
fn Comparison(outcome: Outcome) -> Element {
    let Outcome { mixes, benchmark, benchmark_name, note } = outcome;
    let risk_free = use_context::<AppSettings>().0.read().risk_free.to_f64().unwrap_or(0.0) / 100.0;
    let money = |v: f64| fmt_usd(Decimal::try_from(v).unwrap_or_default(), 0);
    let pct = |v: Option<f64>| v.map_or("—".into(), |v| format!("{:+.2}%", v * 100.0));
    let year = |y: &Option<(String, f64)>| y.as_ref().map_or("—".into(), |(y, r)| format!("{:+.1}% ({y})", r * 100.0));
    let labels: Vec<String> = mixes[0].1.points.iter().map(|p| p.0.get(..7).unwrap_or_default().to_string()).collect();
    let mut columns: Vec<(String, &'static str, BacktestResult)> =
        mixes.into_iter().zip(MIX_COLORS).map(|((name, r), color)| (name, color, r)).collect();
    columns.push((benchmark_name, "var(--catppuccin-color-sky)", benchmark));
    let export: Vec<(String, BacktestResult)> = columns.iter().map(|(name, _, r)| (name.clone(), r.clone())).collect();
    let rows: Vec<(&'static str, Vec<String>)> = vec![
        (tr("Would be worth"), columns.iter().map(|c| money(c.2.final_value)).collect()),
        (tr("Invested"), columns.iter().map(|c| money(c.2.invested)).collect()),
        (tr("Yearly return"), columns.iter().map(|c| pct(c.2.irr)).collect()),
        (tr("Growth rate"), columns.iter().map(|c| pct(c.2.cagr)).collect()),
        (tr("Worst fall"), columns.iter().map(|c| format!("{:.1}%", c.2.max_drawdown * 100.0)).collect()),
        (
            tr("Back to its peak"),
            columns
                .iter()
                .map(|c| match (c.2.max_drawdown < 0.0, c.2.recovery_months) {
                    (false, _) => "—".to_string(),
                    (true, Some(months)) => crate::i18n::trf("{} months", &[&months]),
                    (true, None) => tr("Not yet").to_string(),
                })
                .collect(),
        ),
        (tr("Ups and downs"), columns.iter().map(|c| c.2.volatility.map_or("—".into(), |v| format!("{:.1}%", v * 100.0))).collect()),
        (tr("Sharpe ratio"), columns.iter().map(|c| c.2.sharpe(risk_free).map_or("—".into(), |v| format!("{v:.2}"))).collect()),
        (tr("Best year"), columns.iter().map(|c| year(&c.2.best_year)).collect()),
        (tr("Worst year"), columns.iter().map(|c| year(&c.2.worst_year)).collect()),
        (tr("Fees"), columns.iter().map(|c| money(c.2.fees_paid)).collect()),
        (tr("Rebalances"), columns.iter().map(|c| c.2.rebalances.to_string()).collect()),
    ];
    let growth = |r: &BacktestResult| -> Vec<Option<Decimal>> {
        r.points
            .iter()
            .map(|(_, v, inv)| (*inv > 0.0).then(|| Decimal::try_from((v / inv - 1.0) * 100.0).ok()).flatten())
            .collect()
    };
    let series: Vec<Series> =
        columns.iter().map(|(name, color, r)| Series { name: name.clone(), color: (*color).into(), values: growth(r) }).collect();
    rsx! {
        div { class: "mt-5 motion-safe:animate-rise",
            Card { title: tr("Compare"), flush: true,
                div { class: "overflow-x-auto",
                    table { class: "w-full text-sm whitespace-nowrap",
                        thead {
                            tr { class: "text-xs text-ctp-subtext0",
                                th { class: "pl-6 pr-4 py-2.5 text-left font-medium", "" }
                                for (name, color, _) in columns.iter() {
                                    th { key: "{name}", class: "px-4 py-2.5 text-right font-medium",
                                        span { class: "inline-flex items-center gap-1.5",
                                            span { class: "h-2 w-2 rounded-full", style: "background:{color};" }
                                            "{name}"
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            for (label, values) in rows {
                                tr { key: "{label}", class: "border-t border-ctp-surface0/60",
                                    td { class: "pl-6 pr-4 py-2.5 text-ctp-subtext1", "{label}" }
                                    for (i, value) in values.into_iter().enumerate() {
                                        td { key: "{i}", class: "px-4 py-2.5 text-right tabular-nums", "{value}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        p { class: "mt-2 text-xs text-ctp-overlay1", {tr("Prices include dividends, reinvested; taxes aren't counted.")} }
        div { class: "mt-5 motion-safe:animate-rise",
            Card { title: tr("Return on money put in"), subtitle: note.unwrap_or_else(|| tr("Value ÷ amount invested so far").into()),
                actions: rsx! { ExportButtons { filename: "akhsakov-backtest.csv", csv: move || backtest_csv(&export) } },
                document::Script { src: asset!("/assets/js/growth_chart.js") }
                GrowthChart { chart_dates: labels, series, height: Decimal::from(280) }
            }
        }
    }
}

/// Every ticker in the mixes once, in the order first seen.
fn unique_tickers(mixes: &[Vec<(TickerSymbol, f64)>]) -> Vec<TickerSymbol> {
    let mut tickers: Vec<TickerSymbol> = Vec::new();
    for (t, _) in mixes.iter().flatten() {
        if !tickers.contains(t) {
            tickers.push(t.clone());
        }
    }
    tickers
}

/// One mix's price columns, picked from `prices` (one per `tickers`).
fn mix_prices(mix: &[(TickerSymbol, f64)], tickers: &[TickerSymbol], prices: &[Vec<Option<f64>>]) -> Vec<Vec<Option<f64>>> {
    mix.iter()
        .filter_map(|(t, _)| tickers.iter().position(|x| x == t).map(|i| prices[i].clone()))
        .collect()
}

/// Fetches monthly prices, aligns them by month and runs the plan for each
/// mix and the benchmark.
async fn simulate(
    mixes: Vec<Vec<Asset>>,
    initial: &str,
    monthly: &str,
    years: Years,
    rebalance: Rebalance,
    fees: FeeInputs,
    benchmark: TickerSymbol,
) -> Result<Outcome, String> {
    let (_, rate) = display_currency();
    let fees = fees.parse(rate)?;
    let amount = |s: &str| s.trim().replace(',', "").parse::<f64>().ok().filter(|v| *v >= 0.0);
    let (Some(initial), Some(monthly)) = (amount(initial), amount(monthly)) else {
        return Err(tr("Enter amounts of zero or more").into());
    };
    if initial + monthly <= 0.0 {
        return Err(tr("Invest something at the start or each month").into());
    }
    // Amounts are typed in the display currency; prices are USD.
    let (initial, monthly) = (initial / rate, monthly / rate);
    let several = mixes.len() > 1;
    let mut picked: Vec<Vec<(TickerSymbol, f64)>> = Vec::new();
    for (k, assets) in mixes.iter().enumerate() {
        let mut mix = Vec::new();
        for a in assets {
            if a.ticker.trim().is_empty() {
                continue;
            }
            let t = TickerSymbol::new(&a.ticker).map_err(|_| crate::i18n::trf("“{}” isn't a ticker", &[&a.ticker]))?;
            let w = a.weight.trim().parse::<f64>().ok().filter(|w| *w > 0.0).ok_or(crate::i18n::trf("Enter a weight for {}", &[&t]))?;
            mix.push((t, w));
        }
        if mix.is_empty() {
            return Err(if several {
                crate::i18n::trf("Add at least one ticker to {}", &[&mix_name(k)])
            } else {
                tr("Add at least one ticker").into()
            });
        }
        picked.push(mix);
    }

    let tickers = unique_tickers(&picked);
    let mut symbols = tickers.clone();
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
    let series: Vec<BTreeMap<String, f64>> = tickers.iter().map(monthly_closes).collect();
    if let Some(t) = tickers.iter().zip(&series).find(|(_, s)| s.is_empty()).map(|(t, _)| t) {
        return Err(crate::i18n::trf("No price history for {}", &[&t]));
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
    let late = tickers
        .iter()
        .zip(&prices)
        .filter(|(_, p)| p.first().is_some_and(Option::is_none))
        .map(|(t, p)| {
            let first = p.iter().position(Option::is_some).and_then(|i| dates.get(i)).cloned().unwrap_or_default();
            crate::i18n::trf("{} from {}", &[&t, &first.get(..7).unwrap_or_default()])
        })
        .collect::<Vec<_>>();

    let plan = BacktestPlan { weights: vec![], initial, contribution: monthly, rebalance, fees };
    let mut results = Vec::new();
    for (k, mix) in picked.iter().enumerate() {
        let columns = mix_prices(mix, &tickers, &prices);
        let weights = mix.iter().map(|(_, w)| *w).collect();
        let result = backtest(&dates, &columns, &BacktestPlan { weights, ..plan.clone() });
        if result.points.is_empty() {
            return Err(tr("Not enough price history").into());
        }
        results.push((mix_name(k), result));
    }
    let bench_prices = vec![dates.iter().map(|d| bench.get(d).copied()).collect()];
    let benchmark_result = backtest(
        &dates,
        &bench_prices,
        &BacktestPlan { weights: vec![1.0], ..plan },
    );
    Ok(Outcome {
        mixes: results,
        benchmark: benchmark_result,
        benchmark_name: dtos::settings::benchmark_name(benchmark.as_str()),
        note: (!late.is_empty()).then(|| crate::i18n::trf("Not listed yet at the start: {}. Until then the others take their share.", &[&late.join(", ")])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(rate: &str, per_trade: &str, yearly: &str) -> FeeInputs {
        FeeInputs { rate: rate.into(), per_trade: per_trade.into(), yearly: yearly.into() }
    }

    #[test]
    fn mixes_share_one_price_fetch() {
        let t = |s: &str| TickerSymbol::new(s).unwrap();
        let mixes = vec![vec![(t("VOO"), 60.0), (t("QQQ"), 40.0)], vec![(t("BND"), 50.0), (t("VOO"), 50.0)]];
        let tickers = unique_tickers(&mixes);
        assert_eq!(tickers, [t("VOO"), t("QQQ"), t("BND")]);
        let prices = vec![vec![Some(1.0)], vec![Some(2.0)], vec![Some(3.0)]];
        assert_eq!(mix_prices(&mixes[1], &tickers, &prices), [vec![Some(3.0)], vec![Some(1.0)]]);
    }

    #[test]
    fn reads_rebalance_choices() {
        assert_eq!(RebalanceChoice::Quarterly.plan("x"), Ok(Rebalance::Every(3)));
        assert_eq!(RebalanceChoice::Drift.plan(" 5 "), Ok(Rebalance::Drift(0.05)));
        assert!(RebalanceChoice::Drift.plan("0").is_err());
        assert!(RebalanceChoice::Drift.plan("").is_err());
    }

    #[test]
    fn reads_fee_inputs() {
        // A per-trade fee of ฿35 at 35 baht to the dollar is $1.
        let fees = inputs("0.15", "35", "").parse(35.0).unwrap();
        assert!((fees.rate - 0.0015).abs() < 1e-12);
        assert!((fees.per_trade - 1.0).abs() < 1e-12);
        assert_eq!(fees.yearly, 0.0);
        assert!(inputs("-1", "0", "0").parse(1.0).is_err());
        assert!(inputs("0", "0", "12").parse(1.0).is_err());
        assert!(inputs("0", "abc", "0").parse(1.0).is_err());
    }
}
