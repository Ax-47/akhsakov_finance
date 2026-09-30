use crate::i18n::tr;
use dioxus::prelude::*;
use dtos::portfolio::GetDashBoardResponse;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
/// Bundled so charts work offline. Loaded on demand by `growth_chart.js`
/// the first time a chart draws, not on every page.
const ECHARTS_JS: Asset = asset!("/assets/js/echarts.min.js");

/// Which portfolio the portfolio page shows: an id, or `None` for all
/// holdings. App-wide so other pages can open a specific portfolio.
#[derive(Clone, Copy)]
pub struct PortfolioScope(pub Signal<Option<String>>);

/// A page to open, set by code outside the router (the dialogs are mounted
/// beside it, where `navigator()` panics). [`crate::page::Page`], which
/// every page renders inside the router, carries it out.
#[derive(Clone, Copy)]
pub struct PendingNav(pub Signal<Option<String>>);

impl PendingNav {
    pub fn go(mut self, path: impl Into<String>) {
        self.0.set(Some(path.into()));
    }
}

/// User settings, loaded at start-up; see [`dtos::settings::Settings`].
#[derive(Clone, Copy)]
pub struct AppSettings(pub Signal<dtos::settings::Settings>);

impl AppSettings {
    /// Risk-free rate as a fraction.
    pub fn risk_free(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.0.read().risk_free.to_f64().unwrap_or(4.0) / 100.0
    }

    pub fn benchmark(&self) -> types::ticker_symbol::TickerSymbol {
        self.0.read().benchmark.clone()
    }

    /// The benchmark's display name, e.g. `S&P 500`.
    pub fn benchmark_name(&self) -> String {
        dtos::settings::benchmark_name(self.0.read().benchmark.as_str())
    }

    /// Display currency code, e.g. `THB`.
    pub fn currency(&self) -> String {
        self.0.read().currency.clone()
    }

    /// Switches the display currency and saves it. Every amount on screen
    /// re-renders in the new currency; nothing reloads.
    pub async fn change_currency(mut self, code: &str) -> Result<(), String> {
        let settings = dtos::settings::Settings {
            currency: code.to_string(),
            ..self.0.peek().clone()
        };
        settings.validate()?;
        let rate = match settings.fx_ticker() {
            Some(_) => Some(
                fx_rate(&settings)
                    .await
                    .ok_or_else(|| crate::i18n::trf("Couldn't get the USD → {} rate. Try again shortly.", &[&code]))?,
            ),
            None => None,
        };
        api::save_settings(settings.clone())
            .await
            .map_err(|e| match e {
                ServerFnError::ServerError { message, .. } => message,
                e => e.to_string(),
            })?;
        if let Some(rate) = rate {
            save_rate(&settings, rate);
        }
        apply_currency(&settings, rate);
        self.0.set(settings);
        Ok(())
    }
}

const FX_KEY: &str = "akhsakov.fx.";

/// The last rate this device saw for the display currency, so the app can
/// open in that currency without waiting for Yahoo.
async fn saved_rate(settings: &dtos::settings::Settings) -> Option<rust_decimal::Decimal> {
    settings.fx_ticker()?;
    let key = format!("{FX_KEY}{}", settings.currency);
    let text = document::eval(&format!(
        "try {{ return localStorage.getItem({key:?}) || ''; }} catch (e) {{ return ''; }}"
    ))
    .join::<String>()
    .await
    .ok()?;
    text.parse::<rust_decimal::Decimal>()
        .ok()
        .filter(|r| *r > rust_decimal::Decimal::ZERO)
}

fn save_rate(settings: &dtos::settings::Settings, rate: rust_decimal::Decimal) {
    let key = format!("{FX_KEY}{}", settings.currency);
    document::eval(&format!(
        "try {{ localStorage.setItem({key:?}, {:?}); }} catch (e) {{}}",
        rate.to_string()
    ));
}

