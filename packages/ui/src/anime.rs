use dioxus::prelude::*;

const ANIME_CSS: Asset = asset!("/assets/styling/anime.css");

/// Things the mascot says when poked, cycled in order.
const CHEERS: [&str; 6] = [
    "Nya~! Diversify, diversify! ✨",
    "Buy low, nap high~ 🐾",
    "Compound interest is real magic! 🌸",
    "Stay calm~ Markets always wobble!",
    "Snack break? 🍡 Then rebalance!",
    "Time in the market > timing the market nya!",
];

/// Injects the anime theme stylesheet. Rendered once from `ui::App`.
#[component]
pub fn AnimeTheme() -> Element {
    rsx! {
        document::Stylesheet { href: ANIME_CSS }
    }
}

/// Fixed, non-interactive night sky behind the app: drifting glow blobs,
/// twinkling stars and falling sakura petals. Pure CSS animation.
#[component]
pub fn AnimeSky(
    #[props(default = 18)] petals: usize,
    #[props(default = 28)] stars: usize,
) -> Element {
    rsx! {
        div { class: "ak-sky", aria_hidden: "true",
            div { class: "ak-blob b1" }
            div { class: "ak-blob b2" }
            div { class: "ak-blob b3" }
            for i in 0..stars {
                div {
                    key: "star-{i}",
                    class: "ak-star",
                    style: "left:{scatter(i, 7) % 100}%;top:{scatter(i, 13) % 70}%;animation-delay:-{scatter(i, 3) % 40 / 10}.{scatter(i, 5) % 10}s;",
                }
            }
            for i in 0..petals {
                div {
                    key: "petal-{i}",
                    class: "ak-petal",
                    style: "left:{scatter(i, 11) % 100}%;--fall:{10 + scatter(i, 17) % 10}s;--sway:{2 + scatter(i, 19) % 3}s;--delay:-{scatter(i, 23) % 18}s;scale:{6 + scatter(i, 29) % 7}0%;",
                }
            }
        }
    }
}

/// Chibi neko mascot with a speech bubble. Click her to get a new line and a hop.
#[component]
pub fn Mascot(
    /// Initial line; clicking cycles through built-in cheers.
    #[props(into, default)]
    message: Option<String>,
    #[props(default = 72)] size: u32,
    /// Put the bubble above the mascot instead of to her right.
    #[props(default)]
    stacked: bool,
) -> Element {
    let mut pokes = use_signal(|| 0usize);

    let line = match (pokes(), message.as_deref()) {
        (0, Some(m)) => m.to_string(),
        (n, _) => CHEERS[n % CHEERS.len()].to_string(),
    };

    rsx! {
        div { class: if stacked { "ak-mascot stacked" } else { "ak-mascot" },
            // Keyed single-item loops remount on each poke so the CSS
            // animations (hop, bubble pop) replay.
            for n in [pokes()] {
                svg {
                    key: "neko-{n}",
                    class: if n > 0 { "ak-mascot-svg happy" } else { "ak-mascot-svg" },
                    width: "{size}",
                    height: "{size}",
                    view_box: "0 0 100 100",
                    onclick: move |_| pokes += 1,
                    NekoArt {}
                }
            }
            for n in [pokes()] {
                div { key: "bubble-{n}", class: "ak-bubble", "{line}" }
            }
        }
    }
}

