//! Per-stock analysis: returns, risk vs the S&P 500, 52-week range, trend
//! and momentum, a 1-year chart against the market, and your position.

use crate::i18n::tr;
use super::stats::{daily_returns, RiskReport};
use crate::{
    components::{
        card::{Card, MetricTile},
        charts::{
            performance::{compare, LabelStyle, Subject},
            GrowthChart, Series,
        },
    },
    format::{fmt_signed, fmt_usd, signed_color},
};
use api::quote::quote::get_charts;
use dioxus::prelude::*;
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

const STOCK_HEX: &str = "var(--catppuccin-color-mauve)"; // mauve
const MARKET_HEX: &str = "var(--catppuccin-color-blue)"; // blue

// ─── Stats ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct StockStats {
    pub price: f64,
    /// (label, change %) for 1W, 1M, 3M, 6M, 1Y where history allows.
    pub returns: Vec<(&'static str, f64)>,
    pub high_52w: f64,
    pub low_52w: f64,
    pub sma50: Option<f64>,
    pub sma200: Option<f64>,
    pub rsi14: Option<f64>,
    /// Risk vs the market; `None` with too little shared history.
    pub risk: Option<RiskReport>,
}

impl StockStats {
    /// `candles` and `market` are daily candles covering about a year.
    /// `risk_free` is the annual rate as a fraction.
    pub fn compute(candles: &[Candle], market: &[Candle], risk_free: f64) -> Option<Self> {
        let mut sorted: Vec<&Candle> = candles.iter().collect();
        sorted.sort_by_key(|c| c.ts);
        let closes: Vec<f64> = sorted
            .iter()
            .filter_map(|c| c.close.to_f64())
            .filter(|c| *c > 0.0)
            .collect();
        let price = *closes.last()?;

        let change = |days: usize| {
            (closes.len() > days).then(|| (price / closes[closes.len() - 1 - days] - 1.0) * 100.0)
        };
        let mut returns: Vec<(&'static str, f64)> =
            [("1W", 5), ("1M", 21), ("3M", 63), ("6M", 126)]
                .into_iter()
                .filter_map(|(label, days)| Some((label, change(days)?)))
                .collect();
        returns.push(("1Y", (price / closes[0] - 1.0) * 100.0));

        Some(Self {
            price,
            returns,
            high_52w: closes.iter().cloned().fold(f64::MIN, f64::max),
            low_52w: closes.iter().cloned().fold(f64::MAX, f64::min),
            sma50: sma(&closes, 50),
            sma200: sma(&closes, 200),
            rsi14: rsi(&closes, 14),
            risk: RiskReport::compute(
                &[daily_returns(candles)],
                &[1.0],
                &daily_returns(market),
                risk_free,
            ),
        })
    }

    /// Where today's price sits in the 52-week range, 0 (low) to 1 (high).
    pub fn range_position(&self) -> f64 {
        let span = self.high_52w - self.low_52w;
        if span > 0.0 {
            ((self.price - self.low_52w) / span).clamp(0.0, 1.0)
        } else {
            0.5
        }
    }

    pub fn trend(&self) -> Trend {
        match (self.sma50, self.sma200) {
            (Some(s50), Some(s200)) if self.price > s50 && s50 > s200 => Trend::Up,
            (Some(s50), Some(s200)) if self.price < s50 && s50 < s200 => Trend::Down,
            (Some(_), Some(_)) => Trend::Mixed,
            _ => Trend::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trend {
    Up,
    Down,
    Mixed,
    Unknown,
}

fn sma(closes: &[f64], n: usize) -> Option<f64> {
    (closes.len() >= n).then(|| closes[closes.len() - n..].iter().sum::<f64>() / n as f64)
}

/// Wilder's relative strength index.
fn rsi(closes: &[f64], n: usize) -> Option<f64> {
    if closes.len() <= n {
        return None;
    }
    let diffs: Vec<f64> = closes.windows(2).map(|w| w[1] - w[0]).collect();
    let (mut gain, mut loss) = diffs[..n]
        .iter()
        .fold((0.0, 0.0), |(g, l), d| (g + d.max(0.0), l + (-d).max(0.0)));
    gain /= n as f64;
    loss /= n as f64;
    for d in &diffs[n..] {
        gain = (gain * (n as f64 - 1.0) + d.max(0.0)) / n as f64;
        loss = (loss * (n as f64 - 1.0) + (-d).max(0.0)) / n as f64;
    }
    Some(if loss == 0.0 {
        100.0
    } else {
        100.0 - 100.0 / (1.0 + gain / loss)
    })
}

// ─── UI ───────────────────────────────────────────────────────────────────────

/// Navigates to a stock's page.
pub fn open_stock(ticker: &TickerSymbol) {
    navigator().push(format!("/stock/{}", ticker.as_str().replace('^', "%5E")));
}

/// Price-based analysis of one stock: signals, returns, a year vs the
/// S&P 500, risk, range & trend, and your position.
#[component]
pub(crate) fn StockReport(
    ticker: TickerSymbol,
    position: Option<Position>,
    total_value: Decimal,
) -> Element {
    let app_settings = use_context::<crate::app::AppSettings>();
    let (benchmark, risk_free) = (app_settings.benchmark(), app_settings.risk_free());
    let (symbol, key) = (ticker.clone(), format!("stock-report/{ticker}/{benchmark}"));
    let data = crate::cache::use_cached(move || key.clone(), move || {
        let ticker = symbol.clone();
        let benchmark = benchmark.clone();
        async move {
            let market = benchmark.clone();
            let charts = get_charts(
                vec![ticker.clone(), market.clone()],
                Range::Y1,
                Interval::D1,
                false,
            )
            .await
            .ok()?;
            let stats = StockStats::compute(charts.get(&ticker)?, charts.get(&market)?, risk_free)?;
            let cmp = compare(
                &charts,
                &[Subject::Ticker(ticker.clone()), Subject::Ticker(market)],
                LabelStyle::Date,
                false,
            );
            Some((stats, cmp))
        }
    });

    let result = data.read().clone();
    match result {
        None => rsx! {
            Card { title: "{ticker}", p { class: "py-16 text-center text-sm text-ctp-subtext0", {tr("Loading a year of prices…")} } }
        },
        Some(None) => rsx! {
            Card { title: "{ticker}", p { class: "py-16 text-center text-sm text-ctp-subtext0", "No price history for {ticker}." } }
        },
        Some(Some((stats, cmp))) => {
            let series = vec![
                Series {
                    name: ticker.to_string(),
                    color: STOCK_HEX.into(),
                    values: cmp
                        .lines
                        .first()
                        .map(|l| to_decimals(&l.values))
                        .unwrap_or_default(),
                },
                Series {
                    name: "S&P 500".into(),
                    color: MARKET_HEX.into(),
                    values: cmp
                        .lines
                        .get(1)
                        .map(|l| to_decimals(&l.values))
                        .unwrap_or_default(),
                },
            ];
            rsx! {
                div { class: "grid gap-5 min-w-0",
                    ReportHeader { stats: stats.clone() }
                    Card { title: tr("Past year vs S&P 500"), subtitle: tr("Both start at 0%").to_string(),
                        document::Script { src: asset!("/assets/js/growth_chart.js") }
                        GrowthChart { chart_dates: cmp.labels.clone(), series, height: dec!(220) }
                    }
                    RiskTiles { stats: stats.clone() }
                    div { class: "grid gap-5 md:grid-cols-2",
                        RangeAndTrend { stats: stats.clone() }
                        if let Some(p) = position {
                            PositionCard { position: p, total_value }
                        }
                    }
                }
            }
        }
    }
}

fn to_decimals(values: &[f64]) -> Vec<Option<Decimal>> {
    values.iter().map(|v| Decimal::try_from(*v).ok()).collect()
}

/// Price, returns by period and plain-English signal chips.
#[component]
fn ReportHeader(stats: StockStats) -> Element {
    let s = &stats;
    let mut chips: Vec<(String, &'static str)> = Vec::new();
    match s.trend() {
        Trend::Up => chips.push((
            "Uptrend · above 50 & 200-day".into(),
            "bg-ctp-green/15 text-ctp-green",
        )),
        Trend::Down => chips.push((
            "Downtrend · below 50 & 200-day".into(),
            "bg-ctp-red/15 text-ctp-red",
        )),
        Trend::Mixed => chips.push(("Mixed trend".into(), "bg-ctp-yellow/15 text-ctp-yellow")),
        Trend::Unknown => {}
    }
    if let Some(rsi) = s.rsi14 {
        if rsi >= 70.0 {
            chips.push((
                format!("RSI {rsi:.0} · overbought"),
                "bg-ctp-peach/15 text-ctp-peach",
            ));
        } else if rsi <= 30.0 {
            chips.push((
                format!("RSI {rsi:.0} · oversold"),
                "bg-ctp-sky/15 text-ctp-sky",
            ));
        }
    }
    let pos = s.range_position();
    if pos >= 0.9 {
        chips.push(("Near 52-week high".into(), "bg-ctp-green/15 text-ctp-green"));
    } else if pos <= 0.1 {
        chips.push(("Near 52-week low".into(), "bg-ctp-red/15 text-ctp-red"));
    }
    if let Some(r) = &s.risk {
        if r.beta > 1.3 {
            chips.push((
                format!("Swings {:.1}× the market", r.beta),
                "bg-ctp-peach/15 text-ctp-peach",
            ));
        } else if r.beta < 0.7 {
            chips.push((
                "Defensive · low beta".into(),
                "bg-ctp-teal/15 text-ctp-teal",
            ));
        }
    }

    rsx! {
        Card { title: tr("Signals & returns"), subtitle: tr("From daily prices over the past year").to_string(),
            if !chips.is_empty() {
                div { class: "mt-3 flex flex-wrap gap-2",
                    for (label, style) in chips {
                        span { class: "rounded-full px-3 py-1 text-xs font-semibold {style}", "{label}" }
                    }
                }
            }
            div { class: "mt-5 grid grid-cols-5 gap-2",
                for (label, change) in s.returns.clone() {
                    div { class: "rounded-2xl border border-ctp-surface0/70 bg-ctp-base/50 px-3 py-2 text-center",
                        div { class: "text-xs text-ctp-subtext0", "{label}" }
                        div {
                            class: if change >= 0.0 { "mt-0.5 text-sm font-semibold tabular-nums text-ctp-green" } else { "mt-0.5 text-sm font-semibold tabular-nums text-ctp-red" },
                            "{change:+.1}%"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RiskTiles(stats: StockStats) -> Element {
    let Some(r) = stats.risk else {
        return rsx! {};
    };
    let pct = |x: f64| x * 100.0;
    let (vol_tone, dd_tone, sharpe_tone) = (
        if r.volatility <= r.market_volatility {
            "text-ctp-green"
        } else {
            "text-ctp-peach"
        },
        if r.max_drawdown <= r.market_max_drawdown {
            "text-ctp-green"
        } else {
            "text-ctp-peach"
        },
        if r.sharpe >= 1.0 {
            "text-ctp-green"
        } else {
            "text-ctp-peach"
        },
    );
    rsx! {
        div { class: "grid grid-cols-2 lg:grid-cols-4 gap-3",
            MetricTile {
                label: tr("Volatility"),
                value: format!("{:.1}%", pct(r.volatility)),
                hint: format!("Yearly swing · S&P {:.1}%", pct(r.market_volatility)),
                tone: vol_tone,
            }
            MetricTile {
                label: tr("Beta"),
                value: format!("{:.2}", r.beta),
                hint: format!("Correlation {:.2} with the S&P", r.market_correlation),
            }
            MetricTile {
                label: tr("Max drawdown"),
                value: format!("−{:.1}%", pct(r.max_drawdown)),
                hint: format!("Worst fall · S&P −{:.1}%", pct(r.market_max_drawdown)),
                tone: dd_tone,
            }
            MetricTile {
                label: tr("Sharpe ratio"),
                value: format!("{:.2}", r.sharpe),
                hint: format!("Return per risk · alpha {:+.1}%", pct(r.alpha)),
                tone: sharpe_tone,
            }
        }
    }
}

#[component]
fn RangeAndTrend(stats: StockStats) -> Element {
    let s = &stats;
    let pos = s.range_position() * 100.0;
    let money = |x: f64| fmt_usd(Decimal::try_from(x).unwrap_or_default(), 2);
    let vs = |avg: Option<f64>| {
        avg.map(|a| {
            let diff = (s.price / a - 1.0) * 100.0;
            (
                money(a),
                format!("{diff:+.1}%"),
                signed_color(Decimal::try_from(diff).unwrap_or_default()),
            )
        })
    };
    rsx! {
        Card { title: tr("Range & trend"),
            div { class: "flex justify-between text-xs text-ctp-subtext0",
                span { "52-week low" }
                span { "52-week high" }
            }
            div { class: "relative mt-2 h-2 rounded-full bg-gradient-to-r from-ctp-red/50 via-ctp-yellow/50 to-ctp-green/50",
                span {
                    class: "absolute top-1/2 h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ctp-base bg-ctp-text",
                    style: "left:{pos:.1}%;",
                    title: "{pos:.0}% of the way from low to high",
                }
            }
            div { class: "mt-2 flex justify-between text-sm tabular-nums text-ctp-subtext1",
                span { "{money(s.low_52w)}" }
                span { "{money(s.high_52w)}" }
            }
            div { class: "mt-5 grid gap-2 text-sm",
                for (label, value) in [("50-day average", vs(s.sma50)), ("200-day average", vs(s.sma200))] {
                    div { class: "flex items-center justify-between",
                        span { class: "text-ctp-subtext0", "{label}" }
                        if let Some((avg, diff, color)) = value {
                            span { class: "tabular-nums",
                                span { class: "text-ctp-subtext1", "{avg} " }
                                span { class: "{color}", "({diff})" }
                            }
                        } else {
                            span { class: "text-ctp-overlay1", "not enough history" }
                        }
                    }
                }
                if let Some(rsi) = s.rsi14 {
                    div { class: "flex items-center justify-between",
                        span { class: "text-ctp-subtext0", {tr("RSI (14)")} }
                        span { class: "tabular-nums text-ctp-subtext1", "{rsi:.0}" }
                    }
                }
            }
        }
    }
}

#[component]
fn PositionCard(position: Position, total_value: Decimal) -> Element {
    let p = &position;
    let weight = p.market_value() / total_value.max(dec!(1)) * dec!(100);
    rsx! {
        Card { title: tr("Your position"),
            div { class: "grid gap-2 text-sm",
                Row { label: tr("Shares"), value: crate::format::fmt_shares(p.shares) }
                Row { label: tr("Average cost"), value: fmt_usd(p.avg_cost, 2) }
                Row { label: tr("Market value"), value: fmt_usd(p.market_value(), 2) }
                Row { label: tr("Weight"), value: format!("{weight:.1}%") }
                Row {
                    label: tr("Return"),
                    value: format!("{} ({:+.2}%)", fmt_signed(p.unrealized_pnl(), 2), p.unrealized_pnl_pct()),
                    color: signed_color(p.unrealized_pnl()),
                }
            }
        }
    }
}

#[component]
fn Row(
    label: String,
    value: String,
    #[props(default = "text-ctp-text")] color: &'static str,
) -> Element {
    rsx! {
        div { class: "flex items-center justify-between",
            span { class: "text-ctp-subtext0", "{label}" }
            span { class: "font-medium tabular-nums {color}", "{value}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_averages_and_rsi() {
        let rising: Vec<f64> = (1..=250).map(|i| i as f64).collect();
        assert_eq!(sma(&rising, 50), Some(225.5));
        assert_eq!(sma(&rising[..10], 50), None);
        // Only gains: RSI saturates at 100.
        assert_eq!(rsi(&rising, 14), Some(100.0));
        // Alternating ±1 is balanced: RSI 50.
        let flat: Vec<f64> = (0..60)
            .map(|i| if i % 2 == 0 { 10.0 } else { 11.0 })
            .collect();
        assert!((rsi(&flat, 14).unwrap() - 50.0).abs() < 5.0);
    }
}
