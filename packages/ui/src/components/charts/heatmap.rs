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
const GROUP_LABEL_PX: f64 = 18.0;

/// Change (%) at which a tile reaches full green / red.
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
                    detail: format!("{:.1}% of portfolio", value / total * 100.0),
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
/// with a `group` are laid out in labelled blocks, largest group first.
/// The map measures its own width, so tiles are shaped for the screen and
/// carry only the text that fits them.
#[component]
pub fn Treemap(items: ReadSignal<Vec<HeatItem>>, saturation: f64, height: u32) -> Element {
    let mut width = use_signal(|| FALLBACK_WIDTH_PX);
    let layout = use_memo(move || layout(&items.read(), width(), height as f64));
    let dense = items.read().len() > 40;

    if layout.read().tiles.is_empty() {
        return rsx! {
            div {
                class: "flex items-center justify-center text-ctp-overlay1 text-xs",
                style: "height:{height}px;",
                {tr("Waiting for prices…")}
            }
        };
    }
    rsx! {
        div {
            // Dense maps skip rounding: clipping ~500 tiles to rounded corners,
            // and rounding each, doubled the time to paint a scroll frame in
            // the desktop app (WebKit painting without the GPU).
            class: if dense { "relative w-full" } else { "relative w-full overflow-hidden rounded-2xl" },
            style: "height:{height}px;",
            onresize: move |e| {
                if let Ok(size) = e.get_content_box_size() {
                    // Whole pixels, so sub-pixel jitter doesn't redo the layout.
                    let measured = size.width.round();
                    if measured > 0.0 && measured != width() {
                        width.set(measured);
                    }
                }
            },
            for group in layout.read().groups.iter() {
                GroupFrame { key: "g-{group.name}", group: group.clone(), map: layout.read().size }
            }
            for tile in layout.read().tiles.iter() {
                HeatmapTile { key: "{tile.item.ticker}", tile: tile.clone(), map: layout.read().size, saturation, dense }
            }
        }
    }
}

/// A group's backdrop and its label strip: the name and weighted change
/// when both fit, the name alone when only it fits, else nothing (the
/// tooltip still has both).
#[component]
fn GroupFrame(group: GroupLabel, map: Size) -> Element {
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
    let fit = strip_fit(group.rect.w, group.name.chars().count(), change.chars().count());
    rsx! {
        div {
            class: "absolute overflow-hidden rounded-xl border-2 border-ctp-base bg-ctp-crust/70",
            style: "left:{left:.3}%;top:{top:.3}%;width:{width:.3}%;height:{height:.3}%;",
            title: "{group.name} {change}",
            if group.show_label {
                div {
                    class: "flex items-center justify-between gap-2 px-2 text-xs font-semibold uppercase tracking-wide text-ctp-subtext0",
                    style: "height:{GROUP_LABEL_PX}px;",
                    if fit != StripFit::Nothing {
                        span { class: "truncate", "{group.name}" }
                    }
                    if fit == StripFit::NameAndChange {
                        span { class: "tabular-nums {change_class}", "{change}" }
                    }
                }
            }
        }
    }
}

#[component]
fn HeatmapTile(tile: Tile, map: Size, saturation: f64, dense: bool) -> Element {
    let (left, top, width, height) = percent_box(tile.rect, map);
    let item = &tile.item;
    let change = item.change.unwrap_or(0.0);
    let change_text = item
        .change
        .map(|c| format!("{c:+.2}%"))
        .unwrap_or_else(|| "—".into());
    // Text scales with the tile, and only what fits is shown.
    let (ticker_class, change_class, show_change) =
        match label_fit(tile.rect, tile.label.chars().count(), change_text.chars().count()) {
            LabelFit::Large => ("text-base", "text-sm", true),
            LabelFit::Small => ("text-xs", "text-xs", true),
            LabelFit::TickerOnly => ("text-xs", "", false),
            LabelFit::Nothing => ("hidden", "", false),
        };
    let shape = if dense {
        "border-[1.5px]"
    } else {
        "border-[3px] rounded-xl"
    };

    let ticker = TickerSymbol::new(&item.ticker).ok();
    rsx! {
        div {
            // No per-tile filters or transitions: an index map has 500 tiles.
            class: "absolute flex flex-col items-center justify-center overflow-hidden cursor-pointer \
                    {shape} border-transparent bg-clip-padding hover:border-ctp-text/40",
            onclick: move |_| {
                if let Some(t) = &ticker {
                    open_stock(t);
                }
            },
            style: "left:{left:.3}%;top:{top:.3}%;width:{width:.3}%;height:{height:.3}%;\
                    background-color:{change_color(change, saturation)};color:{text_color(change, saturation)};",
            title: "{item.ticker} · {item.name}\n{change_text} · {item.detail}",
            span { class: "font-bold leading-tight whitespace-nowrap {ticker_class}", "{tile.label}" }
            if show_change {
                span { class: "font-semibold tabular-nums opacity-80 {change_class}", "{change_text}" }
            }
        }
    }
}

