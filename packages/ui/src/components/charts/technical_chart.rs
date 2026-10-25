//! Interactive price chart for one stock: candles or line, moving averages,
//! Bollinger bands, volume, and an RSI or MACD pane. Indicators are
//! computed here (see `dtos::technicals`); `stock_chart.js` draws them.
//!
//! The chart is live: each streamed price moves the last candle, or starts
//! the next one, between fetches. One- and five-day spans use one- and
//! five-minute candles and fetch again every few minutes.
//!
//! Prices are the ones actually traded, in the stock's own currency, as a
//! broker shows them: converting a Thai stock's history to dollars (or
//! back) at each day's rate would mix exchange-rate moves into the price,
//! and dividend-adjusted history would put past prices, and so the moving
//! averages and bands, below where the stock really traded.

use crate::i18n::tr;
use crate::components::card::{Card, Segmented, ToggleButton};
use crate::hooks::LiveTick;
use api::quote::quote::get_native_chart;
use dioxus::prelude::*;
use dtos::technicals::{bollinger, ema, macd, rsi, sma};
use rust_decimal::prelude::ToPrimitive;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol};

static CHART_CTR: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Span {
    D1,
    D5,
    M1,
    M3,
    M6,
    Y1,
    Y2,
    Y5,
}

impl Span {
    const ALL: [Self; 8] = [Self::D1, Self::D5, Self::M1, Self::M3, Self::M6, Self::Y1, Self::Y2, Self::Y5];

