//! Ticker search: type a symbol or company name, pick a result to open its
//! stock page. Enter opens the typed symbol directly.

use crate::i18n::tr;
use crate::{components::analysis::stock::open_stock, notify::sleep_ms};
use dioxus::prelude::*;
use types::ticker_symbol::TickerSymbol;

/// Pause after typing before searching, so we don't query every keystroke.
const DEBOUNCE_MS: u32 = 250;

#[component]
pub fn SearchBox() -> Element {
    let mut query = use_signal(String::new);
    let mut focused = use_signal(|| false);

    // Restarting the resource cancels the previous (debouncing) request.
    let results = use_resource(move || async move {
        let q = query().trim().to_string();
        if q.is_empty() {
            return Some(vec![]);
        }
        sleep_ms(DEBOUNCE_MS).await;
        api::quote::quote::search_tickers(q).await.ok()
    });

    let mut go = move |symbol: &str| {
        if let Ok(t) = TickerSymbol::new(symbol) {
            query.set(String::new());
            focused.set(false);
            open_stock(&t);
        }
    };
    let show = focused() && !query.read().trim().is_empty();

    rsx! {
        div { class: "relative",
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    let q = query();
                    go(q.trim());
                },
                input {
                    class: "w-full rounded-xl border border-ctp-surface0 bg-ctp-base/60 px-3 py-2 text-sm text-ctp-text \
                            placeholder:text-ctp-overlay0 outline-none transition-colors focus:border-ctp-mauve",
                    r#type: "search",
                    placeholder: tr("Search stocks…"),
                    "aria-label": "Search stocks",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                    onfocus: move |_| focused.set(true),
                    onkeydown: move |e| {
                        if e.key() == Key::Escape {
                            focused.set(false);
                        }
                    },
                }
            }
            if show {
                div { class: "fixed inset-0 z-20", onclick: move |_| focused.set(false) }
                div { class: "absolute left-0 right-0 top-full z-30 mt-2 min-w-64 overflow-hidden rounded-2xl border border-ctp-surface0 \
                              bg-ctp-mantle p-1.5 shadow-2xl shadow-ctp-crust/60",
                    match results.read().clone() {
                        None => rsx! { p { class: "px-3 py-2 text-xs text-ctp-overlay1", {tr("Searching…")} } },
                        Some(None) => rsx! { p { class: "px-3 py-2 text-xs text-ctp-overlay1", {tr("Search is unavailable right now. Press Enter to open the symbol.")} } },
                        Some(Some(hits)) if hits.is_empty() => rsx! {
                            p { class: "px-3 py-2 text-xs text-ctp-overlay1", "No matches. Press Enter to open “{query}”." }
                        },
                        Some(Some(hits)) => rsx! {
                            for hit in hits {
                                button {
                                    key: "{hit.symbol}",
                                    class: "flex w-full items-center justify-between gap-3 rounded-xl px-3 py-2 text-left cursor-pointer transition-colors hover:bg-ctp-surface0/60",
                                    onclick: {
                                        let symbol = hit.symbol.clone();
                                        move |_| go(&symbol)
                                    },
                                    span { class: "min-w-0",
                                        span { class: "block text-sm font-semibold text-ctp-text", "{hit.symbol}" }
                                        span { class: "block truncate text-xs text-ctp-overlay1", {hit.name.clone().unwrap_or_default()} }
                                    }
                                    span { class: "shrink-0 text-[0.7rem] text-ctp-overlay0",
                                        {[hit.exchange.clone().unwrap_or_default(), hit.kind.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")}
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}
