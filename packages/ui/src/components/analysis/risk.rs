//! History-based risk analysis: how much the portfolio swings, how bad a
//! bad day gets, which holdings drive the risk, what past crises would do
//! to it, where the money is exposed, and how it compares with the
//! benchmark. Uses today's weights applied to past daily returns, measured
//! in the display currency.

use super::stats::{day_label, days_from_civil, Exclusion, PriceSeries, RiskReport};
use crate::i18n::{tr, tr_str, trf};
use crate::hooks::mpt::MptAnalysis;
use crate::{
    app::AppSettings,
    components::{
        analysis::{CAPMCard, MptAnalysisCard},
        card::{Card, MetricTile, Segmented, ToggleButton},
        charts::CorrelationGraph,
        color_schema::CHART_COLOR_CLASSES,
    },
    format::{display_currency, fmt_usd},
};
use api::quote::quote::{get_charts, get_currencies};
use dioxus::prelude::*;
use dtos::{planning::fifo_lots, watch::AlertKind, Position, Transaction};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::{BTreeMap, HashMap};
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};

const PORTFOLIO_HEX: &str = "var(--catppuccin-color-mauve)"; // mauve
const MARKET_HEX: &str = "var(--catppuccin-color-overlay2)"; // overlay2
const LOSS_HEX: &str = "var(--catppuccin-color-red)"; // red

/// Share of a day's volume you could sell without moving the price much.
const PARTICIPATION: f64 = 0.2;

#[derive(Clone, Copy, PartialEq)]
enum Grade {
    Low,
    Moderate,
    High,
}

impl Grade {
    fn from_volatility(vol: f64) -> Self {
        match vol {
            v if v < 0.12 => Self::Low,
            v if v < 0.22 => Self::Moderate,
            _ => Self::High,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Low => tr("Low risk"),
            Self::Moderate => tr("Moderate risk"),
            Self::High => tr("High risk"),
        }
    }

    fn pill(self) -> &'static str {
        match self {
            Self::Low => "bg-ctp-green/15 text-ctp-green",
            Self::Moderate => "bg-ctp-yellow/15 text-ctp-yellow",
            Self::High => "bg-ctp-red/15 text-ctp-red",
        }
    }
}

/// How far back the risk figures look.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Window {
    M3,
    Y1,
    Y2,
    Y5,
}

impl Window {
    const ALL: [Self; 4] = [Self::M3, Self::Y1, Self::Y2, Self::Y5];

    fn label(self) -> &'static str {
        match self {
            Self::M3 => "3M",
            Self::Y1 => "1Y",
            Self::Y2 => "2Y",
            Self::Y5 => "5Y",
        }
    }

    fn range(self) -> Range {
        match self {
            Self::M3 => Range::M3,
            Self::Y1 => Range::Y1,
            Self::Y2 => Range::Y2,
            Self::Y5 => Range::Y5,
        }
    }

    fn phrase(self) -> &'static str {
        match self {
            Self::M3 => tr("3 months"),
            Self::Y1 => tr("year"),
            Self::Y2 => tr("2 years"),
            Self::Y5 => tr("5 years"),
        }
    }
}

// ─── Price history ────────────────────────────────────────────────────────────

/// Price history of the holdings (index-aligned with the tickers asked
/// for; empty when a ticker has none) and of the benchmark, all in
/// `currency`.
#[derive(Clone, PartialEq)]
pub(crate) struct History {
    pub holdings: Vec<PriceSeries>,
    pub market: PriceSeries,
    /// The display currency, or USD if its exchange rate couldn't be had.
    pub currency: String,
}

/// Whether the server already sends `ticker`'s prices in USD: stocks and
/// funds are converted, indices and FX pairs keep their own units.
fn priced_in_usd(ticker: &str) -> bool {
    !ticker.starts_with('^') && !ticker.ends_with("=X")
}

/// Yahoo pair giving the USD value of one unit of `code`; `None` for USD.
fn usd_rate_ticker(code: &str) -> Option<TickerSymbol> {
    (code != "USD")
        .then(|| TickerSymbol::new(&format!("{code}USD=X")).ok())
        .flatten()
}

/// Candles for the holdings and the benchmark, converted day by day into
/// `currency` (the benchmark first from its own currency when it's an
/// index), or left in USD if that currency's rates can't be had. `None`
/// if the benchmark, or the rate to bring it to USD, is missing.
pub(crate) async fn fetch_history(
    tickers: Vec<TickerSymbol>,
    benchmark: TickerSymbol,
    currency: String,
    range: Range,
    interval: Interval,
) -> Option<History> {
    Some(fetch_history_and_charts(tickers, benchmark, currency, range, interval).await?.0)
}

/// [`fetch_history`], plus the candles as fetched (USD for stocks, the
/// benchmark in its own units), for charts drawn from them.
pub(crate) async fn fetch_history_and_charts(
    tickers: Vec<TickerSymbol>,
    benchmark: TickerSymbol,
    currency: String,
    range: Range,
    interval: Interval,
) -> Option<(History, HashMap<TickerSymbol, Vec<types::candle::Candle>>)> {
    let bench_currency = if priced_in_usd(&benchmark) {
        "USD".to_string()
    } else {
        get_currencies(vec![benchmark.clone()])
            .await
            .ok()
            .and_then(|mut found| found.remove(&benchmark))
            .unwrap_or_else(|| "USD".into())
    };
    let (bench_fx, display_fx) = (usd_rate_ticker(&bench_currency), usd_rate_ticker(&currency));

    let mut symbols = tickers.clone();
    symbols.push(benchmark.clone());
    symbols.extend(bench_fx.clone());
    symbols.extend(display_fx.clone());
    symbols.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    symbols.dedup();
    let charts = get_charts(symbols, range, interval, false).await.ok()?;

    // A benchmark rate that's missing: better nothing than a benchmark in
    // the wrong currency. A missing display rate: stay in USD, and say so.
    let rate = |t: &Option<TickerSymbol>| -> Option<Option<PriceSeries>> {
        match t {
            None => Some(None),
            Some(t) => Some(Some(PriceSeries::from_fx_candles(charts.get(t)?))),
        }
    };
    let bench_rate = rate(&bench_fx)?;
    let (display_rate, currency) = match rate(&display_fx) {
        Some(r) => (r, currency),
        None => (None, "USD".to_string()),
    };
    let holdings = tickers
        .iter()
        .map(|t| {
            charts
                .get(t)
                .map(|c| PriceSeries::from_candles(c).converted(None, display_rate.as_ref()))
                .unwrap_or_default()
        })
        .collect();
    let market = PriceSeries::from_candles(charts.get(&benchmark)?)
        .converted(bench_rate.as_ref(), display_rate.as_ref());
    Some((History { holdings, market, currency }, charts))
}

