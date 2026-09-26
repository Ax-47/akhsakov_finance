//! Small calculators inside lessons: change a number and watch the idea
//! move. Amounts are in whatever currency you think in; they're shown with
//! the display currency's symbol but not converted.

use super::math;
use crate::{
    app::AppSettings,
    components::card::{Card, MetricTile, Stepper},
    format::{display_currency, fmt_shares, or_dash},
    i18n::{tr, trf},
};
use dioxus::prelude::*;
use rust_decimal::Decimal;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Valuation,
    Volatility,
    Recovery,
    Var,
    Beta,
    Sharpe,
    Diversify,
    Position,
}

#[component]
pub fn ToolCard(tool: Tool) -> Element {
    let (title, subtitle) = match tool {
        Tool::Valuation => (tr("P/E and PEG"), tr("Price against profit and growth")),
        Tool::Volatility => (tr("One-year range"), tr("What a volatility means for a year")),
        Tool::Recovery => (tr("Climbing back"), tr("The gain a fall needs to recover")),
        Tool::Var => (tr("Value at risk"), tr("A bell-curve estimate for one day")),
        Tool::Beta => (tr("Beta stress test"), tr("Your move when the market moves")),
        Tool::Sharpe => (tr("Sharpe ratio"), tr("Which pays more per unit of risk?")),
        Tool::Diversify => (tr("Mixing two assets"), tr("How correlation lowers risk")),
        Tool::Position => (tr("Position size"), tr("Size a position from the loss you accept")),
    };
    rsx! {
        Card { title, subtitle: subtitle.to_string(),
            match tool {
                Tool::Valuation => rsx! { Valuation {} },
                Tool::Volatility => rsx! { Volatility {} },
                Tool::Recovery => rsx! { Recovery {} },
                Tool::Var => rsx! { Var {} },
                Tool::Beta => rsx! { Beta {} },
                Tool::Sharpe => rsx! { Sharpe {} },
                Tool::Diversify => rsx! { Diversify {} },
                Tool::Position => rsx! { Position {} },
            }
        }
    }
}

// ─── Fields ───────────────────────────────────────────────────────────────────

/// A number field's text, and the value it stands for: its default while
/// the field is empty or mistyped.
#[derive(Clone, Copy, PartialEq)]
struct Num {
    text: Signal<String>,
    default: f64,
}

impl Num {
    fn value(self) -> f64 {
        self.text
            .read()
            .trim()
            .replace(',', "")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .unwrap_or(self.default)
    }
}

fn use_num(default: f64) -> Num {
    Num {
        text: use_signal(|| default.to_string()),
        default,
    }
}

#[component]
fn Field(
    num: Num,
    label: String,
    step: f64,
    #[props(default)] suffix: String,
    #[props(default = 2)] decimals: usize,
    #[props(default = "w-12")] width: &'static str,
) -> Element {
    let mut text = num.text;
    rsx! {
        Stepper {
            value: text(),
            step,
            aria_label: label.clone(),
            label,
            suffix,
            decimals,
            width,
            placeholder: num.default.to_string(),
            on_change: move |v: String| text.set(v),
        }
    }
}

#[component]
fn Inputs(children: Element) -> Element {
    rsx! { div { class: "flex flex-wrap items-center gap-2", {children} } }
}

#[component]
fn Results(#[props(default = 3)] cols: u8, children: Element) -> Element {
    let grid = match cols {
        2 => "sm:grid-cols-2",
        4 => "sm:grid-cols-2 lg:grid-cols-4",
        _ => "sm:grid-cols-3",
    };
    rsx! { div { class: "mt-4 grid gap-3 {grid}", {children} } }
}

#[component]
fn Note(text: String) -> Element {
    rsx! { p { class: "mt-4 text-xs leading-relaxed text-ctp-overlay1", "{text}" } }
}

