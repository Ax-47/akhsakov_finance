//! The Statistics tab's fair-value checks: today's P/E against the stock's
//! own history, the Graham number, and a discounted cash flow whose
//! assumptions you can change. The maths is in `dtos::valuation`.

use crate::{
    components::card::{Card, MetricTile},
    format::{fmt_money, or_dash},
    i18n::{tr, trf},
    page::GhostButton,
};
use dioxus::prelude::*;
use dtos::{
    fundamentals::{current_valuation, ttm, StockFundamentals},
    valuation::{discounted_cash_flow, graham_number, DcfAssumptions, DcfError, PeBand},
};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// `$12,345.67`: fundamentals are in USD whatever the display currency.
fn dollars(value: f64) -> String {
    fmt_money(Decimal::try_from(value).unwrap_or_default(), "$", 2)
}

/// "12.0% below" / "8.5% above" for a change against a reference.
fn above_or_below(change: f64) -> String {
    let percent = format!("{:.1}", change.abs() * 100.0);
    if change < 0.0 {
        trf("{}% below", &[&percent])
    } else {
        trf("{}% above", &[&percent])
    }
}

/// P/E against its usual range, and the Graham number against the price.
#[component]
pub fn PriceChecks(fundamentals: StockFundamentals, price: Option<f64>) -> Element {
    let f = &fundamentals;
    let now = current_valuation(&f.stats, &f.quarterly);
    // One P/E per date: a fiscal year end can also be a quarter end.
    let past: BTreeMap<&str, f64> = f
        .valuation
        .iter()
        .chain(&f.yearly_valuation)
        .filter_map(|v| Some((v.date.as_str(), v.pe_ttm?)))
        .collect();
    let band = PeBand::new(now.pe_ttm, past.into_values());

    let latest = f.quarterly.first();
    let shares = f.stats.shares_outstanding.or_else(|| latest?.shares);
    let eps = f.stats.eps_ttm.or_else(|| Some(ttm(&f.quarterly, |p| p.net_income)? / shares?));
    let book_per_share = latest.and_then(|q| Some(q.total_equity? / shares.filter(|s| *s > 0.0)?));
    let graham = eps.zip(book_per_share).and_then(|(e, b)| graham_number(e, b));
    let price = price.or(f.stats.price);

    rsx! {
        Card { title: tr("Price checks"), subtitle: tr("Rough yardsticks, not verdicts").to_string(),
            div { class: "grid gap-6 md:grid-cols-2",
                div {
                    div { class: "text-sm font-medium text-ctp-text", {tr("P/E against its own history")} }
                    if let Some(b) = band {
                        PeRange { band: b }
                        p { class: "mt-3 text-sm text-ctp-subtext1",
                            {trf("Today's P/E of {} is {} its usual {}.", &[&format!("{:.1}", b.current), &above_or_below(b.against_median()), &format!("{:.1}", b.median)])}
                        }
                        p { class: "mt-1 text-xs text-ctp-overlay1",
                            {trf("Usual = the median of {} trailing P/Es at past quarter and year ends.", &[&b.samples])}
                        }
                    } else {
                        p { class: "mt-2 text-sm text-ctp-subtext0",
                            {tr("Not enough profitable periods to show a usual range.")}
                        }
                    }
                }
                div {
                    div { class: "text-sm font-medium text-ctp-text", {tr("Graham number")} }
                    div { class: "mt-3 grid grid-cols-2 gap-3",
                        MetricTile { label: tr("Graham number"), value: or_dash(graham, dollars) }
                        MetricTile {
                            label: tr("Price against it"),
                            value: or_dash(price.zip(graham).map(|(p, g)| p / g - 1.0), above_or_below),
                        }
                    }
                    p { class: "mt-3 text-xs text-ctp-overlay1",
                        {tr("√(22.5 × EPS × book value per share): the most Benjamin Graham would pay for a steady, profitable company. Not meant for fast growers or loss makers.")}
                    }
                }
            }
        }
    }
}

/// The P/E's past range as a track: low to high, a tick at the median and
/// a dot for today (which can sit beyond either end).
#[component]
fn PeRange(band: PeBand) -> Element {
    let lo = band.low.min(band.current);
    let hi = band.high.max(band.current);
    let at = |v: f64| (v - lo) / (hi - lo).max(f64::EPSILON) * 100.0;
    rsx! {
        div { class: "relative mt-8 h-1.5 rounded-full bg-ctp-surface1",
            span { class: "absolute top-0 h-full rounded-full bg-ctp-overlay0/40", style: "left:{at(band.low):.1}%;width:{at(band.high) - at(band.low):.1}%;" }
            span { class: "absolute -top-1.5 h-4.5 w-0.5 -translate-x-1/2 bg-ctp-subtext0", style: "left:{at(band.median):.1}%;" }
            span { class: "absolute -top-6 -translate-x-1/2 text-xs text-ctp-subtext0", style: "left:{at(band.median):.1}%;",
                {trf("Usual {}", &[&format!("{:.1}", band.median)])}
            }
            span { class: "absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ctp-base bg-ctp-text", style: "left:{at(band.current):.1}%;" }
        }
        div { class: "mt-2 flex justify-between text-xs tabular-nums text-ctp-subtext0",
            span { {trf("Low {}", &[&format!("{:.1}", band.low)])} }
            span { {trf("High {}", &[&format!("{:.1}", band.high)])} }
        }
    }
}

