//! In-app notifications (toasts), and a timer that works on web and desktop.

use dioxus::prelude::*;

/// Waits `ms` milliseconds using the webview's timer, so it works in both
/// the browser and the desktop app.
pub async fn sleep_ms(ms: u32) {
    let _ = document::eval(&format!(
        "await new Promise(r => setTimeout(r, {ms})); return 0;"
    ))
    .join::<i32>()
    .await;
}

/// Like [`sleep_ms`] for polling loops: waits `ms`, then, if the app is
/// hidden (minimised, another tab), keeps waiting until it's visible again
/// so background pages don't keep fetching.
pub async fn poll_delay(ms: u32) {
    let _ = document::eval(&format!(
        "await new Promise(r => setTimeout(r, {ms}));
         if (document.hidden) {{
             await new Promise(r => {{
                 const go = () => {{ if (!document.hidden) {{ document.removeEventListener('visibilitychange', go); r(); }} }};
                 document.addEventListener('visibilitychange', go);
             }});
         }}
         return 0;"
    ))
    .join::<i32>()
    .await;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    id: u32,
    title: String,
    body: String,
}

/// App-wide handle for showing notifications.
#[derive(Clone, Copy)]
pub struct Toasts {
    list: Signal<Vec<Toast>>,
    next: Signal<u32>,
}

impl Toasts {
    pub fn new() -> Self {
        Self {
            list: Signal::new(vec![]),
            next: Signal::new(0),
        }
    }

    /// Shows a notification for ten seconds (or until dismissed).
    pub fn show(mut self, title: impl Into<String>, body: impl Into<String>) {
        let id = *self.next.peek();
        self.next += 1;
        self.list.write().push(Toast {
            id,
            title: title.into(),
            body: body.into(),
        });
        spawn(async move {
            sleep_ms(10_000).await;
            self.dismiss(id);
        });
    }

    fn dismiss(mut self, id: u32) {
        self.list.write().retain(|t| t.id != id);
    }
}

/// Renders notifications in the corner. Mounted once, in `App`.
#[component]
pub fn ToastHost() -> Element {
    let toasts = use_context::<Toasts>();
    rsx! {
        div { class: "pointer-events-none fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2",
            for t in toasts.list.read().iter().cloned() {
                div {
                    key: "{t.id}",
                    class: "pointer-events-auto rounded-2xl border border-ctp-surface0 bg-ctp-mantle p-4 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                    role: "status",
                    div { class: "flex items-start justify-between gap-3",
                        div { class: "text-sm font-semibold text-ctp-text", "{t.title}" }
                        button {
                            class: "text-ctp-subtext0 cursor-pointer hover:text-ctp-text",
                            "aria-label": "Dismiss",
                            onclick: move |_| toasts.dismiss(t.id),
                            "×"
                        }
                    }
                    if !t.body.is_empty() {
                        p { class: "mt-1 text-xs text-ctp-subtext0", "{t.body}" }
                    }
                }
            }
        }
    }
}

/// Today's local date as `YYYY-MM-DD`, from the webview's clock.
pub async fn today() -> Option<String> {
    document::eval("return new Date().toLocaleDateString('sv-SE');")
        .join::<String>()
        .await
        .ok()
}
