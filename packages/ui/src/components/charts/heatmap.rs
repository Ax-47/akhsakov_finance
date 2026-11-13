//! Treemaps coloured by change: holdings by value, an index by market cap.
//!
//! Colour comes in nine steps, from a strong fall to a strong rise, so a
//! tile reads as one of a few classes rather than a shade to be judged; the
//! steps and the label ink are set per theme in `input.css`, where every
//! step keeps its label readable. Labels grow and shrink with their tile,
//! and whatever a tile is too small to say shows in a card beside it when
//! it's pointed at.

use crate::i18n::tr;
use crate::components::{
    analysis::stock::open_stock,
    card::{Card, Segmented, ToggleButton},
};
use dioxus::prelude::*;
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::ticker_symbol::TickerSymbol;

const HEIGHT_PX: u32 = 300;
/// Width assumed until the map has measured itself.
const FALLBACK_WIDTH_PX: f64 = 900.0;
/// Height of a group's label strip.
const GROUP_LABEL_PX: f64 = 20.0;

/// Change (%) at which a tile reaches its strongest green / red.
pub const DAY_SATURATION: f64 = 3.0;
const TOTAL_SATURATION: f64 = 30.0;

#[derive(Clone, Copy, PartialEq)]
enum Metric {
    Day,
    Total,
}

/// One tile: area is `size`, colour is `change` (%).
#[derive(Clone, Debug, PartialEq)]
pub struct HeatItem {
    pub ticker: String,
    /// Shown in the tooltip.
    pub name: String,
    pub size: f64,
    pub change: Option<f64>,
    /// Tiles with the same group are laid out together under a label.
    pub group: Option<String>,
    /// Extra tooltip line, e.g. "12.3% of portfolio".
    pub detail: String,
}

/// Treemap of holdings: tile area is market value, colour is % change.
#[component]
pub fn StockHeatmap(positions: ReadSignal<Vec<Position>>) -> Element {
    let mut metric = use_signal(|| Metric::Day);

    let items = use_memo(move || {
        let positions = positions.read();
        let total: f64 = positions.iter().map(|p| to_f64(p.market_value())).sum();
        positions
            .iter()
            .filter(|p| p.current_price > Decimal::ZERO)
            .map(|p| {
                let change = match metric() {
                    Metric::Day => p.daily_change_pct,
                    Metric::Total => p.unrealized_pnl_pct(),
                };
                let value = to_f64(p.market_value());
                HeatItem {
                    ticker: p.ticker.to_string(),
                    name: p.ticker.to_string(),
                    size: value,
                    change: Some(to_f64(change)),
                    group: None,
                    detail: crate::i18n::trf("{}% of portfolio", &[&format!("{:.1}", value / total * 100.0)]),
                }
            })
            .collect::<Vec<_>>()
    });

    let saturation = match metric() {
        Metric::Day => DAY_SATURATION,
        Metric::Total => TOTAL_SATURATION,
    };

    rsx! {
        Card {
            title: tr("Heatmap"),
            subtitle: tr("Size is value, colour is change").to_string(),
            actions: rsx! {
                Segmented {
                    ToggleButton { label: tr("Today"), active: metric() == Metric::Day, onclick: move |_| metric.set(Metric::Day) }
                    ToggleButton { label: tr("All time"), active: metric() == Metric::Total, onclick: move |_| metric.set(Metric::Total) }
                }
            },
            Treemap { items, saturation, height: HEIGHT_PX }
            HeatmapLegend { saturation }
        }
    }
}

