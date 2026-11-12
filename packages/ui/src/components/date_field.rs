//! A date field with its own calendar.
//!
//! The webview's native date picker is different on every platform and
//! can't be styled. WebKitGTK's is small, shows the date in the system's
//! order (is 03/04 March or April?) and has no way to pick today. This one
//! spells the month out, jumps by month or year, takes a typed date and
//! offers today and yesterday.

use crate::components::card::INPUT;
use crate::format::MONTHS;
use crate::i18n::{tr, tr_str};
use chrono::{Datelike, Duration, Months, NaiveDate};
use dioxus::prelude::*;
use std::rc::Rc;

/// Column headers; weeks start on Sunday.
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
/// Years offered before the current one, and after it (for goals).
const YEARS_BACK: i32 = 50;
const YEARS_AHEAD: i32 = 40;

/// Today in the viewer's time zone, once the webview says (the server
/// would answer in UTC, a day behind in the morning east of Greenwich).
pub fn use_today() -> Signal<Option<NaiveDate>> {
    let mut today = use_signal(|| None);
    use_effect(move || {
        spawn(async move {
            let local = document::eval("return new Date().toLocaleDateString('sv-SE');").join::<String>().await;
            if let Some(date) = local.ok().and_then(|d| parse_iso(&d)) {
                today.set(Some(date));
            }
        });
    });
    today
}

fn parse_iso(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// `Wed 4 Nov 2026`, in the app's language.
fn spelled(date: NaiveDate) -> String {
    let weekday = tr_str(WEEKDAYS[date.weekday().num_days_from_sunday() as usize]);
    let month = tr_str(MONTHS[date.month0() as usize]);
    format!("{weekday} {} {month} {}", date.day(), date.year())
}

/// A date as typed, day first: `4/11/2026`, `4-11-2026`, `4 11 2026`,
/// `4/11` (this year) or `2026-11-04`. Buddhist-era years are understood.
fn parse_typed(text: &str, this_year: i32) -> Option<NaiveDate> {
    let parts: Vec<&str> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .collect();
    let joined = match parts.as_slice() {
        [day, month] => format!("{day}/{month}/{this_year}"),
        _ => parts.join("/"),
    };
    parse_iso(&dtos::csv_import::parse_date(&joined, true)?)
}

/// The six weeks shown for a month, starting on the Sunday on or before
/// its first day.
fn month_grid(year: i32, month: u32) -> Vec<NaiveDate> {
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return vec![];
    };
    let start = first - Duration::days(first.weekday().num_days_from_sunday().into());
    (0..42).map(|i| start + Duration::days(i)).collect()
}