/// Units of the display currency per USD; `None` for USD or when the rate
/// can't be fetched.
async fn fx_rate(settings: &dtos::settings::Settings) -> Option<rust_decimal::Decimal> {
    let pair = settings.fx_ticker()?;
    api::quote::quote::get_quote(pair)
        .await
        .ok()
        .map(|q| q.current_price)
        .filter(|r| !r.is_zero())
}

/// Shows amounts in `settings.currency`, or USD without a rate.
fn apply_currency(settings: &dtos::settings::Settings, rate: Option<rust_decimal::Decimal>) {
    match rate {
        Some(r) => crate::format::set_display_currency(settings.currency_symbol(), r),
        None => crate::format::set_display_currency("$", rust_decimal::Decimal::ONE),
    }
}

/// Bump to reload portfolios and transactions after a change.
#[derive(Clone, Copy)]
pub struct DataRefresh(pub Signal<u32>);

impl DataRefresh {
    pub fn reload(mut self) {
        self.0 += 1;
    }
}

/// Injects the shared Catppuccin-Mocha Tailwind stylesheet and renders children.
/// Use this in any platform's `App` root to get Tailwind styles globally.
#[component]
pub fn App(children: Element) -> Element {
    use_hook(|| {
        document::eval(&format!("window.ECHARTS_URL = {:?};", ECHARTS_JS.to_string()));
        // What scrolls the page: the content column on wide screens (see
        // sidebar.rs), else the window. Used by the vim keys too.
        document::eval(
            "window.pageScroller = () => {
                 const box = document.querySelector('[data-scroll-root]');
                 return box && getComputedStyle(box).overflowY !== 'visible' ? box : document.scrollingElement;
             };
             if (!window.__columnKeys) {
                 window.__columnKeys = true;
                 // Arrows, Page Up/Down, Space, Home/End scroll the column even
                 // when focus is elsewhere (a sidebar link, nothing at all):
                 // the webview only scrolls the focused box or the window.
                 window.addEventListener('keydown', e => {
                     if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
                     const box = window.pageScroller();
                     if (box === document.scrollingElement || document.querySelector('[role=\"dialog\"]')) return;
                     const a = document.activeElement;
                     if (a && a !== document.body && (box.contains(a) || a.isContentEditable
                         || /^(INPUT|TEXTAREA|SELECT|BUTTON|SUMMARY)$/.test(a.tagName) || a.getAttribute('role') === 'button')) return;
                     const page = box.clientHeight * 0.875;
                     const by = { ArrowDown: 40, ArrowUp: -40, PageDown: page, PageUp: -page, ' ': e.shiftKey ? -page : page }[e.key];
                     if (e.key === 'Home') box.scrollTo({ top: 0, behavior: 'smooth' });
                     else if (e.key === 'End') box.scrollTo({ top: box.scrollHeight, behavior: 'smooth' });
                     else if (by !== undefined) box.scrollBy({ top: by, behavior: 'smooth' });
                     else return;
                     e.preventDefault();
                 });
             }",
        );
        // "/" or Ctrl/⌘+K jumps to stock search (unless you're typing).
        document::eval(
            "if (!window.__searchKey) {
                 window.__searchKey = true;
                 window.addEventListener('keydown', e => {
                     const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || '')
                         || document.activeElement?.isContentEditable;
                     // Physical keys (e.code), so a Thai layout works too.
                     const combo = (e.ctrlKey || e.metaKey) && e.code === 'KeyK';
                     const slash = e.code === 'Slash' && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey;
                     if (!combo && (typing || !slash)) return;
                     const find = () => [...document.querySelectorAll('[data-global-search]')].find(el => el.offsetParent);
                     const box = find();
                     if (box) { e.preventDefault(); box.focus(); box.select(); return; }
                     // Collapsed sidebar: open it, then focus its search.
                     e.preventDefault();
                     window.dispatchEvent(new CustomEvent('sidebar-expand'));
                     setTimeout(() => { const b = find(); if (b) { b.focus(); b.select(); } }, 50);
                 });
             }",
        );
    });
    crate::theme::use_theme_init();
    crate::perf::use_effects_init();
    crate::i18n::use_language_init();
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        crate::MotionStyles {}
        crate::vim::VimKeys {}
        crate::server_address::ServerGate {
            crate::auth::AuthGate {
                AppInner { {children} }
            }
        }
    }
}