/// A horizontal bar with a label and value, `share` (0–1) of the width.
#[component]
fn Bar(label: String, share: f64, color: &'static str, value: String, #[props(default)] strong: bool) -> Element {
    let label_class = if strong { "font-semibold text-ctp-text" } else { "text-ctp-subtext0" };
    rsx! {
        div { class: "grid grid-cols-[5.5rem_1fr_3.5rem] items-center gap-3 text-xs",
            span { class: "truncate {label_class}", "{label}" }
            div { class: "h-2.5 rounded-full bg-ctp-surface0/60",
                div { class: "h-full rounded-full {color}", style: "width:{share.clamp(0.0, 1.0) * 100.0:.1}%;" }
            }
            span { class: "text-right tabular-nums {label_class}", "{value}" }
        }
    }
}

fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

fn pct2(x: f64) -> String {
    format!("{:.2}%", x * 100.0)
}

fn signed_pct(x: f64) -> String {
    format!("{:+.1}%", x * 100.0)
}

/// Whole amount with the display currency's symbol: `฿12,500`.
fn money(x: f64) -> String {
    let rounded = x.round();
    let whole = Decimal::from(rounded.abs().min(1e15) as i64);
    let sign = if rounded < 0.0 { "-" } else { "" };
    format!("{sign}{}{}", display_currency().0, fmt_shares(whole))
}

fn signed_money(x: f64) -> String {
    if x.round() > 0.0 {
        format!("+{}", money(x))
    } else {
        money(x)
    }
}

fn gain_tone(x: f64) -> &'static str {
    if x >= 0.0 {
        "text-ctp-green"
    } else {
        "text-ctp-red"
    }
}

// ─── Calculators ──────────────────────────────────────────────────────────────

#[component]
fn Valuation() -> Element {
    let risk_free = use_context::<AppSettings>().risk_free();
    let (price, eps, growth) = (use_num(150.0), use_num(6.0), use_num(12.0));
    let pe = math::pe(price.value(), eps.value());
    let peg = pe.and_then(|pe| math::peg(pe, growth.value()));
    let earnings_yield = pe.map(|pe| 1.0 / pe);
    let peg_hint = match peg {
        None => tr("Needs positive growth"),
        Some(p) if p < 1.0 => tr("Cheap for its growth — if the growth comes"),
        Some(p) if p <= 2.0 => tr("Fair for its growth"),
        Some(_) => tr("Paying a lot for the growth"),
    };
    let yield_tone = match earnings_yield {
        Some(y) if y > risk_free => "text-ctp-green",
        Some(_) => "text-ctp-peach",
        None => "text-ctp-text",
    };

    rsx! {
        Inputs {
            Field { num: price, label: tr("Price"), step: 5.0, width: "w-16" }
            Field { num: eps, label: "EPS", step: 0.5 }
            Field { num: growth, label: tr("EPS growth"), step: 1.0, suffix: "%", decimals: 0 }
        }
        Results {
            MetricTile {
                label: "P/E",
                value: or_dash(pe, |v| format!("{v:.1}")),
                hint: if pe.is_some() { tr("Years of today's profit you pay for") } else { tr("Needs positive EPS") },
            }
            MetricTile {
                label: tr("Earnings yield"),
                value: or_dash(earnings_yield, pct),
                hint: trf("Risk-free rate {}", &[&pct(risk_free)]),
                tone: yield_tone,
            }
            MetricTile { label: "PEG", value: or_dash(peg, |v| format!("{v:.2}")), hint: peg_hint }
        }
    }
}

