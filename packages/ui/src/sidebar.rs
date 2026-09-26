//! App shell with a sidebar. Each app passes its own router links (only it
//! knows its `Route` type), styled with [`NAV_LINK`] / [`NAV_LINK_ACTIVE`].

use crate::i18n::tr;
use crate::{
    app::AppSettings,
    components::card::{Chevron, MenuItem},
    notify::Toasts,
};
use dioxus::prelude::*;
use dtos::settings::CURRENCIES;

/// Classes for a sidebar link; pair with `active_class: NAV_LINK_ACTIVE`.
pub const NAV_LINK: &str = "group flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm font-medium \
                            text-ctp-subtext0 transition-colors hover:bg-ctp-surface0/50 hover:text-ctp-text";
pub const NAV_LINK_ACTIVE: &str = "bg-ctp-surface0! text-ctp-text! [&_svg]:text-ctp-mauve";

/// Sidebar on wide screens, a top bar on narrow ones; `children` is the page.
#[component]
pub fn Sidebar(links: Element, children: Element) -> Element {
    rsx! {
        div { class: "{crate::theme::theme_class()} flex h-screen flex-col overflow-hidden bg-ctp-base text-ctp-text md:flex-row print:block print:h-auto print:overflow-visible",
            // Narrow screens: brand + links in a top bar.
            header { class: "flex items-center gap-4 border-b border-ctp-surface0/70 bg-ctp-mantle px-4 py-3 md:hidden print:hidden",
                Brand {}
                div { class: "min-w-0 flex-1", crate::search::SearchBox {} }
                CurrencyPicker { compact: true }
                nav { class: "flex gap-1", {links.clone()} }
            }
            aside { class: "hidden w-60 shrink-0 flex-col border-r border-ctp-surface0/70 bg-ctp-mantle px-4 py-6 md:flex print:hidden",
                div { class: "px-2", Brand {} }
                div { class: "mt-6", crate::search::SearchBox {} }
                div { class: "mt-6 mb-2 px-3 text-xs text-ctp-overlay0", {tr("Menu")} }
                nav { class: "flex flex-col gap-1", {links} }
                div { class: "mt-auto mb-3", CurrencyPicker { compact: false } }
                div { class: "rounded-2xl border border-ctp-surface0/70 bg-ctp-base/50 p-3",
                    if crate::hooks::use_price_stream::OFFLINE() {
                        div { class: "flex items-center gap-2 text-xs text-ctp-peach",
                            span { class: "h-1.5 w-1.5 rounded-full bg-ctp-peach" }
                            {tr("Offline · last saved prices")}
                        }
                    } else {
                        div { class: "flex items-center gap-2 text-xs text-ctp-subtext0",
                            span { class: "h-1.5 w-1.5 rounded-full bg-ctp-green" }
                            {tr("Prices from Yahoo Finance")}
                        }
                    }
                    div { class: "mt-1 text-[0.7rem] text-ctp-overlay0", {tr("Returns include recorded fees; not tax.")} }
                }
            }
            div { class: "flex-1 overflow-auto print:overflow-visible", {children} }
        }
    }
}

