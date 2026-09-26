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
        div { class: "pointer-events-none fixed inset-x-4 bottom-[calc(4.5rem+env(safe-area-inset-bottom))] z-[60] flex flex-col gap-2 sm:inset-x-auto sm:right-4 sm:w-80 md:bottom-4",
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

/// Remembered on this device: show system notifications for alerts.
const SYSTEM_KEY: &str = "akhsakov.system-notifications";

/// Shows `title` as a notification of the operating system (phone or
/// desktop), when you've turned that on for this device and allowed it.
/// Only while the app is in the background: in front, a toast shows it.
pub fn system_notify(title: &str, body: &str) {
    document::eval(&format!(
        "try {{
             if (localStorage.getItem({SYSTEM_KEY:?}) === '1' && 'Notification' in window
                 && Notification.permission === 'granted' && document.visibilityState !== 'visible') {{
                 new Notification({title:?}, {{ body: {body:?}, tag: {title:?} }});
             }}
         }} catch (e) {{}}"
    ));
}

/// Whether system notifications work here (`None`), are on, or are off.
pub async fn system_notifications() -> Result<bool, String> {
    let state = document::eval(&format!(
        "if (!('Notification' in window)) return 'unsupported';
         try {{ return localStorage.getItem({SYSTEM_KEY:?}) === '1' && Notification.permission === 'granted' ? 'on' : 'off'; }}
         catch (e) {{ return 'off'; }}"
    ))
    .join::<String>()
    .await
    .unwrap_or_default();
    match state.as_str() {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("unsupported".into()),
    }
}

/// Turns system notifications on (asking the system for permission) or
/// off; resolves to whether they're on.
pub async fn set_system_notifications(on: bool) -> bool {
    document::eval(&format!(
        "try {{
             if (!{on}) {{ localStorage.setItem({SYSTEM_KEY:?}, '0'); return false; }}
             if (!('Notification' in window)) return false;
             const p = Notification.permission === 'granted' ? 'granted' : await Notification.requestPermission();
             localStorage.setItem({SYSTEM_KEY:?}, p === 'granted' ? '1' : '0');
             return p === 'granted';
         }} catch (e) {{ return false; }}"
    ))
    .join::<bool>()
    .await
    .unwrap_or(false)
}
