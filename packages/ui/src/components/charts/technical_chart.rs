//! Interactive price chart for one stock: candles or line, moving averages,
//! Bollinger bands, volume, and an RSI or MACD pane. Indicators are
//! computed here (see `dtos::technicals`); `stock_chart.js` draws them.
//!
//! The chart is live: each streamed price moves the last candle, or starts
//! the next one, between fetches. One- and five-day spans use one- and
//! five-minute candles and fetch again every few minutes.
//!
//! Levels, trend lines and Fibonacci retracements drawn on it are saved
//! with the stock (`api::drawings`); a measure shows the change between
//! two points without being saved.
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
use api::{add_drawing, clear_drawings, delete_drawing, get_drawings};
use dioxus::prelude::*;
use dtos::drawings::{ChartDrawing, ChartPoint, DrawingKind};
use dtos::technicals::{atr, bollinger, ema, macd, obv, rsi, sma, stochastic, vwap};
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
    Stochastic,
    Atr,
    Obv,
}

impl Lower {
    const ALL: [Self; 5] = [Self::Rsi, Self::Macd, Self::Stochastic, Self::Atr, Self::Obv];

    fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Rsi => tr("RSI"),
            Self::Macd => tr("MACD"),
            Self::Stochastic => tr("Stochastic"),
            Self::Atr => tr("ATR"),
            Self::Obv => tr("OBV"),
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::None | Self::Rsi => "teal",
            Self::Macd => "blue",
            Self::Stochastic => "mauve",
            Self::Atr => "flamingo",
            Self::Obv => "sapphire",
        }
    }

    /// What it shows, under the chart.
    fn note(self) -> &'static str {
        match self {
            Self::None | Self::Rsi | Self::Macd => tr("RSI above 70 is often read as overbought and below 30 as oversold; a MACD line crossing above its signal as momentum turning up. Signals, not predictions."),
            Self::Stochastic => tr("Stochastic above 80 is often read as overbought and below 20 as oversold; %K crossing %D as a turn. Signals, not predictions."),
            Self::Atr => tr("ATR is how far the price typically moves in one candle, gaps included: a guide to how wide a stop needs to be."),
            Self::Obv => tr("OBV adds each up candle's volume and takes away each down candle's: rising with the price, it backs the move."),
        }
    }
}

/// Which lines sit on the price.
#[derive(Clone, Copy, PartialEq)]
struct Overlays {
    /// The fast, middle and slow simple averages.
    sma: [bool; 3],
    ema: bool,
    bollinger: bool,
    /// Only on the minute spans, where each session has its own.
    vwap: bool,
}

/// Indicator lengths, in candles; remembered on this device.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Periods {
    sma: [usize; 3],
    ema: usize,
    bollinger: usize,
    /// Standard deviations from the middle band.
    bollinger_width: f64,
    rsi: usize,
    /// Fast, slow and signal.
    macd: [usize; 3],
    /// %K and %D.
    stochastic: [usize; 2],
    atr: usize,
}

impl Default for Periods {
    fn default() -> Self {
        Self {
            sma: [20, 50, 200],
            ema: 20,
            bollinger: 20,
            bollinger_width: 2.0,
            rsi: 14,
            macd: [12, 26, 9],
            stochastic: [14, 3],
            atr: 14,
        }
    }
}

/// The longest period allowed.
const MAX_PERIOD: usize = 500;
/// Where [`Periods`] are kept on this device.
const PERIODS_KEY: &str = "akhsakov.chart.periods";

impl Periods {
    /// Within 1–500 candles and 0.5–5 deviations (a stored copy may not be).
    fn clamped(mut self) -> Self {
        let fix = |n: &mut usize| *n = (*n).clamp(1, MAX_PERIOD);
        self.sma.iter_mut().chain(self.macd.iter_mut()).chain(self.stochastic.iter_mut()).for_each(fix);
        [&mut self.ema, &mut self.bollinger, &mut self.rsi, &mut self.atr].into_iter().for_each(fix);
        self.bollinger_width = if self.bollinger_width.is_finite() { self.bollinger_width.clamp(0.5, 5.0) } else { 2.0 };
        self
    }
}

/// How often the minute spans fetch their candles again.
const REFETCH_MS: u32 = 5 * 60 * 1000;

/// Waits for the chart scripts, which may still be loading the first time.
const WAIT_FOR_SCRIPTS: &str = "for (let i = 0; i < 200 && !(window.StockChart && window.StockChart.listen && window.GrowthChart); i++)
     await new Promise(r => setTimeout(r, 25));";