#[component]
fn NekoArt() -> Element {
    rsx! {
        defs {
            linearGradient { id: "ak-fur", x1: "0", y1: "0", x2: "0", y2: "1",
                stop { offset: "0%", stop_color: "#fff5fb" }
                stop { offset: "100%", stop_color: "#f5d3ec" }
            }
            linearGradient { id: "ak-iris", x1: "0", y1: "0", x2: "0", y2: "1",
                stop { offset: "0%", stop_color: "#5b3fa0" }
                stop { offset: "100%", stop_color: "#b48cf2" }
            }
        }
        // tail
        path {
            class: "ak-tail",
            d: "M70 84 C88 82 94 66 86 56",
            stroke: "#f0c4e3",
            stroke_width: "7",
            stroke_linecap: "round",
            fill: "none",
        }
        // body
        ellipse { cx: "50", cy: "84", rx: "20", ry: "13", fill: "url(#ak-fur)", stroke: "#e9b3d9", stroke_width: "1.5" }
        ellipse { cx: "41", cy: "94", rx: "5", ry: "3.5", fill: "#fff5fb", stroke: "#e9b3d9", stroke_width: "1.2" }
        ellipse { cx: "59", cy: "94", rx: "5", ry: "3.5", fill: "#fff5fb", stroke: "#e9b3d9", stroke_width: "1.2" }
        // ears
        g { class: "ak-ear-l",
            path { d: "M22 36 L24 8 L44 24 Z", fill: "url(#ak-fur)", stroke: "#e9b3d9", stroke_width: "1.5", stroke_linejoin: "round" }
            path { d: "M26 30 L27 15 L38 24 Z", fill: "#f7a8d3" }
        }
        g { class: "ak-ear-r",
            path { d: "M78 36 L76 8 L56 24 Z", fill: "url(#ak-fur)", stroke: "#e9b3d9", stroke_width: "1.5", stroke_linejoin: "round" }
            path { d: "M74 30 L73 15 L62 24 Z", fill: "#f7a8d3" }
        }
        // head
        ellipse { cx: "50", cy: "46", rx: "32", ry: "28", fill: "url(#ak-fur)", stroke: "#e9b3d9", stroke_width: "1.5" }
        // bow
        g { transform: "translate(66 22) rotate(18)",
            path { d: "M0 0 L-8 -5 L-8 5 Z M0 0 L8 -5 L8 5 Z", fill: "#f38ba8" }
            circle { r: "2.4", fill: "#eb6f92" }
        }
        // eyes
        g { class: "ak-eye",
            ellipse { cx: "37", cy: "47", rx: "6.5", ry: "8.5", fill: "url(#ak-iris)" }
            circle { cx: "35", cy: "43.5", r: "2.6", fill: "#fff" }
            circle { cx: "39.5", cy: "50.5", r: "1.2", fill: "#fff", opacity: "0.8" }
        }
        g { class: "ak-eye",
            ellipse { cx: "63", cy: "47", rx: "6.5", ry: "8.5", fill: "url(#ak-iris)" }
            circle { cx: "61", cy: "43.5", r: "2.6", fill: "#fff" }
            circle { cx: "65.5", cy: "50.5", r: "1.2", fill: "#fff", opacity: "0.8" }
        }
        // blush
        ellipse { cx: "27", cy: "57", rx: "5", ry: "2.6", fill: "#f38ba8", opacity: "0.45" }
        ellipse { cx: "73", cy: "57", rx: "5", ry: "2.6", fill: "#f38ba8", opacity: "0.45" }
        // ω mouth
        path {
            d: "M44 58 Q47 62 50 58 Q53 62 56 58",
            stroke: "#b5669a",
            stroke_width: "1.6",
            fill: "none",
            stroke_linecap: "round",
        }
        // whiskers
        path {
            d: "M16 50 L26 52 M16 56 L26 55 M84 50 L74 52 M84 56 L74 55",
            stroke: "#e2a6cf",
            stroke_width: "1.2",
            stroke_linecap: "round",
        }
        // sparkles
        path { class: "ak-sparkle", d: "M92 18 L94 23 L99 25 L94 27 L92 32 L90 27 L85 25 L90 23 Z", fill: "#f9e2af" }
        path { class: "ak-sparkle s2", d: "M8 62 L9.5 66 L13.5 67.5 L9.5 69 L8 73 L6.5 69 L2.5 67.5 L6.5 66 Z", fill: "#89dceb" }
    }
}

/// Cheap deterministic scatter so star/petal layout is stable between renders
/// (and identical on server and client for hydration).
fn scatter(i: usize, salt: usize) -> usize {
    let mut x = (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (salt as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x % 10_000) as usize
}
