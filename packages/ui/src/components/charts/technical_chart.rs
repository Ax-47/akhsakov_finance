//! Interactive price chart for one stock, as a trading screen shows it:
//! a candle length and a span picked apart, six ways to draw the price,
//! averages, bands and stops over it, volume under it, and up to three
//! indicator panes. Indicators are computed here (`dtos::technicals`,
//! `dtos::signals`); `stock_chart.js` draws them.
//!
//! The chart is live. Each streamed trade moves the last candle, or starts
//! the next one, and every indicator's newest value with it
//! ([`dtos::chart::Bars`] keeps the candles; a trade outside the hours
//! they trade in is left out). Minute candles are fetched again every
//! minute, longer ones less often, so volume and history stay exact.
//!
//! Levels, trend lines and Fibonacci retracements drawn on it are saved
//! with the stock (`api::drawings`); a measure shows the change between
//! two points without being saved. The viewer's own buys and sales are
//! marked on the candles they fell on, with a line at the average cost of
//! what's still held.
//!
//! Prices are the ones actually traded, in the stock's own currency, as a
//! broker shows them: converting a Thai stock's history to dollars (or
//! back) at each day's rate would mix exchange-rate moves into the price,
//! and dividend-adjusted history would put past prices, and so the moving
//! averages and bands, below where the stock really traded.

use crate::components::card::{Card, Segmented, ToggleButton};
use crate::hooks::LiveTick;
use crate::i18n::{tr, trf};
use api::quote::quote::get_session_chart;
use api::{add_drawing, clear_drawings, delete_drawing, get_drawings};
use dioxus::prelude::*;
use dtos::chart::{average_cost, sessions, Applied, Bars, Clock, SessionChart};
use dtos::drawings::{ChartDrawing, ChartPoint, DrawingKind};
use dtos::portfolio::GetDashBoardResponse;
use dtos::signals::{adx, cci, williams_r};
use dtos::technicals::{
    atr, bollinger, donchian, ema, heikin_ashi, macd, mfi, obv, parabolic_sar, rsi, sma, stochastic, supertrend, vwap,
};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use types::{candle::Candle, interval::Interval, range::Range, ticker_symbol::TickerSymbol, transaction_type::TransactionType};

static CHART_CTR: AtomicUsize = AtomicUsize::new(0);

const DAY: i64 = 86_400;

/// How long one candle lasts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Frame {
    M1,
    M5,
    M15,
    M30,
    H1,
    D1,
    W1,
    Mo1,
}

impl Frame {
    const ALL: [Self; 8] = [Self::M1, Self::M5, Self::M15, Self::M30, Self::H1, Self::D1, Self::W1, Self::Mo1];

    /// Candles shown at first when a length is picked on its own.
    const SHOWN: usize = 160;

    fn label(self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::D1 => "D",
            Self::W1 => "W",
            Self::Mo1 => "M",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::M1 => tr("1-minute candles"),
            Self::M5 => tr("5-minute candles"),
            Self::M15 => tr("15-minute candles"),
            Self::M30 => tr("30-minute candles"),
            Self::H1 => tr("Hourly candles"),
            Self::D1 => tr("Daily candles"),
            Self::W1 => tr("Weekly candles"),
            Self::Mo1 => tr("Monthly candles"),
        }
    }

    /// Seconds each candle covers.
    fn step(self) -> i64 {
        match self {
            Self::M1 => 60,
            Self::M5 => 300,
            Self::M15 => 900,
            Self::M30 => 1800,
            Self::H1 => 3600,
            Self::D1 => DAY,
            Self::W1 => 7 * DAY,
            Self::Mo1 => 30 * DAY,
        }
    }

    fn intraday(self) -> bool {
        self.step() < DAY
    }

    /// What the averages count, e.g. "days".
    fn unit(self) -> &'static str {
        match self {
            Self::M1 => tr("minutes"),
            Self::M5 => tr("5-minute candles"),
            Self::M15 => tr("15-minute candles"),
            Self::M30 => tr("30-minute candles"),
            Self::H1 => tr("hours"),
            Self::D1 => tr("days"),
            Self::W1 => tr("weeks"),
            Self::Mo1 => tr("months"),
        }
    }

    /// History fetched: as much as the provider keeps at this length, or
    /// enough to warm up a 200-candle average behind any span shown.
    fn fetch(self) -> (Range, Interval) {
        match self {
            Self::M1 => (Range::D5, Interval::I1m),
            Self::M5 => (Range::M1, Interval::I5m),
            Self::M15 => (Range::M1, Interval::I15m),
            Self::M30 => (Range::M1, Interval::I30m),
            Self::H1 => (Range::M6, Interval::I1h),
            Self::D1 => (Range::Y5, Interval::D1),
            Self::W1 => (Range::Y10, Interval::W1),
            Self::Mo1 => (Range::Max, Interval::M1),
        }
    }

    /// How often the candles are fetched again.
    fn refetch_ms(self) -> u32 {
        match self {
            Self::M1 => 60_000,
            Self::M5 | Self::M15 | Self::M30 | Self::H1 => 5 * 60_000,
            _ => 15 * 60_000,
        }
    }
}

/// How far back the chart opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Reach {
    D1,
    D5,
    M1,
    M3,
    M6,
    Ytd,
    Y1,
    Y5,
    All,
}

/// Fewer candles than this across the chart draw as slabs: a span opens
/// on at least this many, reaching back into the session before.
const MIN_SHOWN: usize = 60;

impl Reach {
    const ALL: [Self; 9] = [Self::D1, Self::D5, Self::M1, Self::M3, Self::M6, Self::Ytd, Self::Y1, Self::Y5, Self::All];

    fn label(self) -> &'static str {
        match self {
            Self::D1 => "1D",
            Self::D5 => "5D",
            Self::M1 => "1M",
            Self::M3 => "3M",
            Self::M6 => "6M",
            Self::Ytd => "YTD",
            Self::Y1 => "1Y",
            Self::Y5 => "5Y",
            Self::All => "All",
        }
    }

    /// The candle length a span is read in.
    fn frame(self) -> Frame {
        match self {
            Self::D1 => Frame::M1,
            Self::D5 => Frame::M5,
            Self::M1 => Frame::H1,
            Self::M3 | Self::M6 | Self::Ytd | Self::Y1 => Frame::D1,
            Self::Y5 => Frame::W1,
            Self::All => Frame::Mo1,
        }
    }

    /// The first candle shown, of candles starting at `times`.
    fn first_shown(self, times: &[i64]) -> usize {
        let Some(&last) = times.last() else {
            return 0;
        };
        let from = |cutoff: i64| times.iter().position(|t| *t >= cutoff).unwrap_or(0);
        // The last few sessions: runs of candles with a night between
        // them, so trading before the open and after the close stays with
        // its day wherever midnight falls.
        let last_sessions = |n: usize| {
            let runs = sessions(times);
            let first = runs[runs.len().saturating_sub(n)].0;
            from(first).min(times.len().saturating_sub(MIN_SHOWN))
        };
        match self {
            Self::D1 => last_sessions(1),
            Self::D5 => last_sessions(5),
            Self::M1 => from(last - 31 * DAY),
            Self::M3 => from(last - 92 * DAY),
            Self::M6 => from(last - 183 * DAY),
            Self::Y1 => from(last - 366 * DAY),
            Self::Y5 => from(last - 1827 * DAY),
            Self::Ytd => {
                use chrono::{Datelike, TimeZone};
                let year = chrono::Utc.timestamp_opt(last, 0).single().map_or(1970, |d| d.year());
                let start = chrono::Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).single().map_or(0, |d| d.timestamp());
                from(start)
            }
            Self::All => 0,
        }
    }
}

/// How the price is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
enum Style {
    #[default]
    Candles,
    /// Rising candles left empty.
    Hollow,
    HeikinAshi,
    /// Open and close as ticks on a high–low line.
    Bars,
    Line,
    Area,
}

impl Style {
    const ALL: [Self; 6] = [Self::Candles, Self::Hollow, Self::HeikinAshi, Self::Bars, Self::Line, Self::Area];

    fn label(self) -> &'static str {
        match self {
            Self::Candles => tr("Candles"),
            Self::Hollow => tr("Hollow candles"),
            Self::HeikinAshi => tr("Heikin-Ashi"),
            Self::Bars => tr("Bars"),
            Self::Line => tr("Line"),
            Self::Area => tr("Area"),
        }
    }

    /// Its name in `stock_chart.js`.
    fn key(self) -> &'static str {
        match self {
            Self::Candles | Self::HeikinAshi => "candle",
            Self::Hollow => "hollow",
            Self::Bars => "bars",
            Self::Line => "line",
            Self::Area => "area",
        }
    }

    fn from_label(key: &str) -> Self {
        Self::ALL.into_iter().find(|s| format!("{s:?}") == key).unwrap_or_default()
    }
}

