//! Scroll benchmark, run with AKHSAKOV_SCROLL_BENCH=1: opens the main
//! pages one after another, scrolls each at a fixed speed and prints how
//! long frames took, with the renderer settings in use. With
//! AKHSAKOV_SCROLL_BENCH=full each page is also scrolled with one kind of
//! effect switched off at a time, to show what painting costs.

use crate::Route;
use dioxus::{core::spawn_forever, prelude::*, router::Navigator};
use std::sync::atomic::{AtomicBool, Ordering};

/// Runs once per launch: the layout (and this component) can mount again
/// while the benchmark switches pages.
static STARTED: AtomicBool = AtomicBool::new(false);

/// Effects switched off in turn (CSS injected for the run), after a
/// baseline with everything on.
const VARIANTS: [(&str, &str); 7] = [
    ("baseline", ""),
    // Everything below at once: the most that trimming could win.
    ("floor", "*, *::before, *::after { box-shadow: none !important; text-shadow: none !important; background-image: none !important; border-radius: 0 !important; animation: none !important; transition: none !important; } canvas, svg, .decor { visibility: hidden !important; } .bg-clip-text { color: currentColor !important; }"),
    ("no shadows", "*, *::before, *::after { box-shadow: none !important; text-shadow: none !important; }"),
    ("no gradients", "*, *::before, *::after { background-image: none !important; } .bg-clip-text { color: currentColor !important; }"),
    ("no rounding", "*, *::before, *::after { border-radius: 0 !important; }"),
    ("no charts", "canvas, svg { visibility: hidden !important; }"),
    ("lite", "*, *::before, *::after { animation: none !important; transition: none !important; } .decor { display: none !important; }"),
];

/// `busy(t)`: after a frame callback at `t`, resolves with how long the page
/// kept its own thread busy (the rendering update runs after the callback,
/// in the same task), so the rest of a frame is spent waiting on the app.
const BUSY_JS: &str = r#"
const ch = new MessageChannel();
const waiting = [];
ch.port1.onmessage = () => { const w = waiting.shift(); if (w) w(performance.now()); };
const busy = t => new Promise(r => { waiting.push(now => r(now - t)); ch.port2.postMessage(0); });
const median = a => { const b = [...a].sort((x, y) => x - y); return b[Math.floor(b.length / 2)]; };
"#;

/// Scrolls down 40px a frame, up to 150 frames, and back to the top;
/// returns "frames mean p50 p95 max busy" in milliseconds, or "short".
const SCROLL_JS: &str = r#"
const css = await dioxus.recv();
let tag = document.getElementById('bench-style');
if (!tag) { tag = document.createElement('style'); tag.id = 'bench-style'; document.head.appendChild(tag); }
tag.textContent = css;
// The content column when it's the scroller (see "scroll box"), else the page.
const box = document.querySelector('[data-scroll-root]');
const boxScrolls = box && getComputedStyle(box).overflowY !== 'visible' && box.scrollHeight > box.clientHeight + 1;
const el = boxScrolls ? box : document.scrollingElement;
el.scrollTop = 0;
await new Promise(r => setTimeout(r, 600));
const max = el.scrollHeight - el.clientHeight;
if (max < 200) { tag.textContent = ''; return 'short'; }
const times = [], busies = [];
await new Promise(done => {
    let last = 0, y = 0, n = 0;
    const frame = t => {
        busy(t).then(b => busies.push(b));
        if (last) times.push(t - last);
        last = t;
        y += 40; n += 1;
        el.scrollTop = y;
        if (y < max && n < 150) requestAnimationFrame(frame); else done();
    };
    requestAnimationFrame(frame);
});
el.scrollTop = 0;
tag.textContent = '';
times.sort((a, b) => a - b);
const at = q => times[Math.min(times.length - 1, Math.floor(q * times.length))];
const mean = times.reduce((a, b) => a + b, 0) / times.length;
return [times.length, mean, at(0.5), at(0.95), times[times.length - 1], median(busies)].map(v => Math.round(v * 10) / 10).join(' ');
"#;

