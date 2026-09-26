//! Vim-style keyboard shortcuts, active whenever you're not typing in a
//! field or a dialog is open. `?` shows the list.
//!
//! Keys are read from the physical key (`KeyboardEvent.code`), so they work
//! the same with a Thai (Kedmanee) or any other keyboard layout.

use crate::i18n::tr;
use dioxus::prelude::*;

/// Page shortcuts after `g`, as (key, path). Navigation clicks the sidebar
/// link for the path, so the router handles it like a normal click.
const PAGES: [(&str, &str, &str); 9] = [
    ("h", "/", "Dashboard"),
    ("p", "/portfolio", "Portfolio"),
    ("w", "/watchlist", "Watchlist"),
    ("m", "/market", "Markets"),
    ("s", "/screener", "Screener"),
    ("c", "/calendar", "Calendar"),
    ("e", "/economy", "Economy"),
    ("b", "/backtest", "Backtest"),
    ("o", "/settings", "Settings"),
];

const HELP_ID: &str = "vim-help";

fn script() -> String {
    let pages = PAGES
        .iter()
        .map(|(k, path, _)| format!("{k:?}: {path:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"if (!window.__vimKeys) {{
    window.__vimKeys = true;
    const pages = {{ {pages} }};
    const help = () => document.getElementById({HELP_ID:?});
    const step = 80;
    let pending = "", timer;
    // The key as typed on a US layout, whatever layout is active.
    const keyOf = (e) => {{
        if (/^Key[A-Z]$/.test(e.code)) {{
            const k = e.code.slice(3).toLowerCase();
            return e.shiftKey ? k.toUpperCase() : k;
        }}
        if (e.code === "Slash") return e.shiftKey ? "?" : "/";
        return e.key;
    }};
    // Navigation goes through the router ([`VimRouter`]). A synthetic click
    // on a sidebar link isn't handled by the desktop app's router, which then
    // opens the link in the system browser as file:///portfolio.
    const go = (path) => window.dispatchEvent(new CustomEvent("vim-go", {{ detail: path }}));
    const scroll = (dy) => window.scrollBy({{ top: dy, behavior: "auto" }});
    window.addEventListener("keydown", (e) => {{
        if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return;
        const el = document.activeElement;
        const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(el?.tagName || "") || el?.isContentEditable;
        if (typing) {{
            // Esc leaves the field so the shortcuts work again.
            if (e.key === "Escape") el.blur();
            return;
        }}
        const h = help();
        const helpOpen = h && !h.hidden;
        if (e.key === "Escape") {{
            pending = "";
            if (helpOpen) {{ h.hidden = true; e.preventDefault(); }}
            return;
        }}
        // Leave keys to an open dialog (editor, confirm …).
        if (!helpOpen && document.querySelector('[role="dialog"]')) return;
        const key = keyOf(e);
        let handled = true;
        if (pending === "g") {{
            pending = "";
            clearTimeout(timer);
            if (key === "g") window.scrollTo({{ top: 0 }});
            else if (pages[key]) go(pages[key]);
            else handled = false;
        }} else {{
            switch (key) {{
                case "j": scroll(step); break;
                case "k": scroll(-step); break;
                case "d": scroll(window.innerHeight / 2); break;
                case "u": scroll(-window.innerHeight / 2); break;
                case "G": window.scrollTo({{ top: document.documentElement.scrollHeight }}); break;
                case "H": history.back(); break;
                case "L": history.forward(); break;
                case "?": if (h) h.hidden = !h.hidden; break;
                case "g":
                    pending = "g";
                    clearTimeout(timer);
                    timer = setTimeout(() => {{ pending = ""; }}, 1000);
                    break;
                default: handled = false;
            }}
        }}
        if (handled) e.preventDefault();
    }});
}}"#
    )
}

/// Listens for `g`+key page jumps and navigates with the router. Render it
/// inside the router (the app's layout does, via [`crate::Sidebar`]).
#[component]
pub fn VimRouter() -> Element {
    use_future(|| async {
        let mut channel = document::eval(
            r#"const go = (e) => {
                   try { dioxus.send(e.detail); } catch (_) { window.removeEventListener("vim-go", go); }
               };
               window.addEventListener("vim-go", go);
               await new Promise(() => {});"#,
        );
        while let Ok(path) = channel.recv::<String>().await {
            if PAGES.iter().any(|(_, p, _)| *p == path) {
                navigator().push(path);
            }
        }
    });
    rsx! {}
}

/// Installs the shortcuts and renders the `?` help panel (hidden until
/// asked for). Rendered once from [`crate::App`].
#[component]
pub fn VimKeys() -> Element {
    use_hook(|| {
        document::eval(&script());
    });
    let close = move |_| {
        document::eval(&format!("document.getElementById({HELP_ID:?}).hidden = true;"));
    };
    let general: [(&str, &str); 10] = [
        ("j / k", tr("Scroll down / up")),
        ("d / u", tr("Half a page down / up")),
        ("g g", tr("Go to the top")),
        ("G", tr("Go to the bottom")),
        ("H / L", tr("Back / forward")),
        ("/", tr("Search stocks")),
        ("Ctrl K", tr("Search stocks")),
        ("Esc", tr("Leave a field, close this panel")),
        ("?", tr("Show or hide this panel")),
        ("g + …", tr("Go to a page (below)")),
    ];
    rsx! {
        div {
            id: HELP_ID,
            hidden: true,
            class: "{crate::theme::theme_class()} fixed inset-0 z-[70] flex items-center justify-center bg-ctp-crust/60 p-4",
            onclick: close,
            div {
                class: "max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-2xl border border-ctp-surface0 bg-ctp-mantle p-5 text-ctp-text shadow-2xl",
                onclick: move |e| e.stop_propagation(),
                h2 { class: "mb-3 text-lg font-semibold", {tr("Keyboard shortcuts")} }
                p { class: "mb-4 text-xs text-ctp-subtext0",
                    {tr("Vim-style keys work anywhere except while typing, on any keyboard layout (Thai too).")}
                }
                div { class: "grid gap-x-6 gap-y-1.5 text-sm sm:grid-cols-2",
                    for (keys, what) in general {
                        div { key: "{keys}", class: "flex items-center justify-between gap-3",
                            span { class: "text-ctp-subtext1", "{what}" }
                            kbd { class: "rounded-md border border-ctp-surface1 bg-ctp-base px-1.5 py-0.5 font-mono text-xs", "{keys}" }
                        }
                    }
                }
                h3 { class: "mt-4 mb-2 text-sm font-semibold text-ctp-mauve", {tr("Pages")} }
                div { class: "grid gap-x-6 gap-y-1.5 text-sm sm:grid-cols-2",
                    for (key, _, name) in PAGES {
                        div { key: "{key}", class: "flex items-center justify-between gap-3",
                            span { class: "text-ctp-subtext1", {tr(name)} }
                            kbd { class: "rounded-md border border-ctp-surface1 bg-ctp-base px-1.5 py-0.5 font-mono text-xs", "g {key}" }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_lists_every_page() {
        let js = script();
        for (key, path, _) in PAGES {
            assert!(js.contains(&format!("{key:?}: {path:?}")), "{key}");
        }
        assert!(js.contains("\"vim-help\""));
    }
}
