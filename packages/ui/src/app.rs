use dioxus::prelude::*;
use dtos::portfolio::GetDashBoardResponse;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

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
                    .ok_or_else(|| format!("Couldn't get the USD → {code} rate. Try again shortly."))?,
            ),
            None => None,
        };
        api::save_settings(settings.clone())
            .await
            .map_err(|e| match e {
                ServerFnError::ServerError { message, .. } => message,
                e => e.to_string(),
            })?;
        apply_currency(&settings, rate);
        self.0.set(settings);
        Ok(())
    }
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
    let mut app_data: Signal<GetDashBoardResponse> = use_signal(GetDashBoardResponse::default);
    use_context_provider(|| app_data);
    use_context_provider(|| PortfolioScope(Signal::new(None)));
    let refresh = use_context_provider(|| DataRefresh(Signal::new(0)));
    use_context_provider(|| crate::editors::Dialogs(Signal::new(None)));
    use_context_provider(crate::notify::Toasts::new);
    let mut settings = use_context_provider(|| AppSettings(Signal::new(Default::default()))).0;
    let mut ready = use_signal(|| false);
    // Settings and the display currency's exchange rate. Amounts are shown
    // in that currency; if the rate can't be fetched we stay in USD.
    let loaded = use_resource(move || async move {
        let _reload = refresh.0();
        let s = api::get_settings().await.unwrap_or_default();
        let rate = fx_rate(&s).await;
        (s, rate)
    });
    use_effect(move || {
        if let Some((s, rate)) = loaded.read().clone() {
            apply_currency(&s, rate);
            settings.set(s);
            ready.set(true);
        }
    });
    let _ = use_resource(move || async move {
        let _reload = refresh.0();
        match api::get_dashboard().await {
            Ok(data) => *app_data.write() = data,
            Err(e) => tracing_error(&format!("Failed to load portfolios: {e}")),
        }
    });

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        crate::MotionStyles {}
        if ready() {
            {children}
        } else {
            div { class: "mocha flex h-screen items-center justify-center bg-ctp-base text-sm text-ctp-overlay1", "Loading…" }
        }
        crate::editors::EditorHost {}
        crate::alerts::AlertWatcher {}
        crate::notify::ToastHost {}
    }
}

fn tracing_error(message: &str) {
    dioxus::logger::tracing::error!("{message}");
}
