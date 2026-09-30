//! Which server the phone app talks to.
//!
//! A phone reaches the app's server over the network, so it needs the
//! address of the computer running it (`akhsakov-finance run server --lan`
//! prints it). Builds made with `AKHSAKOV_SERVER_URL` use that address;
//! the others ask on first launch and remember the answer on the device.
//!
//! Dioxus fixes the server URL when the app launches, before anything can
//! be asked. So those builds point it at a forwarder on the phone itself
//! (127.0.0.1), which passes connections on to the address chosen later.
//! Changing the address closes the app, and the next launch asks again.

use crate::components::card::{ActionButton, ButtonTone, Card, Field, INPUT};
use crate::i18n::{tr, trf};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

const STORAGE_KEY: &str = "akhsakov.server";

/// The port `akhsakov-finance run server` listens on unless told otherwise.
const DEFAULT_PORT: u16 = 8080;

static ASK: AtomicBool = AtomicBool::new(false);

/// The chosen server, e.g. `http://192.168.1.20:8080`.
static ADDRESS: Mutex<Option<String>> = Mutex::new(None);

/// Ask for the server address on this device (phone builds without one).
/// Call before launching the app.
pub fn ask_on_device() {
    ASK.store(true, Ordering::Relaxed);
    #[cfg(not(target_arch = "wasm32"))]
    match forwarder::start() {
        Ok(port) => dioxus::fullstack::set_server_url(format!("http://127.0.0.1:{port}").leak()),
        Err(e) => dioxus::logger::tracing::error!("Couldn't start the server forwarder: {e}"),
    }
}

/// Whether this build asks for its server address.
pub fn asked_on_device() -> bool {
    ASK.load(Ordering::Relaxed)
}

fn address() -> Option<String> {
    ADDRESS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn needs_address() -> bool {
    asked_on_device() && address().is_none()
}

fn connect(url: &str) {
    *ADDRESS.lock().unwrap_or_else(|e| e.into_inner()) = Some(url.to_string());
}

async fn stored_address() -> Option<String> {
    document::eval(&format!(
        "try {{ return localStorage.getItem('{STORAGE_KEY}') || ''; }} catch (e) {{ return ''; }}"
    ))
    .join::<String>()
    .await
    .ok()
    .filter(|url| !url.is_empty())
}

fn store_address(url: Option<&str>) {
    document::eval(&match url {
        Some(url) => format!("try {{ localStorage.setItem('{STORAGE_KEY}', {url:?}); }} catch (e) {{}}"),
        None => format!("try {{ localStorage.removeItem('{STORAGE_KEY}'); }} catch (e) {{}}"),
    });
}

/// Forgets the address and closes the app; the next launch asks again.
pub fn change_server() {
    store_address(None);
    spawn(async {
        // Let the webview write local storage first.
        crate::notify::sleep_ms(300).await;
        std::process::exit(0);
    });
}

/// Shows `children` once the app knows its server.
#[component]
pub fn ServerGate(children: Element) -> Element {
    let mut chosen = use_signal(|| None::<String>);
    let saved = use_resource(|| async {
        match needs_address() {
            true => stored_address().await,
            false => None,
        }
    });
    if !needs_address() {
        return rsx! { {children} };
    }
    match chosen().or_else(|| saved.read().clone().flatten()) {
        Some(url) => {
            connect(&url);
            rsx! { {children} }
        }
        None if saved.read().is_none() => rsx! {
            div { class: "{crate::theme::theme_class()} flex h-screen items-center justify-center bg-ctp-base text-sm text-ctp-subtext0", {tr("Loading…")} }
        },
        None => rsx! {
            AddressScreen {
                on_connect: move |url: String| {
                    store_address(Some(&url));
                    chosen.set(Some(url));
                },
            }
        },
    }
}

#[component]
fn AddressScreen(on_connect: EventHandler<String>) -> Element {
    let mut address = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let button = if busy() { "…" } else { tr("Connect") };
    let submit = move |_| async move {
        let url = match normalize(&address()) {
            Ok(url) => url,
            Err(e) => return error.set(Some(e)),
        };
        busy.set(true);
        let reachable = reachable(&url).await;
        busy.set(false);
        if reachable {
            on_connect.call(url);
        } else {
            error.set(Some(trf(
                "Nothing answers at {}. Is the server running on your computer, and is this phone on the same network?",
                &[&url],
            )));
        }
    };
    rsx! {
        div { class: "{crate::theme::theme_class()} flex min-h-screen items-center justify-center bg-ctp-base px-4 text-ctp-text",
            form {
                class: "w-full max-w-sm rounded-3xl border border-ctp-surface0 bg-ctp-mantle p-8 shadow-2xl shadow-ctp-crust/60 motion-safe:animate-rise",
                onsubmit: move |e| e.prevent_default(),
                div { class: "mb-6 flex items-center gap-3",
                    span { class: "flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-ctp-pink via-ctp-mauve to-ctp-sky text-base font-bold text-ctp-crust", {tr("A")} }
                    div {
                        div { class: "text-lg font-semibold", {tr("Connect to your server")} }
                        div { class: "text-xs text-ctp-subtext0", {tr("Akhsakov Finance")} }
                    }
                }
                div { class: "grid gap-4",
                    p { class: "text-sm text-ctp-subtext0",
                        {tr("Your portfolio lives on the computer that runs the server. On that computer, run:")}
                    }
                    code { class: "block rounded-xl bg-ctp-crust/60 px-3 py-2 text-xs text-ctp-text", "akhsakov-finance run server --lan" }
                    Field { label: tr("Server address"), hint: tr("The address it prints, e.g. http://192.168.1.20:8080"),
                        input {
                            class: INPUT,
                            // Text, not "url": the scheme and port may be left out.
                            inputmode: "url",
                            autocomplete: "off",
                            autofocus: true,
                            placeholder: "http://192.168.1.20:8080",
                            value: "{address}",
                            oninput: move |e| address.set(e.value()),
                        }
                    }
                    if let Some(e) = error() {
                        p { class: "text-sm text-ctp-red break-words", "{e}" }
                    }
                    ActionButton { label: button, disabled: busy(), onclick: submit }
                }
            }
        }
    }
}

/// Settings card on phones that chose their server.
#[component]
pub fn ServerCard() -> Element {
    let mut confirm = use_signal(|| false);
    if !asked_on_device() {
        return rsx! {};
    }
    rsx! {
        Card { title: tr("Server"),
            div { class: "grid gap-3 text-sm",
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    div {
                        div { class: "font-medium text-ctp-text", {address().unwrap_or_default()} }
                        div { class: "text-xs text-ctp-subtext0", {tr("Where this phone reads and saves your data.")} }
                    }
                    if confirm() {
                        ActionButton { label: tr("Close app and change"), tone: ButtonTone::Danger, onclick: move |_| change_server() }
                    } else {
                        ActionButton { label: tr("Change server"), tone: ButtonTone::Quiet, onclick: move |_| confirm.set(true) }
                    }
                }
                if confirm() {
                    p { class: "text-xs text-ctp-subtext0", {tr("The app closes. Open it again to enter the new address.")} }
                }
            }
        }
    }
}