/// Cost-weighted average age, in years, of the lots of each holding still
/// held on `today` (`YYYY-MM-DD`).
fn held_years(transactions: &[Transaction], today: &str) -> HashMap<TickerSymbol, f64> {
    let Some(now) = day_number(today) else {
        return HashMap::new();
    };
    let mut sums: HashMap<TickerSymbol, (f64, f64)> = HashMap::new();
    for lot in fifo_lots(transactions).0 {
        let (Some(bought), Some(cost)) = (day_number(&lot.date), (lot.shares * lot.cost).to_f64()) else {
            continue;
        };
        let e = sums.entry(lot.ticker).or_default();
        e.0 += cost * (now - bought).max(0) as f64;
        e.1 += cost;
    }
    sums.into_iter()
        .filter(|(_, (_, cost))| *cost > 0.0)
        .map(|(t, (aged, cost))| (t, aged / cost / 365.25))
        .collect()
}

fn day_number(date: &str) -> Option<i64> {
    let mut parts = date.get(..10)?.splitn(3, '-');
    let y = parts.next()?.parse().ok()?;
    let m = parts.next()?.parse().ok()?;
    let d = parts.next()?.parse().ok()?;
    Some(days_from_civil(y, m, d))
}

// ─── Tab ──────────────────────────────────────────────────────────────────────

/// The portfolio page's Risk tab: risk overview, performance vs the
/// benchmark, stress tests, risk contribution and liquidity, past crises,
/// exposure, relations, diversification and CAPM. Price history is
/// fetched once here and shared, so CAPM uses the same betas.
/// `allocation` is `(ticker, weight %)`; `cash` is uninvested cash (USD).
#[component]
pub fn RiskTab(
    allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>,
    positions: Vec<Position>,
    total_value: Decimal,
    mpt: Option<MptAnalysis>,
    transactions: Vec<Transaction>,
    cash: Option<Decimal>,
) -> Element {
    let mut window = use_signal(|| Window::Y1);
    let mut with_cash = use_signal(|| false);

    // Refetch only when the set of tickers changes, not on price ticks.
    let tickers = use_memo(move || {
        allocation
            .read()
            .iter()
            .map(|(t, _)| t.clone())
            .collect::<Vec<_>>()
    });

    let app_settings = use_context::<AppSettings>();
    let history = crate::cache::use_cached(
        move || {
            format!(
                "risk/{:?}/{:?}/{}/{}",
                tickers(),
                window(),
                app_settings.benchmark(),
                app_settings.currency()
            )
        },
        move || {
            let (tickers, range) = (tickers(), window().range());
            let (benchmark, currency) = (app_settings.benchmark(), app_settings.currency());
            async move { fetch_history(tickers, benchmark, currency, range, Interval::D1).await }
        },
    );

    // Cash in the allocation's units (% of holdings), rounded so price
    // ticks don't recompute the report.
    let cash_usd = cash.unwrap_or_default().max(Decimal::ZERO);
    let cash_pct = if with_cash() && total_value > Decimal::ZERO {
        ((cash_usd / total_value * Decimal::ONE_HUNDRED).round_dp(1)).to_f64().unwrap_or(0.0)
    } else {
        0.0
    };
    let report = use_memo(use_reactive!(|cash_pct| {
        let weights: Vec<f64> = allocation
            .read()
            .iter()
            .map(|(_, w)| w.to_f64().unwrap_or(0.0))
            .collect();
        let history = history.read();
        // Outer None: still loading. Inner None: not enough data.
        let loaded = history.as_ref()?;
        Some(loaded.as_ref().and_then(|h| {
            RiskReport::compute(&h.holdings, &weights, cash_pct, &h.market, app_settings.risk_free())
        }))
    }));

    let invested = total_value + if with_cash() { cash_usd } else { Decimal::ZERO };
    let bench = app_settings.benchmark_name();
    let price_index = dtos::settings::is_price_index(app_settings.benchmark().as_str());

    let actions = rsx! {
        div { class: "flex flex-wrap items-center gap-2",
            if cash_usd > Decimal::ZERO {
                Segmented {
                    ToggleButton { label: tr("Incl. cash"), active: with_cash(), onclick: move |_| with_cash.toggle() }
                }
            }
            Segmented {
                for w in Window::ALL {
                    ToggleButton { key: "{w.label()}", label: w.label(), active: window() == w, onclick: move |_| window.set(w) }
                }
            }
        }
    };

    let report = report();
    // Historical betas for CAPM and the crisis estimates.
    let betas: HashMap<TickerSymbol, f64> = match &report {
        Some(Some(r)) => r
            .holdings
            .iter()
            .filter_map(|h| Some((tickers.read().get(h.index)?.clone(), h.beta)))
            .collect(),
        _ => HashMap::new(),
    };
    let beta_map: HashMap<TickerSymbol, Decimal> = betas
        .iter()
        .filter_map(|(t, b)| Some((t.clone(), Decimal::try_from(*b).ok()?.round_dp(2))))
        .collect();

    let today = use_resource(crate::notify::today);
    let held = today
        .read()
        .clone()
        .flatten()
        .map(|d| held_years(&transactions, &d))
        .unwrap_or_default();

    let independent_bets = match &report {
        Some(Some(r)) => Some(r.independent_bets),
        _ => None,
    };

    let risk = match report {
        None => rsx! {
            Card { title: tr("Risk overview"), actions,
                p { class: "py-10 text-center text-sm text-ctp-subtext0", {tr("Crunching price history…")} }
            }
        },
        Some(None) => rsx! {
            Card { title: tr("Risk overview"), actions,
                p { class: "py-10 text-center text-sm text-ctp-subtext0",
                    {tr("Not enough shared price history yet to measure risk.")}
                }
            }
        },
        Some(Some(report)) => {
            let value = (invested.to_f64().unwrap_or(0.0)) * report.coverage;
            let shares: HashMap<TickerSymbol, f64> = positions
                .iter()
                .map(|p| (p.ticker.clone(), p.shares.to_f64().unwrap_or(0.0)))
                .collect();
            // Borrowed, not cloned: this runs on every price tick.
            let (volumes, currency): (Vec<Option<f64>>, String) = match history.read().as_ref() {
                Some(Some(h)) => (h.holdings.iter().map(|s| s.avg_volume).collect(), h.currency.clone()),
                _ => (Vec::new(), String::new()),
            };
            rsx! {
                RiskOverview {
                    report: report.clone(),
                    currency,
                    value,
                    period: window().phrase(),
                    bench: bench.clone(),
                    tickers: tickers(),
                    price_index,
                    actions,
                }
                div { class: "grid gap-5 lg:grid-cols-[1.4fr_1fr]",
                    PerformanceChart { report: report.clone(), bench: bench.clone() }
                    StressTest { report: report.clone(), value, bench: bench.clone() }
                }
                RiskContribution { report: report.clone(), tickers: tickers(), shares, volumes }
            }
        }
    };

    rsx! {
        {risk}
        ScenarioCard { allocation, betas, value: total_value.to_f64().unwrap_or(0.0) }
        ExposureCard { allocation }
        CorrelationGraph { allocation }
        MptAnalysisCard { mpt, allocation, independent_bets }
        CAPMCard { positions, total_value, beta_map, held_years: held }
    }
}