#[component]
fn Volatility() -> Element {
    let (amount, mean, vol) = (use_num(100_000.0), use_num(8.0), use_num(20.0));
    let a = amount.value().max(0.0);
    let (m, v) = (mean.value() / 100.0, vol.value().max(0.0) / 100.0);
    let day = math::daily_vol(v);
    let (lo1, hi1) = math::yearly_range(m, v, 1.0);
    let (lo2, hi2) = math::yearly_range(m, v, 2.0);
    let span = |lo: f64, hi: f64| {
        trf("{} to {}", &[&format!("{:+.0}%", lo * 100.0), &format!("{:+.0}%", hi * 100.0)])
    };
    let worth = |lo: f64, hi: f64| format!("{} – {}", money(a * (1.0 + lo)), money(a * (1.0 + hi)));

    // The range bar spans both bands and always shows 0%.
    let (min, max) = (lo2.min(-0.1), hi2.max(0.1));
    let at = |x: f64| (x - min) / (max - min) * 100.0;

    rsx! {
        Inputs {
            Field { num: amount, label: tr("Amount"), step: 10_000.0, decimals: 0, width: "w-20" }
            Field { num: mean, label: tr("Expected return"), step: 1.0, suffix: "%", decimals: 0 }
            Field { num: vol, label: tr("Volatility"), step: 5.0, suffix: "%", decimals: 0 }
        }
        Results {
            MetricTile { label: tr("Typical day"), value: format!("±{}", pct2(day)), hint: format!("±{}", money(a * day)) }
            MetricTile { label: tr("2 years in 3"), value: span(lo1, hi1), hint: worth(lo1, hi1) }
            MetricTile { label: tr("19 years in 20"), value: span(lo2, hi2), hint: worth(lo2, hi2) }
        }
        div { class: "mt-5",
            div { class: "relative h-3 rounded-full bg-ctp-surface0/60",
                div { class: "absolute inset-y-0 rounded-full bg-ctp-mauve/25", style: "left:{at(lo2):.1}%;width:{at(hi2) - at(lo2):.1}%;" }
                div { class: "absolute inset-y-0 rounded-full bg-ctp-mauve/60", style: "left:{at(lo1):.1}%;width:{at(hi1) - at(lo1):.1}%;" }
                div { class: "absolute -inset-y-1 w-0.5 rounded-full bg-ctp-text", style: "left:{at(0.0):.1}%;" }
            }
            div { class: "mt-1.5 flex justify-between text-xs tabular-nums text-ctp-overlay1",
                span { "{min * 100.0:+.0}%" }
                span { "{max * 100.0:+.0}%" }
            }
        }
        Note { text: tr("Dark band: 2 years in 3 · light band: 19 in 20 · line: 0%. Real markets have fatter tails than this bell curve.") }
    }
}

#[component]
fn Recovery() -> Element {
    let mut loss = use_signal(|| 30.0_f64);
    let rate = use_num(8.0);
    let l = loss() / 100.0;
    let gain = math::recovery_gain(l);
    let r = rate.value() / 100.0;
    let years = math::years_to_recover(l, r);

    rsx! {
        div { class: "flex flex-wrap items-center gap-x-4 gap-y-2",
            label { class: "flex min-w-56 flex-1 items-center gap-3 text-xs text-ctp-subtext0",
                span { class: "shrink-0", {tr("Fall")} }
                input {
                    r#type: "range",
                    min: "1",
                    max: "90",
                    step: "1",
                    value: "{loss}",
                    class: "w-full cursor-pointer accent-ctp-red",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f64>() {
                            loss.set(v);
                        }
                    },
                }
                span { class: "w-10 shrink-0 text-right text-sm font-semibold tabular-nums text-ctp-red", "−{loss:.0}%" }
            }
            Field { num: rate, label: tr("Yearly return"), step: 1.0, suffix: "%", decimals: 0 }
        }
        Results { cols: 2,
            MetricTile {
                label: tr("Gain needed"),
                value: format!("+{:.0}%", gain * 100.0),
                hint: trf("{}× the fall", &[&format!("{:.1}", gain / l)]),
                tone: "text-ctp-green",
            }
            MetricTile {
                label: tr("Time to recover"),
                value: years.map_or_else(|| "—".to_string(), |y| trf("{} years", &[&format!("{y:.1}")])),
                hint: if years.is_some() { trf("At {}% a year", &[&format!("{:.0}", r * 100.0)]) } else { tr("Never, without growth").to_string() },
            }
        }
        div { class: "mt-5 grid gap-2",
            Bar { label: tr("Fall"), share: l / gain, color: "bg-ctp-red/70", value: format!("−{:.0}%", l * 100.0) }
            Bar { label: tr("Climb back"), share: 1.0, color: "bg-ctp-green/70", value: format!("+{:.0}%", gain * 100.0) }
        }
    }
}