/// App-wide state and data, once signed in.
#[component]
fn AppInner(children: Element) -> Element {
    let mut app_data: Signal<GetDashBoardResponse> = use_signal(GetDashBoardResponse::default);
    use_context_provider(|| app_data);
    crate::hooks::use_live_quotes_provider();
    use_context_provider(|| PortfolioScope(Signal::new(None)));
    let refresh = use_context_provider(|| DataRefresh(Signal::new(0)));
    use_context_provider(|| crate::editors::Dialogs(Signal::new(None)));
    use_context_provider(|| PendingNav(Signal::new(None)));
    use_context_provider(crate::notify::Toasts::new);
    let mut settings = use_context_provider(|| AppSettings(Signal::new(Default::default()))).0;
    let mut ready = use_signal(|| false);
    let loaded = use_resource(move || async move {
        let _reload = refresh.0();
        match api::get_settings().await {
            Ok(s) => {
                crate::offline::save("settings", &s);
                s
            }
            Err(_) => crate::offline::load("settings").await.unwrap_or_default(),
        }
    });
    // Amounts are shown in the display currency. The app opens as soon as
    // the settings are in, at the last rate this device saw; today's rate
    // replaces it once Yahoo answers. With no saved rate it waits for Yahoo
    // a little, then shows USD until the rate arrives (or for good, if it
    // can't be fetched).
    use_effect(move || {
        let Some(s) = loaded.read().clone() else {
            return;
        };
        settings.set(s.clone());
        spawn(async move {
            let saved = saved_rate(&s).await;
            if s.fx_ticker().is_none() || saved.is_some() {
                apply_currency(&s, saved);
                ready.set(true);
            } else {
                spawn(async move {
                    crate::notify::sleep_ms(3_000).await;
                    ready.set(true);
                });
            }
            if s.fx_ticker().is_some() {
                let fresh = fx_rate(&s).await;
                // Skip if the currency was switched meanwhile.
                if settings.peek().currency == s.currency {
                    if let Some(rate) = fresh {
                        save_rate(&s, rate);
                        apply_currency(&s, Some(rate));
                    } else if saved.is_none() {
                        apply_currency(&s, None);
                    }
                }
            }
            ready.set(true);
        });
    });
    let _ = use_resource(move || async move {
        let _reload = refresh.0();
        match api::get_dashboard().await {
            Ok(data) => {
                crate::offline::save("dashboard", &data);
                *app_data.write() = data;
            }
            Err(e) => {
                tracing_error(&format!("Failed to load portfolios: {e}"));
                if let Some(saved) = crate::offline::load::<GetDashBoardResponse>("dashboard").await {
                    *app_data.write() = saved;
                }
            }
        }
    });

    rsx! {
        if ready() {
            {children}
        } else {
            div { class: "{crate::theme::theme_class()} flex h-screen flex-col items-center justify-center gap-4 bg-ctp-base text-sm text-ctp-subtext0",
                span { class: "flex h-12 w-12 items-center justify-center rounded-2xl bg-gradient-to-br from-ctp-pink via-ctp-mauve to-ctp-sky \
                               text-lg font-bold text-ctp-crust shadow-lg shadow-ctp-mauve/20 motion-safe:animate-pulse",
                    {tr("A")}
                }
                span { role: "status", {tr("Loading…")} }
            }
        }
        crate::offline::OfflineBanner {}
        crate::editors::EditorHost {}
        crate::alerts::AlertWatcher {}
        crate::notify::ToastHost {}
    }
}

fn tracing_error(message: &str) {
    dioxus::logger::tracing::error!("{message}");
}