// ─── Overview ─────────────────────────────────────────────────────────────────

#[component]
fn RiskOverview(
    report: RiskReport,
    currency: String,
    value: f64,
    period: &'static str,
    bench: String,
    tickers: Vec<TickerSymbol>,
    price_index: bool,
    actions: Element,
) -> Element {
    let grade = Grade::from_volatility(report.volatility);
    let r = &report;
    let pct = |x: f64| x * 100.0;
    let usd = |x: f64| fmt_usd(Decimal::try_from(x).unwrap_or_default(), 0);

    let vs_market = if r.volatility > r.market_volatility * 1.1 {
        tr("more volatile than")
    } else if r.volatility < r.market_volatility * 0.9 {
        tr("calmer than")
    } else {
        tr("about as volatile as")
    };
    let summary = if r.full_year() {
        trf(
            "Over the last {} your portfolio returned {} a year with {} volatility, {} the {} ({} / {}).",
            &[
                &period,
                &format!("{:+.1}%", pct(r.annual_return)),
                &format!("{:.0}%", pct(r.volatility)),
                &vs_market,
                &bench,
                &format!("{:+.1}%", pct(r.market_return)),
                &format!("{:.0}%", pct(r.market_volatility)),
            ],
        )
    } else {
        // Returns over less than a year aren't annualised.
        trf(
            "Over the last {} your portfolio returned {} (not annualised) with {} yearly volatility, {} the {} ({} / {}).",
            &[
                &period,
                &format!("{:+.1}%", pct(r.period_return)),
                &format!("{:.0}%", pct(r.volatility)),
                &vs_market,
                &bench,
                &format!("{:+.1}%", pct(r.market_period_return)),
                &format!("{:.0}%", pct(r.market_volatility)),
            ],
        )
    };

    let test = r.var_test;
    let verdict = if test.p_value >= 0.05 {
        tr("in line")
    } else if test.breach_rate() > 0.05 {
        tr("it underestimates risk")
    } else {
        tr("it overestimates risk")
    };
    let excluded: Vec<String> = r
        .excluded
        .iter()
        .filter_map(|(i, why)| {
            let t = tickers.get(*i)?;
            Some(match why {
                Exclusion::NoHistory => trf("{} (no price history)", &[t]),
                Exclusion::TooNew => trf("{} (listed too recently)", &[t]),
            })
        })
        .collect();

    let mut alert_state = use_signal(|| None::<Result<(), String>>);
    let var_pct = Decimal::try_from(pct(r.var_95)).unwrap_or_default().round_dp(1);
    let set_alert = move |_| async move {
        let result = match TickerSymbol::new(dtos::watch::PORTFOLIO_TICKER) {
            Ok(t) => api::create_alert(t, AlertKind::PortfolioDayDrop, var_pct)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        alert_state.set(Some(result));
    };

    rsx! {
        Card {
            title: tr("Risk overview"),
            subtitle: trf("{} trading days · {} – {}", &[&r.days, &day_label(r.first_day), &day_label(r.last_day)]),
            actions,
            div { class: "flex flex-col sm:flex-row sm:items-start gap-4 mb-6",
                span { class: "self-start shrink-0 rounded-full px-3 py-1 text-sm font-semibold {grade.pill()}",
                    "{grade.label()}"
                }
                div { class: "text-sm leading-relaxed text-ctp-subtext0",
                    p {
                        "{summary} "
                        {trf("On a bad day — 1 in 20 — expect to lose about {} or more.", &[&usd(r.var_95 * value)])}
                    }
                    if !r.var_reliable() {
                        p { class: "mt-1 text-xs text-ctp-peach",
                            {trf("Only {} days of history: the VaR and shortfall rest on a handful of bad days, so treat them as rough.", &[&r.days])}
                        }
                    }
                    div { class: "mt-2 flex flex-wrap items-center gap-2 text-xs",
                        match alert_state() {
                            None => rsx! {
                                button {
                                    class: "rounded-full border border-ctp-surface1 px-3 py-1 text-ctp-subtext1 hover:border-ctp-mauve hover:text-ctp-text cursor-pointer",
                                    onclick: set_alert,
                                    {trf("🔔 Alert me if a day loses more than {}%", &[&var_pct])}
                                }
                            },
                            Some(Ok(())) => rsx! { span { class: "text-ctp-green", {tr("Alert set ✓ (see Watchlist → Alerts)")} } },
                            Some(Err(e)) => rsx! { span { class: "text-ctp-red", "{e}" } },
                        }
                    }
                }
            }
            div { class: "grid grid-cols-2 lg:grid-cols-4 gap-3",
                MetricTile {
                    label: tr("Volatility"),
                    value: format!("{:.1}%", pct(r.volatility)),
                    hint: trf("Lately {}% · {} {}%", &[&format!("{:.1}", pct(r.volatility_now)), &bench, &format!("{:.1}", pct(r.market_volatility))]),
                    tone: tone(r.volatility <= r.market_volatility),
                }
                MetricTile {
                    label: tr("Max drawdown"),
                    value: format!("−{:.1}%", pct(r.max_drawdown)),
                    hint: trf("Worst peak-to-trough · {} −{}%", &[&bench, &format!("{:.1}", pct(r.market_max_drawdown))]),
                    tone: tone(r.max_drawdown <= r.market_max_drawdown),
                }
                MetricTile {
                    label: tr("Value at risk (95%)"),
                    value: format!("−{:.2}%", pct(r.var_95)),
                    hint: trf("1 in 20 days: ≥ {} loss · now {}%", &[&usd(r.var_95 * value), &format!("{:.2}", pct(r.var_95_now))]),
                }
                MetricTile {
                    label: tr("Expected shortfall"),
                    value: format!("−{:.2}%", pct(r.cvar_95)),
                    hint: trf("Average bad day: {}", &[&usd(r.cvar_95 * value)]),
                }
                MetricTile {
                    label: tr("Sharpe ratio"),
                    value: format!("{:.2}", r.sharpe),
                    hint: trf("Return per risk · Sortino {}", &[&format!("{:.2}", r.sortino)]),
                    tone: tone(r.sharpe >= 1.0),
                }
                MetricTile {
                    label: tr("Beta"),
                    value: format!("{:.2}", r.beta),
                    hint: trf("Moves {}× the {} · corr {}", &[&format!("{:.1}", r.beta), &bench, &format!("{:.2}", r.market_correlation)]),
                }
                MetricTile {
                    label: tr("Alpha"),
                    value: format!("{:+.1}%", pct(r.alpha)),
                    hint: tr("Yearly return beyond beta").to_string(),
                    tone: tone(r.alpha >= 0.0),
                }
                MetricTile {
                    label: tr("Diversification"),
                    value: format!("{:.2}×", r.diversification_ratio),
                    hint: trf("≈ {} independent bets · higher is better", &[&format!("{:.1}", r.independent_bets)]),
                    tone: tone(r.diversification_ratio >= 1.2),
                }
            }
            div { class: "mt-4 flex flex-col gap-1 text-xs text-ctp-overlay1",
                if test.days > 0 {
                    p {
                        {trf("Back-test: the EWMA VaR was beaten on {} of {} days ({}; about 5% expected) — {}.", &[&test.breaches, &test.days, &format!("{:.1}%", pct(test.breach_rate())), &verdict])}
                    }
                }
                p {
                    {trf("Today's weights applied to past daily returns, in {}. Risk-free rate {}% (change it in Settings). Markets that close at different times are compared a day apart.", &[&currency, &format!("{:.1}", pct(r.risk_free))])}
                }
                if r.cash_weight > 0.0 {
                    p { {trf("Includes {}% cash earning the risk-free rate.", &[&format!("{:.0}", pct(r.cash_weight))])} }
                }
                if !excluded.is_empty() {
                    p { class: "text-ctp-peach",
                        {trf("Left out: {}. Figures cover the other {}% of the money.", &[&excluded.join(", "), &format!("{:.0}", pct(r.coverage))])}
                    }
                }
                if price_index {
                    p { class: "text-ctp-peach",
                        {trf("{} is an index level without dividends, while your holdings' prices include them: pick a fund such as SPY in Settings for a fair comparison.", &[&bench])}
                    }
                }
            }
            Link { to: "/learn/volatility", class: "mt-2 inline-block text-xs font-medium text-ctp-mauve hover:underline",
                {tr("New to these numbers? Learn what they mean →")}
            }
        }
    }
}

fn tone(good: bool) -> &'static str {
    if good {
        "text-ctp-green"
    } else {
        "text-ctp-peach"
    }
}