/// An indicator with a pane of its own under the price.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum Lower {
    Rsi,
    Macd,
    Stochastic,
    Atr,
    Obv,
    Adx,
    Cci,
    Williams,
    Mfi,
}

/// Panes shown at once: more leave too little of the price.
const MAX_PANES: usize = 3;

impl Lower {
    const ALL: [Self; 9] =
        [Self::Rsi, Self::Macd, Self::Stochastic, Self::Atr, Self::Obv, Self::Adx, Self::Cci, Self::Williams, Self::Mfi];

    fn label(self) -> &'static str {
        match self {
            Self::Rsi => tr("RSI"),
            Self::Macd => tr("MACD"),
            Self::Stochastic => tr("Stochastic"),
            Self::Atr => tr("ATR"),
            Self::Obv => tr("OBV"),
            Self::Adx => tr("ADX"),
            Self::Cci => tr("CCI"),
            Self::Williams => tr("Williams %R"),
            Self::Mfi => tr("MFI"),
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::Rsi => "teal",
            Self::Macd => "blue",
            Self::Stochastic => "mauve",
            Self::Atr => "flamingo",
            Self::Obv => "sapphire",
            Self::Adx => "yellow",
            Self::Cci => "sky",
            Self::Williams => "pink",
            Self::Mfi => "green",
        }
    }

    /// What it shows, under the chart.
    fn note(self) -> &'static str {
        match self {
            Self::Rsi | Self::Macd => tr("RSI above 70 is often read as overbought and below 30 as oversold; a MACD line crossing above its signal as momentum turning up. Signals, not predictions."),
            Self::Stochastic => tr("Stochastic above 80 is often read as overbought and below 20 as oversold; %K crossing %D as a turn. Signals, not predictions."),
            Self::Atr => tr("ATR is how far the price typically moves in one candle, gaps included: a guide to how wide a stop needs to be."),
            Self::Obv => tr("OBV adds each up candle's volume and takes away each down candle's: rising with the price, it backs the move."),
            Self::Adx => tr("ADX is how strong the trend is, whichever way it points: above 25 is usually called a trend. +DI above −DI says the trend is up."),
            Self::Cci => tr("CCI is how far the price sits from its average, in typical deviations: beyond ±100 it is stretched."),
            Self::Williams => tr("Williams %R is where the close sits in the recent range, from −100 (the low) to 0 (the high): above −20 is often read as overbought, below −80 as oversold."),
            Self::Mfi => tr("MFI is RSI with volume in it: above 80 is often read as overbought and below 20 as oversold."),
        }
    }
}

/// Which lines sit on the price.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Overlays {
    /// The fast, middle and slow simple averages.
    sma: [bool; 3],
    ema: bool,
    bollinger: bool,
    /// Only on the minute lengths, where each session has its own.
    vwap: bool,
    donchian: bool,
    supertrend: bool,
    psar: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self { sma: [false, true, true], ema: false, bollinger: false, vwap: false, donchian: false, supertrend: false, psar: false }
    }
}

impl Overlays {
    fn count(&self) -> usize {
        self.sma.iter().filter(|on| **on).count()
            + [self.ema, self.bollinger, self.vwap, self.donchian, self.supertrend, self.psar].into_iter().filter(|on| *on).count()
    }
}

/// How the chart is set up; remembered on this device.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Prefs {
    style: Style,
    overlays: Overlays,
    lowers: Vec<Lower>,
    volume: bool,
    /// Equal distances for equal percentage moves.
    log: bool,
    /// Trading before the open and after the close, on minute lengths.
    extended: bool,
    /// The viewer's own buys and sales, and their average cost.
    trades: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            style: Style::default(),
            overlays: Overlays::default(),
            lowers: vec![Lower::Rsi],
            volume: true,
            log: false,
            extended: false,
            trades: true,
        }
    }
}

impl Prefs {
    /// A stored copy may list a pane twice or too many of them.
    fn cleaned(mut self) -> Self {
        let mut seen = Vec::new();
        self.lowers.retain(|l| !seen.contains(l) && {
            seen.push(*l);
            true
        });
        self.lowers.truncate(MAX_PANES);
        self
    }

    /// Shows or hides a pane; a fourth takes the place of the oldest.
    fn toggle_pane(&mut self, pane: Lower) {
        if let Some(at) = self.lowers.iter().position(|l| *l == pane) {
            self.lowers.remove(at);
        } else {
            if self.lowers.len() >= MAX_PANES {
                self.lowers.remove(0);
            }
            self.lowers.push(pane);
        }
    }
}

/// Indicator lengths, in candles; remembered on this device.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
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
    donchian: usize,
    supertrend: usize,
    /// ATRs between the price and the Supertrend stop.
    supertrend_width: f64,
    adx: usize,
    cci: usize,
    williams: usize,
    mfi: usize,
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
            donchian: 20,
            supertrend: 10,
            supertrend_width: 3.0,
            adx: 14,
            cci: 20,
            williams: 14,
            mfi: 14,
        }
    }
}

/// The longest period allowed.
const MAX_PERIOD: usize = 500;
/// Where [`Periods`] and [`Prefs`] are kept on this device.
const PERIODS_KEY: &str = "akhsakov.chart.periods";
const PREFS_KEY: &str = "akhsakov.chart.view";

impl Periods {
    /// Within 1–500 candles and 0.5–5 deviations (a stored copy may not be).
    fn clamped(mut self) -> Self {
        let fix = |n: &mut usize| *n = (*n).clamp(1, MAX_PERIOD);
        self.sma.iter_mut().chain(self.macd.iter_mut()).chain(self.stochastic.iter_mut()).for_each(fix);
        [
            &mut self.ema,
            &mut self.bollinger,
            &mut self.rsi,
            &mut self.atr,
            &mut self.donchian,
            &mut self.supertrend,
            &mut self.adx,
            &mut self.cci,
            &mut self.williams,
            &mut self.mfi,
        ]
        .into_iter()
        .for_each(fix);
        let width = |w: f64, default: f64| if w.is_finite() { w.clamp(0.5, 5.0) } else { default };
        self.bollinger_width = width(self.bollinger_width, 2.0);
        self.supertrend_width = width(self.supertrend_width, 3.0);
        self
    }
}

/// How often the refetch clock is looked at.
const REFETCH_TICK_MS: u32 = 30_000;

/// Waits for the chart scripts, which may still be loading the first time.
const WAIT_FOR_SCRIPTS: &str = "for (let i = 0; i < 200 && !(window.StockChart && window.StockChart.patch && window.GrowthChart); i++)
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

/// A message from the chart: a drawing placed, one clicked with the
/// eraser, or Esc pressed (dropping the tool, or leaving full screen).
#[derive(Deserialize)]
struct ChartMessage {
    add: Option<DrawingKind>,
    points: Option<Vec<ChartPoint>>,
    erase: Option<i64>,
    #[serde(default)]
    cancel: bool,
    #[serde(default)]
    exit: bool,
}

/// Runs `js` on the chart once its scripts have loaded.
fn chart_call(js: String) {
    let script = format!("{WAIT_FOR_SCRIPTS}\n{js}");
    spawn(async move {
        let _ = document::eval(&script).await;
    });
}

/// One of the viewer's trades in this stock.
#[derive(Clone, Debug, PartialEq)]
struct Trade {
    /// Midnight UTC of the day it was made.
    day: i64,
    bought: bool,
    shares: f64,
    /// In the stock's own currency.
    price: f64,
}