    fn label(self) -> &'static str {
        match self {
            Self::D1 => "1D",
            Self::D5 => "5D",
            Self::M1 => "1M",
            Self::M3 => "3M",
            Self::M6 => "6M",
            Self::Y1 => "1Y",
            Self::Y2 => "2Y",
            Self::Y5 => "5Y",
        }
    }

    /// Trading sessions shown at first for the minute spans.
    fn sessions(self) -> Option<usize> {
        match self {
            Self::D1 => Some(1),
            Self::D5 => Some(5),
            _ => None,
        }
    }

    /// Seconds each candle covers.
    fn step(self) -> i64 {
        match self {
            Self::D1 => 60,
            Self::D5 => 300,
            Self::Y5 => 7 * 86_400,
            _ => 86_400,
        }
    }

    /// What the averages count, e.g. "days".
    fn unit(self) -> &'static str {
        match self {
            Self::D1 => tr("minutes"),
            Self::D5 => tr("5-minute candles"),
            Self::Y5 => tr("weeks"),
            _ => tr("days"),
        }
    }

    /// Days shown at first; the rest of the fetched history warms up the
    /// indicators and stays reachable by zooming out.
    fn days(self) -> i64 {
        match self {
            Self::D1 | Self::D5 => 0,
            Self::M1 => 31,
            Self::M3 => 92,
            Self::M6 => 183,
            Self::Y1 => 366,
            Self::Y2 => 731,
            Self::Y5 => 1827,
        }
    }

    /// History fetched (longer than shown, so a 200-day average is ready).
    fn fetch(self) -> (Range, Interval) {
        match self {
            Self::D1 => (Range::D5, Interval::I1m),
            Self::D5 => (Range::M1, Interval::I5m),
            Self::M1 | Self::M3 => (Range::Y1, Interval::D1),
            Self::M6 | Self::Y1 => (Range::Y2, Interval::D1),
            Self::Y2 => (Range::Y5, Interval::D1),
            Self::Y5 => (Range::Y10, Interval::W1),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Lower {
    None,
    Rsi,
    Macd,
}

#[derive(Clone, Copy, PartialEq)]
struct Overlays {
    sma20: bool,
    sma50: bool,
    sma200: bool,
    ema20: bool,
    bollinger: bool,
}

/// How often the minute spans fetch their candles again.
const REFETCH_MS: u32 = 5 * 60 * 1000;

#[component]
pub fn TechnicalChart(ticker: TickerSymbol, live: ReadSignal<Option<LiveTick>>) -> Element {
    let mut span = use_signal(|| Span::Y1);
    let mut candles_style = use_signal(|| true);
    let mut lower = use_signal(|| Lower::Rsi);
    let mut overlays = use_signal(|| Overlays {
        sma20: false,
        sma50: true,
        sma200: true,
        ema20: false,
        bollinger: false,
    });
    let chart_id = use_hook(|| format!("stock-chart-{}", CHART_CTR.fetch_add(1, Ordering::Relaxed)));

    // Bumped to fetch the minute spans again.
    let mut refetch = use_signal(|| 0_u32);
    use_future(move || async move {
        loop {
            crate::notify::poll_delay(REFETCH_MS).await;
            if span.peek().sessions().is_some() {
                refetch += 1;
            }
        }
    });
    let (t, key) = (ticker.clone(), ticker.clone());
    let history = crate::cache::use_cached(move || format!("technical/{key}/{:?}", span()), move || {
        let _ = refetch();
        let t = t.clone();
        async move {
            let (range, interval) = span().fetch();
            get_native_chart(t, range, interval).await.map_err(|e| e.to_string())
        }
    });

    let drop_id = chart_id.clone();
    use_drop(move || {
        document::eval(&format!(
            "window.StockChart && window.StockChart.dispose && window.StockChart.dispose({drop_id:?});"
        ));
    });

    let id = chart_id.clone();
    use_effect(move || {
        let Some(Ok((candles, currency))) = &*history.read() else {
            return;
        };
        let cfg = chart_config(candles, currency_symbol(currency), span(), candles_style(), overlays(), lower());
        // The chart scripts may still be loading the first time.
        let script = format!(
            "for (let i = 0; i < 200 && !(window.StockChart && window.StockChart.init && window.GrowthChart); i++)
                 await new Promise(r => setTimeout(r, 25));
             window.StockChart.init({}, {});",
            json!(id),
            cfg
        );
        spawn(async move {
            let _ = document::eval(&script).await;
        });
    });

    // Each streamed price moves the drawn chart without redrawing it.
    let id = chart_id.clone();
    use_effect(move || {
        let Some(tick) = live() else {
            return;
        };
        let Some(price) = tick.native.to_f64() else {
            return;
        };
        document::eval(&format!(
            "window.StockChart && window.StockChart.tick && window.StockChart.tick({}, {price}, {});",
            json!(id),
            tick.ts
        ));
    });

    let o = overlays();
    let unit = span().unit();
    let status = match &*history.read() {
        None => Some(tr("Loading chart…").to_string()),
        // The raw error is a server message with the data source's URL in
        // it: not something to show, and too long to fit on a phone.
        Some(Err(_)) => Some(tr("Couldn't load prices right now. The data source may be busy; try again shortly.").to_string()),
        Some(Ok((c, _))) if c.is_empty() => Some(tr("No price history.").to_string()),
        _ => None,
    };

    rsx! {
        document::Script { src: asset!("/assets/js/growth_chart.js") }
        document::Script { src: asset!("/assets/js/stock_chart.js") }
        Card {
            title: tr("Chart"),
            subtitle: crate::i18n::trf("Drag to pan, Ctrl + scroll to zoom · averages in {}", &[&unit]),
            actions: rsx! {
                div { class: "flex flex-wrap items-center gap-2",
                    Segmented {
                        for s in Span::ALL {
                            ToggleButton { key: "{s.label()}", label: tr(s.label()), active: span() == s, onclick: move |_| span.set(s) }
                        }
                    }
                    Segmented {
                        ToggleButton { label: tr("Candles"), active: candles_style(), onclick: move |_| candles_style.set(true) }
                        ToggleButton { label: tr("Line"), active: !candles_style(), onclick: move |_| candles_style.set(false) }
                    }
                }
            },
            div { class: "mb-3 flex flex-wrap items-center gap-2 text-xs",
                span { class: "text-ctp-subtext0", {tr("Overlays")} }
                Chip { label: tr("SMA 20"), color: "yellow", on: o.sma20, onclick: move |_| overlays.with_mut(|o| o.sma20 = !o.sma20) }
                Chip { label: tr("SMA 50"), color: "sky", on: o.sma50, onclick: move |_| overlays.with_mut(|o| o.sma50 = !o.sma50) }
                Chip { label: tr("SMA 200"), color: "peach", on: o.sma200, onclick: move |_| overlays.with_mut(|o| o.sma200 = !o.sma200) }
                Chip { label: tr("EMA 20"), color: "pink", on: o.ema20, onclick: move |_| overlays.with_mut(|o| o.ema20 = !o.ema20) }
                Chip { label: tr("Bollinger"), color: "lavender", on: o.bollinger, onclick: move |_| overlays.with_mut(|o| o.bollinger = !o.bollinger) }
                span { class: "ml-3 text-ctp-subtext0", {tr("Lower pane")} }
                Chip { label: tr("RSI"), color: "teal", on: lower() == Lower::Rsi, onclick: move |_| lower.set(if lower() == Lower::Rsi { Lower::None } else { Lower::Rsi }) }
                Chip { label: tr("MACD"), color: "blue", on: lower() == Lower::Macd, onclick: move |_| lower.set(if lower() == Lower::Macd { Lower::None } else { Lower::Macd }) }
            }
            div { class: "relative",
                div { id: "{chart_id}", style: "width:100%;height:460px;" }
                if let Some(status) = status {
                    div { class: "absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-ctp-subtext0 break-words", "{status}" }
                }
            }
            p { class: "mt-2 text-xs text-ctp-overlay1",
                {tr("RSI above 70 is often read as overbought and below 30 as oversold; a MACD line crossing above its signal as momentum turning up. Signals, not predictions.")}
            }
        }
    }
}

/// Toggle chip with a colour dot matching its line.
#[component]
fn Chip(label: String, color: &'static str, on: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let opacity = if on { 1.0 } else { 0.4 };
    rsx! {
        button {
            class: if on {
                "inline-flex items-center gap-1.5 rounded-full border border-ctp-surface1 bg-ctp-surface0 px-2.5 py-1 text-ctp-text cursor-pointer"
            } else {
                "inline-flex items-center gap-1.5 rounded-full border border-ctp-surface0 px-2.5 py-1 text-ctp-subtext0 cursor-pointer hover:text-ctp-text"
            },
            onclick: move |e| onclick.call(e),
            span { class: "h-2 w-2 rounded-full", style: "background:var(--catppuccin-color-{color});opacity:{opacity};" }
            "{label}"
        }
    }
}

/// Symbol for a currency code, e.g. `฿` for THB; the code itself when
/// there's no short symbol.
fn currency_symbol(code: &str) -> String {
    dtos::settings::CURRENCIES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or_else(|| format!("{code} "), |(_, symbol)| symbol.to_string())
}

/// Everything the chart draws. `candles` are in the stock's currency
/// (`symbol`) and adjusted for dividends; they're drawn as traded.
fn chart_config(
    candles: &[Candle],
    symbol: String,
    span: Span,
    candle_style: bool,
    o: Overlays,
    lower: Lower,
) -> Value {
    // Undo the dividend adjustment, back to the price that traded.
    let px = |d: rust_decimal::Decimal, c: &Candle| d.to_f64().unwrap_or(0.0) / c.adjustment();
    let dates: Vec<String> = candles.iter().map(|c| c.ts.date_naive().to_string()).collect();
    let close: Vec<f64> = candles.iter().map(|c| px(c.close, c)).collect();
    let round = |v: Option<f64>| v.map(|x| (x * 1e4).round() / 1e4);
    let line = |v: Vec<Option<f64>>| v.into_iter().map(round).collect::<Vec<_>>();

    let mut overlays = Vec::new();
    let mut add = |on: bool, name: &str, color: &str, values: Vec<Option<f64>>| {
        if on {
            overlays.push(json!({"name": name, "color": color, "values": line(values)}));
        }
    };
    add(o.sma20, "SMA 20", "yellow", sma(&close, 20));
    add(o.sma50, "SMA 50", "sky", sma(&close, 50));
    add(o.sma200, "SMA 200", "peach", sma(&close, 200));
    add(o.ema20, "EMA 20", "pink", ema(&close, 20));
    if o.bollinger {
        let bands = bollinger(&close, 20, 2.0);
        for (name, pick) in [("Upper band", 1usize), ("Lower band", 2)] {
            let values: Vec<Option<f64>> = bands
                .iter()
                .map(|b| b.map(|(m, u, l)| [m, u, l][pick]))
                .collect();
            overlays.push(json!({"name": name, "color": "lavender", "dashed": true, "values": line(values)}));
        }
    }

    let lower_cfg = match lower {
        Lower::None => json!({"kind": "none"}),
        Lower::Rsi => json!({
            "kind": "rsi",
            "lines": [{"name": "RSI 14", "color": "teal", "values": line(rsi(&close, 14)), "levels": [30, 70]}],
        }),
        Lower::Macd => {
            let m = macd(&close, 12, 26, 9);
            json!({
                "kind": "macd",
                "bars": [{"name": "Histogram", "values": line(m.histogram)}],
                "lines": [
                    {"name": "MACD", "color": "blue", "values": line(m.macd)},
                    {"name": "Signal", "color": "peach", "values": line(m.signal)},
                ],
            })
        }
    };

    // Start zoomed to the chosen span; earlier history is a scroll away.
    let first_shown = match span.sessions() {
        // The last few days that traded (candles are dated in UTC, and a
        // session sits within one UTC day on the exchanges people use).
        Some(n) => {
            let mut days: Vec<&String> = dates.iter().collect();
            days.dedup();
            let first_day = days.len().saturating_sub(n);
            days.get(first_day).and_then(|d| dates.iter().position(|x| x == *d)).unwrap_or(0)
        }
        None => {
            let cutoff = candles
                .last()
                .map(|c| (c.ts - chrono::Duration::days(span.days())).date_naive().to_string())
                .unwrap_or_default();
            dates.iter().position(|d| *d >= cutoff).unwrap_or(0)
        }
    };
    let zoom_start = if dates.is_empty() {
        0.0
    } else {
        first_shown as f64 / dates.len() as f64 * 100.0
    };

    json!({
        "dates": dates,
        // ECharts candlesticks are [open, close, low, high].
        "ohlc": candles.iter().map(|c| [px(c.open, c), px(c.close, c), px(c.low, c), px(c.high, c)].map(|v| (v * 1e4).round() / 1e4)).collect::<Vec<_>>(),
        "close": close.iter().map(|v| (v * 1e4).round() / 1e4).collect::<Vec<_>>(),
        "volume": candles.iter().map(|c| c.volume.unwrap_or(0)).collect::<Vec<_>>(),
        "up": candles.iter().map(|c| c.close >= c.open).collect::<Vec<_>>(),
        "style": if candle_style { "candle" } else { "line" },
        "overlays": overlays,
        "lower": lower_cfg,
        "symbol": symbol,
        "zoomStart": zoom_start,
        // For live prices: when each candle starts and how long it lasts;
        // minute candles are labelled with the viewer's local time.
        "times": candles.iter().map(|c| c.ts.timestamp()).collect::<Vec<_>>(),
        "step": span.step(),
        "intraday": span.sessions().is_some(),
        // A redraw of the same span keeps where the viewer has zoomed to.
        "view": span.label(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn candles(n: i64) -> Vec<Candle> {
        (0..n)
            .map(|i| {
                let p = Decimal::from(100 + i % 7);
                Candle {
                    ts: Utc.timestamp_opt(1_700_000_000 + i * 86_400, 0).unwrap(),
                    open: p,
                    high: p + Decimal::ONE,
                    low: p - Decimal::ONE,
                    close: p + Decimal::new(5, 1),
                    volume: Some(1000),
                    adj_factor: None,
                }
            })
            .collect()
    }

    #[test]
    fn config_lines_up_every_series_and_zooms_to_the_span() {
        let c = candles(400);
        let o = Overlays { sma20: true, sma50: false, sma200: true, ema20: false, bollinger: true };
        let cfg = chart_config(&c, "$".into(), Span::M3, true, o, Lower::Macd);
        assert_eq!(cfg["ohlc"].as_array().unwrap().len(), 400);
        let overlays = cfg["overlays"].as_array().unwrap();
        assert_eq!(overlays.len(), 4, "SMA 20, SMA 200 and two bands");
        assert!(overlays.iter().all(|o| o["values"].as_array().unwrap().len() == 400));
        assert!(overlays[1]["values"][198].is_null() && !overlays[1]["values"][199].is_null());
        assert_eq!(cfg["lower"]["kind"], "macd");
        // Three months of 400 days: the view starts about 77% in.
        let zoom = cfg["zoomStart"].as_f64().unwrap();
        assert!((75.0..80.0).contains(&zoom), "{zoom}");
    }

    #[test]
    fn minute_spans_open_on_the_last_sessions() {
        // Three days of five-minute candles, 13:30 to 20:00 UTC.
        let day = |d: i64| (0..78).map(move |i| 1_700_000_000 - 1_700_000_000 % 86_400 + d * 86_400 + 48_600 + i * 300);
        let c: Vec<Candle> = (0..3)
            .flat_map(day)
            .map(|ts| Candle { ts: Utc.timestamp_opt(ts, 0).unwrap(), ..candles(1)[0].clone() })
            .collect();
        let o = Overlays { sma20: true, sma50: false, sma200: false, ema20: false, bollinger: false };
        let one_day = chart_config(&c, "$".into(), Span::D1, true, o, Lower::None);
        // The last day's 78 candles of 234: the view starts two thirds in.
        assert!((one_day["zoomStart"].as_f64().unwrap() - 200.0 / 3.0).abs() < 1e-9);
        assert_eq!(one_day["step"], 60);
        assert_eq!(one_day["intraday"], true);
        assert_eq!(one_day["times"][0], c[0].ts.timestamp());
        assert_eq!(one_day["times"].as_array().unwrap().len(), 234);
        // Five sessions asked of three: everything shows.
        assert_eq!(chart_config(&c, "$".into(), Span::D5, true, o, Lower::None)["zoomStart"], 0.0);
        let daily = chart_config(&candles(400), "$".into(), Span::Y1, true, o, Lower::None);
        assert_eq!((daily["step"].as_i64(), daily["intraday"].as_bool()), (Some(86_400), Some(false)));
    }

    #[test]
    fn draws_prices_as_traded() {
        let mut c = candles(2);
        c[0].adj_factor = Some(0.5);
        let o = Overlays { sma20: false, sma50: false, sma200: false, ema20: false, bollinger: false };
        let cfg = chart_config(&c, "฿".into(), Span::M1, false, o, Lower::None);
        // 100.5 adjusted at half is 201 as traded; the next day is as is.
        assert_eq!(cfg["close"][0], 201.0);
        assert_eq!(cfg["close"][1], 101.5);
        assert_eq!(currency_symbol("THB"), "฿");
        assert_eq!(currency_symbol("XYZ"), "XYZ ");
    }
}