// ─── Performance chart ────────────────────────────────────────────────────────

const CHART_W: f64 = 600.0;
const CHART_H: f64 = 200.0;
const DD_H: f64 = 60.0;

/// Growth of 100 for the portfolio vs the benchmark, with a drawdown strip.
#[component]
fn PerformanceChart(report: RiskReport, bench: String) -> Element {
    let g = &report.growth;
    let (lo, hi) = g.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, p, m)| {
        (lo.min(*p).min(*m), hi.max(*p).max(*m))
    });
    let pad = ((hi - lo) * 0.08).max(0.01);
    let (lo, hi) = (lo - pad, hi + pad);
    let x = |i: usize| i as f64 / (g.len().max(2) - 1) as f64 * CHART_W;
    let y = |v: f64| CHART_H - (v - lo) / (hi - lo) * CHART_H;

    let path = |pick: fn(&(i64, f64, f64)) -> f64| {
        g.iter()
            .enumerate()
            .map(|(i, p)| format!("{:.1},{:.1}", x(i), y(pick(p))))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let port_line = path(|p| p.1);
    let market_line = path(|p| p.2);
    let port_area = format!("0,{CHART_H} {port_line} {CHART_W},{CHART_H}");

    let mut peak = 1.0_f64;
    let dd: Vec<f64> = g
        .iter()
        .map(|(_, p, _)| {
            peak = peak.max(*p);
            p / peak - 1.0
        })
        .collect();
    let dd_floor = dd.iter().cloned().fold(0.0_f64, f64::min).min(-0.01);
    let dd_area = format!(
        "0,0 {} {CHART_W},0",
        dd.iter()
            .enumerate()
            .map(|(i, d)| format!("{:.1},{:.1}", x(i), d / dd_floor * DD_H))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let (port_end, market_end) = g.last().map(|(_, p, m)| (*p, *m)).unwrap_or((1.0, 1.0));
    let base_y = y(1.0);
    let (symbol, _) = display_currency();

    rsx! {
        Card { title: tr("Performance"), subtitle: trf("Growth of {}100 vs the {}", &[&symbol, &bench]),
            div { class: "flex flex-wrap gap-x-5 gap-y-1 mb-3 text-xs",
                LegendDot { color: PORTFOLIO_HEX, label: format!("{} {symbol}{:.0}", tr("Portfolio"), port_end * 100.0) }
                LegendDot { color: MARKET_HEX, label: format!("{bench} {symbol}{:.0}", market_end * 100.0) }
            }
            svg { class: "w-full", view_box: "0 0 {CHART_W} {CHART_H}", preserve_aspect_ratio: "none",
                defs {
                    linearGradient { id: "perf-fill", x1: "0", y1: "0", x2: "0", y2: "1",
                        stop { offset: "0%", stop_color: PORTFOLIO_HEX, stop_opacity: "0.25" }
                        stop { offset: "100%", stop_color: PORTFOLIO_HEX, stop_opacity: "0" }
                    }
                }
                line {
                    x1: "0", x2: "{CHART_W}", y1: "{base_y:.1}", y2: "{base_y:.1}",
                    stroke: "var(--catppuccin-color-surface1)", stroke_dasharray: "4 4", vector_effect: "non-scaling-stroke",
                }
                polygon { points: "{port_area}", fill: "url(#perf-fill)" }
                polyline {
                    points: "{market_line}", fill: "none", stroke: MARKET_HEX,
                    stroke_width: "1.5", vector_effect: "non-scaling-stroke",
                }
                polyline {
                    points: "{port_line}", fill: "none", stroke: PORTFOLIO_HEX,
                    stroke_width: "2", stroke_linejoin: "round", vector_effect: "non-scaling-stroke",
                }
            }
            div { class: "mt-4 mb-1 flex justify-between text-xs text-ctp-subtext0",
                span { {tr("Drawdown")} }
                span { class: "text-ctp-red", {trf("worst −{}%", &[&format!("{:.1}", -dd_floor * 100.0)])} }
            }
            svg { class: "w-full h-12", view_box: "0 0 {CHART_W} {DD_H}", preserve_aspect_ratio: "none",
                polygon { points: "{dd_area}", fill: LOSS_HEX, fill_opacity: "0.25" }
            }
            div { class: "mt-2 flex justify-between text-xs text-ctp-overlay1",
                span { "{day_label(report.first_day)}" }
                span { "{day_label(report.last_day)}" }
            }
        }
    }
}

#[component]
fn LegendDot(color: &'static str, label: String) -> Element {
    rsx! {
        span { class: "flex items-center gap-1.5 text-ctp-subtext0",
            span { class: "h-2 w-2 rounded-full", style: "background:{color};" }
            "{label}"
        }
    }
}

// ─── Stress test ──────────────────────────────────────────────────────────────

#[component]
fn StressTest(report: RiskReport, value: f64, bench: String) -> Element {
    let shocks = [0.10, 0.20, 0.30].map(|drop| {
        (
            trf("{} falls {}%", &[&bench, &format!("{:.0}", drop * 100.0)]),
            tr("Estimated from beta").to_string(),
            -drop * report.beta,
        )
    });
    let history = [
        (tr("Worst day").to_string(), day_label(report.worst_day.0), report.worst_day.1),
        (
            tr("Worst week").to_string(),
            trf("Ending {}", &[&day_label(report.worst_week.0)]),
            report.worst_week.1,
        ),
        (
            tr("Max drawdown again").to_string(),
            tr("Peak-to-trough repeat").to_string(),
            -report.max_drawdown,
        ),
    ];

    rsx! {
        Card { title: tr("Stress test"), subtitle: tr("What a bad stretch would cost today").to_string(),
            div { class: "flex flex-col",
                for (label, note, change) in shocks.into_iter().chain(history) {
                    StressRow { label, note, change, value }
                }
            }
        }
    }
}

#[component]
fn StressRow(label: String, note: String, change: f64, value: f64) -> Element {
    let amount = Decimal::try_from(change * value).unwrap_or_default();
    // A holding set that rises when the market falls (negative beta) gains.
    let color = if change < 0.0 { "text-ctp-red" } else { "text-ctp-green" };
    rsx! {
        div { class: "flex items-center justify-between gap-3 py-2.5 border-t border-ctp-surface0/60 first:border-t-0",
            div { class: "min-w-0",
                div { class: "text-sm text-ctp-text", "{label}" }
                div { class: "text-xs text-ctp-overlay1 truncate", "{note}" }
            }
            div { class: "text-right shrink-0 tabular-nums",
                div { class: "text-sm font-semibold {color}", "{crate::format::fmt_signed(amount, 0)}" }
                div { class: "text-xs text-ctp-subtext0", "{change * 100.0:+.1}%" }
            }
        }
    }
}

// ─── Risk contribution ────────────────────────────────────────────────────────

/// `shares` held per ticker; `volumes` is each ticker's average daily
/// volume, index-aligned with `tickers`.
#[component]
fn RiskContribution(
    report: RiskReport,
    tickers: Vec<TickerSymbol>,
    shares: HashMap<TickerSymbol, f64>,
    volumes: Vec<Option<f64>>,
) -> Element {
    let mut rows: Vec<&super::stats::HoldingRisk> = report.holdings.iter().collect();
    rows.sort_by(|a, b| b.risk_share.total_cmp(&a.risk_share));

    rsx! {
        Card {
            title: tr("Where your risk comes from"),
            subtitle: tr("Share of portfolio swings each holding causes, next to its weight").to_string(),
            flush: true,
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            th { class: "pl-6 pr-4 py-2.5 text-left font-medium", {tr("Asset")} }
                            th { class: "px-4 py-2.5 text-left font-medium", {tr("Weight → risk share")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Volatility")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Beta")} }
                            th { class: "px-4 py-2.5 text-right font-medium", {tr("Max drawdown")} }
                            th {
                                class: "px-4 py-2.5 text-right font-medium",
                                title: tr("Trading days to sell the whole position at 20% of its average daily volume"),
                                {tr("Days to sell")}
                            }
                            th { class: "pl-4 pr-6 py-2.5 text-right font-medium", "" }
                        }
                    }
                    tbody {
                        for h in rows {
                            ContributionRow {
                                key: "{h.index}",
                                ticker: tickers.get(h.index).map(|t| t.to_string()).unwrap_or_default(),
                                color: CHART_COLOR_CLASSES[h.index % CHART_COLOR_CLASSES.len()],
                                holding: h.clone(),
                                days_to_sell: days_to_sell(
                                    tickers.get(h.index).and_then(|t| shares.get(t)).copied(),
                                    volumes.get(h.index).copied().flatten(),
                                ),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Trading days to sell `shares` at [`PARTICIPATION`] of the average daily
/// volume.
fn days_to_sell(shares: Option<f64>, avg_volume: Option<f64>) -> Option<f64> {
    let (shares, volume) = (shares?, avg_volume?);
    (volume > 0.0).then(|| shares / (volume * PARTICIPATION))
}

#[component]
fn ContributionRow(
    ticker: String,
    color: &'static str,
    holding: super::stats::HoldingRisk,
    days_to_sell: Option<f64>,
) -> Element {
    let h = &holding;
    let (weight, share) = (h.weight * 100.0, h.risk_share * 100.0);
    let flag = if h.risk_share > h.weight * 1.3 && h.risk_share > 0.1 {
        Some((tr("Outsized"), "bg-ctp-peach/15 text-ctp-peach"))
    } else if h.risk_share < h.weight * 0.7 {
        Some((tr("Stabiliser"), "bg-ctp-green/15 text-ctp-green"))
    } else {
        None
    };
    let cell = "px-4 py-3.5 text-right tabular-nums text-ctp-subtext0";
    let (liquidity, liquidity_tone) = match days_to_sell {
        None => ("—".to_string(), "text-ctp-overlay1"),
        Some(d) if d < 0.1 => ("< 0.1".to_string(), "text-ctp-subtext0"),
        Some(d) if d <= 1.0 => (format!("{d:.1}"), "text-ctp-subtext0"),
        Some(d) if d <= 5.0 => (format!("{d:.1}"), "text-ctp-peach"),
        Some(d) => (format!("{d:.0}"), "text-ctp-red"),
    };

    rsx! {
        tr { class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
            td { class: "pl-6 pr-4 py-3.5",
                span { class: "flex items-center gap-2.5",
                    span { class: "h-2.5 w-2.5 rounded-full shrink-0 {color}" }
                    span { class: "font-semibold text-ctp-text", "{ticker}" }
                }
            }
            td { class: "px-4 py-3.5",
                div { class: "flex items-center gap-3",
                    div { class: "flex w-32 flex-col gap-1",
                        span { class: "h-1 rounded-full bg-ctp-surface2", style: "width:{weight.clamp(2.0, 100.0):.0}%;" }
                        span { class: "h-1 rounded-full {color}", style: "width:{share.clamp(2.0, 100.0):.0}%;" }
                    }
                    span { class: "text-xs tabular-nums text-ctp-subtext0",
                        "{weight:.0}% → "
                        span { class: "font-semibold text-ctp-text", "{share:.0}%" }
                    }
                }
            }
            td { class: cell, "{h.volatility * 100.0:.1}%" }
            td { class: cell, "{h.beta:.2}" }
            td { class: cell, "−{h.max_drawdown * 100.0:.1}%" }
            td { class: "px-4 py-3.5 text-right tabular-nums {liquidity_tone}", "{liquidity}" }
            td { class: "pl-4 pr-6 py-3.5 text-right",
                if let Some((label, style)) = flag {
                    span { class: "rounded-full px-2.5 py-0.5 text-xs font-semibold {style}", "{label}" }
                }
            }
        }
    }
}

// ─── Past crises ──────────────────────────────────────────────────────────────

/// (year, month, day).
type Date = (i64, u32, u32);

/// Big sell-offs of the S&P 500, peak to trough, as (name, from, to).
/// Dates are the Fridays closing each end's week, since weekly closes are
/// used (daily prices for decades would be a very large download).
const SCENARIOS: [(&str, Date, Date); 5] = [
    ("2008 financial crisis", (2007, 10, 12), (2009, 3, 6)),
    ("2011 debt-ceiling scare", (2011, 4, 29), (2011, 9, 30)),
    ("2018 Q4 sell-off", (2018, 9, 21), (2018, 12, 21)),
    ("COVID crash, 2020", (2020, 2, 21), (2020, 3, 20)),
    ("2022 rate-hike bear market", (2022, 1, 7), (2022, 10, 14)),
];
/// A weekly close counts as "at" a date when it's this recent.
const WEEK_TOLERANCE: i64 = 10;

#[derive(Clone, PartialEq)]
struct ScenarioResult {
    name: &'static str,
    from: i64,
    to: i64,
    market: Option<f64>,
    portfolio: f64,
    /// Share of the portfolio estimated from beta (no prices back then).
    estimated: f64,
}

/// Each holding's own move over the scenario when it has prices for it,
/// else its beta times the benchmark's move (beta 1 when unknown).
fn run_scenarios(history: &History, weights: &[f64], betas: &[Option<f64>]) -> Vec<ScenarioResult> {
    let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
    SCENARIOS
        .iter()
        .map(|(name, from, to)| {
            let from = days_from_civil(from.0, from.1, from.2);
            let to = days_from_civil(to.0, to.1, to.2);
            let market = history.market.change_between(from, to, WEEK_TOLERANCE);
            let (mut portfolio, mut estimated) = (0.0, 0.0);
            for (i, w) in weights.iter().enumerate() {
                if *w <= 0.0 || total <= 0.0 {
                    continue;
                }
                let w = w / total;
                let own = history
                    .holdings
                    .get(i)
                    .and_then(|s| s.change_between(from, to, WEEK_TOLERANCE));
                let change = match (own, market) {
                    (Some(c), _) => c,
                    (None, Some(m)) => {
                        estimated += w;
                        (betas.get(i).copied().flatten().unwrap_or(1.0) * m).max(-1.0)
                    }
                    (None, None) => {
                        estimated += w;
                        0.0
                    }
                };
                portfolio += w * change;
            }
            ScenarioResult { name, from, to, market, portfolio, estimated }
        })
        .collect()
}

#[component]
fn ScenarioCard(
    allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>,
    betas: HashMap<TickerSymbol, f64>,
    value: f64,
) -> Element {
    let app_settings = use_context::<AppSettings>();
    let tickers = use_memo(move || allocation.read().iter().map(|(t, _)| t.clone()).collect::<Vec<_>>());
    let history = crate::cache::use_cached(
        move || format!("crises/{:?}/{}/{}", tickers(), app_settings.benchmark(), app_settings.currency()),
        move || {
            let tickers = tickers();
            let (benchmark, currency) = (app_settings.benchmark(), app_settings.currency());
            async move { fetch_history(tickers, benchmark, currency, Range::Max, Interval::W1).await }
        },
    );
    let weights: Vec<f64> = allocation.read().iter().map(|(_, w)| w.to_f64().unwrap_or(0.0)).collect();
    let beta_list: Vec<Option<f64>> = tickers().iter().map(|t| betas.get(t).copied()).collect();
    let bench = app_settings.benchmark_name();

    rsx! {
        Card { title: tr("Past crises"), subtitle: tr("What history's big sell-offs would do to today's holdings").to_string(),
            match history() {
                None => rsx! { p { class: "py-8 text-center text-sm text-ctp-subtext0", {tr("Loading decades of prices…")} } },
                Some(None) => rsx! { p { class: "py-8 text-center text-sm text-ctp-subtext0", {tr("Couldn't load the price history for this.")} } },
                Some(Some(h)) => rsx! {
                    div { class: "flex flex-col",
                        for s in run_scenarios(&h, &weights, &beta_list) {
                            ScenarioRow { key: "{s.name}", result: s.clone(), value, bench: bench.clone() }
                        }
                    }
                    p { class: "mt-3 text-xs text-ctp-overlay1",
                        {tr("Weekly closes over each S&P 500 fall, peak to trough, in today's weights. Holdings without prices back then are assumed to move with their beta.")}
                    }
                },
            }
        }
    }
}

#[component]
fn ScenarioRow(result: ScenarioResult, value: f64, bench: String) -> Element {
    let r = &result;
    let amount = Decimal::try_from(r.portfolio * value).unwrap_or_default();
    let color = if r.portfolio < 0.0 { "text-ctp-red" } else { "text-ctp-green" };
    let market = r
        .market
        .map(|m| trf("{} {}", &[&bench, &format!("{:+.1}%", m * 100.0)]))
        .unwrap_or_default();
    rsx! {
        div { class: "flex items-center justify-between gap-3 py-2.5 border-t border-ctp-surface0/60 first:border-t-0",
            div { class: "min-w-0",
                div { class: "text-sm text-ctp-text", {tr_str(r.name)} }
                div { class: "text-xs text-ctp-overlay1 truncate",
                    "{day_label(r.from)} – {day_label(r.to)} · {market}"
                    if r.estimated > 0.005 {
                        {trf(" · {}% estimated from beta", &[&format!("{:.0}", r.estimated * 100.0)])}
                    }
                }
            }
            div { class: "text-right shrink-0 tabular-nums",
                div { class: "text-sm font-semibold {color}", "{crate::format::fmt_signed(amount, 0)}" }
                div { class: "text-xs text-ctp-subtext0", "{r.portfolio * 100.0:+.1}%" }
            }
        }
    }
}

// ─── Exposure ─────────────────────────────────────────────────────────────────

/// A sector or currency holding more than this share is flagged.
const CONCENTRATED: f64 = 40.0;

/// Weight per group, largest first.
fn group_weights<'a>(
    allocation: &[(TickerSymbol, Decimal)],
    group_of: impl Fn(&TickerSymbol) -> Option<&'a str>,
) -> Vec<(String, f64)> {
    let mut groups: BTreeMap<String, f64> = BTreeMap::new();
    for (t, w) in allocation {
        let name = group_of(t).unwrap_or_else(|| tr("Unknown")).to_string();
        *groups.entry(name).or_default() += w.to_f64().unwrap_or(0.0);
    }
    let mut out: Vec<(String, f64)> = groups.into_iter().collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// Where the money is by currency, sector and country.
#[component]
fn ExposureCard(allocation: ReadSignal<Vec<(TickerSymbol, Decimal)>>) -> Element {
    let tickers = use_memo(move || allocation.read().iter().map(|(t, _)| t.clone()).collect::<Vec<_>>());
    let info = crate::cache::use_cached(
        move || format!("exposure/{:?}", tickers()),
        move || {
            let tickers = tickers();
            async move {
                let (currencies, profiles) =
                    futures::join!(get_currencies(tickers.clone()), api::get_profiles(tickers));
                (currencies.unwrap_or_default(), profiles.unwrap_or_default())
            }
        },
    );
    let display = use_context::<AppSettings>().currency();

    let Some((currencies, profiles)) = info() else {
        return rsx! {
            Card { title: tr("Exposure"),
                p { class: "py-8 text-center text-sm text-ctp-subtext0", {tr("Loading…")} }
            }
        };
    };
    let allocation = allocation.read().clone();
    let by_currency = group_weights(&allocation, |t| currencies.get(t).map(String::as_str));
    let by_sector = group_weights(&allocation, |t| {
        let p = profiles.get(t)?;
        if p.fund {
            Some(tr("Funds"))
        } else {
            p.sector.as_deref()
        }
    });
    let by_country = group_weights(&allocation, |t| {
        let p = profiles.get(t)?;
        if p.fund {
            Some(tr("Funds"))
        } else {
            p.country.as_deref()
        }
    });

    let mut warnings = Vec::new();
    if let Some((sector, w)) = by_sector.first() {
        if *w > CONCENTRATED && sector != tr("Funds") && sector != tr("Unknown") {
            warnings.push(trf("{}% in {}: one sector drives much of your risk.", &[&format!("{w:.0}"), sector]));
        }
    }
    let foreign: f64 = by_currency.iter().filter(|(c, _)| *c != display).map(|(_, w)| w).sum();
    if foreign > 50.0 {
        warnings.push(trf(
            "{}% is in currencies other than {}: exchange rates move its value as much as prices do.",
            &[&format!("{foreign:.0}"), &display],
        ));
    }

    rsx! {
        Card { title: tr("Exposure"), subtitle: tr("Where your money is, beyond single stocks").to_string(),
            div { class: "grid gap-6 md:grid-cols-3",
                ExposureList { title: tr("By currency"), groups: by_currency }
                ExposureList { title: tr("By sector"), groups: by_sector }
                ExposureList { title: tr("By country"), groups: by_country }
            }
            for w in warnings {
                p { class: "mt-3 text-xs text-ctp-peach", "{w}" }
            }
        }
    }
}

#[component]
fn ExposureList(title: String, groups: Vec<(String, f64)>) -> Element {
    rsx! {
        div {
            div { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-ctp-subtext0", "{title}" }
            div { class: "flex flex-col gap-2",
                for (i, (name, w)) in groups.into_iter().enumerate() {
                    div { key: "{name}", class: "text-xs",
                        div { class: "flex justify-between gap-2",
                            span { class: "truncate text-ctp-subtext1", {tr_str(&name).to_string()} }
                            span { class: "tabular-nums text-ctp-overlay1", "{w:.1}%" }
                        }
                        div { class: "mt-1 h-1.5 rounded-full bg-ctp-surface0 overflow-hidden",
                            div {
                                class: "h-full rounded-full {CHART_COLOR_CLASSES[i % CHART_COLOR_CLASSES.len()]}",
                                style: "width:{w.clamp(1.0, 100.0):.0}%;",
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn series(points: &[(i64, f64)]) -> PriceSeries {
        PriceSeries { closes: points.iter().copied().collect(), ..PriceSeries::default() }
    }

    #[test]
    fn scenarios_use_own_history_or_beta() {
        let covid = (days_from_civil(2020, 2, 17), days_from_civil(2020, 3, 16));
        let market = series(&[(covid.0, 100.0), (covid.1, 70.0)]);
        // AAA fell 50%; BBB has no prices then (listed later).
        let aaa = series(&[(covid.0, 10.0), (covid.1, 5.0)]);
        let history = History { holdings: vec![aaa, PriceSeries::default()], market, currency: "USD".into() };
        let results = run_scenarios(&history, &[50.0, 50.0], &[None, Some(2.0)]);
        let c = results.iter().find(|r| r.name == "COVID crash, 2020").unwrap();
        assert!((c.market.unwrap() + 0.3).abs() < 1e-9);
        // Half at −50%, half at 2 × −30%.
        assert!((c.portfolio + 0.55).abs() < 1e-9, "{}", c.portfolio);
        assert!((c.estimated - 0.5).abs() < 1e-9);
    }

    #[test]
    fn liquidity_and_holding_age() {
        assert_eq!(days_to_sell(Some(1000.0), Some(1000.0)), Some(5.0));
        assert_eq!(days_to_sell(Some(1000.0), None), None);

        let buy = |date: &str, cost: Decimal| Transaction {
            id: Uuid::nil(),
            portfolio_id: Uuid::nil(),
            ticker: TickerSymbol::new("AAA").unwrap(),
            transaction_type: types::transaction_type::TransactionType::Buy,
            shares: dec!(1),
            price: cost,
            date: date.into(),
            fee: dec!(0),
            currency: "USD".into(),
            fx_to_usd: dec!(1),
        };
        // $100 held two years and $300 held one: 1.25 years on average.
        let years = held_years(&[buy("2024-01-01", dec!(100)), buy("2025-01-01", dec!(300))], "2026-01-01");
        let aaa = years[&TickerSymbol::new("AAA").unwrap()];
        assert!((aaa - (100.0 * 731.0 + 300.0 * 365.0) / 400.0 / 365.25).abs() < 1e-9, "{aaa}");
    }

    #[test]
    fn groups_add_up_by_name() {
        let allocation = vec![
            (TickerSymbol::new("A").unwrap(), dec!(50)),
            (TickerSymbol::new("B").unwrap(), dec!(30)),
            (TickerSymbol::new("C").unwrap(), dec!(20)),
        ];
        let groups = group_weights(&allocation, |t| (t.as_str() != "C").then_some("Tech"));
        assert_eq!(groups, vec![("Tech".to_string(), 80.0), ("Unknown".to_string(), 20.0)]);
    }
}
