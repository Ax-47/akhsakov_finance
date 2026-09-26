//! A table of stocks with price, move, size, valuation and 52-week range,
//! shared by the screener, sector lists and peer comparisons.

use crate::i18n::tr;
use crate::{
    app::DataRefresh,
    components::analysis::stock::open_stock,
    format::{fmt_compact, fmt_usd},
};
use dioxus::prelude::*;
use dtos::market::StockRow;
use rust_decimal::Decimal;
use types::ticker_symbol::TickerSymbol;

/// Click a row to open the stock; ☆ adds it to the watchlist.
/// `highlight` marks one ticker, e.g. the stock whose peers these are.
#[component]
pub fn StockTable(rows: Vec<StockRow>, #[props(default)] highlight: Option<String>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "w-full min-w-[46rem] text-sm",
                thead {
                    tr { class: "text-left text-xs text-ctp-subtext0",
                        th { class: "px-6 py-2 font-normal", {tr("Stock")} }
                        th { class: "px-3 py-2 text-right font-normal", {tr("Price")} }
                        th { class: "px-3 py-2 text-right font-normal", {tr("Today")} }
                        th { class: "px-3 py-2 text-right font-normal", {tr("Market cap")} }
                        th { class: "px-3 py-2 text-right font-normal", {tr("P/E")} }
                        th { class: "px-3 py-2 text-right font-normal", {tr("Fwd P/E")} }
                        th { class: "px-3 py-2 text-right font-normal", {tr("Yield")} }
                        th { class: "px-3 py-2 font-normal", "52-week range" }
                        th { class: "w-10" }
                    }
                }
                tbody {
                    for row in rows {
                        StockTableRow {
                            key: "{row.ticker}",
                            highlighted: highlight.as_deref() == Some(row.ticker.as_str()),
                            row,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn StockTableRow(row: StockRow, highlighted: bool) -> Element {
    let refresh = use_context::<DataRefresh>();
    let mut watched = use_signal(|| false);
    let ticker = TickerSymbol::new(&row.ticker).ok();
    let (open, watch) = (ticker.clone(), ticker);
    let change_class = match row.change_pct {
        Some(c) if c >= 0.0 => "text-ctp-green",
        Some(_) => "text-ctp-red",
        None => "text-ctp-overlay1",
    };
    let ratio = |v: Option<f64>| v.filter(|v| *v > 0.0).map_or("—".into(), |v| format!("{v:.1}"));
    let range = match (row.low_52w, row.high_52w) {
        (Some(lo), Some(hi)) if hi > lo => Some(((row.price - lo) / (hi - lo)).clamp(0.0, 1.0) * 100.0),
        _ => None,
    };
    let row_class = if highlighted {
        "cursor-pointer border-t border-ctp-surface0/60 bg-ctp-mauve/10 transition-colors hover:bg-ctp-surface0/40"
    } else {
        "cursor-pointer border-t border-ctp-surface0/60 transition-colors hover:bg-ctp-surface0/30"
    };
    rsx! {
        tr {
            class: row_class,
            onclick: move |_| {
                if let Some(t) = &open {
                    open_stock(t);
                }
            },
            td { class: "px-6 py-2.5",
                div { class: "font-semibold text-ctp-text", "{row.ticker}" }
                div { class: "max-w-[14rem] truncate text-xs text-ctp-subtext0", "{row.name}" }
            }
            td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-text",
                {fmt_usd(Decimal::try_from(row.price).unwrap_or_default(), 2)}
            }
            td { class: "px-3 py-2.5 text-right tabular-nums {change_class}",
                {row.change_pct.map_or("—".into(), |c| format!("{c:+.2}%"))}
            }
            td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1",
                {row.market_cap.map_or("—".into(), fmt_compact)}
            }
            td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1", {ratio(row.pe)} }
            td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1", {ratio(row.forward_pe)} }
            td { class: "px-3 py-2.5 text-right tabular-nums text-ctp-subtext1",
                {row.dividend_yield.filter(|y| *y > 0.0).map_or("—".into(), |y| format!("{:.2}%", y * 100.0))}
            }
            td { class: "px-3 py-2.5",
                if let Some(pos) = range {
                    div { class: "relative h-1.5 w-28 rounded-full bg-ctp-surface0",
                        title: tr("Price within its 52-week low–high"),
                        div { class: "absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-ctp-mauve", style: "left:{pos:.0}%;" }
                    }
                }
            }
            td { class: "pr-4 text-right",
                button {
                    class: "inline-flex h-8 min-w-8 items-center justify-center rounded-full px-2 text-sm text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-yellow",
                    title: tr("Add to watchlist"),
                    onclick: move |e| {
                        e.stop_propagation();
                        let Some(t) = watch.clone() else { return };
                        spawn(async move {
                            if api::watch_ticker(t).await.is_ok() {
                                watched.set(true);
                                refresh.reload();
                            }
                        });
                    },
                    if watched() { "★" } else { "☆" }
                }
            }
        }
    }
}
