use crate::i18n::tr;
use crate::components::{
    analysis::stock::open_stock,
    card::{Card, Segmented, ToggleButton},
};
use dioxus::prelude::*;
use dtos::Position;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use types::ticker_symbol::TickerSymbol;

/// Width / height ratio the layout is computed for. Tiles stretch with the
/// container, so this only needs to be roughly right.
const ASPECT: f64 = 2.4;
const HEIGHT_PX: u32 = 300;
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
#[component]
pub fn Treemap(items: ReadSignal<Vec<HeatItem>>, saturation: f64, height: u32) -> Element {
    let layout = use_memo(move || layout(&items.read(), height as f64));
    let dense = items.read().len() > 40;

    if layout.read().tiles.is_empty() {
        return rsx! {
            div {
                class: "flex items-center justify-center text-ctp-overlay0 text-xs",
                style: "height:{height}px;",
                {tr("Waiting for prices…")}
            }
        };
    }
    rsx! {
        div {
            class: "relative w-full overflow-hidden rounded-2xl",
            style: "height:{height}px;",
            for group in layout.read().groups.iter() {
                GroupFrame { key: "g-{group.name}", group: group.clone() }
            }
            for tile in layout.read().tiles.iter() {
                HeatmapTile { key: "{tile.item.ticker}", tile: tile.clone(), saturation, dense }
            }
        }
    }
}

/// A group's backdrop and its label strip (name and weighted change).
#[component]
fn GroupFrame(group: GroupLabel) -> Element {
    let Rect { x, y, w, h } = group.rect;
    let (left, top, width, height) = (x / ASPECT * 100.0, y * 100.0, w / ASPECT * 100.0, h * 100.0);
    let change = group
        .change
        .map(|c| format!("{c:+.2}%"))
        .unwrap_or_default();
    let change_class = match group.change {
        Some(c) if c >= 0.0 => "text-ctp-green",
        Some(_) => "text-ctp-red",
        None => "",
    };
    rsx! {
        div {
            class: "absolute overflow-hidden rounded-xl border-2 border-ctp-base bg-ctp-crust/70",
            style: "left:{left:.3}%;top:{top:.3}%;width:{width:.3}%;height:{height:.3}%;",
            if group.show_label {
                div {
                    class: "flex items-center justify-between gap-2 px-2 text-[0.62rem] font-semibold uppercase tracking-wide text-ctp-subtext0",
                    style: "height:{GROUP_LABEL_PX}px;",
                    span { class: "truncate", "{group.name}" }
                    span { class: "tabular-nums {change_class}", "{change}" }
                }
            }
        }
    }
}

#[component]
fn HeatmapTile(tile: Tile, saturation: f64, dense: bool) -> Element {
    let Rect { x, y, w, h } = tile.rect;
    let left = x / ASPECT * 100.0;
    let width = w / ASPECT * 100.0;
    let top = y * 100.0;
    let height = h * 100.0;
    let item = &tile.item;
    let change = item.change.unwrap_or(0.0);
    let change_text = item
        .change
        .map(|c| format!("{c:+.2}%"))
        .unwrap_or_else(|| "—".into());
    // Text scales with the tile; tiny tiles show nothing.
    let (ticker_class, change_class, show_change) = if width > 12.0 && height > 18.0 {
        ("text-base", "text-sm", true)
    } else if width > 6.0 && height > 10.0 {
        ("text-xs", "text-[0.65rem]", true)
    } else if width > 3.2 && height > 5.0 {
        ("text-[0.6rem]", "", false)
    } else {
        ("hidden", "", false)
    };
    let shape = if dense {
        "border-[1.5px] rounded-md"
    } else {
        "border-[3px] rounded-xl"
    };

    let ticker = TickerSymbol::new(&item.ticker).ok();
    rsx! {
        div {
            class: "absolute flex flex-col items-center justify-center overflow-hidden cursor-pointer \
                    {shape} border-transparent bg-clip-padding \
                    transition-[background-color] duration-500 hover:brightness-110",
            onclick: move |_| {
                if let Some(t) = &ticker {
                    open_stock(t);
                }
            },
            style: "left:{left:.3}%;top:{top:.3}%;width:{width:.3}%;height:{height:.3}%;\
                    background-color:{change_color(change, saturation)};color:{text_color(change, saturation)};",
            title: "{item.ticker} · {item.name}\n{change_text} · {item.detail}",
            span { class: "font-bold leading-tight {ticker_class}", "{item.ticker}" }
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
        div { class: "flex items-center justify-end gap-1 mt-3 text-[0.68rem] text-ctp-subtext0 tabular-nums",
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

#[derive(Clone, Debug, PartialEq)]
struct Tile {
    item: HeatItem,
    rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
struct GroupLabel {
    name: String,
    rect: Rect,
    /// Size-weighted change of the group's tiles.
    change: Option<f64>,
    show_label: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Layout {
    tiles: Vec<Tile>,
    groups: Vec<GroupLabel>,
}

/// Places `items` in the unit box (ASPECT × 1) of a map `height_px` tall.
fn layout(items: &[HeatItem], height_px: f64) -> Layout {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        w: ASPECT,
        h: 1.0,
    };
    let mut items: Vec<&HeatItem> = items.iter().filter(|i| i.size > 0.0).collect();
    if items.iter().all(|i| i.group.is_none()) {
        return Layout {
            tiles: place(&mut items, bounds),
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
    let label_h = GROUP_LABEL_PX / height_px;

    let mut out = Layout::default();
    for ((name, _), rect) in groups.into_iter().zip(squarify(&sizes, bounds)) {
        let mut members: Vec<&HeatItem> = items
            .iter()
            .copied()
            .filter(|i| group_of(i) == name)
            .collect();
        // Only label blocks with room for the strip and some tiles below it.
        let show_label = rect.h > label_h * 3.0 && rect.w / ASPECT > 0.06;
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
        out.tiles.extend(place(&mut members, inner));
    }
    out
}

fn group_of(item: &HeatItem) -> &str {
    item.group.as_deref().unwrap_or("Other")
}

/// Squarifies `items` (largest first) into `bounds`.
fn place(items: &mut [&HeatItem], bounds: Rect) -> Vec<Tile> {
    items.sort_by(|a, b| b.size.total_cmp(&a.size));
    let sizes: Vec<f64> = items.iter().map(|i| i.size).collect();
    squarify(&sizes, bounds)
        .into_iter()
        .zip(items.iter())
        .map(|(rect, item)| Tile {
            item: (*item).clone(),
            rect,
        })
        .collect()
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
        let out = layout(&items, 500.0);
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
        let ungrouped = layout(&[item("A", 1.0, None), item("B", 3.0, None)], 300.0);
        assert!(ungrouped.groups.is_empty());
        assert_eq!(ungrouped.tiles[0].item.ticker, "B");
    }

    #[test]
    fn squarify_fills_bounds_without_overlap() {
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
}