/// What a click on the chart does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Level,
    Trend,
    Fibonacci,
    Measure,
    Erase,
}

impl Tool {
    const ALL: [Self; 5] = [Self::Level, Self::Trend, Self::Fibonacci, Self::Measure, Self::Erase];

    /// Its name in `stock_chart.js`.
    fn key(self) -> &'static str {
        match self {
            Self::Level => "level",
            Self::Trend => "trend",
            Self::Fibonacci => "fibonacci",
            Self::Measure => "measure",
            Self::Erase => "erase",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Level => tr("Level"),
            Self::Trend => tr("Trend line"),
            Self::Fibonacci => tr("Fibonacci"),
            Self::Measure => tr("Measure"),
            Self::Erase => tr("Erase"),
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::Level => "peach",
            Self::Trend => "sky",
            Self::Fibonacci => "yellow",
            Self::Measure => "green",
            Self::Erase => "red",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Level => tr("Click the chart where the level goes."),
            Self::Trend => tr("Click the line's two ends."),
            Self::Fibonacci => tr("Click where the move starts, then where it ends."),
            Self::Measure => tr("Click two points to see the change between them."),
            Self::Erase => tr("Click a drawing to remove it."),
        }
    }
}

/// A message from the chart: a drawing placed, or one clicked with the eraser.
#[derive(serde::Deserialize)]
struct ChartMessage {
    add: Option<DrawingKind>,
    points: Option<Vec<ChartPoint>>,
    erase: Option<i64>,
}

/// Runs `js` on the chart once its scripts have loaded.
fn chart_call(js: String) {
    let script = format!("{WAIT_FOR_SCRIPTS}\n{js}");
    spawn(async move {
        let _ = document::eval(&script).await;
    });
}

