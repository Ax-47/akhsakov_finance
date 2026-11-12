//! What the charts say about one stock right now: the usual indicators
//! read as buy, neutral or sell and tallied, the pivot levels from the
//! last full session, and what each month of the year has done.

use super::quant::{monthly_returns, seasonality, MonthStat};
use super::stats::MONTHS;
use crate::{
    components::card::Card,
    format::fmt_usd,
    i18n::{tr, tr_str, trf},
};
use dioxus::prelude::*;
use dtos::signals::{counts, pivots, score, verdict, Action, Reading, TechnicalRating, Verdict};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

fn money(v: f64) -> String {
    Decimal::try_from(v).map_or_else(|_| "—".to_string(), |d| fmt_usd(d, 2))
}

/// Highs, lows and closes of `candles`, oldest first.
fn columns(candles: &[Candle]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut sorted: Vec<&Candle> = candles.iter().filter(|c| c.close > Decimal::ZERO).collect();
    sorted.sort_by_key(|c| c.ts);
    let of = |pick: fn(&Candle) -> Decimal| sorted.iter().map(|c| pick(c).to_f64().unwrap_or(0.0)).collect::<Vec<f64>>();
    (of(|c| c.high), of(|c| c.low), of(|c| c.close))
}

/// The tally of the indicators as a headline, a meter and the readings
/// behind it, then the pivot levels. `candles` are about a year of daily
/// candles; `today` (`YYYY-MM-DD`, UTC) marks a candle still forming.
#[component]
pub fn TechnicalCard(candles: Vec<Candle>, today: String) -> Element {
    let (high, low, close) = columns(&candles);
    let Some(rating) = TechnicalRating::compute(&high, &low, &close) else {
        return rsx! {};
    };
    let all = rating.all();
    let price = close.last().copied().unwrap_or(0.0);
    // The last session that has ended: today's candle is still moving.
    let mut sorted: Vec<&Candle> = candles.iter().filter(|c| c.close > Decimal::ZERO).collect();
    sorted.sort_by_key(|c| c.ts);
    let session = sorted.iter().rev().find(|c| c.ts.date_naive().to_string() < today).copied();
    let levels = session.map(|c| {
        let f = |d: Decimal| d.to_f64().unwrap_or(0.0);
        (pivots(f(c.high), f(c.low), f(c.close)), c.ts.date_naive().to_string())
    });
    rsx! {
        Card {
            title: tr("Technical summary"),
            subtitle: tr("What the usual indicators say on daily prices. A tally of rules, not advice.").to_string(),
            div { class: "grid gap-4 sm:grid-cols-3",
                Tally { label: tr("Overall"), readings: all.clone(), strong: true }
                Tally { label: tr("Moving averages"), readings: rating.averages.clone() }
                Tally { label: tr("Oscillators"), readings: rating.oscillators.clone() }
            }
            div { class: "mt-5 grid gap-6 lg:grid-cols-2",
                Readings { title: tr("Moving averages"), hint: tr("Price above an average is a buy, below it a sell"), readings: rating.averages.clone(), as_price: true }
                Readings { title: tr("Oscillators"), hint: tr("Stretched too far one way is expected to come back; MACD, momentum and ADX follow the trend"), readings: rating.oscillators.clone() }
            }
            if let Some((p, day)) = levels {
                div { class: "mt-6",
                    div { class: "flex flex-wrap items-baseline justify-between gap-2",
                        span { class: "text-sm font-medium text-ctp-text", {tr("Pivot levels")} }
                        span { class: "text-xs text-ctp-subtext0", {trf("From the session of {} · price now {}", &[&crate::format::fmt_when(&day), &money(price)])} }
                    }
                    div { class: "mt-2 grid grid-cols-4 gap-2 sm:grid-cols-7",
                        for (name, level, tone) in [
                            ("S3", p.support[2], "text-ctp-green"),
                            ("S2", p.support[1], "text-ctp-green"),
                            ("S1", p.support[0], "text-ctp-green"),
                            ("Pivot", p.pivot, "text-ctp-text"),
                            ("R1", p.resistance[0], "text-ctp-red"),
                            ("R2", p.resistance[1], "text-ctp-red"),
                            ("R3", p.resistance[2], "text-ctp-red"),
                        ] {
                            div { key: "{name}", class: "rounded-xl border border-ctp-surface0/70 bg-ctp-base/50 px-2.5 py-2 text-center",
                                div { class: "text-[11px] {tone}", {tr_str(name)} }
                                div { class: "text-sm font-semibold tabular-nums text-ctp-text", "{money(level)}" }
                                div { class: "text-[11px] tabular-nums text-ctp-overlay1", {if price > 0.0 { format!("{:+.1}%", (level / price - 1.0) * 100.0) } else { String::new() }} }
                            }
                        }
                    }
                    p { class: "mt-2 text-xs text-ctp-overlay1", {tr("S is support (a floor buyers have defended), R is resistance (a ceiling). The percentage is the distance from the price now.")} }
                }
            }
        }
    }
}