#[component]
pub fn TechnicalChart(ticker: TickerSymbol, live: ReadSignal<Option<LiveTick>>) -> Element {
    let mut frame = use_signal(|| Frame::D1);
    let mut reach = use_signal(|| Some(Reach::Y1));
    let mut prefs = use_signal(Prefs::default);
    let mut periods = use_signal(Periods::default);
    let mut show_studies = use_signal(|| false);
    let mut show_periods = use_signal(|| false);
    let mut full = use_signal(|| false);

    // What this device remembers; nothing is saved until it has been read.
    let mut restored = use_signal(|| false);
    use_future(move || async move {
        let read = format!(
            "try {{ return [localStorage.getItem({PERIODS_KEY:?}) || '', localStorage.getItem({PREFS_KEY:?}) || '']; }} catch (e) {{ return ['', '']; }}"
        );
        if let Ok([saved_periods, saved_prefs]) = document::eval(&read).join::<[String; 2]>().await {
            if let Ok(saved) = serde_json::from_str::<Periods>(&saved_periods) {
                periods.set(saved.clamped());
            }
            if let Ok(saved) = serde_json::from_str::<Prefs>(&saved_prefs) {
                prefs.set(saved.cleaned());
            }
        }
        restored.set(true);
    });
    use_effect(move || {
        let (p, v) = (periods(), prefs());
        if !restored() {
            return;
        }
        let (p, v) = (serde_json::to_string(&p).unwrap_or_default(), serde_json::to_string(&v).unwrap_or_default());
        document::eval(&format!(
            "try {{ localStorage.setItem({PERIODS_KEY:?}, {}); localStorage.setItem({PREFS_KEY:?}, {}); }} catch (e) {{}}",
            json!(p),
            json!(v)
        ));
    });
    let chart_id = use_hook(|| format!("stock-chart-{}", CHART_CTR.fetch_add(1, Ordering::Relaxed)));

    // Bumped to fetch the candles again.
    let mut refetch = use_signal(|| 0_u32);
    use_future(move || async move {
        let mut waited = 0_u32;
        loop {
            crate::notify::poll_delay(REFETCH_TICK_MS).await;
            waited += REFETCH_TICK_MS;
            if waited >= frame.peek().refetch_ms() {
                waited = 0;
                refetch += 1;
            }
        }
    });
    // Trading outside regular hours only exists on the minute lengths.
    let extended = use_memo(move || prefs.read().extended && frame().intraday());
    let (t, key) = (ticker.clone(), ticker.clone());
    let history = crate::cache::use_cached(
        move || format!("chart/{key}/{:?}/{}", frame(), extended()),
        move || {
            let _ = refetch();
            let (t, f, ext) = (t.clone(), frame(), extended());
            async move {
                let (range, interval) = f.fetch();
                get_session_chart(t, range, interval, ext).await.map_err(|e| e.to_string())
            }
        },
    );

    // The candles on screen: the fetched ones, moved on by streamed trades.
    let mut bars = use_signal(Bars::default);
    let mut clock = use_signal(|| Clock { step: DAY, hours: None });
    // Whose candles `bars` holds, and a count bumped each time a fetch
    // (not a trade) replaces them.
    let mut shown = use_signal(|| None::<(Frame, bool)>);
    let mut loaded = use_signal(|| 0_u32);
    use_effect(move || {
        let Some(Ok(chart)) = &*history.read() else {
            return;
        };
        let (f, ext) = (*frame.peek(), *extended.peek());
        let mut next = to_bars(&chart.candles);
        next.align_last(f.step());
        let hours = Clock::of(&next.times, f.step());
        // A trade newer than the fetch is already in.
        if let Some(tick) = *live.peek() {
            apply_tick(&mut next, &tick, &hours);
        }
        bars.set(next);
        clock.set(hours);
        shown.set(Some((f, ext)));
        loaded += 1;
    });

    // The viewer's own trades in this stock, oldest first.
    let data = try_use_context::<Signal<GetDashBoardResponse>>();
    let t = ticker.clone();
    let trades = use_memo(move || data.map(|d| own_trades(&d.read().without_ai(), &t)).unwrap_or_default());

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
    let id = chart_id.clone();
    use_effect(move || {
        let on = full();
        // Focused, so the arrow keys work at once.
        chart_call(format!(
            "window.StockChart.setFull({id}, {on}); if ({on}) {{ const el = document.getElementById({id}); if (el) el.focus(); }}",
            id = json!(id)
        ));
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
                if msg.cancel {
                    tool.set(None);
                    continue;
                }
                if msg.exit {
                    full.set(false);
                    continue;
                }
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

    // Draws from scratch: new candles, another span, a setting changed.
    let id = chart_id.clone();
    use_effect(move || {
        let _ = loaded();
        let (f, r, view, p, own) = (frame(), reach(), prefs(), periods(), trades());
        // The candles of the length just picked haven't arrived yet.
        if *shown.peek() != Some((f, view.extended && f.intraday())) {
            return;
        }
        let Some(Ok(chart)) = &*history.peek() else {
            return;
        };
        let b = bars.peek();
        if b.is_empty() {
            return;
        }
        let cfg = chart_config(&b, &studies(&b, f, &view, &p), f, r, &view, &Marks::of(&b, chart, f, &own, &view));
        chart_call(format!("window.StockChart.init({}, {cfg});", json!(id)));
    });

    // Each streamed trade moves the candles, and the drawn chart with them.
    let id = chart_id.clone();
    use_effect(move || {
        let Some(tick) = live() else {
            return;
        };
        let Some((f, _)) = *shown.peek() else {
            return;
        };
        let hours = *clock.peek();
        let Some(applied) = bars.with_mut(|b| apply_tick(b, &tick, &hours)) else {
            return;
        };
        let b = bars.peek();
        let patch = chart_patch(&b, &studies(&b, f, &prefs.peek(), &periods.peek()), applied);
        document::eval(&format!(
            "window.StockChart && window.StockChart.patch && window.StockChart.patch({}, {patch});",
            json!(id)
        ));
    });

    let view = prefs();
    let o = view.overlays;
    let p = periods();
    let f = frame();
    let unit = f.unit();
    let status = match &*history.read() {
        None => Some(tr("Loading chart…").to_string()),
        // The raw error is a server message with the data source's URL in
        // it: not something to show, and too long to fit on a phone.
        Some(Err(_)) => Some(tr("Couldn't load prices right now. The data source may be busy; try again shortly.").to_string()),
        Some(Ok(c)) if c.candles.is_empty() => Some(tr("No price history.").to_string()),
        _ => None,
    };
    let currency = match &*history.read() {
        Some(Ok(c)) => c.currency.clone(),
        _ => String::new(),
    };
    let waiting = status.is_some();
    let studies_on = o.count() + view.lowers.len();
    let panes = view.lowers.len() as u32;
    // Room for the price whatever is under it; full screen fills the window.
    let height = if full() {
        "height:max(360px, calc(100vh - 13.5rem));".to_string()
    } else {
        format!("height:{}px;", 430 + 104 * panes)
    };
    let command = {
        let id = chart_id.clone();
        move |name: &'static str| chart_call(format!("window.StockChart.command({}, {});", json!(id), json!(name)))
    };
    let (zoom_out, zoom_in, to_latest, reset) = (command.clone(), command.clone(), command.clone(), command);
    let (png_id, png_name) = (chart_id.clone(), ticker.to_string());
    let save_png = move |_| {
        let (id, name) = (png_id.clone(), png_name.clone());
        spawn(async move {
            let script = format!("{WAIT_FOR_SCRIPTS}\nreturn window.StockChart.png({}) || '';", json!(id));
            if let Ok(url) = document::eval(&script).join::<String>().await {
                if let Some(image) = url.strip_prefix("data:image/png;base64,") {
                    crate::files::download_base64(&format!("{name}-chart.png"), "image/png", image);
                }
            }
        });
    };

    rsx! {
        document::Script { src: asset!("/assets/js/growth_chart.js") }
        document::Script { src: asset!("/assets/js/stock_chart.js") }
        // `min-w-0`: as an item of the page's grid it would otherwise grow
        // to its widest row and push the card off a phone's screen.
        div { class: if full() { "fixed inset-0 z-40 overflow-y-auto bg-ctp-base p-2 sm:p-4" } else { "min-w-0" },
            Card {
                title: tr("Chart"),
                subtitle: if currency.is_empty() {
                    trf("Drag to pan, Ctrl + scroll or pinch to zoom, double-click to reset · averages in {}", &[&unit])
                } else {
                    trf("Drag to pan, Ctrl + scroll or pinch to zoom, double-click to reset · prices in {} · averages in {}", &[&currency, &unit])
                },
                actions: rsx! {
                    Segmented {
                        for r in Reach::ALL {
                            ToggleButton {
                                key: "{r.label()}",
                                label: tr(r.label()),
                                active: reach() == Some(r),
                                onclick: move |_| {
                                    reach.set(Some(r));
                                    frame.set(r.frame());
                                },
                            }
                        }
                    }
                },
                div { class: "mb-2 flex flex-wrap items-center gap-x-1.5 gap-y-2 text-xs",
                    div { class: "inline-flex max-w-full gap-0.5 overflow-x-auto rounded-xl border border-ctp-surface0 bg-ctp-crust/40 p-0.5", role: "group", "aria-label": tr("Candle length"),
                        for candle in Frame::ALL {
                            Key {
                                key: "{candle.label()}",
                                label: tr(candle.label()),
                                title: candle.title(),
                                active: f == candle,
                                onclick: move |_| {
                                    if frame() != candle {
                                        frame.set(candle);
                                        // A span belongs to the length it was read in.
                                        if reach().is_some_and(|r| r.frame() != candle) {
                                            reach.set(None);
                                        }
                                    }
                                },
                            }
                        }
                    }
                    select {
                        class: "min-h-7 rounded-xl border border-ctp-surface0 bg-ctp-mantle px-2 py-1 text-xs text-ctp-text cursor-pointer",
                        "aria-label": tr("Chart type"),
                        onchange: move |e| prefs.with_mut(|v| v.style = Style::from_label(&e.value())),
                        for s in Style::ALL {
                            option { key: "{s:?}", value: "{s:?}", selected: view.style == s, {s.label()} }
                        }
                    }
                    Key {
                        label: if studies_on > 0 { trf("Indicators · {}", &[&studies_on]) } else { tr("Indicators").to_string() },
                        title: tr("Averages, bands and stops on the price, and the panes under it"),
                        active: show_studies(),
                        onclick: move |_| show_studies.toggle(),
                    }
                    Key {
                        label: tr("Log"),
                        title: tr("Logarithmic scale: equal distances for equal percentage moves"),
                        active: view.log,
                        onclick: move |_| prefs.with_mut(|v| v.log = !v.log),
                    }
                    if f.intraday() {
                        Key {
                            label: tr("Ext. hours"),
                            title: tr("Show trading before the open and after the close, on a shaded ground"),
                            active: view.extended,
                            onclick: move |_| prefs.with_mut(|v| v.extended = !v.extended),
                        }
                    }
                    if !trades.read().is_empty() {
                        Key {
                            label: tr("My trades"),
                            title: tr("Mark your buys and sales on the candles, and your average cost"),
                            active: view.trades,
                            onclick: move |_| prefs.with_mut(|v| v.trades = !v.trades),
                        }
                    }
                    span { class: "flex-1" }
                    div { class: "inline-flex gap-0.5",
                        Key { label: "−", title: tr("Zoom out (↓)"), onclick: move |_| zoom_out("out") }
                        Key { label: "+", title: tr("Zoom in (↑)"), onclick: move |_| zoom_in("in") }
                        Key { label: "⇥", title: tr("Go to the latest candle (End)"), onclick: move |_| to_latest("latest") }
                        Key { label: "⟲", title: tr("Back to the span first shown (Home)"), onclick: move |_| reset("reset") }
                        Key { label: tr("PNG"), title: tr("Save the chart as an image"), onclick: save_png }
                        Key {
                            label: if full() { tr("Exit full screen") } else { tr("Full screen") },
                            title: tr("Fill the window with the chart (Esc leaves)"),
                            active: full(),
                            onclick: move |_| full.toggle(),
                        }
                    }
                }
                if show_studies() {
                    div { class: "mb-3 grid gap-2 rounded-2xl border border-ctp-surface0/70 bg-ctp-crust/30 p-3 text-xs",
                        div { class: "flex flex-wrap items-center gap-2",
                            span { class: "w-full text-ctp-subtext0 sm:w-24", {tr("On the price")} }
                            for (i, color) in SMA_COLORS.into_iter().enumerate() {
                                Chip {
                                    key: "sma{i}",
                                    label: trf("SMA {}", &[&p.sma[i]]),
                                    color,
                                    on: o.sma[i],
                                    onclick: move |_| prefs.with_mut(|v| v.overlays.sma[i] = !v.overlays.sma[i]),
                                }
                            }
                            Chip { label: trf("EMA {}", &[&p.ema]), color: "pink", on: o.ema, onclick: move |_| prefs.with_mut(|v| v.overlays.ema = !v.overlays.ema) }
                            Chip { label: tr("Bollinger"), color: "lavender", on: o.bollinger, onclick: move |_| prefs.with_mut(|v| v.overlays.bollinger = !v.overlays.bollinger) }
                            Chip { label: tr("Donchian"), color: "sapphire", on: o.donchian, onclick: move |_| prefs.with_mut(|v| v.overlays.donchian = !v.overlays.donchian) }
                            Chip { label: tr("Supertrend"), color: "green", on: o.supertrend, onclick: move |_| prefs.with_mut(|v| v.overlays.supertrend = !v.overlays.supertrend) }
                            Chip { label: tr("Parabolic SAR"), color: "flamingo", on: o.psar, onclick: move |_| prefs.with_mut(|v| v.overlays.psar = !v.overlays.psar) }
                            if f.intraday() {
                                Chip { label: tr("VWAP"), color: "rosewater", on: o.vwap, onclick: move |_| prefs.with_mut(|v| v.overlays.vwap = !v.overlays.vwap) }
                            }
                            Chip { label: tr("Volume"), color: "overlay1", on: view.volume, onclick: move |_| prefs.with_mut(|v| v.volume = !v.volume) }
                        }
                        div { class: "flex flex-wrap items-center gap-2",
                            span { class: "w-full text-ctp-subtext0 sm:w-24", title: tr("Up to three at once; a fourth takes the place of the oldest"), {tr("Panes below")} }
                            for l in Lower::ALL {
                                Chip {
                                    key: "{l:?}",
                                    label: l.label(),
                                    color: l.color(),
                                    on: view.lowers.contains(&l),
                                    onclick: move |_| prefs.with_mut(|v| v.toggle_pane(l)),
                                }
                            }
                            button {
                                r#type: "button",
                                class: "ml-1 rounded-full px-2.5 py-1 text-ctp-subtext0 underline-offset-2 hover:text-ctp-text hover:underline cursor-pointer",
                                onclick: move |_| show_periods.toggle(),
                                if show_periods() { {tr("Hide periods")} } else { {tr("Periods…")} }
                            }
                        }
                        if show_periods() {
                            div { class: "grid grid-cols-2 gap-2 sm:grid-cols-4",
                                for (i, name) in [tr("Fast SMA"), tr("Middle SMA"), tr("Slow SMA")].into_iter().enumerate() {
                                    PeriodInput { key: "{name}", label: name, value: p.sma[i] as f64, onchange: move |v: f64| periods.with_mut(|q| q.sma[i] = v as usize) }
                                }
                                PeriodInput { label: tr("EMA"), value: p.ema as f64, onchange: move |v: f64| periods.with_mut(|q| q.ema = v as usize) }
                                PeriodInput { label: tr("Bollinger"), value: p.bollinger as f64, onchange: move |v: f64| periods.with_mut(|q| q.bollinger = v as usize) }
                                PeriodInput { label: tr("Band width (σ)"), value: p.bollinger_width, step: 0.1, onchange: move |v: f64| periods.with_mut(|q| q.bollinger_width = v) }
                                PeriodInput { label: tr("Donchian"), value: p.donchian as f64, onchange: move |v: f64| periods.with_mut(|q| q.donchian = v as usize) }
                                PeriodInput { label: tr("Supertrend"), value: p.supertrend as f64, onchange: move |v: f64| periods.with_mut(|q| q.supertrend = v as usize) }
                                PeriodInput { label: tr("Supertrend width (ATR)"), value: p.supertrend_width, step: 0.1, onchange: move |v: f64| periods.with_mut(|q| q.supertrend_width = v) }
                                PeriodInput { label: tr("RSI"), value: p.rsi as f64, onchange: move |v: f64| periods.with_mut(|q| q.rsi = v as usize) }
                                for (i, name) in [tr("MACD fast"), tr("MACD slow"), tr("MACD signal")].into_iter().enumerate() {
                                    PeriodInput { key: "{name}", label: name, value: p.macd[i] as f64, onchange: move |v: f64| periods.with_mut(|q| q.macd[i] = v as usize) }
                                }
                                for (i, name) in [tr("Stochastic %K"), tr("Stochastic %D")].into_iter().enumerate() {
                                    PeriodInput { key: "{name}", label: name, value: p.stochastic[i] as f64, onchange: move |v: f64| periods.with_mut(|q| q.stochastic[i] = v as usize) }
                                }
                                PeriodInput { label: tr("ATR"), value: p.atr as f64, onchange: move |v: f64| periods.with_mut(|q| q.atr = v as usize) }
                                PeriodInput { label: tr("ADX"), value: p.adx as f64, onchange: move |v: f64| periods.with_mut(|q| q.adx = v as usize) }
                                PeriodInput { label: tr("CCI"), value: p.cci as f64, onchange: move |v: f64| periods.with_mut(|q| q.cci = v as usize) }
                                PeriodInput { label: tr("Williams %R"), value: p.williams as f64, onchange: move |v: f64| periods.with_mut(|q| q.williams = v as usize) }
                                PeriodInput { label: tr("MFI"), value: p.mfi as f64, onchange: move |v: f64| periods.with_mut(|q| q.mfi = v as usize) }
                                button {
                                    r#type: "button",
                                    class: "rounded-xl border border-ctp-surface0 px-2.5 py-1.5 text-ctp-subtext0 hover:text-ctp-text cursor-pointer",
                                    onclick: move |_| periods.set(Periods::default()),
                                    {tr("Reset periods")}
                                }
                            }
                        }
                    }
                }
                div { class: "mb-2 flex flex-wrap items-center gap-2 text-xs",
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
                    if let Some(t) = tool() {
                        span { class: "text-ctp-subtext0", {t.hint()} }
                    }
                }
                if let Some(e) = draw_error() {
                    p { class: "mb-2 text-xs text-ctp-red break-words", "{e}" }
                }
                div { class: "relative",
                    // While other candles load, the ones drawn stay, dimmed.
                    div { id: "{chart_id}", class: if waiting { "opacity-30 transition-opacity" } else { "transition-opacity" }, style: "width:100%;{height}" }
                    div { id: "{chart_id}-legend", class: "pointer-events-none absolute inset-0 overflow-hidden", "aria-hidden": "true" }
                    if let Some(status) = status {
                        div { class: "absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-ctp-subtext0 break-words", "{status}" }
                    }
                }
                if let Some(pane) = view.lowers.last() {
                    p { class: "mt-2 text-xs text-ctp-overlay1", {pane.note()} }
                }
            }
        }
    }
}

/// Colours of the fast, middle and slow averages.
const SMA_COLORS: [&str; 3] = ["yellow", "sky", "peach"];

/// A small toolbar button; `active` ones are held down.
#[component]
fn Key(label: String, #[props(into, default)] title: String, #[props(default)] active: bool, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if active {
                "min-h-7 shrink-0 whitespace-nowrap rounded-lg bg-ctp-surface0 px-2 py-1 text-xs font-semibold text-ctp-text cursor-pointer"
            } else {
                "min-h-7 shrink-0 whitespace-nowrap rounded-lg px-2 py-1 text-xs font-medium text-ctp-subtext0 hover:bg-ctp-surface0/50 hover:text-ctp-text cursor-pointer"
            },
            title: "{title}",
            aria_pressed: active,
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}

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
    rsx! {
        button {
            r#type: "button",
            class: if on {
                "inline-flex items-center gap-1.5 rounded-full border border-ctp-surface1 bg-ctp-surface0 px-2.5 py-1 text-ctp-text cursor-pointer"
            } else {
                "inline-flex items-center gap-1.5 rounded-full border border-ctp-surface0 px-2.5 py-1 text-ctp-subtext0 cursor-pointer hover:text-ctp-text"
            },
            aria_pressed: on,
            onclick: move |e| onclick.call(e),
            // Dimmed by class: rewriting the style on a toggle lost its colour.
            span { class: if on { "h-2 w-2 rounded-full" } else { "h-2 w-2 rounded-full opacity-40" }, style: "background:var(--catppuccin-color-{color});" }
            "{label}"
        }
    }
}

