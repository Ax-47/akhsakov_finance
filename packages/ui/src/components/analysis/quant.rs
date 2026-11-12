//! What a performance report adds to the risk numbers: the portfolio (or
//! one stock) measured against its benchmark, month by month returns, the
//! deepest drawdowns and how risk drifted over the window. Everything here
//! is worked out from a [`RiskReport`]'s growth series, so it describes the
//! same thing the rest of the Risk tab does: today's mix, held throughout.

use super::stats::{civil_from_days, day_label, RiskReport, MONTHS};
use crate::{
    components::card::{Card, Segmented, ToggleButton},
    i18n::{tr, tr_str, trf},
};
use dioxus::prelude::*;

const PORTFOLIO_HEX: &str = "var(--catppuccin-color-mauve)";
const MARKET_HEX: &str = "var(--catppuccin-color-blue)";

// ─── Numbers ──────────────────────────────────────────────────────────────────

/// Step-to-step returns of a value series.
pub fn returns(values: &[f64]) -> Vec<f64> {
    values.windows(2).map(|w| if w[0] > 0.0 { w[1] / w[0] - 1.0 } else { 0.0 }).collect()
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() { 0.0 } else { xs.iter().sum::<f64>() / xs.len() as f64 }
}

/// Sample standard deviation.
fn deviation(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64).sqrt()
}

/// (skewness, excess kurtosis) of `xs`: below zero skew means the bad days
/// are bigger than the good ones; kurtosis above zero means more extreme
/// days than a bell curve has.
pub fn shape(xs: &[f64]) -> (f64, f64) {
    let (n, m) = (xs.len() as f64, mean(xs));
    let moment = |p: i32| xs.iter().map(|x| (x - m).powi(p)).sum::<f64>() / n.max(1.0);
    let variance = moment(2);
    if xs.len() < 3 || variance <= 0.0 {
        return (0.0, 0.0);
    }
    (moment(3) / variance.powf(1.5), moment(4) / variance.powi(2) - 3.0)
}

/// One fall from a high, and the way back.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawdown {
    pub peak_day: i64,
    pub trough_day: i64,
    /// The day the old high was reached again; `None` if still below it.
    pub recovery_day: Option<i64>,
    /// As a negative fraction, e.g. −0.25.
    pub depth: f64,
}

/// Every fall from a high in `series` (day, value), deepest first.
pub fn drawdowns(series: &[(i64, f64)]) -> Vec<Drawdown> {
    let mut out = Vec::new();
    let Some(&(mut peak_day, mut peak)) = series.first() else {
        return out;
    };
    let mut current: Option<Drawdown> = None;
    for &(day, value) in &series[1..] {
        if value >= peak {
            if let Some(mut done) = current.take() {
                done.recovery_day = Some(day);
                out.push(done);
            }
            (peak_day, peak) = (day, value);
            continue;
        }
        let depth = value / peak - 1.0;
        match &mut current {
            Some(d) if depth < d.depth => (d.trough_day, d.depth) = (day, depth),
            Some(_) => {}
            None => current = Some(Drawdown { peak_day, trough_day: day, recovery_day: None, depth }),
        }
    }
    out.extend(current);
    out.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    out
}

/// How far below its high so far each value is (0 at a high).
pub fn underwater(values: &[f64]) -> Vec<f64> {
    let mut peak = f64::MIN;
    values
        .iter()
        .map(|v| {
            peak = peak.max(*v);
            if peak > 0.0 { v / peak - 1.0 } else { 0.0 }
        })
        .collect()
}

/// Return of each calendar month in `series` (day, value), as (year,
/// month 1–12, return): from the last value of the month before to the
/// last of this one. The first month runs from the first value, and is
/// left out if that's its only one.
pub fn monthly_returns(series: &[(i64, f64)]) -> Vec<(i64, u32, f64)> {
    let mut out = Vec::new();
    let mut base: Option<f64> = None;
    let mut i = 0;
    while i < series.len() {
        let (year, month, _) = civil_from_days(series[i].0);
        let mut end = i;
        while end + 1 < series.len() && {
            let (y, m, _) = civil_from_days(series[end + 1].0);
            (y, m) == (year, month)
        } {
            end += 1;
        }
        let start = base.or_else(|| (end > i).then_some(series[i].1));
        if let Some(start) = start.filter(|s| *s > 0.0) {
            out.push((year, month, series[end].1 / start - 1.0));
        }
        base = Some(series[end].1);
        i = end + 1;
    }
    out
}

/// What the months of the year have done on average.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct MonthStat {
    /// Years with this month in the history.
    pub years: usize,
    pub average: f64,
    /// Years the month ended higher.
    pub positive: usize,
    pub best: f64,
    pub worst: f64,
}

