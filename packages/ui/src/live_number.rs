use dioxus::prelude::*;
use rust_decimal::Decimal;

const MOTION_CSS: Asset = asset!("/assets/styling/motion.css");

/// Injects the tick-animation stylesheet. Rendered once from `ui::App`.
#[component]
pub fn MotionStyles() -> Element {
    rsx! {
        document::Stylesheet { href: MOTION_CSS }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tick {
    None,
    Up,
    Down,
}

/// A formatted number that animates whenever `value` changes:
/// it slides in from the direction of the move and briefly flashes
/// Catppuccin green (up) or red (down) before settling to its normal colour.
///
/// `value` drives the direction; `text` is what is displayed.
#[component]
pub fn LiveNumber(
    value: ReadSignal<Decimal>,
    text: String,
    #[props(into, default)] class: String,
) -> Element {
    let mut prev = use_signal(|| None::<Decimal>);
    let mut tick = use_signal(|| Tick::None);
    let mut generation = use_signal(|| 0u32);

    use_effect(move || {
        let now = value();
        let before = *prev.peek();
        // Skip the first value and the jump from an unloaded zero.
        if let Some(before) = before.filter(|b| !b.is_zero()) {
            if now != before {
                tick.set(if now > before { Tick::Up } else { Tick::Down });
                generation += 1;
            }
        }
        prev.set(Some(now));
    });

    let dir = match tick() {
        Tick::Up => "tick-up",
        Tick::Down => "tick-down",
        Tick::None => "",
    };

    rsx! {
        span { class: "tick {class}",
            // Single keyed item: a new key remounts the span, replaying the animation.
            for g in [generation()] {
                span { key: "{g}", class: "tick-value {dir}", "{text}" }
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
    fn ticks_in_direction_of_change() {
        let mut dom = VirtualDom::new(Harness);
        dom.rebuild_in_place();
        settle(&mut dom);
        let html = dioxus_ssr::render(&dom);
        assert!(!html.contains("tick-up") && !html.contains("tick-down"), "{html}");

        let html = set_value(&mut dom, 105);
        assert!(html.contains("tick-up") && html.contains(">105<"), "{html}");

        let html = set_value(&mut dom, 99);
        assert!(html.contains("tick-down") && html.contains(">99<"), "{html}");
    }
}