/// Fetched candles as drawn: dividend adjustment undone, back to the
/// price that traded.
fn to_bars(candles: &[Candle]) -> Bars {
    let mut bars = Bars::default();
    for c in candles {
        let px = |d: rust_decimal::Decimal| d.to_f64().unwrap_or(0.0) / c.adjustment();
        bars.push(c.ts.timestamp(), px(c.open), px(c.high), px(c.low), px(c.close), c.volume.unwrap_or(0) as f64);
    }
    bars
}

fn apply_tick(bars: &mut Bars, tick: &LiveTick, clock: &Clock) -> Option<Applied> {
    bars.apply(tick.native.to_f64()?, tick.ts, tick.day_volume.map(|v| v as f64), clock)
}

/// The viewer's buys and sales of `ticker`, oldest first.
fn own_trades(data: &GetDashBoardResponse, ticker: &TickerSymbol) -> Vec<Trade> {
    let mut trades: Vec<Trade> = data
        .transactions
        .iter()
        .filter(|tx| tx.ticker == *ticker)
        .filter_map(|tx| {
            let bought = match tx.transaction_type {
                TransactionType::Buy => true,
                TransactionType::Sell => false,
                _ => return None,
            };
            let date = chrono::NaiveDate::parse_from_str(tx.date.get(..10)?, "%Y-%m-%d").ok()?;
            Some(Trade {
                day: date.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
                bought,
                shares: tx.shares.to_f64()?,
                price: tx.price.to_f64()?,
            })
        })
        .collect();
    trades.sort_by_key(|t| t.day);
    trades
}

