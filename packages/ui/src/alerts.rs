//! Shows notifications (e.g. fired price alerts) as they arrive. The
//! server checks alerts every minute, even with no app open, and pushes
//! them to your phone if set up; this just surfaces them in the app.

use crate::i18n::tr;
use crate::{
    app::DataRefresh,
    components::card::Card,
    notify::Toasts,
};
use dioxus::prelude::*;
use dtos::notifications::Notification;

const POLL_MS: u32 = 30_000;

/// Mounted once, in `App`: toasts each unread notification once, then
/// marks them read.
#[component]
pub fn AlertWatcher() -> Element {
    let toasts = use_context::<Toasts>();
    let refresh = use_context::<DataRefresh>();
    let auth = use_context::<crate::auth::AuthState>();
    use_future(move || async move {
        loop {
            let result = api::get_notifications().await;
            // Signed out elsewhere or the session expired: show sign-in.
            if matches!(&result, Err(ServerFnError::ServerError { code: 401, .. }))
                || matches!(&result, Err(e) if e.to_string().contains("Sign in first"))
            {
                auth.recheck();
            }
            if let Ok(list) = result {
                let unread: Vec<Notification> = list.into_iter().filter(|n| !n.read).collect();
                if !unread.is_empty() {
                    for n in unread.iter().rev() {
                        toasts.show(n.title.clone(), n.body.clone());
                    }
                    let _ = api::mark_notifications_read().await;
                    // Fired alerts change the alert list.
                    refresh.reload();
                }
            }
            crate::notify::poll_delay(POLL_MS).await;
        }
    });
    rsx! {}
}

/// The latest notifications.
#[component]
pub fn NotificationsCard() -> Element {
    let refresh = use_context::<DataRefresh>();
    let list = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_notifications().await.unwrap_or_default()
    });
    let items = list.read().clone().unwrap_or_default();
    rsx! {
        Card {
            title: tr("Notifications"),
            subtitle: tr("Alerts are checked on the server every minute. Push them to your phone in Settings.").to_string(),
            flush: true,
            if items.is_empty() {
                p { class: "px-6 pb-8 text-sm text-ctp-subtext0", {tr("Nothing yet.")} }
            }
            for n in items.into_iter().take(10) {
                div { key: "{n.id}", class: "flex items-start gap-4 border-t border-ctp-surface0/60 px-6 py-3",
                    div { class: "min-w-0 flex-1",
                        div { class: "text-sm text-ctp-text", "{n.title}" }
                        if !n.body.is_empty() {
                            div { class: "text-xs text-ctp-subtext0", "{n.body}" }
                        }
                    }
                    span { class: "shrink-0 text-xs tabular-nums text-ctp-overlay1", "{n.created_at} UTC" }
                }
            }
        }
    }
}
