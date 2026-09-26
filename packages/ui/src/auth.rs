//! Sign-in: the gate in front of the app, and the security settings.
//!
//! The session token is kept in this device's local storage and sent with
//! every server call as `Authorization: Bearer …`.

use crate::i18n::tr;
use crate::components::card::{ActionButton, ButtonTone, Card, Field, INPUT};
use crate::page::GhostButton;
use dioxus::{
    fullstack::{set_request_headers, HeaderMap, HeaderValue},
    prelude::*,
};
use dtos::auth::AuthStatus;

const STORAGE_KEY: &str = "akhsakov.session";

/// Re-checks sign-in (e.g. after signing in or out).
#[derive(Clone, Copy)]
pub struct AuthState {
    status: Resource<Result<AuthStatus, String>>,
}

impl AuthState {
    pub fn recheck(mut self) {
        self.status.restart();
    }

    pub fn status(&self) -> Option<AuthStatus> {
        self.status.read().clone().and_then(Result::ok)
    }
}

/// Sends `token` with every server call from now on.
fn use_token(token: Option<&str>) {
    let mut headers = HeaderMap::new();
    if let Some(value) = token.and_then(|t| HeaderValue::from_str(&format!("Bearer {t}")).ok()) {
        headers.insert("authorization", value);
    }
    set_request_headers(headers);
}

async fn stored_token() -> Option<String> {
    document::eval(&format!(
        "try {{ return localStorage.getItem('{STORAGE_KEY}') || ''; }} catch (e) {{ return ''; }}"
    ))
    .join::<String>()
    .await
    .ok()
    .filter(|t| !t.is_empty())
}

fn store_token(token: Option<&str>) {
    let script = match token {
        Some(t) => format!("try {{ localStorage.setItem('{STORAGE_KEY}', {t:?}); }} catch (e) {{}}"),
        None => format!("try {{ localStorage.removeItem('{STORAGE_KEY}'); }} catch (e) {{}}"),
    };
    document::eval(&script);
    use_token(token);
}

fn message(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    }
}

/// Shows `children` once signed in, or when no sign-in is needed.
#[component]
pub fn AuthGate(children: Element) -> Element {
    let status = use_resource(|| async {
        use_token(stored_token().await.as_deref());
        api::auth_status().await.map_err(message)
    });
    use_context_provider(|| AuthState { status });

    let current = status.read().clone();
    match current {
        None => rsx! {
            div { class: "{crate::theme::theme_class()} flex h-screen items-center justify-center bg-ctp-base text-sm text-ctp-subtext0", {tr("Loading…")} }
        },
        Some(Err(e)) => rsx! {
            div { class: "{crate::theme::theme_class()} flex h-screen flex-col items-center justify-center gap-3 bg-ctp-base text-sm",
                p { class: "text-ctp-red", "Can't reach the server: {e}" }
                GhostButton { label: tr("Try again"), onclick: move |_| AuthState { status }.recheck() }
            }
        },
        Some(Ok(ref s)) if s.required && s.user.is_none() => rsx! {
            SignInScreen { setup: s.needs_setup }
        },
        Some(Ok(_)) => rsx! { {children} },
    }
}

#[component]
fn SignInScreen(setup: bool) -> Element {
    let auth = use_context::<AuthState>();
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let submit = move |_| async move {
        busy.set(true);
        let result = if setup {
            api::auth_setup(username(), password()).await
        } else {
            api::auth_login(username(), password()).await
        };
        busy.set(false);
        match result {
            Ok(token) => {
                store_token(Some(&token));
                auth.recheck();
            }
            Err(e) => error.set(Some(message(e))),
        }
    };
    let title = if setup { tr("Create your account") } else { tr("Sign in") };
    let hint = if setup { tr("At least 8 characters.") } else { "" };
    let autocomplete = if setup { "new-password" } else { "current-password" };
    let button = if busy() { "…" } else if setup { tr("Create account") } else { tr("Sign in") };
    rsx! {
        div { class: "{crate::theme::theme_class()} flex min-h-screen items-center justify-center bg-ctp-base px-4 text-ctp-text",
            form {
                class: "w-full max-w-sm rounded-3xl border border-ctp-surface0 bg-ctp-mantle p-8 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                onsubmit: move |e| e.prevent_default(),
                div { class: "mb-6 flex items-center gap-3",
                    span { class: "flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-ctp-pink via-ctp-mauve to-ctp-sky text-base font-bold text-ctp-crust", {tr("A")} }
                    div {
                        div { class: "text-lg font-semibold", "{title}" }
                        div { class: "text-xs text-ctp-subtext0", {tr("Akhsakov Finance")} }
                    }
                }
                div { class: "grid gap-4",
                    Field { label: tr("Username"),
                        input { class: INPUT, autocomplete: "username", autofocus: true, value: "{username}", oninput: move |e| username.set(e.value().to_lowercase()) }
                    }
                    Field { label: tr("Password"), hint,
                        input {
                            class: INPUT,
                            r#type: "password",
                            autocomplete,
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                        }
                    }
                    if let Some(e) = error() {
                        p { class: "text-sm text-ctp-red", "{e}" }
                    }
                    ActionButton { label: button, disabled: busy(), onclick: submit }
                }
            }
        }
    }
}