#[component]
pub fn HeatmapLegend(saturation: f64) -> Element {
    let stops: Vec<f64> = (-2..=2).map(|i| i as f64 * saturation / 2.0).collect();
    rsx! {
        div { class: "flex items-center justify-end gap-1 mt-3 text-xs text-ctp-subtext0 tabular-nums",
            for stop in stops {
                div { class: "flex flex-col items-center gap-1",
                    div { class: "w-10 h-2 rounded-sm", style: "background:{change_color(stop, saturation)};" }
                    "{stop:+.1}%"
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

/// Width of a bold capital letter or digit, as a share of the font size;
/// on the generous side, so text never overflows its tile.
const CHAR_EM: f64 = 0.68;
/// Room a tile's border and edges take, across and down.
const TILE_PADDING_PX: f64 = 10.0;
/// A group strip narrower than this shows nothing.
const MIN_STRIP_PX: f64 = 56.0;
/// Fewer letters of a group's name than this say nothing ("E…").
const MIN_NAME_CHARS: usize = 7;

fn text_px(chars: usize, font_px: f64) -> f64 {
    chars as f64 * font_px * CHAR_EM
}

/// How much of a tile's label fits its tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LabelFit {
    /// Ticker and change, in larger type.
    Large,
    Small,
    TickerOnly,
    Nothing,
}

fn label_fit(rect: Rect, ticker_chars: usize, change_chars: usize) -> LabelFit {
    let fits = |ticker_font: f64, change_font: Option<f64>| {
        let wide = text_px(ticker_chars, ticker_font)
            .max(change_font.map_or(0.0, |f| text_px(change_chars, f)));
        let tall = ticker_font * 1.25 + change_font.map_or(0.0, |f| f * 1.25);
        rect.w >= wide + TILE_PADDING_PX && rect.h >= tall + TILE_PADDING_PX
    };
    if fits(16.0, Some(14.0)) {
        LabelFit::Large
    } else if fits(12.0, Some(12.0)) {
        LabelFit::Small
    } else if fits(12.0, None) {
        LabelFit::TickerOnly
    } else {
        LabelFit::Nothing
    }
}

/// What a group's label strip shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StripFit {
    NameAndChange,
    /// The name, cut short with "…" if it must be.
    Name,
    Nothing,
}

/// Strip text is 12px capitals with wide spacing, inside 8px padding.
fn strip_fit(width_px: f64, name_chars: usize, change_chars: usize) -> StripFit {
    let room = width_px - 16.0;
    let letter = |chars: usize| chars as f64 * 12.0 * (CHAR_EM + 0.05);
    if room >= letter(name_chars) + 8.0 + letter(change_chars) {
        StripFit::NameAndChange
    } else if width_px >= MIN_STRIP_PX && room >= letter(name_chars.min(MIN_NAME_CHARS)) {
        StripFit::Name
    } else {
        StripFit::Nothing
    }
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

/// Blends from the neutral surface toward green / red as `change`
/// approaches ±`saturation`, in the current theme's colours.
fn change_color(change: f64, saturation: f64) -> String {
    let t = (change.abs() / saturation).clamp(0.0, 1.0).sqrt() * 100.0;
    let target = if change >= 0.0 { "green" } else { "red" };
    format!(
        "color-mix(in oklab, var(--catppuccin-color-{target}) {t:.0}%, var(--catppuccin-color-surface2))"
    )
}

/// Dark text once the tile is saturated enough, light text on near-neutral tiles.
fn text_color(change: f64, saturation: f64) -> &'static str {
    if change.abs() / saturation > 0.3 {
        "var(--catppuccin-color-crust)" // ctp-crust
    } else {
        "var(--catppuccin-color-text)" // ctp-text
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
    fn labels_show_only_what_fits() {
        let rect = |w, h| Rect { x: 0.0, y: 0.0, w, h };
        assert_eq!(label_fit(rect(200.0, 120.0), 4, 6), LabelFit::Large);
        assert_eq!(label_fit(rect(70.0, 45.0), 4, 6), LabelFit::Small);
        assert_eq!(label_fit(rect(70.0, 25.0), 4, 6), LabelFit::TickerOnly);
        // "RATCH" in a 40px tile would be cut off: nothing instead.
        assert_eq!(label_fit(rect(40.0, 60.0), 5, 6), LabelFit::Nothing);
    }

    #[test]
    fn group_strips_never_show_a_stub_of_a_name() {
        assert_eq!(strip_fit(400.0, 22, 6), StripFit::NameAndChange);
        assert_eq!(strip_fit(150.0, 22, 6), StripFit::Name);
        assert_eq!(strip_fit(60.0, 11, 6), StripFit::Nothing);
        assert_eq!(strip_fit(90.0, 6, 6), StripFit::Name, "a short name shows whole");
    }
}