/// Squarified treemap of `items`; click a tile to open the stock. Items
/// with a `group` are laid out in labelled blocks, largest group first,
/// and a block's label calls `on_group` when clicked. The map measures
/// itself, so tiles are shaped for the screen and carry only the text that
/// fits them; `height_css` lets it grow with the window (`height`, in
/// pixels, is used until it has been measured).
#[component]
pub fn Treemap(
    items: ReadSignal<Vec<HeatItem>>,
    saturation: f64,
    height: u32,
    #[props(default)] height_css: Option<String>,
    #[props(default)] on_group: Option<EventHandler<String>>,
) -> Element {
    let mut measured = use_signal(|| Size { w: FALLBACK_WIDTH_PX, h: height as f64 });
    let grows = height_css.is_some();
    let layout = use_memo(move || {
        let size = measured();
        layout(&items.read(), size.w, if grows { size.h } else { height as f64 })
    });
    // The ticker pointed at, for the card beside its tile.
    let mut hover = use_signal(|| None::<String>);
    let dense = items.read().len() > 40;
    let height_style = height_css.unwrap_or_else(|| format!("{height}px"));

    if layout.read().tiles.is_empty() {
        return rsx! {
            div {
                class: "flex items-center justify-center text-ctp-overlay1 text-xs",
                style: "height:{height_style};",
                {tr("Waiting for prices…")}
            }
        };
    }
    rsx! {
        div {
            // Dense maps skip rounding: clipping ~500 tiles to rounded corners,
            // and rounding each, doubled the time to paint a scroll frame in
            // the desktop app (WebKit painting without the GPU).
            class: if dense { "relative w-full select-none" } else { "relative w-full select-none overflow-hidden rounded-2xl" },
            style: "height:{height_style};",
            onresize: move |e| {
                if let Ok(size) = e.get_content_box_size() {
                    // Whole pixels, so sub-pixel jitter doesn't redo the layout.
                    let now = Size { w: size.width.round(), h: size.height.round() };
                    if now.w > 0.0 && now.h > 0.0 && now != measured() {
                        measured.set(now);
                    }
                }
            },
            onmouseleave: move |_| hover.set(None),
            for group in layout.read().groups.iter() {
                GroupFrame { key: "g-{group.name}", group: group.clone(), map: layout.read().size, on_group }
            }
            for tile in layout.read().tiles.iter() {
                HeatmapTile { key: "{tile.item.ticker}", tile: tile.clone(), map: layout.read().size, saturation, dense, hover }
            }
            HoverCard { layout, hover, saturation }
        }
    }
}

/// A group's backdrop and its label strip: the name (a shorter form of it
/// when the block is narrow) and the group's weighted change when both
/// fit, the name alone when only it fits, else nothing; the card beside a
/// tile names the group too. Clicking the strip picks the group.
#[component]
fn GroupFrame(group: GroupLabel, map: Size, on_group: Option<EventHandler<String>>) -> Element {
    let (left, top, width, height) = percent_box(group.rect, map);
    let change = group
        .change
        .map(|c| format!("{c:+.2}%"))
        .unwrap_or_default();
    let change_class = match group.change {
        Some(c) if c >= 0.0 => "text-ctp-green",
        Some(_) => "text-ctp-red",
        None => "",
    };
    let (name, with_change) = strip_text(group.rect.w, &group.name, change.chars().count());
    let pick = group.name.clone();
    let clickable = on_group.is_some();
    rsx! {
        div {
            class: "absolute overflow-hidden rounded-lg border-2 border-ctp-base bg-ctp-crust",
            style: "left:{left:.3}%;top:{top:.3}%;width:{width:.3}%;height:{height:.3}%;",
            if group.show_label {
                div {
                    class: if clickable {
                        "flex items-center justify-between gap-2 px-1.5 text-[11.5px] font-semibold text-ctp-subtext1 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text"
                    } else {
                        "flex items-center justify-between gap-2 px-1.5 text-[11.5px] font-semibold text-ctp-subtext1"
                    },
                    style: "height:{GROUP_LABEL_PX}px;",
                    role: if clickable { "button" },
                    "aria-label": "{group.name} {change}",
                    onclick: move |_| {
                        if let Some(handler) = on_group {
                            handler.call(pick.clone());
                        }
                    },
                    if let Some(name) = name {
                        span { class: "whitespace-nowrap", "{name}" }
                    }
                    if with_change {
                        span { class: "tabular-nums {change_class}", "{change}" }
                    }
                }
            }
        }
    }
}

#[component]
fn HeatmapTile(tile: Tile, map: Size, saturation: f64, dense: bool, hover: Signal<Option<String>>) -> Element {
    let (left, top, width, height) = percent_box(tile.rect, map);
    let item = &tile.item;
    let change_text = change_text(item.change);
    let named = item.name != item.ticker;
    let label = fit_label(tile.rect, &tile.label, change_text.chars().count(), named);
    // A hairline of the backdrop between tiles; a wider one on sparse maps.
    let shape = if dense { "border" } else { "border-2 rounded-lg" };
    let step = heat_class(heat_step(item.change, saturation));

    let ticker = TickerSymbol::new(&item.ticker).ok();
    let pointed = item.ticker.clone();
    rsx! {
        div {
            // No per-tile filters or transitions: an index map has 500 tiles.
            class: "heat-tile absolute flex flex-col items-center justify-center overflow-hidden cursor-pointer \
                    {shape} border-transparent bg-clip-padding {step}",
            onclick: move |_| {
                if let Some(t) = &ticker {
                    open_stock(t);
                }
            },
            onmouseenter: move |_| hover.set(Some(pointed.clone())),
            style: "left:{left:.3}%;top:{top:.3}%;width:{width:.3}%;height:{height:.3}%;",
            "aria-label": "{item.ticker} {item.name} {change_text}",
            if let Some(px) = label.ticker_px {
                span { class: "font-bold leading-none whitespace-nowrap", style: "font-size:{px:.1}px;", "{tile.label}" }
            }
            if let Some(px) = label.change_px {
                span { class: "font-medium leading-none tabular-nums whitespace-nowrap", style: "font-size:{px:.1}px;margin-top:{px * 0.3:.1}px;", "{change_text}" }
            }
            if label.name {
                span { class: "mt-1.5 max-w-[88%] truncate text-xs leading-none opacity-80", "{item.name}" }
            }
        }
    }
}