/// Settings card: protect the app with a password, manage accounts,
/// change your password, sign out.
#[component]
pub fn SecurityCard() -> Element {
    let auth = use_context::<AuthState>();
    let status = auth.status().unwrap_or_default();
    let mut new_user = use_signal(String::new);
    let mut new_password = use_signal(String::new);
    let mut current = use_signal(String::new);
    let mut changed = use_signal(String::new);
    let mut note = use_signal(|| None::<Result<String, String>>);
    let users = use_resource(move || async move {
        let _ = auth.status();
        api::list_users().await.unwrap_or_default()
    });

    let create = move |_| async move {
        let result = if auth.status().is_some_and(|s| s.needs_setup) {
            api::auth_setup(new_user(), new_password()).await.map(|token| {
                store_token(Some(&token));
                "Account created. Sign-in is now required on every device.".to_string()
            })
        } else {
            api::add_user(new_user(), new_password()).await.map(|()| format!("Added {}.", new_user()))
        };
        note.set(Some(result.map_err(message)));
        new_user.set(String::new());
        new_password.set(String::new());
        auth.recheck();
    };
    let change = move |_| async move {
        let result = api::change_password(current(), changed()).await;
        note.set(Some(result.map(|()| "Password changed; other devices were signed out.".to_string()).map_err(message)));
        current.set(String::new());
        changed.set(String::new());
    };
    let sign_out = move |_| async move {
        let _ = api::auth_logout().await;
        store_token(None);
        auth.recheck();
    };

    let subtitle = if status.needs_setup {
        tr("Anyone who can open this app can see and change your data. Create an account to require signing in.")
    } else {
        tr("Signing in is required. Accounts share the same portfolios.")
    };
    let create_label = if status.needs_setup { tr("Create & require sign-in") } else { tr("Add account") };
    rsx! {
        Card {
            title: tr("Security"),
            subtitle: subtitle.to_string(),
            if let Some(user) = &status.user {
                div { class: "mb-4 flex flex-wrap items-center justify-between gap-3 text-sm",
                    span { class: "text-ctp-subtext1", "Signed in as " span { class: "font-semibold text-ctp-text", "{user}" } }
                    ActionButton { label: tr("Sign out"), tone: ButtonTone::Quiet, onclick: sign_out }
                }
            }
            if !status.needs_setup {
                div { class: "mb-5 flex flex-wrap gap-2",
                    for u in users.read().clone().unwrap_or_default() {
                        {
                            let name = u.username.clone();
                            let me = status.user.as_deref() == Some(u.username.as_str());
                            rsx! {
                                span { key: "{u.username}", class: "inline-flex items-center gap-2 rounded-full bg-ctp-surface0 px-3 py-1 text-sm",
                                    "{u.username}"
                                    if !me {
                                        button {
                                            class: "text-ctp-subtext0 cursor-pointer hover:text-ctp-red",
                                            title: tr("Remove account"),
                                            onclick: move |_| {
                                                let name = name.clone();
                                                async move {
                                                    note.set(Some(api::remove_user(name.clone()).await.map(|()| format!("Removed {name}.")).map_err(message)));
                                                    auth.recheck();
                                                }
                                            },
                                            "×"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "grid gap-5 md:grid-cols-2",
                form { class: "grid gap-3", onsubmit: move |e| e.prevent_default(),
                    div { class: "text-sm font-medium text-ctp-text",
                        if status.needs_setup { {tr("Create an account")} } else { {tr("Add an account")} }
                    }
                    input { class: INPUT, placeholder: tr("Username"), autocomplete: "off", value: "{new_user}", oninput: move |e| new_user.set(e.value().to_lowercase()) }
                    input { class: INPUT, r#type: "password", placeholder: tr("Password (8+ characters)"), autocomplete: "new-password", value: "{new_password}", oninput: move |e| new_password.set(e.value()) }
                    div { ActionButton { label: create_label, onclick: create } }
                }
                if status.user.is_some() {
                    form { class: "grid gap-3", onsubmit: move |e| e.prevent_default(),
                        div { class: "text-sm font-medium text-ctp-text", {tr("Change your password")} }
                        input { class: INPUT, r#type: "password", placeholder: tr("Current password"), autocomplete: "current-password", value: "{current}", oninput: move |e| current.set(e.value()) }
                        input { class: INPUT, r#type: "password", placeholder: tr("New password"), autocomplete: "new-password", value: "{changed}", oninput: move |e| changed.set(e.value()) }
                        div { ActionButton { label: tr("Change password"), onclick: change } }
                    }
                }
            }
            match note() {
                Some(Ok(m)) => rsx! { p { class: "mt-4 text-sm text-ctp-green", "{m}" } },
                Some(Err(m)) => rsx! { p { class: "mt-4 text-sm text-ctp-red", "{m}" } },
                None => rsx! {},
            }
            p { class: "mt-4 text-xs text-ctp-overlay1",
                {tr("Reaching the app over the internet? Put it behind HTTPS (e.g. Caddy or nginx) so passwords aren't sent in the clear.")}
            }
        }
    }
}