/// January to December from [`monthly_returns`].
pub fn seasonality(months: &[(i64, u32, f64)]) -> [MonthStat; 12] {
    let mut out = [MonthStat::default(); 12];
    for (slot, stat) in out.iter_mut().enumerate() {
        let of: Vec<f64> = months.iter().filter(|(_, m, _)| *m as usize == slot + 1).map(|(_, _, r)| *r).collect();
        if of.is_empty() {
            continue;
        }
        *stat = MonthStat {
            years: of.len(),
            average: mean(&of),
            positive: of.iter().filter(|r| **r > 0.0).count(),
            best: of.iter().cloned().fold(f64::MIN, f64::max),
            worst: of.iter().cloned().fold(f64::MAX, f64::min),
        };
    }
    out
}

/// One side of the comparison: the subject or its benchmark.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub skew: f64,
    pub kurtosis: f64,
    /// Share of days that ended higher.
    pub positive_days: f64,
    pub best_month: Option<f64>,
    pub worst_month: Option<f64>,
    /// Share of months that ended higher.
    pub positive_months: Option<f64>,
    /// Annual return over the worst drawdown; `None` under a year or
    /// without a drawdown.
    pub calmar: Option<f64>,
}

impl Profile {
    fn of(series: &[(i64, f64)], annual_return: Option<f64>, max_drawdown: f64) -> Self {
        let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
        let daily = returns(&values);
        let (skew, kurtosis) = shape(&daily);
        let months: Vec<f64> = monthly_returns(series).into_iter().map(|(_, _, r)| r).collect();
        let some = !months.is_empty();
        Self {
            skew,
            kurtosis,
            positive_days: daily.iter().filter(|r| **r > 0.0).count() as f64 / daily.len().max(1) as f64,
            best_month: some.then(|| months.iter().cloned().fold(f64::MIN, f64::max)),
            worst_month: some.then(|| months.iter().cloned().fold(f64::MAX, f64::min)),
            positive_months: some.then(|| months.iter().filter(|r| **r > 0.0).count() as f64 / months.len() as f64),
            calmar: annual_return.filter(|_| max_drawdown.abs() > 1e-9).map(|r| r / max_drawdown.abs()),
        }
    }
}

/// The subject against its benchmark over a [`RiskReport`]'s window.
#[derive(Clone, Debug, PartialEq)]
pub struct Versus {
    pub subject: Profile,
    pub market: Profile,
    /// Benchmark's Sharpe ratio, for the comparison column.
    pub market_sharpe: f64,
    /// Return over the window beyond the benchmark's.
    pub active_return: f64,
    /// Annualised volatility of the daily differences from the benchmark.
    pub tracking_error: f64,
    /// Annual return beyond the benchmark per unit of tracking error.
    pub information_ratio: Option<f64>,
    /// Annual return beyond the risk-free rate per unit of beta.
    pub treynor: Option<f64>,
    /// Share of the benchmark's rise captured on the days it rose (1 = all
    /// of it), and of its fall on the days it fell (under 1 = fell less).
    pub up_capture: Option<f64>,
    pub down_capture: Option<f64>,
}

impl Versus {
    pub fn of(report: &RiskReport) -> Self {
        let subject: Vec<(i64, f64)> = report.growth.iter().map(|(d, p, _)| (*d, *p)).collect();
        let market: Vec<(i64, f64)> = report.growth.iter().map(|(d, _, m)| (*d, *m)).collect();
        let s = returns(&subject.iter().map(|(_, v)| *v).collect::<Vec<_>>());
        let m = returns(&market.iter().map(|(_, v)| *v).collect::<Vec<_>>());
        let per_year = if report.years > 0.0 { report.days as f64 / report.years } else { 252.0 };
        let full = report.full_year();

        let active: Vec<f64> = s.iter().zip(&m).map(|(a, b)| a - b).collect();
        let tracking_error = deviation(&active) * per_year.sqrt();
        let capture = |up: bool| {
            let (mut ours, mut theirs) = (0.0, 0.0);
            for (a, b) in s.iter().zip(&m).filter(|(_, b)| if up { **b > 0.0 } else { **b < 0.0 }) {
                ours += a;
                theirs += b;
            }
            (theirs.abs() > 1e-12).then(|| ours / theirs)
        };
        let market_excess = mean(&m) * per_year - report.risk_free;
        Self {
            subject: Profile::of(&subject, full.then_some(report.annual_return), report.max_drawdown),
            market: Profile::of(&market, full.then_some(report.market_return), report.market_max_drawdown),
            market_sharpe: if report.market_volatility > 0.0 { market_excess / report.market_volatility } else { 0.0 },
            active_return: report.period_return - report.market_period_return,
            tracking_error,
            information_ratio: (tracking_error > 1e-9).then(|| mean(&active) * per_year / tracking_error),
            // Sharpe × volatility is the annual return beyond the risk-free rate.
            treynor: (report.beta.abs() > 0.05).then(|| report.sharpe * report.volatility / report.beta),
            up_capture: capture(true),
            down_capture: capture(false),
        }
    }
}