/// `value` is `YYYY-MM-DD`, or empty when no date is chosen.
#[component]
pub fn DateField(
    label: String,
    value: String,
    on_change: EventHandler<String>,
    /// Offer today and yesterday, for things that have already happened.
    #[props(default)]
    recent: bool,
    /// The date can be cleared.
    #[props(default)]
    optional: bool,
) -> Element {
    let mut open = use_signal(|| false);
    let mut anchor = use_signal(|| None::<Rc<MountedData>>);
    let mut place = use_signal(|| None::<Place>);
    // The month on show; `None` follows the chosen date.
    let mut view = use_signal(|| None::<NaiveDate>);
    let mut typed = use_signal(String::new);
    let mut mistyped = use_signal(|| false);
    let today = use_today();

    let selected = parse_iso(&value);
    let shown = selected.map(spelled);
    let month = view()
        .or(selected)
        .or(today())
        .and_then(|d| d.with_day(1));
    let this_year = today().or(selected).map_or(2000, |d| d.year());
    let years = (this_year - YEARS_BACK).min(month.map_or(this_year, |m| m.year()))
        ..=(this_year + YEARS_AHEAD).max(month.map_or(this_year, |m| m.year()));

    let mut choose = move |date: Option<NaiveDate>| {
        on_change.call(date.map(|d| d.to_string()).unwrap_or_default());
        open.set(false);
    };
    let shortcuts = recent || (optional && selected.is_some());
    let toggle = move |_| async move {
        if open() {
            return open.set(false);
        }
        // Fixed to the window, so a dialog that scrolls can't clip it.
        let rect = match anchor() {
            Some(field) => field.get_client_rect().await.ok(),
            None => None,
        };
        let window = document::eval("return window.innerHeight;").join::<f64>().await.ok();
        place.set(rect.map(|r| Place::for_field(r.min_x(), r.min_y(), r.max_y(), window, shortcuts)));
        view.set(None);
        typed.set(String::new());
        mistyped.set(false);
        open.set(true);
    };
    let position = match place() {
        Some(place) => place.style(),
        None => "left: 50%; top: 50%; transform: translate(-50%, -50%);".to_string(),
    };
    let chip = "min-h-8 rounded-full border border-ctp-surface1 px-3 py-1 text-xs font-medium text-ctp-subtext1 \
                cursor-pointer transition-colors hover:border-ctp-mauve hover:text-ctp-text";
    let arrow = "flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-lg text-ctp-subtext0 \
                 cursor-pointer transition-colors hover:bg-ctp-surface0 hover:text-ctp-text";
    let jump = "rounded-lg bg-transparent px-1.5 py-1 text-sm font-semibold text-ctp-text cursor-pointer \
                outline-none transition-colors hover:bg-ctp-surface0 focus-visible:bg-ctp-surface0 [&_option]:bg-ctp-mantle";

    rsx! {
        // Not a <label>: a click anywhere in one presses its first button,
        // which would reopen the calendar as it closes.
        div {
            onkeydown: move |e| {
                if open() && e.key() == Key::Escape {
                    e.stop_propagation();
                    open.set(false);
                }
            },
            span { class: "mb-1.5 block text-xs text-ctp-subtext0", "{label}" }
            button {
                r#type: "button",
                class: if open() {
                    "{INPUT} flex items-center justify-between gap-2 border-ctp-mauve text-left cursor-pointer"
                } else {
                    "{INPUT} flex items-center justify-between gap-2 text-left cursor-pointer hover:border-ctp-surface1"
                },
                "aria-label": "{label}",
                "aria-haspopup": "dialog",
                aria_expanded: open(),
                onmounted: move |e| anchor.set(Some(e.data())),
                onclick: toggle,
                if let Some(shown) = shown {
                    span { class: "truncate tabular-nums", "{shown}" }
                } else {
                    span { class: "truncate text-ctp-overlay1", {tr("Choose a date")} }
                }
                svg {
                    class: "h-4 w-4 shrink-0 text-ctp-subtext0",
                    view_box: "0 0 20 20",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "1.6",
                    stroke_linecap: "round",
                    rect { x: "3", y: "4.5", width: "14", height: "12.5", rx: "2" }
                    path { d: "M3 8.5h14M7 2.5v4M13 2.5v4" }
                }
            }
            if open() {
                // Esc presses this, wherever the focus is (see `Modal`).
                div { class: "fixed inset-0 z-[60]", "data-popover-backdrop": "true", onclick: move |_| open.set(false) }
                div {
                    class: "fixed z-[70] w-[19rem] max-w-[calc(100vw-1rem)] max-h-[calc(100dvh-1rem)] overflow-y-auto rounded-2xl \
                            border border-ctp-surface0 bg-ctp-mantle p-3 shadow-2xl shadow-ctp-crust/60",
                    style: "{position}",
                    role: "dialog",
                    "aria-label": "{label}",
                    if shortcuts {
                        div { class: "mb-2 flex flex-wrap items-center gap-1.5",
                            if recent {
                                if let Some(today) = today() {
                                    button { r#type: "button", class: chip, onclick: move |_| choose(Some(today)), {tr("Today")} }
                                    button {
                                        r#type: "button",
                                        class: chip,
                                        onclick: move |_| choose(Some(today - Duration::days(1))),
                                        {tr("Yesterday")}
                                    }
                                }
                            }
                            if optional && selected.is_some() {
                                button { r#type: "button", class: "{chip} ml-auto", onclick: move |_| choose(None), {tr("Clear")} }
                            }
                        }
                    }
                    if let Some(month) = month {
                        div { class: "flex items-center gap-1",
                            button {
                                r#type: "button",
                                class: arrow,
                                "aria-label": tr("Previous month"),
                                onclick: move |_| view.set(month.checked_sub_months(Months::new(1))),
                                "‹"
                            }
                            div { class: "flex flex-1 items-center justify-center gap-1",
                                select {
                                    class: jump,
                                    "aria-label": tr("Month"),
                                    onchange: move |e| {
                                        if let Some(m) = e.value().parse().ok().and_then(|m| month.with_month(m)) {
                                            view.set(Some(m));
                                        }
                                    },
                                    for (i, name) in MONTHS.iter().enumerate() {
                                        option { key: "{name}", value: "{i + 1}", selected: month.month0() as usize == i, {tr_str(name)} }
                                    }
                                }
                                select {
                                    class: "{jump} tabular-nums",
                                    "aria-label": tr("Year"),
                                    onchange: move |e| {
                                        if let Some(y) = e.value().parse().ok().and_then(|y| month.with_year(y)) {
                                            view.set(Some(y));
                                        }
                                    },
                                    for year in years {
                                        option { key: "{year}", value: "{year}", selected: month.year() == year, "{year}" }
                                    }
                                }
                            }
                            button {
                                r#type: "button",
                                class: arrow,
                                "aria-label": tr("Next month"),
                                onclick: move |_| view.set(month.checked_add_months(Months::new(1))),
                                "›"
                            }
                        }
                        div { class: "mt-1 grid grid-cols-7 justify-items-center gap-y-0.5",
                            for name in WEEKDAYS {
                                span { key: "{name}", class: "flex h-7 items-center text-xs text-ctp-overlay1", {tr_str(name)} }
                            }
                            for day in month_grid(month.year(), month.month()) {
                                button {
                                    key: "{day}",
                                    r#type: "button",
                                    class: day_class(day, month, selected, today()),
                                    "aria-label": spelled(day),
                                    aria_pressed: selected == Some(day),
                                    onclick: move |_| choose(Some(day)),
                                    "{day.day()}"
                                }
                            }
                        }
                    }
                    input {
                        class: if mistyped() {
                            "{INPUT} mt-2 border-ctp-red tabular-nums"
                        } else {
                            "{INPUT} mt-2 tabular-nums"
                        },
                        inputmode: "numeric",
                        placeholder: tr("Or type it: day/month/year"),
                        "aria-label": tr("Or type it: day/month/year"),
                        value: "{typed}",
                        oninput: move |e| {
                            typed.set(e.value());
                            mistyped.set(false);
                            // Show the month being typed as soon as it's a date.
                            if let Some(date) = parse_typed(&e.value(), this_year) {
                                view.set(Some(date));
                            }
                        },
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                e.prevent_default();
                                match parse_typed(&typed(), this_year) {
                                    Some(date) => choose(Some(date)),
                                    None => mistyped.set(true),
                                }
                            }
                        },
                    }
                    if mistyped() {
                        p { class: "mt-1 text-xs text-ctp-red", {tr("That isn't a date. Try 4/11/2026.")} }
                    }
                }
            }
        }
    }
}