fn verdict_tone(v: Verdict) -> &'static str {
    match v {
        Verdict::StrongBuy | Verdict::Buy => "text-ctp-green",
        Verdict::StrongSell | Verdict::Sell => "text-ctp-red",
        Verdict::Neutral => "text-ctp-subtext1",
    }
}

/// One group's verdict, a meter from sell to buy and the counts.
#[component]
fn Tally(label: String, readings: Vec<Reading>, #[props(default)] strong: bool) -> Element {
    let Some(score) = score(&readings) else {
        return rsx! {};
    };
    let v = verdict(score);
    let (buy, neutral, sell) = counts(&readings);
    let at = (score + 1.0) / 2.0 * 100.0;
    rsx! {
        div { class: if strong { "rounded-2xl border border-ctp-mauve/40 bg-ctp-base/50 px-4 py-3" } else { "rounded-2xl border border-ctp-surface0/70 bg-ctp-base/50 px-4 py-3" },
            div { class: "text-xs text-ctp-subtext0", "{label}" }
            div { class: "mt-0.5 text-xl font-semibold {verdict_tone(v)}", {tr_str(v.label())} }
            div { class: "relative mt-2 h-1.5 rounded-full bg-gradient-to-r from-ctp-red/70 via-ctp-surface1 to-ctp-green/70",
                role: "img",
                "aria-label": trf("{} buy, {} neutral, {} sell", &[&buy, &neutral, &sell]),
                span { class: "absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ctp-mantle bg-ctp-text", style: "left:{at:.1}%" }
            }
            div { class: "mt-2 flex justify-between text-[11px] tabular-nums",
                span { class: "text-ctp-red", {trf("Sell {}", &[&sell])} }
                span { class: "text-ctp-subtext0", {trf("Neutral {}", &[&neutral])} }
                span { class: "text-ctp-green", {trf("Buy {}", &[&buy])} }
            }
        }
    }
}