/// Rolling annualised volatility of `xs` over `window` steps.
pub fn rolling_volatility(xs: &[f64], window: usize, per_year: f64) -> Vec<Option<f64>> {
    (0..xs.len())
        .map(|i| (window >= 2 && i + 1 >= window).then(|| deviation(&xs[i + 1 - window..=i]) * per_year.sqrt()))
        .collect()
}

/// Rolling beta of `xs` to `market` over `window` steps.
pub fn rolling_beta(xs: &[f64], market: &[f64], window: usize) -> Vec<Option<f64>> {
    (0..xs.len().min(market.len()))
        .map(|i| {
            if window < 2 || i + 1 < window {
                return None;
            }
            let (a, b) = (&xs[i + 1 - window..=i], &market[i + 1 - window..=i]);
            let (ma, mb) = (mean(a), mean(b));
            let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
            let var: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
            (var > 0.0).then(|| cov / var)
        })
        .collect()
}

// ─── Cards ────────────────────────────────────────────────────────────────────

fn pct(v: f64) -> String {
    format!("{:+.1}%", v * 100.0)
}

fn plain_pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

fn opt(v: Option<f64>, f: impl Fn(f64) -> String) -> String {
    v.map_or_else(|| "—".to_string(), f)
}

fn two(v: f64) -> String {
    format!("{v:.2}")
}

/// Which of two numbers is the better one, for colouring.
#[derive(Clone, Copy, PartialEq)]
enum Better {
    Higher,
    Lower,
    Neither,
}

/// A row of the comparison: label, what it means, the subject's value and
/// the benchmark's as text, the two as numbers, and which is the better.
type VersusRow = (&'static str, &'static str, String, String, Option<(f64, f64)>, Better);