fn change_text(change: Option<f64>) -> String {
    change.map(|c| format!("{c:+.2}%")).unwrap_or_else(|| "—".into())
}

/// The card's size: it's placed whole, so it has to be known.
const CARD_W: f64 = 250.0;
const CARD_H: f64 = 112.0;

/// What a tile is too small to say, beside the tile being pointed at:
/// ticker, name, group, change and the item's own detail line. It sits
/// inside the map, on whichever side of the tile has room.
#[component]
fn HoverCard(layout: Memo<Layout>, hover: Signal<Option<String>>, saturation: f64) -> Element {
    let Some(ticker) = hover() else {
        return rsx! {};
    };
    let layout = layout.read();
    let Some(tile) = layout.tiles.iter().find(|t| t.item.ticker == ticker) else {
        return rsx! {};
    };
    let (x, y) = card_place(tile.rect, layout.size);
    let item = &tile.item;
    let step = heat_class(heat_step(item.change, saturation));
    rsx! {
        div {
            class: "pointer-events-none absolute z-20 flex flex-col justify-center gap-1 rounded-xl border border-ctp-surface1 bg-ctp-mantle px-3.5 py-2.5 shadow-xl shadow-black/30",
            style: "left:{x:.0}px;top:{y:.0}px;width:{CARD_W.min(layout.size.w)}px;min-height:{CARD_H}px;",
            div { class: "flex items-center justify-between gap-3",
                span { class: "truncate text-base font-bold text-ctp-text", "{item.ticker}" }
                span { class: "heat-tile shrink-0 rounded-md px-2 py-0.5 text-sm font-semibold tabular-nums {step}", {change_text(item.change)} }
            }
            if item.name != item.ticker {
                div { class: "truncate text-sm text-ctp-subtext1", "{item.name}" }
            }
            if let Some(group) = &item.group {
                div { class: "truncate text-xs text-ctp-subtext0", "{group}" }
            }
            if !item.detail.is_empty() {
                div { class: "truncate text-xs tabular-nums text-ctp-subtext0", "{item.detail}" }
            }
        }
    }
}

/// Where the card goes for a tile at `tile` on a map of `map`: beside the
/// tile (right, else left), else under or over it, and always inside the
/// map.
fn card_place(tile: Rect, map: Size) -> (f64, f64) {
    const GAP: f64 = 8.0;
    let w = CARD_W.min(map.w);
    let inside_x = |x: f64| x.clamp(0.0, (map.w - w).max(0.0));
    let inside_y = |y: f64| y.clamp(0.0, (map.h - CARD_H).max(0.0));
    let right = tile.x + tile.w + GAP;
    let left = tile.x - GAP - w;
    if right + w <= map.w {
        (right, inside_y(tile.y))
    } else if left >= 0.0 {
        (left, inside_y(tile.y))
    } else if tile.y + tile.h + GAP + CARD_H <= map.h {
        (inside_x(tile.x), tile.y + tile.h + GAP)
    } else {
        (inside_x(tile.x), inside_y(tile.y - GAP - CARD_H))
    }
}

/// The nine steps with the change each starts at: what a colour means.
#[component]
pub fn HeatmapLegend(saturation: f64) -> Element {
    let unit = saturation / 3.0;
    let mark = |v: f64| if unit.fract() == 0.0 { format!("{v:+.0}%") } else { format!("{v:+.1}%") };
    // Where one step ends and the next begins, in ninths of the bar; the
    // flat step sits in the middle, over the zero.
    let marks: Vec<(f64, String)> = (1..=3)
        .rev()
        .map(|i| ((4 - i) as f64, mark(-(i as f64) * unit)))
        .chain([(4.5, "0".to_string())])
        .chain((1..=3).map(|i| ((5 + i) as f64, mark(i as f64 * unit))))
        .collect();
    rsx! {
        div { class: "mt-3 flex justify-end",
            div { class: "w-full max-w-[22rem]", "aria-label": tr("Colour scale"),
                div { class: "grid grid-cols-9 gap-px overflow-hidden rounded-md",
                    for step in -4..=4_i8 {
                        div { key: "{step}", class: "h-3 {heat_class(step)}" }
                    }
                }
                div { class: "relative mt-1 h-4 text-[11px] tabular-nums text-ctp-subtext0",
                    for (at, text) in marks {
                        span { key: "{text}", class: "absolute -translate-x-1/2 whitespace-nowrap", style: "left:{at / 9.0 * 100.0:.2}%;", "{text}" }
                    }
                }
            }
        }
    }
}