/// Display-currency switcher. `compact` is the top-bar form: just the
/// code, with the menu dropping down instead of up.
#[component]
fn CurrencyPicker(compact: bool) -> Element {
    let settings = use_context::<AppSettings>();
    let toasts = use_context::<Toasts>();
    let mut open = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let current = settings.0.read().currency.clone();
    let symbol = settings.0.read().currency_symbol().trim();

    let button_class = if compact {
        "flex items-center gap-1.5 rounded-full border border-ctp-surface0/70 px-2.5 py-1.5 text-xs font-medium \
         text-ctp-subtext1 cursor-pointer transition-colors hover:border-ctp-surface1 hover:text-ctp-text"
    } else {
        "flex w-full items-center justify-between rounded-xl border border-ctp-surface0/70 px-3 py-2 text-sm \
         text-ctp-subtext1 cursor-pointer transition-colors hover:border-ctp-surface1 hover:text-ctp-text"
    };
    let menu_class = if compact {
        "absolute right-0 top-full z-50 mt-2 w-44"
    } else {
        "absolute inset-x-0 bottom-full z-50 mb-2"
    };
    let value = if busy() {
        "Switching…".to_string()
    } else {
        format!("{symbol} {current}")
    };

    rsx! {
        div { class: "relative",
            button {
                class: button_class,
                title: tr("Display currency"),
                "aria-expanded": open(),
                onclick: move |_| open.toggle(),
                if !compact {
                    span { class: "text-xs text-ctp-overlay1", {tr("Currency")} }
                }
                span { class: "flex items-center gap-2 font-medium tabular-nums", "{value}" Chevron { open: open() } }
            }
            if open() {
                // Click anywhere else to close.
                div { class: "fixed inset-0 z-40", onclick: move |_| open.set(false) }
                div { class: "{menu_class} max-h-80 overflow-auto rounded-2xl border border-ctp-surface0 bg-ctp-mantle p-1.5 shadow-xl shadow-ctp-crust/50",
                    for (code, sym) in CURRENCIES {
                        MenuItem {
                            key: "{code}",
                            label: format!("{} {code}", sym.trim()),
                            selected: code == current,
                            taken: false,
                            onclick: move |_| {
                                open.set(false);
                                spawn(async move {
                                    busy.set(true);
                                    if let Err(message) = settings.change_currency(code).await {
                                        toasts.show("Currency not changed", message);
                                    }
                                    busy.set(false);
                                });
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Brand() -> Element {
    rsx! {
        div { class: "flex items-center gap-3",
            span { class: "flex h-9 w-9 items-center justify-center rounded-xl text-base font-bold text-ctp-crust \
                           bg-gradient-to-br from-ctp-pink via-ctp-mauve to-ctp-sky",
                {tr("A")}
            }
            div { class: "leading-tight",
                div { class: "text-sm font-semibold text-ctp-text", {tr("Akhsakov")} }
                div { class: "text-xs text-ctp-overlay1", {tr("Finance")} }
            }
        }
    }
}

/// Four-square "dashboard" icon.
#[component]
pub fn DashboardIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linejoin: "round",
            rect { x: "3", y: "3", width: "6", height: "6", rx: "1.5" }
            rect { x: "11", y: "3", width: "6", height: "6", rx: "1.5" }
            rect { x: "3", y: "11", width: "6", height: "6", rx: "1.5" }
            rect { x: "11", y: "11", width: "6", height: "6", rx: "1.5" }
        }
    }
}

/// Pie-slice "portfolio" icon.
#[component]
pub fn PortfolioIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6",
            stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M10 3a7 7 0 1 0 7 7h-7z" }
            path { d: "M12.5 2.5a5 5 0 0 1 5 5h-5z" }
        }
    }
}

/// Rising-bars "markets" icon.
#[component]
pub fn MarketIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6",
            stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M3 16.5h14" }
            path { d: "M5.5 13.5v-3M9 13.5v-6M12.5 13.5v-4.5M16 13.5V4.5" }
        }
    }
}

/// Funnel "screener" icon.
#[component]
pub fn ScreenerIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linejoin: "round",
            path { d: "M3 4h14l-5.5 6.5V16l-3 1.5v-7z" }
        }
    }
}

/// Rewind-clock "backtest" icon.
#[component]
pub fn BacktestIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6",
            stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M3.5 10a6.5 6.5 0 1 0 2-4.7" }
            path { d: "M3.5 3.5v3h3" }
            path { d: "M10 6.5V10l2.5 1.5" }
        }
    }
}

/// Calendar icon.
#[component]
pub fn CalendarIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linecap: "round",
            rect { x: "3", y: "4.5", width: "14", height: "12.5", rx: "2" }
            path { d: "M3 8.5h14M7 2.5v4M13 2.5v4" }
        }
    }
}

/// Globe "economy" icon.
#[component]
pub fn EconomyIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6",
            circle { cx: "10", cy: "10", r: "7" }
            path { d: "M3 10h14M10 3c2 2.2 2.8 4.5 2.8 7s-.8 4.8-2.8 7c-2-2.2-2.8-4.5-2.8-7s.8-4.8 2.8-7z" }
        }
    }
}

/// Star "watchlist" icon.
#[component]
pub fn WatchlistIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linejoin: "round",
            path { d: "M10 2.8l2.2 4.6 5 .6-3.7 3.4 1 5-4.5-2.6-4.5 2.6 1-5L2.8 8l5-.6z" }
        }
    }
}

/// Gear "settings" icon.
#[component]
pub fn SettingsIcon() -> Element {
    rsx! {
        svg { class: "h-4.5 w-4.5 shrink-0 text-ctp-overlay1 transition-colors group-hover:text-ctp-text",
            view_box: "0 0 20 20", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linecap: "round",
            circle { cx: "10", cy: "10", r: "2.6" }
            path { d: "M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" }
        }
    }
}