/// `http://host:port` from what was typed: the scheme and port are optional
/// (the server speaks plain http, on its default port unless told otherwise).
fn normalize(input: &str) -> Result<String, String> {
    let input = input.trim().trim_end_matches('/');
    let rest = match input.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") => rest,
        Some(_) => return Err(tr("Enter an address like http://192.168.1.20:8080").to_string()),
        None => input,
    };
    let host_port = rest.split('/').next().unwrap_or_default();
    if host_port.is_empty() || host_port.contains(char::is_whitespace) {
        return Err(tr("Enter an address like http://192.168.1.20:8080").to_string());
    }
    let has_port = host_port
        .rsplit_once(':')
        .is_some_and(|(host, port)| !host.ends_with(':') && port.parse::<u16>().is_ok());
    Ok(match has_port {
        true => format!("http://{host_port}"),
        false => format!("http://{host_port}:{DEFAULT_PORT}"),
    })
}

/// Passes connections from the app on to the chosen server.
#[cfg(not(target_arch = "wasm32"))]
mod forwarder {
    use std::io::{self, Write};
    use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
    use std::time::Duration;

    /// Listens on a free port on 127.0.0.1 and returns it.
    pub fn start() -> io::Result<u16> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        std::thread::spawn(move || {
            for app in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let _ = forward(app);
                });
            }
        });
        Ok(port)
    }

    fn forward(app: TcpStream) -> io::Result<()> {
        let Some(address) = super::address() else {
            return Ok(());
        };
        let host_port = address.trim_start_matches("http://");
        let target = host_port
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::other("no address"))?;
        let server = TcpStream::connect_timeout(&target, Duration::from_secs(5))?;
        let (mut app_in, mut server_out) = (app.try_clone()?, server.try_clone()?);
        std::thread::spawn(move || {
            let _ = io::copy(&mut app_in, &mut server_out);
            let _ = server_out.shutdown(Shutdown::Write);
        });
        let (mut server_in, mut app_out) = (server, app);
        io::copy(&mut server_in, &mut app_out)?;
        app_out.flush()?;
        app_out.shutdown(Shutdown::Write)
    }
}

/// Whether something accepts connections at `url` (a quick check before
/// saving the address; the app shows its own error if it isn't the server).
#[cfg(not(target_arch = "wasm32"))]
async fn reachable(url: &str) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let Some(host_port) = url.split_once("://").map(|(_, rest)| rest.to_string()) else {
        return false;
    };
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let ok = host_port
            .to_socket_addrs()
            .map(|addrs| addrs.into_iter().any(|a| TcpStream::connect_timeout(&a, Duration::from_secs(3)).is_ok()))
            .unwrap_or(false);
        let _ = tx.send(ok);
    });
    rx.await.unwrap_or(false)
}

#[cfg(target_arch = "wasm32")]
async fn reachable(_url: &str) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn fills_in_scheme_and_port() {
        assert_eq!(normalize("192.168.1.20").unwrap(), "http://192.168.1.20:8080");
        assert_eq!(normalize(" 192.168.1.20:9000/ ").unwrap(), "http://192.168.1.20:9000");
        assert_eq!(normalize("HTTP://home.example:80").unwrap(), "http://home.example:80");
        assert_eq!(normalize("http://10.0.2.2:8080/settings").unwrap(), "http://10.0.2.2:8080");
        assert_eq!(normalize("[fe80::1]").unwrap(), "http://[fe80::1]:8080");
        assert_eq!(normalize("http://[fe80::1]:8081").unwrap(), "http://[fe80::1]:8081");
    }

    #[test]
    fn rejects_other_input() {
        assert!(normalize("").is_err());
        assert!(normalize("ftp://host").is_err());
        assert!(normalize("https://host").is_err());
        assert!(normalize("my server").is_err());
    }
}