// ─── Layout ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// The map's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Size {
    w: f64,
    h: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct Tile {
    item: HeatItem,
    /// The ticker as shown: without a suffix every ticker on the map shares.
    label: String,
    /// In pixels.
    rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
struct GroupLabel {
    name: String,
    /// In pixels.
    rect: Rect,
    /// Size-weighted change of the group's tiles.
    change: Option<f64>,
    show_label: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Layout {
    size: Size,
    tiles: Vec<Tile>,
    groups: Vec<GroupLabel>,
}

/// Places `items` on a map `width_px` × `height_px`.
fn layout(items: &[HeatItem], width_px: f64, height_px: f64) -> Layout {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        w: width_px,
        h: height_px,
    };
    let size = Size { w: width_px, h: height_px };
    let suffix = shared_suffix(items);
    let mut items: Vec<&HeatItem> = items.iter().filter(|i| i.size > 0.0).collect();
    if items.iter().all(|i| i.group.is_none()) {
        return Layout {
            size,
            tiles: place(&mut items, bounds, suffix.as_deref()),
            groups: vec![],
        };
    }

    // Groups, largest first.
    let mut groups: Vec<(String, f64)> = Vec::new();
    for item in &items {
        let name = group_of(item);
        match groups.iter_mut().find(|(g, _)| g == name) {
            Some((_, total)) => *total += item.size,
            None => groups.push((name.to_string(), item.size)),
        }
    }
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    let sizes: Vec<f64> = groups.iter().map(|(_, s)| *s).collect();
    let label_h = GROUP_LABEL_PX;

    let mut out = Layout { size, tiles: vec![], groups: vec![] };
    for ((name, _), rect) in groups.into_iter().zip(squarify(&sizes, bounds)) {
        let mut members: Vec<&HeatItem> = items
            .iter()
            .copied()
            .filter(|i| group_of(i) == name)
            .collect();
        // Only label blocks with room for the strip and some tiles below it.
        let show_label = rect.h > label_h * 3.0 && rect.w >= MIN_STRIP_PX;
        let inner = if show_label {
            Rect {
                y: rect.y + label_h,
                h: rect.h - label_h,
                ..rect
            }
        } else {
            rect
        };
        let (weighted, total) = members
            .iter()
            .filter_map(|i| Some((i.change?, i.size)))
            .fold((0.0, 0.0), |(w, t), (c, s)| (w + c * s, t + s));
        out.groups.push(GroupLabel {
            name,
            rect,
            change: (total > 0.0).then(|| weighted / total),
            show_label,
        });
        out.tiles.extend(place(&mut members, inner, suffix.as_deref()));
    }
    out
}

fn group_of(item: &HeatItem) -> &str {
    item.group.as_deref().unwrap_or("Other")
}

/// Squarifies `items` (largest first) into `bounds`; labels drop `suffix`.
fn place(items: &mut [&HeatItem], bounds: Rect, suffix: Option<&str>) -> Vec<Tile> {
    items.sort_by(|a, b| b.size.total_cmp(&a.size));
    let sizes: Vec<f64> = items.iter().map(|i| i.size).collect();
    squarify(&sizes, bounds)
        .into_iter()
        .zip(items.iter())
        .map(|(rect, item)| Tile {
            label: suffix
                .and_then(|s| item.ticker.strip_suffix(s))
                .unwrap_or(&item.ticker)
                .to_string(),
            item: (*item).clone(),
            rect,
        })
        .collect()
}

/// An exchange suffix every ticker on the map has (`.BK` on a SET map),
/// which only takes room on the tiles. Needs two tickers or more.
fn shared_suffix(items: &[HeatItem]) -> Option<String> {
    let suffix_of = |t: &str| t.rfind('.').map(|at| t[at..].to_string());
    let first = suffix_of(&items.first()?.ticker)?;
    (items.len() > 1 && items.iter().all(|i| suffix_of(&i.ticker).as_ref() == Some(&first))).then_some(first)
}

/// `rect` (pixels) as left, top, width and height percentages of the map,
/// so tiles keep their place while the map resizes.
fn percent_box(rect: Rect, map: Size) -> (f64, f64, f64, f64) {
    (
        rect.x / map.w * 100.0,
        rect.y / map.h * 100.0,
        rect.w / map.w * 100.0,
        rect.h / map.h * 100.0,
    )
}

// ─── Fitting text ─────────────────────────────────────────────────────────────

/// Width of a bold capital letter, as a share of the font size; on the
/// generous side, so text never overflows its tile.
const CHAR_EM: f64 = 0.7;

/// How wide `label` sets in bold, in ems: M and W are a third wider than
/// most capitals (enough to push "AMGN" out of a tile sized for four
/// average letters), I and the punctuation in a ticker much narrower.
fn label_em(label: &str) -> f64 {
    label
        .chars()
        .map(|c| match c {
            'M' | 'W' | 'm' | 'w' => 0.92,
            'I' | 'i' | 'l' | '.' | '-' | ':' => 0.36,
            '0'..='9' => DIGIT_EM,
            _ => CHAR_EM,
        })
        .sum()
}
/// The same for the digits and signs of a change.
const DIGIT_EM: f64 = 0.62;
/// Room a tile's gap and edges take, across and down.
const TILE_PADDING_PX: f64 = 6.0;
/// Type smaller than this isn't read: the tile stays blank and the card
/// beside it speaks.
const MIN_FONT_PX: f64 = 10.0;
const MAX_FONT_PX: f64 = 30.0;
/// A change is set this much smaller than its ticker.
const CHANGE_SCALE: f64 = 0.74;
/// A tile at least this big names its company under the change.
const NAME_TILE_PX: (f64, f64) = (150.0, 96.0);
/// A group strip narrower than this shows nothing.
const MIN_STRIP_PX: f64 = 44.0;

/// What a tile shows, and how large: type scales with the tile.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TileLabel {
    ticker_px: Option<f64>,
    change_px: Option<f64>,
    /// The company's name, on the largest tiles.
    name: bool,
}