fn round4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

fn rounded(values: Vec<Option<f64>>) -> Vec<Option<f64>> {
    values.into_iter().map(|v| v.filter(|x| x.is_finite()).map(round4)).collect()
}

/// One line of an indicator.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    /// Names its series in `stock_chart.js`.
    id: String,
    name: String,
    color: &'static str,
    values: Vec<Option<f64>>,
    /// Dots rather than a line (a stop that jumps sides).
    dots: bool,
    dashed: bool,
    /// The other half of a study drawn as two lines that take turns: this
    /// one is left out of the readout, and the first reads for both.
    quiet: bool,
    alt: Option<String>,
}

impl Line {
    fn new(id: impl Into<String>, name: impl Into<String>, color: &'static str, values: Vec<Option<f64>>) -> Self {
        Self { id: id.into(), name: name.into(), color, values: rounded(values), dots: false, dashed: false, quiet: false, alt: None }
    }

    fn json(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "color": self.color, "values": self.values, "dots": self.dots, "dashed": self.dashed, "quiet": self.quiet, "alt": self.alt })
    }
}

/// Two lines with the ground between them filled.
#[derive(Clone, Debug, PartialEq)]
struct Band {
    id: String,
    name: String,
    color: &'static str,
    upper: Vec<Option<f64>>,
    lower: Vec<Option<f64>>,
}

/// An indicator's own plot under the price.
#[derive(Clone, Debug, PartialEq)]
struct Pane {
    title: String,
    /// Its scale, when it has fixed ends (0 to 100, say).
    bounded: Option<(f64, f64)>,
    /// Levels worth a line across the pane.
    levels: Vec<f64>,
    lines: Vec<Line>,
    /// A histogram, green above zero and red below.
    bars: Vec<Line>,
}

/// Everything computed from the candles.
#[derive(Clone, Debug, PartialEq, Default)]
struct Studies {
    overlays: Vec<Line>,
    bands: Vec<Band>,
    panes: Vec<Pane>,
    /// The candles as drawn when they aren't the real ones (Heikin-Ashi),
    /// as (open, high, low, close).
    drawn: Option<Vec<(f64, f64, f64, f64)>>,
}

impl Studies {
    /// Every line's newest value, by series: what a trade just moved.
    fn latest(&self) -> Vec<(String, Option<f64>)> {
        let last = |v: &[Option<f64>]| v.last().copied().flatten();
        let mut out: Vec<(String, Option<f64>)> = self.overlays.iter().map(|l| (l.id.clone(), last(&l.values))).collect();
        for b in &self.bands {
            out.push((format!("{}u", b.id), last(&b.upper)));
            out.push((format!("{}l", b.id), last(&b.lower)));
        }
        for pane in &self.panes {
            out.extend(pane.lines.iter().chain(&pane.bars).map(|l| (l.id.clone(), last(&l.values))));
        }
        out
    }
}

