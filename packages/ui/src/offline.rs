//! Offline use: the last portfolio data, settings, prices and page data are
//! kept on this device (local storage), so the app opens and shows them
//! when the server or the internet can't be reached. Cleared on sign-out.

use dioxus::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

/// Keys start with this, so sign-out can clear them all.
const PREFIX: &str = "akhsakov.offline.";
/// Page data kept at most (least recently saved dropped first).
const MAX_PAGES: usize = 80;
/// Larger values (very long price histories) aren't kept.
const MAX_BYTES: usize = 1_000_000;

/// Set while the app runs from data saved on this device because the
/// server can't be reached; holds when that data was saved.
pub static SERVER_OFFLINE: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Installs the page-data store in the page once: an index of keys in save
/// order, trimmed to [`MAX_PAGES`], dropping the oldest when storage is full.
fn install() {
    document::eval(&install_script());
}

fn install_script() -> String {
    format!(
        "if (!window.__akOffline) {{
             const P = {PREFIX:?}, MAX = {MAX_PAGES};
             const index = () => {{ try {{ return JSON.parse(localStorage.getItem(P + 'index') || '[]'); }} catch (e) {{ return []; }} }};
             window.__akOffline = {{
                 put(key, json, page) {{
                     try {{
                         let keys = index().filter(k => k !== key);
                         if (page) {{
                             keys.push(key);
                             while (keys.length > MAX) localStorage.removeItem(P + keys.shift());
                         }}
                         for (;;) {{
                             try {{
                                 localStorage.setItem(P + key, json);
                                 break;
                             }} catch (e) {{
                                 if (!keys.length) return;
                                 localStorage.removeItem(P + keys.shift());
                             }}
                         }}
                         localStorage.setItem(P + 'index', JSON.stringify(keys));
                     }} catch (e) {{}}
                 }},
                 get(key) {{ try {{ return localStorage.getItem(P + key) || ''; }} catch (e) {{ return ''; }} }},
                 clear() {{
                     try {{
                         Object.keys(localStorage).filter(k => k.startsWith(P)).forEach(k => localStorage.removeItem(k));
                     }} catch (e) {{}}
                 }},
             }};
         }}"
    )
}

fn put(key: &str, value: &impl Serialize, page: bool) {
    let Ok(json) = serde_json::to_string(value) else { return };
    if json.len() > MAX_BYTES {
        return;
    }
    install();
    document::eval(&format!("window.__akOffline.put({key:?}, {json:?}, {page});"));
}

async fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    install();
    let json = document::eval(&format!("return window.__akOffline.get({key:?});"))
        .join::<String>()
        .await
        .ok()?;
    serde_json::from_str(&json).ok()
}

/// Saves app data (portfolio, settings, prices) under `key`, with the
/// time it was saved.
pub fn save<T: Serialize>(key: &str, value: &T) {
    put(key, value, false);
    document::eval(&format!(
        "try {{ localStorage.setItem({:?}, new Date().toLocaleString()); }} catch (e) {{}}",
        format!("{PREFIX}saved-at")
    ));
}

/// App data saved by [`save`].
pub async fn load<T: DeserializeOwned>(key: &str) -> Option<T> {
    get(key).await
}

/// When app data was last saved, in the device's local format.
pub async fn saved_at() -> Option<String> {
    document::eval(&format!("try {{ return localStorage.getItem({:?}) || ''; }} catch (e) {{ return ''; }}", format!("{PREFIX}saved-at")))
        .join::<String>()
        .await
        .ok()
        .filter(|s| !s.is_empty())
}

/// Saves a page's data (see `cache::use_cached`); the oldest pages are
/// dropped past [`MAX_PAGES`].
pub fn save_page<T: Serialize>(key: &str, value: &T) {
    put(&format!("page/{key}"), value, true);
}

pub async fn load_page<T: DeserializeOwned>(key: &str) -> Option<T> {
    get(&format!("page/{key}")).await
}

/// Price histories for charts: from the server, saved on this device, or
/// the saved copy when the server can't be reached.
pub async fn charts(
    tickers: Vec<types::ticker_symbol::TickerSymbol>,
    range: types::range::Range,
    interval: types::interval::Interval,
    prepost: bool,
) -> Result<std::collections::HashMap<types::ticker_symbol::TickerSymbol, Vec<types::candle::Candle>>, ServerFnError> {
    let mut names: Vec<&str> = tickers.iter().map(|t| t.as_str()).collect();
    names.sort_unstable();
    let key = format!("charts/{}/{range:?}/{interval:?}/{prepost}", names.join(","));
    match api::quote::quote::get_charts(tickers, range, interval, prepost).await {
        Ok(charts) => {
            save_page(&key, &charts);
            Ok(charts)
        }
        Err(e) => load_page(&key).await.ok_or(e),
    }
}

/// Forgets everything saved on this device (on sign-out).
pub fn clear() {
    install();
    document::eval("window.__akOffline.clear();");
}

/// Whether a freshly fetched value is a failure (an `Err` or nothing),
/// which shouldn't replace data saved earlier.
pub fn is_failure<T: Serialize>(value: &T) -> bool {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::Null) => true,
        Ok(serde_json::Value::Object(map)) => map.len() == 1 && map.contains_key("Err"),
        _ => false,
    }
}

/// "Offline" banner with when the data was saved and a retry button.
#[component]
pub fn OfflineBanner() -> Element {
    let server = SERVER_OFFLINE();
    let prices = crate::hooks::use_price_stream::OFFLINE();
    let text = match (&server, prices) {
        (Some(at), _) => crate::i18n::trf("Offline — showing data saved on this device ({})", &[at]),
        (None, true) => crate::i18n::tr("Market data offline — showing the last saved prices").to_string(),
        (None, false) => return rsx! {},
    };
    rsx! {
        div { class: "{crate::theme::theme_class()} sticky top-0 z-40 flex items-center justify-center gap-3 bg-ctp-peach/90 px-4 py-1.5 text-xs font-medium text-ctp-crust print:hidden",
            span { "{text}" }
            if server.is_some() {
                button {
                    class: "rounded-full bg-ctp-crust/20 px-2.5 py-0.5 cursor-pointer hover:bg-ctp-crust/30",
                    onclick: move |_| {
                        document::eval("location.reload();");
                    },
                    {crate::i18n::tr("Try again")}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_are_errors_or_nothing() {
        assert!(is_failure(&None::<u32>));
        assert!(is_failure(&Err::<u32, String>("offline".into())));
        assert!(!is_failure(&Ok::<u32, String>(1)));
        assert!(!is_failure(&Some(vec![1, 2])));
        assert!(!is_failure(&Vec::<u32>::new()));
    }
}