/// The largest type at which the ticker and its change both fit `rect`;
/// failing that the ticker alone; failing that nothing.
fn fit_label(rect: Rect, ticker: &str, change_chars: usize, named: bool) -> TileLabel {
    let (w, h) = (rect.w - TILE_PADDING_PX, rect.h - TILE_PADDING_PX);
    let half = |px: f64| (px * 2.0).floor() / 2.0;
    let across = |chars: usize, em: f64| w / (chars.max(1) as f64 * em);
    let ticker_fits = w / label_em(ticker).max(CHAR_EM);
    // Two lines: the ticker, a gap of 0.3 of the change's size, the change.
    let both = ticker_fits
        .min(across(change_chars, DIGIT_EM) / CHANGE_SCALE)
        .min(h / (1.0 + CHANGE_SCALE * 1.3))
        .min(MAX_FONT_PX);
    if both * CHANGE_SCALE >= MIN_FONT_PX {
        return TileLabel {
            ticker_px: Some(half(both)),
            change_px: Some(half(both * CHANGE_SCALE)),
            name: named && rect.w >= NAME_TILE_PX.0 && rect.h >= NAME_TILE_PX.1,
        };
    }
    // A tile too small for both lines at their sizes still fits them with
    // the change at the smallest size, if it's wide enough for that.
    let ticker = ticker_fits.min(h - MIN_FONT_PX * 1.3).min(MAX_FONT_PX);
    if ticker >= MIN_FONT_PX && across(change_chars, DIGIT_EM) >= MIN_FONT_PX {
        return TileLabel { ticker_px: Some(half(ticker.min(14.0))), change_px: Some(MIN_FONT_PX), name: false };
    }
    let alone = ticker_fits.min(h).min(14.0);
    TileLabel {
        ticker_px: (alone >= MIN_FONT_PX).then(|| half(alone)),
        change_px: None,
        name: false,
    }
}

/// Shorter forms of a long sector name, longest first, for a narrow strip.
fn short_names(name: &str) -> &'static [&'static str] {
    match name {
        "Information Technology" => &["Technology", "Tech"],
        "Communication Services" => &["Communication", "Comm."],
        "Consumer Discretionary" => &["Discretionary", "Discr."],
        "Consumer Staples" => &["Staples"],
        "Health Care" => &["Health"],
        "Financials" => &["Finance"],
        "Industrials" => &["Industry"],
        "Real Estate" => &["Property"],
        _ => &[],
    }
}