#[component]
pub fn TechnicalChart(ticker: TickerSymbol, live: ReadSignal<Option<LiveTick>>) -> Element {
    let mut span = use_signal(|| Span::Y1);
    let mut candles_style = use_signal(|| true);
    let mut lower = use_signal(|| Lower::Rsi);
    let mut overlays = use_signal(|| Overlays {
        sma: [false, true, true],
        ema: false,
        bollinger: false,
        vwap: false,
    });
    let mut periods = use_signal(Periods::default);
    let mut show_periods = use_signal(|| false);
    use_future(move || async move {
        let read = format!("try {{ return localStorage.getItem({PERIODS_KEY:?}) || ''; }} catch (e) {{ return ''; }}");
        if let Ok(text) = document::eval(&read).join::<String>().await {
            if let Ok(saved) = serde_json::from_str::<Periods>(&text) {
                periods.set(saved.clamped());
            }
        }
    });
    let mut set_periods = move |p: Periods| {
        let p = p.clamped();
        periods.set(p);
        let text = serde_json::to_string(&p).unwrap_or_default();
        document::eval(&format!("try {{ localStorage.setItem({PERIODS_KEY:?}, {}); }} catch (e) {{}}", json!(text)));
    };
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

    // Lines drawn on this stock's chart, and the tool clicks use.
    let mut drawings = use_signal(Vec::<ChartDrawing>::new);
    let mut tool = use_signal(|| None::<Tool>);
    let mut draw_error = use_signal(|| None::<String>);
    let t = ticker.clone();
    use_future(move || {
        let t = t.clone();
        async move {
            if let Ok(list) = get_drawings(t).await {
                drawings.set(list);
            }
        }
    });
    let id = chart_id.clone();
    use_effect(move || {
        let list = json!(*drawings.read());
        chart_call(format!("window.StockChart.setDrawings({}, {list});", json!(id)));
    });
    let id = chart_id.clone();
    use_effect(move || {
        let key = tool().map(Tool::key);
        chart_call(format!("window.StockChart.setTool({}, {});", json!(id), json!(key)));
    });
    // Placed drawings are saved; erased ones deleted.
    let (id, t) = (chart_id.clone(), ticker.clone());
    use_future(move || {
        let (id, t) = (id.clone(), t.clone());
        async move {
            let mut messages = document::eval(&format!(
                "{WAIT_FOR_SCRIPTS}
                 window.StockChart.listen({}, m => dioxus.send(m));
                 await new Promise(() => {{}});",
                json!(id)
            ));
            while let Ok(msg) = messages.recv::<ChartMessage>().await {
                let result = match (msg.add, msg.points, msg.erase) {
                    (Some(kind), Some(points), _) => add_drawing(t.clone(), kind, points).await.map(|d| {
                        drawings.push(d);
                        tool.set(None);
                    }),
                    (_, _, Some(erase)) => delete_drawing(t.clone(), erase).await.map(|()| {
                        drawings.with_mut(|list| list.retain(|d| d.id != erase));
                    }),
                    _ => Ok(()),
                };
                draw_error.set(result.err().map(crate::settings_page::server_message));
            }
        }
    });
    let t = ticker.clone();
    let clear = move |_| {
        let t = t.clone();
        spawn(async move {
            match clear_drawings(t).await {
                Ok(()) => {
                    drawings.set(Vec::new());
                    draw_error.set(None);
                }
                Err(e) => draw_error.set(Some(crate::settings_page::server_message(e))),
            }
        });
    };

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
        let cfg = chart_config(candles, currency_symbol(currency), span(), candles_style(), overlays(), lower(), periods());
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
    let p = periods();
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
                for (i, color) in SMA_COLORS.into_iter().enumerate() {
                    Chip {
                        key: "sma{i}",
                        label: crate::i18n::trf("SMA {}", &[&p.sma[i]]),
                        color,
                        on: o.sma[i],
                        onclick: move |_| overlays.with_mut(|o| o.sma[i] = !o.sma[i]),
                    }
                }
                Chip { label: crate::i18n::trf("EMA {}", &[&p.ema]), color: "pink", on: o.ema, onclick: move |_| overlays.with_mut(|o| o.ema = !o.ema) }
                Chip { label: tr("Bollinger"), color: "lavender", on: o.bollinger, onclick: move |_| overlays.with_mut(|o| o.bollinger = !o.bollinger) }
                if span().sessions().is_some() {
                    Chip { label: tr("VWAP"), color: "rosewater", on: o.vwap, onclick: move |_| overlays.with_mut(|o| o.vwap = !o.vwap) }
                }
                span { class: "ml-3 text-ctp-subtext0", {tr("Lower pane")} }
                for l in Lower::ALL {
                    Chip {
                        key: "{l.label()}",
                        label: l.label(),
                        color: l.color(),
                        on: lower() == l,
                        onclick: move |_| lower.set(if lower() == l { Lower::None } else { l }),
                    }
                }
                button {
                    class: "ml-1 rounded-full px-2.5 py-1 text-ctp-subtext0 underline-offset-2 hover:text-ctp-text hover:underline cursor-pointer",
                    onclick: move |_| show_periods.toggle(),
                    if show_periods() { {tr("Hide periods")} } else { {tr("Periods…")} }
                }
            }
            if show_periods() {
                div { class: "mb-3 grid grid-cols-2 gap-2 text-xs sm:grid-cols-4",
                    for (i, name) in [tr("Fast SMA"), tr("Middle SMA"), tr("Slow SMA")].into_iter().enumerate() {
                        PeriodInput { key: "{name}", label: name, value: p.sma[i] as f64, onchange: move |v: f64| { let mut q = periods(); q.sma[i] = v as usize; set_periods(q); } }
                    }
                    PeriodInput { label: tr("EMA"), value: p.ema as f64, onchange: move |v: f64| set_periods(Periods { ema: v as usize, ..periods() }) }
                    PeriodInput { label: tr("Bollinger"), value: p.bollinger as f64, onchange: move |v: f64| set_periods(Periods { bollinger: v as usize, ..periods() }) }
                    PeriodInput { label: tr("Band width (σ)"), value: p.bollinger_width, step: 0.1, onchange: move |v: f64| set_periods(Periods { bollinger_width: v, ..periods() }) }
                    PeriodInput { label: tr("RSI"), value: p.rsi as f64, onchange: move |v: f64| set_periods(Periods { rsi: v as usize, ..periods() }) }
                    for (i, name) in [tr("MACD fast"), tr("MACD slow"), tr("MACD signal")].into_iter().enumerate() {
                        PeriodInput { key: "{name}", label: name, value: p.macd[i] as f64, onchange: move |v: f64| { let mut q = periods(); q.macd[i] = v as usize; set_periods(q); } }
                    }
                    for (i, name) in [tr("Stochastic %K"), tr("Stochastic %D")].into_iter().enumerate() {
                        PeriodInput { key: "{name}", label: name, value: p.stochastic[i] as f64, onchange: move |v: f64| { let mut q = periods(); q.stochastic[i] = v as usize; set_periods(q); } }
                    }
                    PeriodInput { label: tr("ATR"), value: p.atr as f64, onchange: move |v: f64| set_periods(Periods { atr: v as usize, ..periods() }) }
                    button {
                        class: "rounded-xl border border-ctp-surface0 px-2.5 py-1.5 text-ctp-subtext0 hover:text-ctp-text cursor-pointer",
                        onclick: move |_| set_periods(Periods::default()),
                        {tr("Reset periods")}
                    }
                }
            }
            div { class: "mb-3 flex flex-wrap items-center gap-2 text-xs",
                span { class: "text-ctp-subtext0", {tr("Draw")} }
                for t in Tool::ALL {
                    Chip {
                        key: "{t.key()}",
                        label: t.label(),
                        color: t.color(),
                        on: tool() == Some(t),
                        onclick: move |_| tool.set(if tool() == Some(t) { None } else { Some(t) }),
                    }
                }
                if !drawings.read().is_empty() {
                    crate::page::ConfirmButton {
                        label: tr("Clear drawings"),
                        warning: tr("Every drawing on this chart goes. Continue?"),
                        onconfirm: clear,
                    }
                }
            }
            if let Some(t) = tool() {
                p { class: "mb-2 text-xs text-ctp-subtext0", {t.hint()} }
            }
            if let Some(e) = draw_error() {
                p { class: "mb-2 text-xs text-ctp-red break-words", "{e}" }
            }
            div { class: "relative",
                div { id: "{chart_id}", style: "width:100%;height:460px;" }
                if let Some(status) = status {
                    div { class: "absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-ctp-subtext0 break-words", "{status}" }
                }
            }
            p { class: "mt-2 text-xs text-ctp-overlay1", {lower().note()} }
        }
    }
}

