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
                     const box = [...document.querySelectorAll('[data-global-search]')].find(el => el.offsetParent);
                     if (box) { e.preventDefault(); box.focus(); box.select(); }
                 });
             }",
        );
        // Tag the page while it scrolls (see `.is-scrolling` in input.css).
        document::eval(
            "if (!window.__scrollTag) {
                 window.__scrollTag = true;
                 // Only the user's own scrolling (wheel, touch, keys) pauses
                 // hovers; programmatic scrolls like going to the top don't.
                 let t, userScroll = 0;
                 const intent = () => { userScroll = Date.now(); };
                 ['wheel', 'touchmove', 'keydown'].forEach(e => window.addEventListener(e, intent, { passive: true, capture: true }));
                 window.addEventListener('scroll', () => {
                     if (Date.now() - userScroll > 300) return;
                     document.documentElement.classList.add('is-scrolling');
                     clearTimeout(t);
                     t = setTimeout(() => document.documentElement.classList.remove('is-scrolling'), 150);
                 }, { passive: true, capture: true });
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
        crate::auth::AuthGate {
            AppInner { {children} }
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
    use_context_provider(crate::notify::Toasts::new);
    let mut settings = use_context_provider(|| AppSettings(Signal::new(Default::default()))).0;
    let mut ready = use_signal(|| false);
    let loaded = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_settings().await.unwrap_or_default()
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
            Ok(data) => *app_data.write() = data,
            Err(e) => tracing_error(&format!("Failed to load portfolios: {e}")),
        }
    });

    rsx! {
        if ready() {
            {children}
        } else {
            div { class: "{crate::theme::theme_class()} flex h-screen items-center justify-center bg-ctp-base text-sm text-ctp-subtext0", {tr("Loading…")} }
        }
        crate::editors::EditorHost {}
        crate::alerts::AlertWatcher {}
        crate::notify::ToastHost {}
    }
}

fn tracing_error(message: &str) {
    dioxus::logger::tracing::error!("{message}");
}