/// What a group's strip of `width_px` shows: the name, or the longest
/// shorter form of it that fits (cut with "…" only when no form does and
/// enough of it is left to mean something), and whether the change fits
/// beside it.
fn strip_text(width_px: f64, name: &str, change_chars: usize) -> (Option<String>, bool) {
    // 11.5px semibold in mixed case, inside 6px padding.
    const LETTER_PX: f64 = 11.5 * 0.54;
    const FIGURE_PX: f64 = 11.5 * DIGIT_EM;
    let room = width_px - 12.0;
    let wide = |text: &str| text.chars().count() as f64 * LETTER_PX;
    let change = change_chars as f64 * FIGURE_PX + 8.0;
    let forms = || std::iter::once(name).chain(short_names(name).iter().copied());
    if change_chars > 0 {
        if let Some(form) = forms().find(|f| wide(f) + change <= room) {
            return (Some(form.to_string()), true);
        }
    }
    if let Some(form) = forms().find(|f| wide(f) <= room) {
        return (Some(form.to_string()), false);
    }
    // Cut the shortest form, if five letters or more of it remain.
    let shortest = forms().last().unwrap_or(name);
    let fits = (room / LETTER_PX).floor() as usize;
    if fits >= 6 {
        let cut: String = shortest.chars().take(fits - 1).collect();
        return (Some(format!("{}…", cut.trim_end())), false);
    }
    (None, false)
}

/// Squarified treemap (Bruls et al.): lays `values` (sorted descending) into
/// `bounds`, keeping tiles as close to square as possible.
fn squarify(values: &[f64], bounds: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    if total <= 0.0 {
        return vec![];
    }
    let scale = bounds.w * bounds.h / total;
    let areas: Vec<f64> = values.iter().map(|v| v * scale).collect();

    let mut out = Vec::with_capacity(areas.len());
    let mut free = bounds;
    let mut start = 0;
    while start < areas.len() {
        let side = free.w.min(free.h);
        let mut end = start + 1;
        while end < areas.len()
            && worst_ratio(&areas[start..=end], side) <= worst_ratio(&areas[start..end], side)
        {
            end += 1;
        }

        let row = &areas[start..end];
        let row_area: f64 = row.iter().sum();
        if free.w >= free.h {
            // Column on the left edge.
            let col_w = row_area / free.h;
            let mut y = free.y;
            for a in row {
                let h = a / col_w;
                out.push(Rect {
                    x: free.x,
                    y,
                    w: col_w,
                    h,
                });
                y += h;
            }
            free.x += col_w;
            free.w -= col_w;
        } else {
            // Row along the top edge.
            let row_h = row_area / free.w;
            let mut x = free.x;
            for a in row {
                let w = a / row_h;
                out.push(Rect {
                    x,
                    y: free.y,
                    w,
                    h: row_h,
                });
                x += w;
            }
            free.y += row_h;
            free.h -= row_h;
        }
        start = end;
    }
    out
}

/// Worst aspect ratio in a row laid along a side of length `side`.
fn worst_ratio(row: &[f64], side: f64) -> f64 {
    let sum: f64 = row.iter().sum();
    let max = row.iter().cloned().fold(f64::MIN, f64::max);
    let min = row.iter().cloned().fold(f64::MAX, f64::min);
    let s2 = side * side;
    (s2 * max / (sum * sum)).max(sum * sum / (s2 * min))
}

// ─── Colour ───────────────────────────────────────────────────────────────────

/// Which of the nine steps a change falls on: 0 for flat or unknown, 1 to
/// 4 for a rise and -1 to -4 for a fall. A step is a third of
/// `saturation` wide, and the fourth takes everything beyond it.
fn heat_step(change: Option<f64>, saturation: f64) -> i8 {
    let Some(change) = change.filter(|c| c.is_finite()) else {
        return 0;
    };
    // Rounds to 0.00% on a day's scale: nothing happened.
    if change.abs() < saturation / 600.0 {
        return 0;
    }
    let step = (1.0 + (change.abs() / (saturation / 3.0)).floor()).min(4.0) as i8;
    if change > 0.0 {
        step
    } else {
        -step
    }
}

/// The class that paints a step (`input.css`).
fn heat_class(step: i8) -> &'static str {
    match step {
        4.. => "heat-u4",
        3 => "heat-u3",
        2 => "heat-u2",
        1 => "heat-u1",
        0 => "heat-0",
        -1 => "heat-d1",
        -2 => "heat-d2",
        -3 => "heat-d3",
        _ => "heat-d4",
    }
}