/// Fair value from projected free cash flow, with the three assumptions
/// open to change.
#[component]
pub fn DiscountedCashFlow(fundamentals: StockFundamentals, price: Option<f64>) -> Element {
    let f = &fundamentals;
    let suggested = DcfAssumptions::suggested(&f.stats);
    let mut assumptions = use_signal(|| suggested);
    let latest = f.quarterly.first();
    let result = discounted_cash_flow(
        ttm(&f.quarterly, |p| p.free_cash_flow),
        latest.and_then(|q| q.net_debt()).unwrap_or(0.0),
        f.stats.shares_outstanding.or_else(|| latest?.shares),
        assumptions(),
    );
    let price = price.or(f.stats.price);
    let a = assumptions();

    rsx! {
        Card {
            title: tr("Discounted cash flow"),
            subtitle: tr("Ten years of free cash flow and what comes after, in today's money").to_string(),
            actions: rsx! {
                if a != suggested {
                    GhostButton { label: tr("Use suggested"), onclick: move |_| assumptions.set(suggested) }
                }
            },
            div { class: "grid gap-2 text-sm sm:grid-cols-3",
                PercentInput { label: tr("Growth, years 1–5"), value: a.growth, onchange: move |v| assumptions.with_mut(|a| a.growth = v) }
                PercentInput { label: tr("Growth after year 10"), value: a.terminal_growth, onchange: move |v| assumptions.with_mut(|a| a.terminal_growth = v) }
                PercentInput { label: tr("Required return"), value: a.discount_rate, onchange: move |v| assumptions.with_mut(|a| a.discount_rate = v) }
            }
            p { class: "mt-2 text-xs text-ctp-overlay1",
                {tr("Growth eases in equal steps from year 6 to year 10. The suggested growth is analysts' 5-year EPS estimate, capped at 20%.")}
            }
            match result {
                Ok(value) => rsx! {
                    div { class: "mt-4 grid grid-cols-2 gap-3 lg:grid-cols-3",
                        MetricTile { label: tr("Fair value per share"), value: dollars(value.per_share) }
                        MetricTile {
                            label: tr("Price against it"),
                            value: or_dash(price.filter(|_| value.per_share > 0.0).map(|p| p / value.per_share - 1.0), above_or_below),
                        }
                        MetricTile {
                            label: tr("From after year 10"),
                            value: format!("{:.0}%", value.terminal_share * 100.0),
                            hint: tr("The more of the value comes from here, the more it rests on long-run guesses").to_string(),
                        }
                    }
                },
                Err(why) => rsx! {
                    p { class: "mt-4 rounded-xl bg-ctp-base/40 px-3 py-2 text-sm text-ctp-subtext0",
                        {match why {
                            DcfError::NoCashFlow => tr("Free cash flow over the last year is zero or negative, so there's nothing to project."),
                            DcfError::NoShareCount => tr("Yahoo reports no share count for this stock."),
                            DcfError::DiscountNotAboveTerminalGrowth => tr("The required return has to be above the growth after year 10."),
                        }}
                    }
                },
            }
            p { class: "mt-3 text-xs text-ctp-overlay1",
                {tr("A model, not a forecast: a point or two on any of these moves the result a lot.")}
            }
        }
    }
}

/// A labelled box for a yearly rate, shown and typed in percent; applied
/// when it's left or Enter is pressed.
#[component]
fn PercentInput(label: String, value: f64, onchange: EventHandler<f64>) -> Element {
    rsx! {
        label { class: "flex items-center justify-between gap-2 rounded-xl border border-ctp-surface0 px-3 py-2",
            span { class: "text-ctp-subtext0", "{label}" }
            span { class: "flex items-center gap-1",
                input {
                    class: "w-16 bg-transparent text-right tabular-nums text-ctp-text outline-none",
                    r#type: "number",
                    step: "0.5",
                    min: "-20",
                    max: "50",
                    value: "{value * 100.0:.1}",
                    onchange: move |e| {
                        if let Ok(v) = e.value().parse::<f64>() {
                            if v.is_finite() && (-20.0..=50.0).contains(&v) {
                                onchange.call(v / 100.0);
                            }
                        }
                    },
                }
                span { class: "text-ctp-subtext0", "%" }
            }
        }
    }
}