#[component]
fn Var() -> Element {
    let (value, vol) = (use_num(100_000.0), use_num(18.0));
    let v = value.value().max(0.0);
    let day = math::daily_vol(vol.value().max(0.0) / 100.0);
    let loss = |z: f64| (format!("−{}", pct2(z * day)), money(v * z * day));
    let (var95, var95_money) = loss(math::Z_95);
    let (var99, var99_money) = loss(math::Z_99);
    let (es, es_money) = loss(math::ES_95);

    rsx! {
        Inputs {
            Field { num: value, label: tr("Portfolio"), step: 10_000.0, decimals: 0, width: "w-20" }
            Field { num: vol, label: tr("Volatility"), step: 1.0, suffix: "%", decimals: 0 }
        }
        Results { cols: 4,
            MetricTile { label: tr("Daily volatility"), value: pct2(day), hint: format!("±{}", money(v * day)) }
            MetricTile { label: "VaR 95%", value: var95, hint: trf("1 day in 20: {} or more", &[&var95_money]) }
            MetricTile { label: "VaR 99%", value: var99, hint: trf("1 day in 100: {} or more", &[&var99_money]) }
            MetricTile { label: tr("Expected shortfall"), value: es, hint: trf("Average bad day: {}", &[&es_money]) }
        }
        Note { text: tr("Your Risk tab uses your real daily returns instead, which usually show fatter tails than a bell curve.") }
    }
}

#[component]
fn Beta() -> Element {
    let (beta, market, amount) = (use_num(1.3), use_num(-20.0), use_num(100_000.0));
    let b = beta.value();
    let a = amount.value().max(0.0);
    let change = b * market.value() / 100.0;
    let compared = if b > 1.05 {
        tr("Swings harder than the market")
    } else if b >= 0.95 {
        tr("Moves like the market")
    } else if b >= 0.0 {
        tr("Calmer than the market")
    } else {
        tr("Tends to move the other way")
    };

    rsx! {
        Inputs {
            Field { num: beta, label: tr("Beta"), step: 0.1, decimals: 1 }
            Field { num: market, label: tr("Market move"), step: 5.0, suffix: "%", decimals: 0 }
            Field { num: amount, label: tr("Amount"), step: 10_000.0, decimals: 0, width: "w-20" }
        }
        Results {
            MetricTile { label: tr("Expected move"), value: signed_pct(change), tone: gain_tone(change) }
            MetricTile { label: tr("In money"), value: signed_money(a * change), tone: gain_tone(change) }
            MetricTile { label: tr("Against the market"), value: format!("{b:.1}×"), hint: compared }
        }
        Note { text: tr("Beta is an average relationship; any single event can turn out very differently.") }
    }
}

#[component]
fn Sharpe() -> Element {
    let risk_free = use_context::<AppSettings>().risk_free() * 100.0;
    let (ret_a, vol_a) = (use_num(12.0), use_num(15.0));
    let (ret_b, vol_b) = (use_num(20.0), use_num(35.0));
    let rf = use_num((risk_free * 100.0).round() / 100.0);
    let sharpe = |r: Num, v: Num| math::sharpe(r.value() / 100.0, v.value() / 100.0, rf.value() / 100.0);
    let (a, b) = (sharpe(ret_a, vol_a), sharpe(ret_b, vol_b));
    let grade = |s: Option<f64>| match s {
        Some(s) if s > 2.0 => tr("Excellent"),
        Some(s) if s > 1.0 => tr("Good"),
        Some(s) if s > 0.0 => tr("Beats cash"),
        Some(_) => tr("Below cash"),
        None => tr("Needs volatility above 0"),
    };
    let winner = match (a, b) {
        (Some(a), Some(b)) if (a - b).abs() < 0.05 => "A ≈ B",
        (Some(a), Some(b)) if a > b => "A",
        (Some(_), Some(_)) => "B",
        _ => "—",
    };

    rsx! {
        div { class: "grid gap-2",
            for (name, ret, vol) in [("A", ret_a, vol_a), ("B", ret_b, vol_b)] {
                div { key: "{name}", class: "flex flex-wrap items-center gap-2",
                    span { class: "w-5 text-sm font-semibold text-ctp-mauve", "{name}" }
                    Field { num: ret, label: tr("Return"), step: 1.0, suffix: "%", decimals: 0 }
                    Field { num: vol, label: tr("Volatility"), step: 1.0, suffix: "%", decimals: 0 }
                }
            }
            div { class: "flex flex-wrap items-center gap-2 pl-7",
                Field { num: rf, label: tr("Risk-free"), step: 0.25, suffix: "%" }
            }
        }
        Results {
            MetricTile { label: "Sharpe A", value: or_dash(a, |v| format!("{v:.2}")), hint: grade(a) }
            MetricTile { label: "Sharpe B", value: or_dash(b, |v| format!("{v:.2}")), hint: grade(b) }
            MetricTile { label: tr("More return per risk"), value: winner, hint: tr("Compare over the same period") }
        }
    }
}