/// Where the calendar opens, in window pixels: under the field, or above
/// it when only that side has room.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    /// `height`: the calendar's, to keep it above the window's bottom edge.
    Below { left: f64, top: f64, height: f64 },
    Above { left: f64, bottom: f64 },
}

impl Place {
    /// Space between the field and the calendar.
    const GAP: f64 = 6.0;

    /// `shortcuts`: the calendar has its row of shortcut buttons.
    fn for_field(left: f64, top: f64, bottom: f64, window_height: Option<f64>, shortcuts: bool) -> Self {
        // The calendar's height, with room for the "not a date" line.
        let height = if shortcuts { 430.0 } else { 390.0 };
        match window_height {
            Some(window) if bottom + Self::GAP + height > window && top - Self::GAP - height >= 0.0 => {
                Place::Above { left, bottom: window - top + Self::GAP }
            }
            _ => Place::Below { left, top: bottom + Self::GAP, height },
        }
    }

    /// Kept inside the window: with room on neither side it covers the
    /// field rather than run off the screen.
    fn style(self) -> String {
        let across = |left: f64| format!("left: max(0.5rem, min({left}px, calc(100vw - 19.5rem)));");
        match self {
            Place::Below { left, top, height } => {
                format!("{} top: max(0.5rem, min({top}px, calc(100dvh - {height}px - 0.5rem)));", across(left))
            }
            Place::Above { left, bottom } => format!("{} bottom: {bottom}px;", across(left)),
        }
    }
}