/// Colours of the fast, middle and slow averages.
const SMA_COLORS: [&str; 3] = ["yellow", "sky", "peach"];

/// A labelled number box for one indicator period; applied when it's left
/// or Enter is pressed.
#[component]
fn PeriodInput(label: String, value: f64, #[props(default = 1.0)] step: f64, onchange: EventHandler<f64>) -> Element {
    rsx! {
        label { class: "flex items-center justify-between gap-2 rounded-xl border border-ctp-surface0 px-2.5 py-1.5",
            span { class: "text-ctp-subtext0", "{label}" }
            input {
                class: "w-14 bg-transparent text-right tabular-nums text-ctp-text outline-none",
                r#type: "number",
                min: if step < 1.0 { "0.5" } else { "1" },
                max: "{MAX_PERIOD}",
                step: "{step}",
                value: "{value}",
                onchange: move |e| {
                    if let Ok(v) = e.value().parse::<f64>() {
                        if v.is_finite() && v > 0.0 {
                            onchange.call(v);
                        }
                    }
                },
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
    p: Periods,
) -> Value {
    // Undo the dividend adjustment, back to the price that traded.
    let px = |d: rust_decimal::Decimal, c: &Candle| d.to_f64().unwrap_or(0.0) / c.adjustment();
    let dates: Vec<String> = candles.iter().map(|c| c.ts.date_naive().to_string()).collect();
    let close: Vec<f64> = candles.iter().map(|c| px(c.close, c)).collect();
    let high: Vec<f64> = candles.iter().map(|c| px(c.high, c)).collect();
    let low: Vec<f64> = candles.iter().map(|c| px(c.low, c)).collect();
    let volume: Vec<f64> = candles.iter().map(|c| c.volume.unwrap_or(0) as f64).collect();
    let round = |v: Option<f64>| v.map(|x| (x * 1e4).round() / 1e4);
    let line = |v: Vec<Option<f64>>| v.into_iter().map(round).collect::<Vec<_>>();

    let mut overlays = Vec::new();
    let mut add = |on: bool, name: &str, color: &str, values: Vec<Option<f64>>| {
        if on {
            overlays.push(json!({"name": name, "color": color, "values": line(values)}));
        }
    };
    for (i, color) in SMA_COLORS.into_iter().enumerate() {
        add(o.sma[i], &format!("SMA {}", p.sma[i]), color, sma(&close, p.sma[i]));
    }
    add(o.ema, &format!("EMA {}", p.ema), "pink", ema(&close, p.ema));
    // Each session's own average: candles grouped by their UTC day.
    let sessions: Vec<i64> = candles.iter().map(|c| c.ts.timestamp().div_euclid(86_400)).collect();
    add(o.vwap && span.sessions().is_some(), "VWAP", "rosewater", vwap(&high, &low, &close, &volume, &sessions));
    if o.bollinger {
        let bands = bollinger(&close, p.bollinger, p.bollinger_width);
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
            "bounded": true,
            "lines": [{"name": format!("RSI {}", p.rsi), "color": "teal", "values": line(rsi(&close, p.rsi)), "levels": [30, 70]}],
        }),
        Lower::Stochastic => {
            let (k, d) = stochastic(&high, &low, &close, p.stochastic[0], p.stochastic[1]);
            json!({
                "kind": "stochastic",
                "bounded": true,
                "lines": [
                    {"name": "%K", "color": "mauve", "values": line(k), "levels": [20, 80]},
                    {"name": "%D", "color": "peach", "values": line(d)},
                ],
            })
        }
        Lower::Atr => json!({
            "kind": "atr",
            "lines": [{"name": format!("ATR {}", p.atr), "color": "flamingo", "values": line(atr(&high, &low, &close, p.atr))}],
        }),
        Lower::Obv => json!({
            "kind": "obv",
            "lines": [{"name": "OBV", "color": "sapphire", "values": line(obv(&close, &volume))}],
        }),
        Lower::Macd => {
            let m = macd(&close, p.macd[0], p.macd[1], p.macd[2]);
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
        "measureUnit": tr("candles"),
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
        let o = Overlays { sma: [true, false, true], ema: false, bollinger: true, vwap: false };
        let cfg = chart_config(&c, "$".into(), Span::M3, true, o, Lower::Macd, Periods::default());
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
        let o = Overlays { sma: [true, false, false], ema: false, bollinger: false, vwap: false };
        let one_day = chart_config(&c, "$".into(), Span::D1, true, o, Lower::None, Periods::default());
        // The last day's 78 candles of 234: the view starts two thirds in.
        assert!((one_day["zoomStart"].as_f64().unwrap() - 200.0 / 3.0).abs() < 1e-9);
        assert_eq!(one_day["step"], 60);
        assert_eq!(one_day["intraday"], true);
        assert_eq!(one_day["times"][0], c[0].ts.timestamp());
        assert_eq!(one_day["times"].as_array().unwrap().len(), 234);
        // Five sessions asked of three: everything shows.
        assert_eq!(chart_config(&c, "$".into(), Span::D5, true, o, Lower::None, Periods::default())["zoomStart"], 0.0);
        let daily = chart_config(&candles(400), "$".into(), Span::Y1, true, o, Lower::None, Periods::default());
        assert_eq!((daily["step"].as_i64(), daily["intraday"].as_bool()), (Some(86_400), Some(false)));
    }

    #[test]
    fn periods_and_new_panes_reach_the_config() {
        let c = candles(120);
        let o = Overlays { sma: [true, false, false], ema: true, bollinger: false, vwap: true };
        let p = Periods { sma: [10, 50, 200], ema: 7, stochastic: [5, 3], ..Periods::default() };
        let cfg = chart_config(&c, "$".into(), Span::Y1, true, o, Lower::Stochastic, p);
        let names: Vec<&str> = cfg["overlays"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap()).collect();
        // VWAP only belongs to the minute spans.
        assert_eq!(names, ["SMA 10", "EMA 7"]);
        assert!(cfg["overlays"][0]["values"][8].is_null() && !cfg["overlays"][0]["values"][9].is_null());
        assert_eq!((cfg["lower"]["kind"].as_str(), cfg["lower"]["bounded"].as_bool()), (Some("stochastic"), Some(true)));
        assert!(!cfg["lower"]["lines"][0]["values"][4].is_null());
        let atr_cfg = chart_config(&c, "$".into(), Span::D1, true, o, Lower::Atr, p);
        assert_eq!(atr_cfg["lower"]["lines"][0]["name"], "ATR 14");
        assert!(atr_cfg["overlays"].as_array().unwrap().iter().any(|o| o["name"] == "VWAP"));
        // A stored copy out of range is pulled back in.
        let wild = Periods { sma: [0, 9999, 50], bollinger_width: f64::NAN, ..Periods::default() }.clamped();
        assert_eq!((wild.sma, wild.bollinger_width), ([1, MAX_PERIOD, 50], 2.0));
        let saved: Periods = serde_json::from_str(r#"{"rsi": 9}"#).unwrap();
        assert_eq!((saved.rsi, saved.sma), (9, [20, 50, 200]));
    }

    #[test]
    fn draws_prices_as_traded() {
        let mut c = candles(2);
        c[0].adj_factor = Some(0.5);
        let o = Overlays { sma: [false; 3], ema: false, bollinger: false, vwap: false };
        let cfg = chart_config(&c, "฿".into(), Span::M1, false, o, Lower::None, Periods::default());
        // 100.5 adjusted at half is 201 as traded; the next day is as is.
        assert_eq!(cfg["close"][0], 201.0);
        assert_eq!(cfg["close"][1], 101.5);
        assert_eq!(currency_symbol("THB"), "฿");
        assert_eq!(currency_symbol("XYZ"), "XYZ ");
    }
}