#[component]
fn Diversify() -> Element {
    let (vol_a, vol_b) = (use_num(30.0), use_num(20.0));
    let (corr, weight) = (use_num(0.3), use_num(50.0));
    let (a, b) = (vol_a.value().max(0.0) / 100.0, vol_b.value().max(0.0) / 100.0);
    let w = (weight.value() / 100.0).clamp(0.0, 1.0);
    let c = corr.value().clamp(-1.0, 1.0);
    let mix = math::two_asset_vol(w, a, b, c);
    // With correlation 1 the mix is just the weighted average.
    let average = w * a + (1.0 - w) * b;
    let ladder: Vec<(f64, f64)> = [1.0, 0.5, 0.0, -0.5, -1.0]
        .into_iter()
        .map(|rho| (rho, math::two_asset_vol(w, a, b, rho)))
        .collect();
    let removed = trf("{} points", &[&format!("{:.1}", (average - mix) * 100.0)]);
    let mix_tone = if mix < average - 0.0005 { "text-ctp-green" } else { "text-ctp-text" };

    rsx! {
        Inputs {
            Field { num: vol_a, label: format!("{} A", tr("Volatility")), step: 5.0, suffix: "%", decimals: 0 }
            Field { num: vol_b, label: format!("{} B", tr("Volatility")), step: 5.0, suffix: "%", decimals: 0 }
            Field { num: corr, label: tr("Correlation"), step: 0.1, decimals: 1 }
            Field { num: weight, label: format!("{} A", tr("Weight")), step: 10.0, suffix: "%", decimals: 0 }
        }
        Results {
            MetricTile {
                label: tr("Mix volatility"),
                value: pct(mix),
                tone: mix_tone,
            }
            MetricTile { label: tr("Average of the two"), value: pct(average), hint: tr("What you'd get with correlation 1") }
            MetricTile { label: tr("Risk removed"), value: removed }
        }
        div { class: "mt-5 grid gap-2",
            div { class: "text-xs text-ctp-subtext0", {tr("The same mix at other correlations")} }
            for (rho, v) in ladder {
                Bar {
                    key: "{rho}",
                    label: format!("ρ {rho:+.1}"),
                    share: v / average.max(1e-9),
                    color: "bg-ctp-mauve/70",
                    value: pct(v),
                    strong: (rho - c).abs() < 0.05,
                }
            }
        }
    }
}

#[component]
fn Position() -> Element {
    let (portfolio, accept, fall) = (use_num(100_000.0), use_num(2.0), use_num(40.0));
    let p = portfolio.value().max(0.0);
    let f = fall.value() / 100.0;
    let size = math::max_position(p, accept.value().max(0.0) / 100.0, f);
    let weight = size.filter(|_| p > 0.0).map(|s| s / p);

    rsx! {
        Inputs {
            Field { num: portfolio, label: tr("Portfolio"), step: 10_000.0, decimals: 0, width: "w-20" }
            Field { num: accept, label: tr("Accept losing"), step: 0.5, suffix: "%", decimals: 1 }
            Field { num: fall, label: tr("Stock could fall"), step: 5.0, suffix: "%", decimals: 0 }
        }
        Results {
            MetricTile {
                label: tr("Max position"),
                value: or_dash(size, money),
                hint: weight.map(|w| trf("{} of the portfolio", &[&pct(w)])).unwrap_or_default(),
            }
            MetricTile {
                label: tr("Loss if it falls that far"),
                value: or_dash(size, |s| money(-s * f)),
                tone: "text-ctp-red",
            }
            MetricTile {
                label: tr("Positions this size"),
                value: or_dash(weight.filter(|w| *w > 0.0), |w| format!("≈ {:.0}", (1.0 / w).max(1.0))),
                hint: tr("To fill the whole portfolio"),
            }
        }
    }
}