/// The indicators switched on, over `bars`.
fn studies(bars: &Bars, frame: Frame, view: &Prefs, p: &Periods) -> Studies {
    let (open, high, low, close, volume) = (&bars.open, &bars.high, &bars.low, &bars.close, &bars.volume);
    let o = view.overlays;
    let mut out = Studies::default();

    for (i, color) in SMA_COLORS.into_iter().enumerate() {
        if o.sma[i] {
            out.overlays.push(Line::new(format!("sma{i}"), format!("SMA {}", p.sma[i]), color, sma(close, p.sma[i])));
        }
    }
    if o.ema {
        out.overlays.push(Line::new("ema", format!("EMA {}", p.ema), "pink", ema(close, p.ema)));
    }
    if o.vwap && frame.intraday() {
        // Each session's own average: candles grouped by their UTC day.
        let days: Vec<i64> = bars.times.iter().map(|t| t.div_euclid(DAY)).collect();
        out.overlays.push(Line::new("vwap", "VWAP", "rosewater", vwap(high, low, close, volume, &days)));
    }
    if o.supertrend {
        let stops = supertrend(high, low, close, p.supertrend, p.supertrend_width);
        let side = |up: bool| stops.iter().map(|s| s.filter(|(_, rising)| *rising == up).map(|(stop, _)| stop)).collect::<Vec<_>>();
        let name = format!("Supertrend {} · {}", p.supertrend, p.supertrend_width);
        let mut under = Line::new("stu", name.clone(), "green", side(true));
        under.alt = Some("std".into());
        let mut over = Line::new("std", name, "red", side(false));
        over.quiet = true;
        out.overlays.extend([under, over]);
    }
    if o.psar {
        let mut stops = Line::new("psar", "SAR", "flamingo", parabolic_sar(high, low, 0.02, 0.2).into_iter().map(|s| s.map(|(stop, _)| stop)).collect());
        stops.dots = true;
        out.overlays.push(stops);
    }
    if o.bollinger {
        let bands = bollinger(close, p.bollinger, p.bollinger_width);
        out.bands.push(Band {
            id: "bb".into(),
            name: format!("BB {} · {}", p.bollinger, p.bollinger_width),
            color: "lavender",
            upper: rounded(bands.iter().map(|b| b.map(|(_, upper, _)| upper)).collect()),
            lower: rounded(bands.iter().map(|b| b.map(|(_, _, lower)| lower)).collect()),
        });
    }
    if o.donchian {
        let channel = donchian(high, low, p.donchian);
        out.bands.push(Band {
            id: "dc".into(),
            name: format!("Donchian {}", p.donchian),
            color: "sapphire",
            upper: rounded(channel.iter().map(|c| c.map(|(top, _)| top)).collect()),
            lower: rounded(channel.iter().map(|c| c.map(|(_, bottom)| bottom)).collect()),
        });
    }

    for (k, pane) in view.lowers.iter().enumerate() {
        let line = |n: usize, name: &str, color: &'static str, values: Vec<Option<f64>>| Line::new(format!("p{k}l{n}"), name, color, values);
        out.panes.push(match pane {
            Lower::Rsi => Pane {
                title: trf("RSI {}", &[&p.rsi]),
                bounded: Some((0.0, 100.0)),
                levels: vec![30.0, 70.0],
                lines: vec![line(0, "RSI", "teal", rsi(close, p.rsi))],
                bars: vec![],
            },
            Lower::Stochastic => {
                let (k_line, d_line) = stochastic(high, low, close, p.stochastic[0], p.stochastic[1]);
                Pane {
                    title: trf("Stoch {} {}", &[&p.stochastic[0], &p.stochastic[1]]),
                    bounded: Some((0.0, 100.0)),
                    levels: vec![20.0, 80.0],
                    lines: vec![line(0, "%K", "mauve", k_line), line(1, "%D", "peach", d_line)],
                    bars: vec![],
                }
            }
            Lower::Atr => Pane {
                title: trf("ATR {}", &[&p.atr]),
                bounded: None,
                levels: vec![],
                lines: vec![line(0, "ATR", "flamingo", atr(high, low, close, p.atr))],
                bars: vec![],
            },
            Lower::Obv => Pane {
                title: "OBV".into(),
                bounded: None,
                levels: vec![],
                lines: vec![line(0, "OBV", "sapphire", obv(close, volume))],
                bars: vec![],
            },
            Lower::Macd => {
                let m = macd(close, p.macd[0], p.macd[1], p.macd[2]);
                let mut histogram = Line::new(format!("p{k}b0"), "Hist", "", m.histogram);
                histogram.color = "";
                Pane {
                    title: trf("MACD {} {} {}", &[&p.macd[0], &p.macd[1], &p.macd[2]]),
                    bounded: None,
                    levels: vec![],
                    lines: vec![line(0, "MACD", "blue", m.macd), line(1, "Signal", "peach", m.signal)],
                    bars: vec![histogram],
                }
            }
            Lower::Adx => {
                let a = adx(high, low, close, p.adx);
                Pane {
                    title: trf("ADX {}", &[&p.adx]),
                    bounded: None,
                    levels: vec![25.0],
                    lines: vec![line(0, "ADX", "yellow", a.adx), line(1, "+DI", "green", a.plus_di), line(2, "−DI", "red", a.minus_di)],
                    bars: vec![],
                }
            }
            Lower::Cci => Pane {
                title: trf("CCI {}", &[&p.cci]),
                bounded: None,
                levels: vec![-100.0, 100.0],
                lines: vec![line(0, "CCI", "sky", cci(high, low, close, p.cci))],
                bars: vec![],
            },
            Lower::Williams => Pane {
                title: trf("%R {}", &[&p.williams]),
                bounded: Some((-100.0, 0.0)),
                levels: vec![-80.0, -20.0],
                lines: vec![line(0, "%R", "pink", williams_r(high, low, close, p.williams))],
                bars: vec![],
            },
            Lower::Mfi => Pane {
                title: trf("MFI {}", &[&p.mfi]),
                bounded: Some((0.0, 100.0)),
                levels: vec![20.0, 80.0],
                lines: vec![line(0, "MFI", "green", mfi(high, low, close, volume, p.mfi))],
                bars: vec![],
            },
        });
    }

    if view.style == Style::HeikinAshi {
        out.drawn = Some(heikin_ashi(open, high, low, close));
    }
    out
}

/// What's marked on the candles besides indicators.
#[derive(Clone, Debug, PartialEq, Default)]
struct Marks {
    /// Runs of regular trading, when the candles hold more than that.
    regular: Vec<(i64, i64)>,
    /// Where the session before the one on screen closed (minute lengths).
    prev_close: Option<f64>,
    /// The viewer's average cost of what they hold.
    cost: Option<f64>,
    /// Their trades: the candle each fell on, bought or sold, and what to
    /// say about it.
    trades: Vec<(usize, bool, String)>,
}

impl Marks {
    fn of(bars: &Bars, chart: &SessionChart, frame: Frame, own: &[Trade], view: &Prefs) -> Self {
        let regular: Vec<(i64, i64)> = if chart.regular.is_empty() { vec![] } else { chart.regular.clone() };
        // The close before the last session: of regular trading when the
        // candles hold more.
        let prev_close = frame.intraday().then(|| {
            let runs = if regular.is_empty() { sessions(&bars.times) } else { regular.clone() };
            let before = runs.len().checked_sub(2).map(|i| runs[i].1)?;
            let at = bars.times.iter().rposition(|t| *t <= before)?;
            Some(bars.close[at])
        });
        let show = view.trades && !own.is_empty();
        Self {
            regular,
            prev_close: prev_close.flatten(),
            cost: show.then(|| average_cost(own.iter().map(|t| (t.bought, t.shares, t.price)))).flatten(),
            trades: if show { trade_marks(&bars.times, frame, own) } else { vec![] },
        }
    }
}

/// The candle each trade fell on. Trades are dated by day: on daily and
/// longer candles that's the last candle starting by the end of the day
/// (a trade from before the first candle isn't shown); on minute candles
/// the first candle of that day. Trades on one candle, on one side, share
/// a mark.
fn trade_marks(times: &[i64], frame: Frame, own: &[Trade]) -> Vec<(usize, bool, String)> {
    let mut marks: Vec<(usize, bool, String)> = Vec::new();
    for trade in own {
        let end = trade.day + DAY;
        let at = if frame.intraday() {
            times.iter().position(|t| (trade.day..end).contains(t))
        } else {
            // Within the candle: a week's or a month's worth of days back.
            times.iter().rposition(|t| *t < end).filter(|i| trade.day - times[*i] < frame.step().max(DAY) + 3 * DAY)
        };
        let Some(at) = at else {
            continue;
        };
        let shares = crate::format::fmt_shares(rust_decimal::Decimal::try_from(trade.shares).unwrap_or_default());
        let price = crate::format::fmt_money(rust_decimal::Decimal::try_from(trade.price).unwrap_or_default(), "", 2);
        let text = if trade.bought { trf("Bought {} @ {}", &[&shares, &price]) } else { trf("Sold {} @ {}", &[&shares, &price]) };
        match marks.iter_mut().find(|(i, bought, _)| *i == at && *bought == trade.bought) {
            Some((_, _, all)) => {
                all.push('\n');
                all.push_str(&text);
            }
            None => marks.push((at, trade.bought, text)),
        }
    }
    marks
}

/// A candle as ECharts takes it: open, close, low, high.
fn candle(open: f64, high: f64, low: f64, close: f64) -> [f64; 4] {
    [open, close, low, high].map(round4)
}

/// Everything the chart draws.
fn chart_config(bars: &Bars, s: &Studies, frame: Frame, reach: Option<Reach>, view: &Prefs, marks: &Marks) -> Value {
    let n = bars.len();
    let first_shown = match reach {
        Some(r) => r.first_shown(&bars.times),
        None => n.saturating_sub(Frame::SHOWN),
    };
    let line_list = |lines: &[Line]| lines.iter().map(Line::json).collect::<Vec<_>>();
    json!({
        "times": bars.times,
        "ohlc": (0..n).map(|i| candle(bars.open[i], bars.high[i], bars.low[i], bars.close[i])).collect::<Vec<_>>(),
        "shown": s.drawn.as_ref().map(|d| d.iter().map(|(o, h, l, c)| candle(*o, *h, *l, *c)).collect::<Vec<_>>()),
        "volume": bars.volume,
        "volumeShown": view.volume,
        "style": view.style.key(),
        "overlays": line_list(&s.overlays),
        "bands": s.bands.iter().map(|b| json!({ "id": b.id, "name": b.name, "color": b.color, "upper": b.upper, "lower": b.lower })).collect::<Vec<_>>(),
        "panes": s.panes.iter().map(|p| json!({
            "title": p.title,
            "bounded": p.bounded.map(|(lo, hi)| [lo, hi]),
            "levels": p.levels,
            "lines": line_list(&p.lines),
            "bars": line_list(&p.bars),
        })).collect::<Vec<_>>(),
        "log": view.log,
        "step": frame.step(),
        "intraday": frame.intraday(),
        // A redraw of the same view keeps where the viewer has zoomed to.
        "view": format!("{frame:?}|{reach:?}|{}", view.extended && frame.intraday()),
        "zoom": { "from": first_shown },
        "regular": marks.regular,
        "prevClose": marks.prev_close,
        "cost": marks.cost,
        "trades": marks.trades.iter().map(|(i, bought, text)| json!({ "i": i, "buy": bought, "text": text })).collect::<Vec<_>>(),
        "labels": {
            "o": tr("O"), "h": tr("H"), "l": tr("L"), "c": tr("C"), "vol": tr("Vol"),
            "prev": tr("Prev close"), "cost": tr("Avg cost"),
        },
        "note": (view.style == Style::HeikinAshi).then(|| tr("Heikin-Ashi candles · figures are the real prices")),
        "measureUnit": tr("candles"),
    })
}