fn day_class(day: NaiveDate, month: NaiveDate, selected: Option<NaiveDate>, today: Option<NaiveDate>) -> &'static str {
    let in_month = day.month() == month.month() && day.year() == month.year();
    if selected == Some(day) {
        "h-9 w-9 rounded-full bg-ctp-mauve text-sm font-semibold tabular-nums text-ctp-crust cursor-pointer"
    } else if today == Some(day) {
        "h-9 w-9 rounded-full border border-ctp-mauve/70 text-sm font-semibold tabular-nums text-ctp-text cursor-pointer \
         transition-colors hover:bg-ctp-surface0"
    } else if in_month {
        "h-9 w-9 rounded-full text-sm tabular-nums text-ctp-text cursor-pointer transition-colors hover:bg-ctp-surface0"
    } else {
        "h-9 w-9 rounded-full text-sm tabular-nums text-ctp-overlay0 cursor-pointer transition-colors \
         hover:bg-ctp-surface0 hover:text-ctp-text"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn typed_dates_are_day_first() {
        let nov_4 = Some(date(2026, 11, 4));
        for text in ["4/11/2026", "04-11-2026", "4.11.2026", "4 11 2026", " 4 / 11 ", "2026-11-04", "4/11/2569"] {
            assert_eq!(parse_typed(text, 2026), nov_4, "{text}");
        }
        for text in ["", "4", "31/2/2026", "11/13/2026", "soon", "4/11/26"] {
            assert_eq!(parse_typed(text, 2026), None, "{text}");
        }
    }

    #[test]
    fn a_month_is_six_weeks_from_a_sunday() {
        let november = month_grid(2026, 11);
        assert_eq!(november.len(), 42);
        assert_eq!((november[0], november[41]), (date(2026, 11, 1), date(2026, 12, 12)), "starts on a Sunday");
        assert_eq!(month_grid(2026, 10)[0], date(2026, 9, 27));
        assert_eq!(spelled(date(2026, 11, 4)), "Wed 4 Nov 2026");
    }

    #[test]
    fn the_calendar_opens_where_it_fits() {
        // Room below: under the field.
        assert_eq!(
            Place::for_field(40.0, 300.0, 340.0, Some(900.0), true),
            Place::Below { left: 40.0, top: 346.0, height: 430.0 }
        );
        // Near the bottom of the window: above the field.
        assert_eq!(Place::for_field(40.0, 700.0, 740.0, Some(900.0), true), Place::Above { left: 40.0, bottom: 206.0 });
        // A short window with room on neither side, or its height unknown:
        // below, where the style keeps it on screen.
        let below = Place::Below { left: 40.0, top: 346.0, height: 390.0 };
        assert_eq!(Place::for_field(40.0, 300.0, 340.0, Some(500.0), false), below);
        assert_eq!(
            Place::for_field(40.0, 700.0, 740.0, None, false),
            Place::Below { left: 40.0, top: 746.0, height: 390.0 }
        );
        assert!(below.style().ends_with("top: max(0.5rem, min(346px, calc(100dvh - 390px - 0.5rem)));"));
        assert!(Place::Above { left: 40.0, bottom: 206.0 }.style().ends_with("bottom: 206px;"));
    }

    #[component]
    fn Harness() -> Element {
        rsx! {
            DateField { label: "Date", value: "2026-11-04", on_change: |_| {} }
            DateField { label: "Review on", value: "", on_change: |_| {}, optional: true }
        }
    }

    #[test]
    fn shows_the_date_spelled_out() {
        let mut dom = VirtualDom::new(Harness);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(html.contains("Wed 4 Nov 2026"), "{html}");
        assert!(html.contains("Choose a date"), "{html}");
        assert!(!html.contains("role=\"dialog\""), "the calendar opens on demand");
        assert!(!html.contains("<label"), "a label would press the field again on every click inside");
    }
}
