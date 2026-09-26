use dioxus::prelude::*;
use rust_decimal::Decimal;

const MOTION_CSS: Asset = asset!("/assets/styling/motion.css");

/// Injects the number-roll stylesheet. Rendered once from `ui::App`.
#[component]
pub fn MotionStyles() -> Element {
    rsx! {
        document::Stylesheet { href: MOTION_CSS }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Roll {
    Up,
    Down,
}

/// A formatted number that rolls like an odometer when `value` changes:
/// only the characters that differ scroll to their new glyph (down when the
/// value rises, up when it falls); unchanged characters stay put.
///
/// `value` drives the direction; `text` is what is displayed.
#[component]
pub fn LiveNumber(
    value: ReadSignal<Decimal>,
    text: ReadSignal<String>,
    #[props(into, default)] class: String,
) -> Element {
    let mut last = use_signal(|| None::<(Decimal, String)>);
    // Text shown before the latest change, and which way it moved.
    let mut from = use_signal(|| None::<(String, Roll)>);
    let mut generation = use_signal(|| 0u32);

    use_effect(move || {
        let (now, now_text) = (value(), text());
        let before = last.peek().clone();
        // Skip the first value and the jump from an unloaded zero.
        if let Some((prev, prev_text)) = before.filter(|(p, _)| !p.is_zero()) {
            if prev_text != now_text {
                let dir = if now < prev { Roll::Down } else { Roll::Up };
                from.set(Some((prev_text, dir)));
                generation += 1;
            }
        }
        last.set(Some((now, now_text)));
    });

    let current = text();
    let chars: Vec<char> = current.chars().collect();
    let gen = generation();
    // Right-align old vs new so "99.50" → "100.25" compares the right digits.
    let (old_chars, dir) = match from() {
        Some((old, dir)) => {
            let old: Vec<char> = old.chars().collect();
            let shift = old.len() as isize - chars.len() as isize;
            let aligned: Vec<Option<char>> = (0..chars.len() as isize)
                .map(|i| {
                    usize::try_from(i + shift)
                        .ok()
                        .and_then(|j| old.get(j).copied())
                })
                .collect();
            (aligned, Some(dir))
        }
        None => (vec![None; chars.len()], None),
    };
    let dir_class = match dir {
        Some(Roll::Up) => "roll-up",
        Some(Roll::Down) => "roll-down",
        None => "",
    };

    rsx! {
        span { class: "odometer {class}", aria_label: "{current}",
            for (i, ch) in chars.iter().copied().enumerate() {
                if dir.is_some() && old_chars[i] != Some(ch) {
                    // Keyed by generation: a new change remounts and replays the roll.
                    span { key: "{i}-{gen}", class: "odo-cell {dir_class}",
                        span { class: "odo-new", "{ch}" }
                        span { class: "odo-old", aria_hidden: "true",
                            if let Some(old) = old_chars[i] { "{old}" }
                        }
                    }
                } else {
                    span { key: "{i}", class: "odo-char", "{ch}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use std::cell::Cell;

    thread_local! {
        static VALUE: Cell<Option<Signal<Decimal>>> = const { Cell::new(None) };
    }

    #[component]
    fn Harness() -> Element {
        let value = use_signal(|| Decimal::from(100));
        VALUE.with(|v| v.set(Some(value)));
        rsx! {
            LiveNumber { value: value(), text: value().to_string() }
        }
    }

    /// Drain queued work (signal writes, effects) and re-render.
    fn settle(dom: &mut VirtualDom) {
        for _ in 0..10 {
            if dom.wait_for_work().now_or_never().is_none() {
                break;
            }
            dom.render_immediate(&mut dioxus_core::NoOpMutations);
        }
    }

    fn set_value(dom: &mut VirtualDom, n: i64) -> String {
        dom.in_runtime(|| VALUE.with(|v| v.get().unwrap().set(Decimal::from(n))));
        settle(dom);
        dioxus_ssr::render(dom)
    }

    #[test]
    fn rolls_only_changed_digits_in_direction_of_change() {
        let mut dom = VirtualDom::new(Harness);
        dom.rebuild_in_place();
        settle(&mut dom);
        let html = dioxus_ssr::render(&dom);
        assert!(!html.contains("odo-cell"), "{html}");

        // 100 → 105: only the last digit rolls, upward-moving value rolls "up".
        let html = set_value(&mut dom, 105);
        assert_eq!(html.matches("odo-cell roll-up").count(), 1, "{html}");
        assert!(html.contains(">5<") && html.contains(">0<"), "{html}");

        // 105 → 99: both remaining digits differ, value fell so it rolls "down".
        let html = set_value(&mut dom, 99);
        assert_eq!(html.matches("odo-cell roll-down").count(), 2, "{html}");

        // 99 → 100: shorter → longer text still aligns from the right.
        let html = set_value(&mut dom, 100);
        assert_eq!(html.matches("odo-cell roll-up").count(), 3, "{html}");
    }
}