/// What a streamed trade changed: the last candle (or a new one) and each
/// indicator's newest value.
fn chart_patch(bars: &Bars, s: &Studies, applied: Applied) -> Value {
    let i = bars.len() - 1;
    json!({
        "time": bars.times[i],
        "added": applied == Applied::Added,
        "ohlc": candle(bars.open[i], bars.high[i], bars.low[i], bars.close[i]),
        "shown": s.drawn.as_ref().and_then(|d| d.last()).map(|(o, h, l, c)| candle(*o, *h, *l, *c)),
        "volume": bars.volume[i],
        "values": s.latest(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    /// 14:30 UTC on a Thursday (2023-11-16).
    const OPEN: i64 = 1_700_145_000;

    fn candles(n: i64) -> Vec<Candle> {
        (0..n)
            .map(|i| {
                let p = Decimal::from(100 + i % 7);
                Candle {
                    ts: Utc.timestamp_opt(1_700_000_000 + i * DAY, 0).unwrap(),
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

    fn view(overlays: Overlays, lowers: &[Lower]) -> Prefs {
        Prefs { overlays, lowers: lowers.to_vec(), ..Prefs::default() }
    }

    const NONE: Overlays = Overlays { sma: [false; 3], ema: false, bollinger: false, vwap: false, donchian: false, supertrend: false, psar: false };

    fn config(bars: &Bars, frame: Frame, reach: Option<Reach>, v: &Prefs, p: &Periods) -> Value {
        chart_config(bars, &studies(bars, frame, v, p), frame, reach, v, &Marks::default())
    }

    #[test]
    fn config_lines_up_every_series_and_opens_on_the_span() {
        let bars = to_bars(&candles(400));
        let v = view(Overlays { sma: [true, false, true], bollinger: true, ..NONE }, &[Lower::Macd]);
        let cfg = config(&bars, Frame::D1, Some(Reach::M3), &v, &Periods::default());
        assert_eq!(cfg["ohlc"].as_array().unwrap().len(), 400);
        assert_eq!(cfg["times"].as_array().unwrap().len(), 400);
        let overlays = cfg["overlays"].as_array().unwrap();
        assert_eq!(overlays.iter().map(|o| o["name"].as_str().unwrap()).collect::<Vec<_>>(), ["SMA 20", "SMA 200"]);
        assert!(overlays.iter().all(|o| o["values"].as_array().unwrap().len() == 400));
        assert!(overlays[1]["values"][198].is_null() && !overlays[1]["values"][199].is_null());
        // The bands come as an upper and a lower line to fill between.
        let band = &cfg["bands"][0];
        assert_eq!((band["id"].as_str(), band["upper"].as_array().unwrap().len()), (Some("bb"), 400));
        assert!(band["upper"][399].as_f64() > band["lower"][399].as_f64());
        let pane = &cfg["panes"][0];
        assert_eq!(pane["title"], "MACD 12 26 9");
        assert_eq!((pane["lines"].as_array().unwrap().len(), pane["bars"].as_array().unwrap().len()), (2, 1));
        // ECharts candles are open, close, low, high.
        assert_eq!(cfg["ohlc"][0], json!([100.0, 100.5, 99.0, 101.0]));
        // Three months of 400 days: the view starts about 77% in.
        let from = cfg["zoom"]["from"].as_u64().unwrap();
        assert!((300..=310).contains(&from), "{from}");
        assert_eq!((cfg["step"].as_i64(), cfg["intraday"].as_bool()), (Some(DAY), Some(false)));
    }

    #[test]
    fn spans_open_where_they_say() {
        // Three days of five-minute candles, 14:30 to 21:00 UTC.
        let times: Vec<i64> = (0..3).flat_map(|d| (0..78).map(move |i| OPEN + d * DAY + i * 300)).collect();
        // The last day's 78 candles of 234.
        assert_eq!(Reach::D1.first_shown(&times), 156);
        // Five sessions asked of three: everything shows.
        assert_eq!(Reach::D5.first_shown(&times), 0);
        // A session ten minutes old opens on the last sixty candles, not ten.
        let early = &times[..166];
        assert_eq!(Reach::D1.first_shown(early), 106);
        // With trading outside regular hours, a session runs from 09:00 UTC
        // through midnight to 01:00: one day's trading, not two.
        let long_day = |d: i64| (0..192).map(move |i| OPEN - 19_800 + d * DAY + i * 300);
        let extended: Vec<i64> = (0..2).flat_map(long_day).collect();
        assert_eq!(Reach::D1.first_shown(&extended), 192);
        let days: Vec<i64> = (0..800).map(|i| OPEN + i * DAY).collect();
        assert_eq!(Reach::M1.first_shown(&days), 800 - 32);
        assert_eq!(Reach::Y1.first_shown(&days), 800 - 367);
        assert_eq!(Reach::All.first_shown(&days), 0);
        // The last candle is 2026-01-23: the year starts 22 candles back.
        assert_eq!(Reach::Ytd.first_shown(&days), 800 - 23);
        assert_eq!(Reach::Y5.first_shown(&[]), 0);
        // Picking a candle length alone shows the newest candles.
        let bars = to_bars(&candles(400));
        let cfg = config(&bars, Frame::H1, None, &Prefs::default(), &Periods::default());
        assert_eq!(cfg["zoom"]["from"], 400 - Frame::SHOWN);
        // Every span is read in a length the provider keeps that far back.
        assert_eq!(Reach::D1.frame().fetch(), (Range::D5, Interval::I1m));
        assert!(Reach::ALL.iter().all(|r| Frame::ALL.contains(&r.frame())));
    }

    #[test]
    fn periods_and_panes_reach_the_config() {
        let bars = to_bars(&candles(120));
        let o = Overlays { sma: [true, false, false], ema: true, vwap: true, donchian: true, supertrend: true, psar: true, ..NONE };
        let p = Periods { sma: [10, 50, 200], ema: 7, stochastic: [5, 3], ..Periods::default() };
        let v = view(o, &[Lower::Stochastic, Lower::Adx, Lower::Williams]);
        let cfg = config(&bars, Frame::D1, Some(Reach::Y1), &v, &p);
        let names: Vec<&str> = cfg["overlays"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap()).collect();
        // VWAP only belongs to the minute lengths; Supertrend is two lines
        // that take turns, the second kept out of the readout.
        assert_eq!(names, ["SMA 10", "EMA 7", "Supertrend 10 · 3", "Supertrend 10 · 3", "SAR"]);
        assert_eq!((cfg["overlays"][2]["alt"].as_str(), cfg["overlays"][3]["quiet"].as_bool()), (Some("std"), Some(true)));
        assert_eq!(cfg["overlays"][4]["dots"], true);
        assert!(cfg["overlays"][0]["values"][8].is_null() && !cfg["overlays"][0]["values"][9].is_null());
        assert_eq!(cfg["bands"][0]["name"], "Donchian 20");
        let panes = cfg["panes"].as_array().unwrap();
        assert_eq!(panes.iter().map(|p| p["title"].as_str().unwrap()).collect::<Vec<_>>(), ["Stoch 5 3", "ADX 14", "%R 14"]);
        assert_eq!(panes[0]["bounded"], json!([0.0, 100.0]));
        assert!(panes[1]["bounded"].is_null());
        assert_eq!(panes[2]["bounded"], json!([-100.0, 0.0]));
        assert_eq!(panes[1]["lines"].as_array().unwrap().len(), 3);
        // Every series has a name of its own to be patched by.
        let mut ids: Vec<String> = studies(&bars, Frame::D1, &v, &p).latest().into_iter().map(|(id, _)| id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
        let minute = config(&bars, Frame::M1, None, &v, &p);
        assert!(minute["overlays"].as_array().unwrap().iter().any(|o| o["name"] == "VWAP"));
        assert_eq!(minute["intraday"], true);
    }

    #[test]
    fn saved_settings_are_pulled_back_in_range() {
        let wild = Periods { sma: [0, 9999, 50], bollinger_width: f64::NAN, supertrend_width: 99.0, ..Periods::default() }.clamped();
        assert_eq!((wild.sma, wild.bollinger_width, wild.supertrend_width), ([1, MAX_PERIOD, 50], 2.0, 5.0));
        // A copy saved before a setting existed takes its default.
        let saved: Periods = serde_json::from_str(r#"{"rsi": 9}"#).unwrap();
        assert_eq!((saved.rsi, saved.sma, saved.donchian), (9, [20, 50, 200], 20));
        let old: Prefs = serde_json::from_str(r#"{"style": "Bars", "overlays": {"ema": true}}"#).unwrap();
        assert_eq!((old.style, old.overlays.ema, old.overlays.sma, old.volume), (Style::Bars, true, [false, true, true], true));
        // Panes listed twice or too many of them.
        let crowded = Prefs { lowers: vec![Lower::Rsi, Lower::Rsi, Lower::Macd, Lower::Atr, Lower::Obv], ..Prefs::default() }.cleaned();
        assert_eq!(crowded.lowers, [Lower::Rsi, Lower::Macd, Lower::Atr]);
        // A fourth pane takes the place of the oldest; picking one again drops it.
        let mut v = crowded;
        v.toggle_pane(Lower::Mfi);
        assert_eq!(v.lowers, [Lower::Macd, Lower::Atr, Lower::Mfi]);
        v.toggle_pane(Lower::Atr);
        assert_eq!(v.lowers, [Lower::Macd, Lower::Mfi]);
        assert_eq!(Style::from_label("HeikinAshi"), Style::HeikinAshi);
        assert_eq!(Style::from_label("?"), Style::Candles);
    }

    #[test]
    fn draws_prices_as_traded() {
        let mut c = candles(2);
        c[0].adj_factor = Some(0.5);
        let bars = to_bars(&c);
        // 100.5 adjusted at half is 201 as traded; the next day is as is.
        assert_eq!(bars.close, [201.0, 101.5]);
        assert_eq!(bars.volume, [1000.0, 1000.0]);
    }

    #[test]
    fn a_trade_moves_the_candle_and_every_indicator() {
        let mut bars = to_bars(&candles(60));
        let v = view(Overlays { sma: [true, false, false], bollinger: true, ..NONE }, &[Lower::Rsi, Lower::Macd]);
        let p = Periods::default();
        let clock = Clock { step: DAY, hours: None };
        let last = *bars.times.last().unwrap();
        let before = studies(&bars, Frame::D1, &v, &p).latest();
        // A jump within the last day: its candle moves, and so does every line.
        assert_eq!(bars.apply(150.0, last + 3600, None, &clock), Some(Applied::Moved));
        let s = studies(&bars, Frame::D1, &v, &p);
        let patch = chart_patch(&bars, &s, Applied::Moved);
        assert_eq!((patch["time"].as_i64(), patch["added"].as_bool()), (Some(last), Some(false)));
        assert_eq!(patch["ohlc"][1], 150.0);
        assert_eq!(patch["ohlc"][3], 150.0, "a new high");
        let after = s.latest();
        assert_eq!(after.len(), before.len());
        assert!(after.iter().zip(&before).all(|(a, b)| a.0 == b.0 && a.1 != b.1), "{after:?}");
        // The patch carries exactly what a full draw would end on.
        let cfg = chart_config(&bars, &s, Frame::D1, None, &v, &Marks::default());
        assert_eq!(patch["values"][0], json!(["sma0", cfg["overlays"][0]["values"][59]]));
        assert_eq!(patch["ohlc"], cfg["ohlc"][59]);
        // The next day: a candle of its own.
        assert_eq!(bars.apply(151.0, last + DAY + 60, None, &clock), Some(Applied::Added));
        let added = chart_patch(&bars, &studies(&bars, Frame::D1, &v, &p), Applied::Added);
        assert_eq!((added["time"].as_i64(), added["added"].as_bool()), (Some(last + DAY), Some(true)));
        assert_eq!(added["ohlc"], json!([151.0, 151.0, 151.0, 151.0]));
    }

    #[test]
    fn heikin_ashi_draws_smoothed_candles_over_the_real_ones() {
        let bars = to_bars(&candles(10));
        let v = Prefs { style: Style::HeikinAshi, ..Prefs::default() };
        let cfg = config(&bars, Frame::D1, None, &v, &Periods::default());
        assert_eq!(cfg["style"], "candle");
        assert_eq!(cfg["shown"].as_array().unwrap().len(), 10);
        // (100 + 101 + 99 + 100.5) / 4, opening half-way between 100 and 100.5.
        assert_eq!(cfg["shown"][0], json!([100.25, 100.125, 99.0, 101.0]));
        assert_eq!(cfg["ohlc"][0], json!([100.0, 100.5, 99.0, 101.0]), "the readout keeps the real prices");
        assert!(config(&bars, Frame::D1, None, &Prefs::default(), &Periods::default())["shown"].is_null());
        let patch = chart_patch(&bars, &studies(&bars, Frame::D1, &v, &Periods::default()), Applied::Moved);
        assert_eq!(patch["shown"], cfg["shown"][9]);
    }

    #[test]
    fn own_trades_are_marked_on_the_candle_they_fell_on() {
        // Daily candles stamped at the 14:30 open, Thursday to the next Friday.
        let times: Vec<i64> = (0..7).map(|i| OPEN + i * DAY).filter(|t| ![2, 3].contains(&((t - OPEN) / DAY))).collect();
        let midnight = OPEN - OPEN.rem_euclid(DAY);
        let trade = |day: i64, bought: bool| Trade { day: midnight + day * DAY, bought, shares: 2.0, price: 150.0 };
        let own = [trade(-30, true), trade(0, true), trade(0, true), trade(1, false), trade(4, true)];
        let marks = trade_marks(&times, Frame::D1, &own);
        // The trade a month before the first candle isn't shown; the two
        // buys on Thursday share a mark.
        assert_eq!(marks.iter().map(|(i, b, _)| (*i, *b)).collect::<Vec<_>>(), [(0, true), (1, false), (2, true)]);
        assert_eq!(marks[0].2.lines().count(), 2);
        assert!(marks[0].2.contains("2") && marks[0].2.contains("150.00"), "{}", marks[0].2);
        // A weekly candle (stamped Monday) takes the whole week's trades.
        let weeks = [midnight - 3 * DAY, midnight + 4 * DAY];
        let weekly = trade_marks(&weeks, Frame::W1, &[trade(0, true), trade(1, false), trade(5, true)]);
        assert_eq!(weekly.iter().map(|(i, b, _)| (*i, *b)).collect::<Vec<_>>(), [(0, true), (0, false), (1, true)]);
        // Minute candles: the first candle of the day.
        let minutes: Vec<i64> = (0..2).flat_map(|d| (0..5).map(move |i| OPEN + d * DAY + i * 60)).collect();
        assert_eq!(trade_marks(&minutes, Frame::M1, &[trade(1, false), trade(9, true)]).iter().map(|(i, _, _)| *i).collect::<Vec<_>>(), [5]);
    }

    #[test]
    fn marks_show_the_last_close_and_the_average_cost() {
        // Two sessions of three minute candles; the first closed at 100.5.
        let mut bars = Bars::default();
        for d in 0..2 {
            for i in 0..3 {
                bars.push(OPEN + d * DAY + i * 60, 100.0, 101.0, 99.0, 100.5 + d as f64, 10.0);
            }
        }
        let chart = SessionChart { candles: vec![], currency: "USD".into(), regular: vec![] };
        let midnight = OPEN - OPEN.rem_euclid(DAY);
        let own = [Trade { day: midnight, bought: true, shares: 1.0, price: 90.0 }, Trade { day: midnight + DAY, bought: true, shares: 1.0, price: 110.0 }];
        let marks = Marks::of(&bars, &chart, Frame::M1, &own, &Prefs::default());
        assert_eq!((marks.prev_close, marks.cost), (Some(100.5), Some(100.0)));
        assert_eq!(marks.trades.len(), 2);
        // Switched off, or with nothing traded: no marks, no cost line.
        let hidden = Marks::of(&bars, &chart, Frame::M1, &own, &Prefs { trades: false, ..Prefs::default() });
        assert_eq!((hidden.cost, hidden.trades.len()), (None, 0));
        // Daily candles have no session before to close.
        assert_eq!(Marks::of(&bars, &chart, Frame::D1, &[], &Prefs::default()).prev_close, None);
        // With trading outside regular hours, it's the regular close.
        let extended = SessionChart { regular: vec![(OPEN, OPEN + 60), (OPEN + DAY + 60, OPEN + DAY + 120)], ..chart };
        let marks = Marks::of(&bars, &extended, Frame::M1, &[], &Prefs::default());
        assert_eq!(marks.prev_close, Some(100.5));
        assert_eq!(marks.regular.len(), 2);
        let cfg = chart_config(&bars, &Studies::default(), Frame::M1, Some(Reach::D1), &Prefs::default(), &marks);
        assert_eq!((cfg["prevClose"].as_f64(), cfg["regular"][1][0].as_i64()), (Some(100.5), Some(OPEN + DAY + 60)));
    }
}