#[component]
pub fn ScrollBench() -> Element {
    let mode = std::env::var("AKHSAKOV_SCROLL_BENCH").unwrap_or_default();
    if matches!(mode.as_str(), "1" | "full") && !STARTED.swap(true, Ordering::SeqCst) {
        // Kept by the app root, so switching pages can't drop it.
        let navigator = navigator();
        spawn_forever(run(navigator, mode == "full"));
    }
    rsx! {}
}

async fn run(navigator: Navigator, full: bool) {
    {
        let variants = if full { &VARIANTS[..] } else { &VARIANTS[..1] };
        let pages = [
            Route::Home {},
            Route::Portfolio {},
            Route::Watchlist {},
            Route::Market {},
            Route::Stock { ticker: "AAPL".into() },
        ];
        // Frames while nothing changes: the pace WebKit allows at all (a
        // hidden, unfocused or power-saving window is held to less).
        let idle = document::eval(
            &format!(
                "{BUSY_JS}
                 const t = [], busies = [];
                 await new Promise(done => {{
                     let last = 0;
                     const f = now => {{ busy(now).then(b => busies.push(b)); if (last) t.push(now - last); last = now; t.length < 60 ? requestAnimationFrame(f) : done(); }};
                     requestAnimationFrame(f);
                 }});
                 return `idle frame p50 ${{median(t).toFixed(1)}} ms (page busy ${{median(busies).toFixed(1)}} ms), visible: ${{document.visibilityState}}, focused: ${{document.hasFocus()}}`;"
            ),
        )
        .join::<String>()
        .await
        .unwrap_or_default();
        let size = document::eval("return `${innerWidth}x${innerHeight} @${devicePixelRatio}x`;")
            .join::<String>()
            .await
            .unwrap_or_default();
        let env = |k: &str| std::env::var(k).unwrap_or_else(|_| "-".into());
        println!(
            "\n=== scroll bench, window {size}, AKHSAKOV_FAST_RENDERING={} WEBKIT_DISABLE_COMPOSITING_MODE={} WEBKIT_DMABUF_RENDERER_FORCE_SHM={} WEBKIT_DISABLE_DMABUF_RENDERER={} GDK_BACKEND={}",
            env("AKHSAKOV_FAST_RENDERING"),
            env("WEBKIT_DISABLE_COMPOSITING_MODE"),
            env("WEBKIT_DMABUF_RENDERER_FORCE_SHM"),
            env("WEBKIT_DISABLE_DMABUF_RENDERER"),
            env("GDK_BACKEND"),
        );
        println!("{idle}");
        println!("{:<16} {:<14} {:>6} {:>7} {:>6} {:>6} {:>6} {:>6}", "page", "variant", "frames", "mean", "p50", "p95", "max", "busy");
        for page in pages {
            navigator.push(page.clone());
            // Let the page load its data and charts.
            let _ = document::eval("await new Promise(r => setTimeout(r, 6000)); return 0;").join::<i32>().await;
            for &(name, css) in variants {
                let run = document::eval(&format!("{BUSY_JS}{SCROLL_JS}"));
                let _ = run.send(css);
                let result = run.join::<String>().await.unwrap_or_else(|e| format!("error {e}"));
                let cols: Vec<&str> = result.split(' ').collect();
                if let [frames, mean, p50, p95, max, busy] = cols[..] {
                    println!("{:<16} {name:<14} {frames:>6} {mean:>7} {p50:>6} {p95:>6} {max:>6} {busy:>6}", page.to_string());
                } else {
                    println!("{:<16} {name:<14} {result}", page.to_string());
                }
            }
        }
        println!("=== done (ms per frame; 16.7 = 60 fps; busy = the page's own work per frame, p50)\n");
    }
}