fn to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(ticker: &str, size: f64, group: Option<&str>) -> HeatItem {
        HeatItem {
            ticker: ticker.into(),
            name: ticker.into(),
            size,
            change: Some(1.0),
            group: group.map(Into::into),
            detail: String::new(),
        }
    }

    #[test]
    fn groups_keep_members_inside_their_block() {
        let items = [
            item("A", 50.0, Some("Tech")),
            item("B", 30.0, Some("Energy")),
            item("C", 20.0, Some("Tech")),
            item("Z", 0.0, Some("Tech")),
        ];
        let out = layout(&items, 1000.0, 500.0);
        assert_eq!(out.tiles.len(), 3, "zero-size items are dropped");
        assert_eq!(out.groups[0].name, "Tech");
        for tile in &out.tiles {
            let g = out
                .groups
                .iter()
                .find(|g| g.name == group_of(&tile.item))
                .unwrap();
            let (t, b) = (tile.rect, g.rect);
            assert!(t.x >= b.x - 1e-9 && t.x + t.w <= b.x + b.w + 1e-9);
            assert!(t.y >= b.y - 1e-9 && t.y + t.h <= b.y + b.h + 1e-9);
        }
        let ungrouped = layout(&[item("A", 1.0, None), item("B", 3.0, None)], 800.0, 300.0);
        assert!(ungrouped.groups.is_empty());
        assert_eq!(ungrouped.tiles[0].item.ticker, "B");
    }

    #[test]
    fn squarify_fills_bounds_without_overlap() {
        const ASPECT: f64 = 2.4;
        let values = [50.0, 25.0, 12.0, 8.0, 5.0];
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: ASPECT,
            h: 1.0,
        };
        let rects = squarify(&values, bounds);
        assert_eq!(rects.len(), values.len());

        let area: f64 = rects.iter().map(|r| r.w * r.h).sum();
        assert!((area - ASPECT).abs() < 1e-9);
        for (r, v) in rects.iter().zip(values) {
            assert!((r.w * r.h - v / 100.0 * ASPECT).abs() < 1e-9);
            assert!(r.x >= -1e-9 && r.y >= -1e-9);
            assert!(r.x + r.w <= ASPECT + 1e-9 && r.y + r.h <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn tiles_fill_the_map_they_measured() {
        let items = [item("A", 3.0, None), item("B", 1.0, None)];
        let wide = layout(&items, 1200.0, 300.0);
        let phone = layout(&items, 350.0, 560.0);
        for out in [&wide, &phone] {
            let area: f64 = out.tiles.iter().map(|t| t.rect.w * t.rect.h).sum();
            assert!((area - out.size.w * out.size.h).abs() < 1e-6);
        }
        // On a tall phone map the big tile spans the width (350 × 420)
        // rather than a tall column, as a fixed wide layout gave.
        assert_eq!(phone.tiles[0].rect.w, 350.0);
        assert!((phone.tiles[0].rect.h - 420.0).abs() < 1e-9);
    }

    #[test]
    fn a_suffix_every_ticker_shares_is_left_off_the_labels() {
        let set = [item("PTT.BK", 2.0, None), item("KBANK.BK", 1.0, None)];
        assert_eq!(shared_suffix(&set).as_deref(), Some(".BK"));
        let labels: Vec<String> = layout(&set, 800.0, 300.0).tiles.into_iter().map(|t| t.label).collect();
        assert_eq!(labels, ["PTT", "KBANK"]);
        assert_eq!(shared_suffix(&[item("BRK.B", 1.0, None), item("AAPL", 1.0, None)]), None);
        assert_eq!(shared_suffix(&[item("PTT.BK", 1.0, None)]), None, "one ticker keeps its name");
    }

    #[test]
    fn labels_scale_with_their_tile_and_show_only_what_fits() {
        let rect = |w, h| Rect { x: 0.0, y: 0.0, w, h };
        // A big tile: large type, capped, with the company named.
        let big = fit_label(rect(260.0, 180.0), "AAPL", 6, true);
        assert_eq!((big.ticker_px, big.name), (Some(MAX_FONT_PX), true));
        assert!(!fit_label(rect(260.0, 180.0), "AAPL", 6, false).name, "nothing to add to a ticker");
        // A mid tile: both lines, smaller, and the change smaller still.
        let mid = fit_label(rect(70.0, 45.0), "AAPL", 6, true);
        let (t, c) = (mid.ticker_px.unwrap(), mid.change_px.unwrap());
        assert!((12.0..=MAX_FONT_PX).contains(&t) && c >= MIN_FONT_PX && c < t && !mid.name, "{mid:?}");
        // Every size chosen fits the tile, wide letters included.
        for (w, h, ticker) in [(70.0, 45.0, "AAPL"), (48.0, 30.0, "TSLA"), (36.0, 18.0, "IBM"), (120.0, 40.0, "GOOGL"), (50.0, 47.0, "AMGN"), (30.0, 31.0, "MMM")] {
            let l = fit_label(rect(w, h), ticker, 6, false);
            if let Some(t) = l.ticker_px {
                assert!(label_em(ticker) * t <= w - TILE_PADDING_PX + 0.5, "{ticker} {w}x{h}: {l:?}");
                let lines = t + l.change_px.map_or(0.0, |c| c * 1.3);
                assert!(lines <= h - TILE_PADDING_PX + 0.5, "{ticker} {w}x{h}: {l:?}");
            }
            if let Some(c) = l.change_px {
                assert!(6.0 * c * DIGIT_EM <= w - TILE_PADDING_PX + 0.5, "{ticker} {w}x{h}: {l:?}");
            }
        }
        // "MMM" is wider than "TJX": the same tile gives it smaller type.
        assert!(fit_label(rect(40.0, 20.0), "MMM", 6, false).ticker_px < fit_label(rect(40.0, 20.0), "TJX", 6, false).ticker_px);
        // A flat tile has room for the ticker alone.
        let flat = fit_label(rect(70.0, 20.0), "AAPL", 6, false);
        assert!(flat.ticker_px.is_some() && flat.change_px.is_none());
        // "RATCH" in a 30px tile would be cut off: nothing instead.
        assert_eq!(fit_label(rect(30.0, 60.0), "RATCH", 6, false).ticker_px, None);
    }

    #[test]
    fn group_strips_use_a_shorter_name_before_cutting_one() {
        let text = |w, name: &str| strip_text(w, name, 6);
        assert_eq!(text(400.0, "Information Technology"), (Some("Information Technology".into()), true));
        // Too narrow for the full name and the change: the shorter form.
        assert_eq!(text(130.0, "Information Technology"), (Some("Technology".into()), true));
        assert_eq!(text(80.0, "Information Technology"), (Some("Technology".into()), false));
        assert_eq!(text(48.0, "Information Technology"), (Some("Tech".into()), false));
        assert_eq!(text(66.0, "Real Estate"), (Some("Property".into()), false));
        assert_eq!(text(70.0, "Materials"), (Some("Materials".into()), false));
        // A name with no shorter form is cut, but never down to a stub.
        assert_eq!(text(70.0, "Semiconductors"), (Some("Semicond…".into()), false));
        assert_eq!(text(40.0, "Semiconductors"), (None, false));
        assert_eq!(text(110.0, "Energy"), (Some("Energy".into()), true), "a short name shows whole");
    }

    #[test]
    fn changes_fall_on_nine_steps() {
        let step = |c: f64| heat_step(Some(c), DAY_SATURATION);
        assert_eq!([step(0.0), step(0.004), step(-0.004)], [0, 0, 0], "shown as 0.00%");
        assert_eq!([step(0.01), step(0.99), step(1.0), step(2.5), step(3.0), step(40.0)], [1, 1, 2, 3, 4, 4]);
        assert_eq!([step(-0.5), step(-1.2), step(-2.0), step(-9.0)], [-1, -2, -3, -4]);
        assert_eq!(heat_step(None, DAY_SATURATION), 0);
        assert_eq!(heat_step(Some(f64::NAN), DAY_SATURATION), 0);
        // All time runs on a wider scale: 10% a step.
        assert_eq!(heat_step(Some(12.0), TOTAL_SATURATION), 2);
        let classes: std::collections::HashSet<&str> = (-4..=4).map(heat_class).collect();
        assert_eq!(classes.len(), 9);
    }

    #[test]
    fn the_card_stays_inside_the_map() {
        let map = Size { w: 1000.0, h: 560.0 };
        let tile = |x, y, w, h| Rect { x, y, w, h };
        // Room on the right: beside the tile, level with its top.
        assert_eq!(card_place(tile(100.0, 40.0, 80.0, 60.0), map), (188.0, 40.0));
        // At the right edge: on the left instead.
        assert_eq!(card_place(tile(900.0, 40.0, 100.0, 60.0), map).0, 900.0 - 8.0 - CARD_W);
        // At the bottom: pulled up so it isn't cut off.
        assert_eq!(card_place(tile(100.0, 540.0, 20.0, 20.0), map).1, 560.0 - CARD_H);
        // A phone-width map with a tile across it: under the tile.
        let phone = Size { w: 350.0, h: 560.0 };
        assert_eq!(card_place(tile(0.0, 0.0, 350.0, 200.0), phone), (0.0, 208.0));
        // ...or over it when the tile is at the bottom.
        let (x, y) = card_place(tile(0.0, 400.0, 350.0, 160.0), phone);
        assert!((x, y) == (0.0, 400.0 - 8.0 - CARD_H));
        // A map narrower than the card: the card takes the map's width.
        let (x, _) = card_place(tile(0.0, 0.0, 100.0, 100.0), Size { w: 200.0, h: 300.0 });
        assert_eq!(x, 0.0);
    }
}