/// The subject next to its benchmark, measure by measure, then the
/// measures that only exist between the two.
#[component]
pub fn BenchmarkCard(report: RiskReport, subject: String, bench: String) -> Element {
    let v = Versus::of(&report);
    let full = report.full_year();
    let mut rows: Vec<VersusRow> = vec![(
        "Return over the period",
        "Growth from the first day of the window to the last",
        pct(report.period_return),
        pct(report.market_period_return),
        Some((report.period_return, report.market_period_return)),
        Better::Higher,
    )];
    if full {
        rows.push((
            "Return a year",
            "The yearly rate that compounds to the return over the period",
            pct(report.annual_return),
            pct(report.market_return),
            Some((report.annual_return, report.market_return)),
            Better::Higher,
        ));
    }
    rows.extend([
        (
            "Volatility",
            "How much it swings in a typical year",
            plain_pct(report.volatility),
            plain_pct(report.market_volatility),
            Some((report.volatility, report.market_volatility)),
            Better::Lower,
        ),
        (
            "Sharpe ratio",
            "Return beyond the risk-free rate for each unit of swing. Above 1 is good.",
            two(report.sharpe),
            two(v.market_sharpe),
            Some((report.sharpe, v.market_sharpe)),
            Better::Higher,
        ),
        (
            "Worst drawdown",
            "The deepest fall from a high in the window",
            // Reports keep a drawdown as the size of the fall.
            pct(-report.max_drawdown.abs()),
            pct(-report.market_max_drawdown.abs()),
            Some((report.max_drawdown.abs(), report.market_max_drawdown.abs())),
            Better::Lower,
        ),
        (
            "Calmar ratio",
            "Return a year for each unit of the worst drawdown. Needs a year of history.",
            opt(v.subject.calmar, two),
            opt(v.market.calmar, two),
            v.subject.calmar.zip(v.market.calmar),
            Better::Higher,
        ),
        (
            "Best month",
            "The best calendar month in the window",
            opt(v.subject.best_month, pct),
            opt(v.market.best_month, pct),
            v.subject.best_month.zip(v.market.best_month),
            Better::Higher,
        ),
        (
            "Worst month",
            "The worst calendar month in the window",
            opt(v.subject.worst_month, pct),
            opt(v.market.worst_month, pct),
            v.subject.worst_month.zip(v.market.worst_month),
            Better::Higher,
        ),
        (
            "Months up",
            "Share of calendar months that ended higher",
            opt(v.subject.positive_months, plain_pct),
            opt(v.market.positive_months, plain_pct),
            v.subject.positive_months.zip(v.market.positive_months),
            Better::Higher,
        ),
        (
            "Days up",
            "Share of trading days that ended higher",
            plain_pct(v.subject.positive_days),
            plain_pct(v.market.positive_days),
            Some((v.subject.positive_days, v.market.positive_days)),
            Better::Higher,
        ),
        (
            "Skew",
            "Below zero: the bad days are bigger than the good ones",
            two(v.subject.skew),
            two(v.market.skew),
            Some((v.subject.skew, v.market.skew)),
            Better::Higher,
        ),
        (
            "Fat tails (excess kurtosis)",
            "Above zero: more extreme days than a bell curve has",
            two(v.subject.kurtosis),
            two(v.market.kurtosis),
            Some((v.subject.kurtosis, v.market.kurtosis)),
            Better::Neither,
        ),
    ]);
    let between: Vec<(&'static str, &'static str, String)> = vec![
        ("Return beyond the benchmark", "Return over the period, minus the benchmark's", pct(v.active_return)),
        ("Beta", "How much it moves when the benchmark moves 1%", two(report.beta)),
        ("Alpha a year", "Return a year beyond what its beta explains", pct(report.alpha)),
        ("Correlation", "How closely it moves with the benchmark: 1 is in step", two(report.market_correlation)),
        ("Tracking error", "How far its returns stray from the benchmark's in a typical year", plain_pct(v.tracking_error)),
        ("Information ratio", "Return a year beyond the benchmark for each unit of tracking error. Above 0.5 is good.", opt(v.information_ratio, two)),
        ("Treynor ratio", "Return a year beyond the risk-free rate for each unit of beta", opt(v.treynor, plain_pct)),
        ("Up capture", "Share of the benchmark's gain it made on the days the benchmark rose. Above 100% is good.", opt(v.up_capture, |c| format!("{:.0}%", c * 100.0))),
        ("Down capture", "Share of the benchmark's loss it took on the days the benchmark fell. Below 100% is good.", opt(v.down_capture, |c| format!("{:.0}%", c * 100.0))),
    ];
    rsx! {
        Card {
            title: trf("{} against the {}", &[&subject, &bench]),
            subtitle: trf("{} to {} · {} trading days", &[&day_label(report.first_day), &day_label(report.last_day), &report.days]),
            div { class: "grid gap-6 lg:grid-cols-[1.25fr_1fr]",
                table { class: "w-full text-sm tabular-nums",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            th { class: "pb-2 text-left font-normal", {tr("Metric")} }
                            th { class: "pb-2 text-right font-normal", "{subject}" }
                            th { class: "pb-2 text-right font-normal", "{bench}" }
                        }
                    }
                    tbody {
                        for (label, hint, ours, theirs, numbers, better) in rows {
                            {
                                let ahead = numbers.is_some_and(|(a, b)| match better {
                                    Better::Higher => a > b,
                                    Better::Lower => a < b,
                                    Better::Neither => false,
                                });
                                rsx! {
                                    tr { key: "{label}", class: "border-t border-ctp-surface0/60", title: tr_str(hint),
                                        td { class: "py-1.5 pr-2 text-ctp-subtext1", {tr_str(label)} }
                                        td { class: if ahead { "py-1.5 text-right font-medium text-ctp-green" } else { "py-1.5 text-right font-medium text-ctp-text" }, "{ours}" }
                                        td { class: "py-1.5 pl-3 text-right text-ctp-subtext0", "{theirs}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div {
                    div { class: "pb-2 text-xs text-ctp-subtext0", {tr("Between the two")} }
                    div { class: "grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-2",
                        for (label, hint, value) in between {
                            div { key: "{label}", class: "rounded-xl border border-ctp-surface0/70 bg-ctp-base/50 px-3 py-2", title: tr_str(hint),
                                div { class: "text-xs text-ctp-subtext0", {tr_str(label)} }
                                div { class: "mt-0.5 text-base font-semibold tabular-nums text-ctp-text", "{value}" }
                            }
                        }
                    }
                    p { class: "mt-3 text-xs leading-relaxed text-ctp-overlay1", {tr("Hover over a row for what it means. Green marks the better of the two.")} }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum MonthView {
    Subject,
    Market,
    Difference,
}

/// A year-by-month grid of returns, each cell tinted by its size.
#[component]
pub fn MonthlyReturnsCard(report: RiskReport, subject: String, bench: String) -> Element {
    let mut view = use_signal(|| MonthView::Subject);
    let ours = monthly_returns(&report.growth.iter().map(|(d, p, _)| (*d, *p)).collect::<Vec<_>>());
    let theirs = monthly_returns(&report.growth.iter().map(|(d, _, m)| (*d, *m)).collect::<Vec<_>>());
    if ours.len() < 2 {
        return rsx! {};
    }
    let cells: Vec<(i64, u32, f64)> = match view() {
        MonthView::Subject => ours.clone(),
        MonthView::Market => theirs.clone(),
        MonthView::Difference => ours
            .iter()
            .filter_map(|(y, m, r)| {
                let other = theirs.iter().find(|(y2, m2, _)| (y2, m2) == (y, m))?.2;
                Some((*y, *m, r - other))
            })
            .collect(),
    };
    let mut years: Vec<i64> = cells.iter().map(|(y, _, _)| *y).collect();
    years.dedup();
    years.reverse();
    // The colour is full at this size of move.
    let scale = cells.iter().map(|(_, _, r)| r.abs()).fold(0.02, f64::max).min(0.15);
    rsx! {
        Card {
            title: tr("Month by month"),
            subtitle: tr("Return of each calendar month; a year's row compounds to its total. The first and last months can be part months.").to_string(),
            actions: rsx! {
                Segmented {
                    ToggleButton { label: subject.clone(), active: view() == MonthView::Subject, onclick: move |_| view.set(MonthView::Subject) }
                    ToggleButton { label: bench.clone(), active: view() == MonthView::Market, onclick: move |_| view.set(MonthView::Market) }
                    ToggleButton { label: tr("Difference"), active: view() == MonthView::Difference, onclick: move |_| view.set(MonthView::Difference) }
                }
            },
            div { class: "overflow-x-auto",
                table { class: "w-full min-w-[40rem] border-separate border-spacing-1 text-xs tabular-nums",
                    thead {
                        tr { class: "text-ctp-subtext0",
                            th { class: "text-left font-normal", {tr("Year")} }
                            for name in MONTHS {
                                th { key: "{name}", class: "font-normal", {tr_str(name)} }
                            }
                            th { class: "font-normal", {tr("Year")} }
                        }
                    }
                    tbody {
                        for year in years {
                            {
                                let of_year: Vec<(u32, f64)> = cells.iter().filter(|(y, _, _)| *y == year).map(|(_, m, r)| (*m, *r)).collect();
                                let total = of_year.iter().map(|(_, r)| 1.0 + r).product::<f64>() - 1.0;
                                rsx! {
                                    tr { key: "{year}",
                                        td { class: "pr-2 text-left text-ctp-subtext1", "{year}" }
                                        for month in 1..=12u32 {
                                            match of_year.iter().find(|(m, _)| *m == month) {
                                                Some((_, r)) => rsx! {
                                                    td { key: "{month}", class: "rounded-md px-1 py-1.5 text-center text-ctp-text", style: "{tint(*r, scale)}", "{r * 100.0:+.1}" }
                                                },
                                                None => rsx! { td { key: "{month}", class: "rounded-md px-1 py-1.5 text-center text-ctp-overlay0", "·" } },
                                            }
                                        }
                                        td { class: if total >= 0.0 { "rounded-md px-1.5 py-1.5 text-center font-semibold text-ctp-green" } else { "rounded-md px-1.5 py-1.5 text-center font-semibold text-ctp-red" },
                                            "{total * 100.0:+.1}%"
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
}

/// Green or red background, stronger the bigger the move (full at `scale`).
fn tint(r: f64, scale: f64) -> String {
    let strength = (r.abs() / scale).clamp(0.0, 1.0) * 55.0;
    let colour = if r >= 0.0 { "green" } else { "red" };
    format!("background-color: color-mix(in srgb, var(--catppuccin-color-{colour}) {strength:.0}%, transparent);")
}

/// The deepest falls from a high: how far, from when to when, and how long
/// the way back took.
#[component]
pub fn DrawdownsCard(report: RiskReport, subject: String) -> Element {
    let series: Vec<(i64, f64)> = report.growth.iter().map(|(d, p, _)| (*d, *p)).collect();
    let all = drawdowns(&series);
    let shown: Vec<Drawdown> = all.iter().filter(|d| d.depth <= -0.01).take(5).cloned().collect();
    let depth = underwater(&series.iter().map(|(_, v)| *v).collect::<Vec<_>>());
    let floor = depth.iter().cloned().fold(0.0_f64, f64::min).min(-0.01);
    let now = depth.last().copied().unwrap_or(0.0);
    let (w, h) = (600.0, 90.0);
    let area = format!(
        "0,0 {} {w},0",
        depth
            .iter()
            .enumerate()
            .map(|(i, d)| format!("{:.1},{:.1}", i as f64 / (depth.len().max(2) - 1) as f64 * w, d / floor * h))
            .collect::<Vec<_>>()
            .join(" ")
    );
    rsx! {
        Card {
            title: tr("Drawdowns"),
            subtitle: trf("How far {} has been below its high, and for how long", &[&subject]),
            div { class: "mb-1 flex justify-between text-xs text-ctp-subtext0",
                span { {trf("Now {} from the high", &[&pct(now)])} }
                span { class: "text-ctp-red", {trf("deepest {}", &[&pct(floor)])} }
            }
            svg { class: "h-24 w-full", view_box: "0 0 {w} {h}", preserve_aspect_ratio: "none", role: "img", "aria-label": tr("Drawdowns"),
                polygon { points: "{area}", fill: "var(--catppuccin-color-red)", fill_opacity: "0.25" }
            }
            div { class: "mt-1 flex justify-between text-xs text-ctp-overlay1",
                span { "{day_label(report.first_day)}" }
                span { "{day_label(report.last_day)}" }
            }
            if shown.is_empty() {
                p { class: "mt-4 text-sm text-ctp-subtext0", {tr("No fall of more than 1% in this window.")} }
            } else {
                div { class: "mt-4 overflow-x-auto",
                    table { class: "w-full min-w-[30rem] text-sm tabular-nums",
                        thead {
                            tr { class: "text-xs text-ctp-subtext0",
                                th { class: "pb-2 text-left font-normal", {tr("Fall")} }
                                th { class: "pb-2 text-left font-normal", {tr("From the high on")} }
                                th { class: "pb-2 text-left font-normal", {tr("Lowest on")} }
                                th { class: "pb-2 text-left font-normal", {tr("Back at the high")} }
                                th { class: "pb-2 text-right font-normal", {tr("Days down")} }
                                th { class: "pb-2 text-right font-normal", {tr("Days back up")} }
                            }
                        }
                        tbody {
                            for d in shown {
                                tr { key: "{d.peak_day}", class: "border-t border-ctp-surface0/60",
                                    td { class: "py-1.5 font-medium text-ctp-red", "{pct(d.depth)}" }
                                    td { class: "py-1.5 text-ctp-subtext1", "{day_label(d.peak_day)}" }
                                    td { class: "py-1.5 text-ctp-subtext1", "{day_label(d.trough_day)}" }
                                    td { class: "py-1.5 text-ctp-subtext1",
                                        match d.recovery_day {
                                            Some(day) => rsx! { "{day_label(day)}" },
                                            None => rsx! { span { class: "text-ctp-peach", {tr("Not yet")} } },
                                        }
                                    }
                                    td { class: "py-1.5 text-right text-ctp-subtext1", "{d.trough_day - d.peak_day}" }
                                    td { class: "py-1.5 text-right text-ctp-subtext1",
                                        {d.recovery_day.map_or_else(|| "—".to_string(), |day| (day - d.trough_day).to_string())}
                                    }
                                }
                            }
                        }
                    }
                }
                p { class: "mt-2 text-xs text-ctp-overlay1", {tr("Days are calendar days.")} }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum RollingView {
    Volatility,
    Beta,
}

/// How the swings and the beta drifted through the window: each point
/// looks back over the last three months (one month in a short window).
#[component]
pub fn RollingCard(report: RiskReport, subject: String, bench: String) -> Element {
    let mut view = use_signal(|| RollingView::Volatility);
    let s = returns(&report.growth.iter().map(|(_, p, _)| *p).collect::<Vec<_>>());
    let m = returns(&report.growth.iter().map(|(_, _, m)| *m).collect::<Vec<_>>());
    let window = if s.len() >= 150 { 63 } else { 21 };
    if s.len() < window * 2 {
        return rsx! {};
    }
    let per_year = if report.years > 0.0 { report.days as f64 / report.years } else { 252.0 };
    let lines: Vec<(String, &'static str, Vec<Option<f64>>)> = match view() {
        RollingView::Volatility => vec![
            (subject.clone(), PORTFOLIO_HEX, rolling_volatility(&s, window, per_year).into_iter().map(|v| v.map(|v| v * 100.0)).collect()),
            (bench.clone(), MARKET_HEX, rolling_volatility(&m, window, per_year).into_iter().map(|v| v.map(|v| v * 100.0)).collect()),
        ],
        RollingView::Beta => vec![(subject.clone(), PORTFOLIO_HEX, rolling_beta(&s, &m, window))],
    };
    let known = || lines.iter().flat_map(|(_, _, v)| v.iter().flatten().copied());
    let (lo, hi) = known().fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let pad = ((hi - lo) * 0.1).max(0.01);
    let (lo, hi) = (lo - pad, hi + pad);
    let (w, h) = (600.0, 180.0);
    // The chart starts at the first point with a full window behind it.
    let first = window - 1;
    let span = (s.len() - first).max(2) - 1;
    let point = |i: usize, v: f64| format!("{:.1},{:.1}", (i - first) as f64 / span as f64 * w, h - (v - lo) / (hi - lo) * h);
    let first_day = report.growth.get(first + 1).map_or(report.first_day, |g| g.0);
    let unit = if view() == RollingView::Volatility { "%" } else { "" };
    // Beta of 1 moves with the benchmark: worth a guide line.
    let guide = (view() == RollingView::Beta && lo < 1.0 && hi > 1.0).then(|| h - (1.0 - lo) / (hi - lo) * h);
    let looks_back = if window == 63 {
        tr("Each point looks back over the last three months")
    } else {
        tr("Each point looks back over the last month")
    };
    rsx! {
        Card {
            title: tr("How risk drifted"),
            subtitle: looks_back.to_string(),
            actions: rsx! {
                Segmented {
                    ToggleButton { label: tr("Volatility"), active: view() == RollingView::Volatility, onclick: move |_| view.set(RollingView::Volatility) }
                    ToggleButton { label: tr("Beta"), active: view() == RollingView::Beta, onclick: move |_| view.set(RollingView::Beta) }
                }
            },
            div { class: "mb-2 flex flex-wrap gap-x-5 gap-y-1 text-xs text-ctp-subtext1",
                for (name, colour, values) in lines.iter() {
                    span { key: "{name}", class: "inline-flex items-center gap-1.5",
                        span { class: "h-2 w-2 rounded-full", style: "background:{colour}" }
                        {format!("{name} {}", values.iter().rev().flatten().next().map_or_else(|| "—".to_string(), |v| format!("{v:.2}{unit}")))}
                    }
                }
            }
            div { class: "flex gap-2",
                div { class: "flex flex-col justify-between py-0.5 text-right text-[10px] tabular-nums text-ctp-overlay1",
                    span { "{hi:.2}{unit}" }
                    span { "{lo:.2}{unit}" }
                }
                svg { class: "h-52 w-full", view_box: "0 0 {w} {h}", preserve_aspect_ratio: "none", role: "img", "aria-label": tr("How risk drifted"),
                    if let Some(y) = guide {
                        line { x1: "0", x2: "{w}", y1: "{y:.1}", y2: "{y:.1}", stroke: "var(--catppuccin-color-surface1)", stroke_dasharray: "4 4", vector_effect: "non-scaling-stroke" }
                    }
                    for (name, colour, values) in lines.iter() {
                        polyline {
                            key: "{name}",
                            points: values.iter().enumerate().filter_map(|(i, v)| Some(point(i, (*v)?))).collect::<Vec<_>>().join(" "),
                            fill: "none",
                            stroke: "{colour}",
                            stroke_width: "1.75",
                            stroke_linejoin: "round",
                            vector_effect: "non-scaling-stroke",
                        }
                    }
                }
            }
            div { class: "mt-1 flex justify-between pl-10 text-xs text-ctp-overlay1",
                span { "{day_label(first_day)}" }
                span { "{day_label(report.last_day)}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::stats::days_from_civil;
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn drawdowns_run_from_a_high_to_the_day_it_is_reached_again() {
        // Up to 120, down to 90 (−25%), back to 120; a dip to 110; then
        // down to 100 with no recovery.
        let values = [100.0, 120.0, 100.0, 90.0, 110.0, 120.0, 110.0, 125.0, 100.0, 105.0];
        let series: Vec<(i64, f64)> = values.iter().enumerate().map(|(i, v)| (i as i64 * 10, *v)).collect();
        let found = drawdowns(&series);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0], Drawdown { peak_day: 10, trough_day: 30, recovery_day: Some(50), depth: -0.25 });
        assert!(close(found[1].depth, -0.2) && (found[1].peak_day, found[1].trough_day, found[1].recovery_day) == (70, 80, None));
        assert!(close(found[2].depth, 110.0 / 120.0 - 1.0) && found[2].recovery_day == Some(70));
        assert!(drawdowns(&[]).is_empty() && drawdowns(&[(0, 1.0), (1, 2.0)]).is_empty());

        let under = underwater(&values);
        assert!(close(under[3], -0.25) && under[5] == 0.0 && close(under[9], 105.0 / 125.0 - 1.0));
    }

    #[test]
    fn months_run_from_one_month_end_to_the_next() {
        let day = |m: u32, d: u32| days_from_civil(2026, m, d);
        // Starts mid-January at 100.
        let series = [(day(1, 15), 100.0), (day(1, 30), 110.0), (day(2, 10), 99.0), (day(2, 27), 121.0), (day(3, 2), 121.0)];
        let months = monthly_returns(&series);
        assert_eq!(months.len(), 3);
        assert!(months[0].0 == 2026 && months[0].1 == 1 && close(months[0].2, 0.10), "from the first value: {months:?}");
        assert!(close(months[1].2, 0.10), "110 → 121");
        assert!(months[2].1 == 3 && close(months[2].2, 0.0));
        // Monthly candles: the first has nothing before it.
        let candles = [(day(1, 1), 100.0), (day(2, 1), 105.0), (day(3, 1), 84.0)];
        let months = monthly_returns(&candles);
        assert_eq!(months.iter().map(|(_, m, _)| *m).collect::<Vec<_>>(), [2, 3]);
        assert!(close(months[1].2, -0.2));
    }

    #[test]
    fn seasonality_averages_each_month_over_the_years() {
        let months = [(2024, 1, 0.04), (2025, 1, -0.02), (2026, 1, 0.01), (2025, 6, -0.05)];
        let s = seasonality(&months);
        assert_eq!((s[0].years, s[0].positive), (3, 2));
        assert!(close(s[0].average, 0.01) && close(s[0].best, 0.04) && close(s[0].worst, -0.02));
        assert_eq!((s[5].years, s[5].positive), (1, 0));
        assert_eq!(s[11], MonthStat::default());
    }

    #[test]
    fn shape_reads_lopsided_and_fat_tailed_returns() {
        let (skew, kurtosis) = shape(&[0.01, 0.01, 0.01, 0.01, -0.10]);
        assert!(skew < -1.0, "one big loss: {skew}");
        assert!(kurtosis > 0.0);
        let (even, _) = shape(&[-0.02, -0.01, 0.0, 0.01, 0.02]);
        assert!(even.abs() < 1e-9);
        assert_eq!(shape(&[0.5, 0.5, 0.5]), (0.0, 0.0), "no spread, no shape");
    }

    #[test]
    fn rolling_measures_need_a_full_window() {
        let market = [0.01, -0.02, 0.015, 0.005, -0.01, 0.02];
        let double: Vec<f64> = market.iter().map(|r| r * 2.0).collect();
        let beta = rolling_beta(&double, &market, 4);
        assert_eq!(&beta[..3], [None, None, None]);
        assert!(beta[3..].iter().all(|b| close(b.unwrap(), 2.0)), "twice the market, always: {beta:?}");
        let vol = rolling_volatility(&market, 4, 252.0);
        assert!(vol[2].is_none() && vol[3].unwrap() > 0.0);
        assert!(rolling_volatility(&market, 1, 252.0).iter().all(Option::is_none));
    }

    /// A report whose subject moves `times` the market's daily move, with
    /// the fields [`Versus`] reads.
    fn report(times: f64) -> RiskReport {
        let moves = [0.01, -0.02, 0.015, 0.005, -0.01, 0.02, -0.005, 0.01];
        let first = days_from_civil(2025, 1, 1);
        let (mut p, mut m) = (1.0, 1.0);
        let mut growth = vec![(first, p, m)];
        for (i, r) in moves.iter().cycle().take(260).enumerate() {
            p *= 1.0 + r * times;
            m *= 1.0 + r;
            growth.push((first + (i as i64 + 1) * 7 / 5, p, m));
        }
        let market: Vec<f64> = moves.iter().cycle().take(260).cloned().collect();
        let sd = deviation(&market) * 260.0_f64.sqrt();
        RiskReport {
            days: 260,
            first_day: first,
            last_day: first + 364,
            years: 1.0,
            period_return: p - 1.0,
            annual_return: p - 1.0,
            volatility: sd * times,
            volatility_now: sd * times,
            sharpe: (mean(&market) * 260.0 * times - 0.02) / (sd * times),
            sortino: 0.0,
            max_drawdown: 0.03 * times,
            var_95: 0.0,
            cvar_95: 0.0,
            var_95_now: 0.0,
            var_test: Default::default(),
            beta: times,
            alpha: 0.0,
            market_correlation: 1.0,
            market_period_return: m - 1.0,
            market_return: m - 1.0,
            market_volatility: sd,
            market_max_drawdown: 0.03,
            diversification_ratio: 1.0,
            independent_bets: 1.0,
            risk_free: 0.02,
            cash_weight: 0.0,
            coverage: 1.0,
            worst_day: (0, 0.0),
            worst_week: (0, 0.0),
            growth,
            holdings: vec![],
            excluded: vec![],
        }
    }

    #[test]
    fn twice_the_market_captures_twice_of_both_ways() {
        let v = Versus::of(&report(2.0));
        assert!(close(v.up_capture.unwrap(), 2.0) && close(v.down_capture.unwrap(), 2.0), "{v:?}");
        assert!(v.tracking_error > 0.0, "it strays from the benchmark");
        assert!(v.active_return > 0.0 && v.information_ratio.unwrap() > 0.0);
        assert!(close(v.subject.positive_days, v.market.positive_days), "same days up");
        assert!(v.subject.best_month.unwrap() > v.market.best_month.unwrap());
        assert!(v.subject.calmar.is_some() && v.market.calmar.is_some(), "a full year has a Calmar ratio");
    }

    #[test]
    fn the_benchmark_against_itself_has_nothing_between() {
        let v = Versus::of(&report(1.0));
        assert!(v.tracking_error < 1e-12 && v.information_ratio.is_none(), "no tracking error, so no ratio");
        assert!(close(v.up_capture.unwrap(), 1.0) && close(v.down_capture.unwrap(), 1.0));
        assert!(close(v.active_return, 0.0));
        assert!(close(v.market_sharpe, report(1.0).sharpe), "the same Sharpe on both sides");
    }

    #[test]
    fn cards_render_with_a_report() {
        #[component]
        fn Cards() -> Element {
            let r = report(1.5);
            rsx! {
                BenchmarkCard { report: r.clone(), subject: "Portfolio", bench: "S&P 500" }
                MonthlyReturnsCard { report: r.clone(), subject: "Portfolio", bench: "S&P 500" }
                DrawdownsCard { report: r.clone(), subject: "Portfolio" }
                RollingCard { report: r, subject: "Portfolio", bench: "S&P 500" }
            }
        }
        let mut dom = VirtualDom::new(Cards);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(html.contains("-4.5%") && html.contains("-3.0%"), "drawdowns read as falls: {html}");
        for text in ["Portfolio against the S&#38;P 500", "Up capture", "150%", "Month by month", "2025", "Drawdowns", "How risk drifted"] {
            assert!(html.contains(text), "missing {text}: {html}");
        }
    }
}