#[component]
fn Readings(title: String, hint: String, readings: Vec<Reading>, #[props(default)] as_price: bool) -> Element {
    rsx! {
        div {
            div { class: "text-sm font-medium text-ctp-text", "{title}" }
            div { class: "mb-2 text-xs text-ctp-subtext0", "{hint}" }
            table { class: "w-full text-sm tabular-nums",
                tbody {
                    for r in readings {
                        tr { key: "{r.name}", class: "border-t border-ctp-surface0/60",
                            td { class: "py-1.5 text-ctp-subtext1", "{r.name}" }
                            td { class: "py-1.5 text-right text-ctp-text", {if as_price { money(r.value) } else { format!("{:.2}", r.value) }} }
                            td {
                                class: match r.action {
                                    Action::Buy => "w-20 py-1.5 text-right font-medium text-ctp-green",
                                    Action::Sell => "w-20 py-1.5 text-right font-medium text-ctp-red",
                                    Action::Neutral => "w-20 py-1.5 text-right text-ctp-subtext0",
                                },
                                {match r.action {
                                    Action::Buy => tr("Buy"),
                                    Action::Sell => tr("Sell"),
                                    Action::Neutral => tr("Neutral"),
                                }}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What each month of the year has done over the past ten years: the
/// average move and how often the month ended higher.
#[component]
pub fn SeasonalityCard(ticker: TickerSymbol) -> Element {
    let symbol = ticker.clone();
    let data = crate::cache::use_cached(
        {
            let ticker = ticker.clone();
            move || format!("seasonality/{ticker}")
        },
        move || {
            let ticker = symbol.clone();
            async move {
                let charts = crate::offline::charts(vec![ticker.clone()], Range::Y10, Interval::M1, false).await.ok()?;
                let mut candles = charts.get(&ticker)?.clone();
                candles.sort_by_key(|c| c.ts);
                Some(
                    candles
                        .iter()
                        .filter_map(|c| Some((c.ts.timestamp().div_euclid(86_400), c.close.to_f64().filter(|v| *v > 0.0)?)))
                        .collect::<Vec<(i64, f64)>>(),
                )
            }
        },
    );
    let Some(Some(series)) = data.read().clone() else {
        return rsx! {};
    };
    let mut months = monthly_returns(&series);
    // The month in progress isn't a month yet.
    let now = chrono::Utc::now().date_naive();
    let this_month = {
        use chrono::Datelike;
        (i64::from(now.year()), now.month())
    };
    months.retain(|(y, m, _)| (*y, *m) != this_month);
    let stats = seasonality(&months);
    let years = stats.iter().map(|s| s.years).max().unwrap_or(0);
    if years < 3 {
        return rsx! {};
    }
    let scale = stats.iter().map(|s| s.average.abs()).fold(0.005, f64::max);
    rsx! {
        Card {
            title: tr("Seasonality"),
            subtitle: trf("What each month has done on average over the past {} years, dividends included. A habit of the past, not a forecast.", &[&years]),
            div { class: "grid grid-cols-6 gap-x-1 gap-y-4 sm:grid-cols-12",
                for (i, s) in stats.iter().enumerate() {
                    MonthBar { key: "{i}", name: MONTHS[i], stat: *s, scale, current: i as u32 + 1 == this_month.1 }
                }
            }
            p { class: "mt-3 text-xs text-ctp-overlay1", {tr("Under each bar: the average move, and the years the month ended higher out of the years counted.")} }
        }
    }
}

#[component]
fn MonthBar(name: &'static str, stat: MonthStat, scale: f64, current: bool) -> Element {
    let size = (stat.average.abs() / scale).clamp(0.0, 1.0) * 50.0;
    let up = stat.average >= 0.0;
    let title = trf("Best {} · worst {}", &[&format!("{:+.1}%", stat.best * 100.0), &format!("{:+.1}%", stat.worst * 100.0)]);
    rsx! {
        div { class: "text-center", title: "{title}",
            div { class: "relative mx-auto h-24 w-full max-w-[2.75rem]",
                span { class: "absolute inset-x-0 top-1/2 h-px bg-ctp-surface1" }
                if stat.years > 0 {
                    span {
                        class: if up { "absolute inset-x-1 rounded-t bg-ctp-green/70" } else { "absolute inset-x-1 rounded-b bg-ctp-red/70" },
                        style: if up { "bottom:50%; height:{size:.1}%;" } else { "top:50%; height:{size:.1}%;" },
                    }
                }
            }
            div { class: if current { "mt-1 text-xs font-semibold text-ctp-mauve" } else { "mt-1 text-xs text-ctp-subtext1" }, {tr_str(name)} }
            if stat.years > 0 {
                div { class: if up { "text-[11px] tabular-nums text-ctp-green" } else { "text-[11px] tabular-nums text-ctp-red" }, "{stat.average * 100.0:+.1}%" }
                div { class: "text-[10px] tabular-nums text-ctp-overlay1", "{stat.positive}/{stat.years}" }
            } else {
                div { class: "text-[11px] text-ctp-overlay1", "—" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::prelude::FromPrimitive;

    /// Daily candles from 1 Jan 2026 climbing a point a day.
    fn climb(days: usize) -> Vec<Candle> {
        (0..days)
            .map(|i| {
                let close = 500.0 + i as f64;
                let d = |v: f64| Decimal::from_f64(v).unwrap();
                Candle {
                    ts: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::days(i as i64),
                    open: d(close - 0.5),
                    high: d(close + 1.0),
                    low: d(close - 1.0),
                    close: d(close),
                    volume: None,
                    adj_factor: None,
                }
            })
            .collect()
    }

    fn render(candles: Vec<Candle>, today: &str) -> String {
        let mut dom = VirtualDom::new_with_props(TechnicalCard, TechnicalCardProps { candles, today: today.into() });
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn a_climb_reads_as_a_buy_with_levels_from_the_last_full_session() {
        // 60 candles: 1 Jan to 1 Mar. "Today" is 1 Mar, so the levels come
        // from 28 Feb: high 559, low 557, close 558.
        let html = render(climb(60), "2026-03-01");
        assert!(html.contains("Technical summary") && html.contains("Strong buy"), "{html}");
        assert!(html.contains("SMA 50") && !html.contains("SMA 100"), "only the averages there is history for");
        assert!(html.contains("28 Feb 2026"), "the session before today: {html}");
        assert!(html.contains("$558.00"), "the pivot is that session's typical price: {html}");
        // The same candles the day after: 1 Mar is a full session now.
        let html = render(climb(60), "2026-03-02");
        assert!(html.contains("1 Mar 2026") && html.contains("$559.00"), "{html}");
    }

    #[test]
    fn too_little_history_shows_nothing() {
        assert_eq!(render(climb(20), "2026-03-01"), "");
    }
}
