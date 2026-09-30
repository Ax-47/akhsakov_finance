// The app opens without a console window on Windows. The server build
// keeps its console for `akhsakov-finance run server`.
#![cfg_attr(
    all(windows, feature = "desktop", not(feature = "server"), not(debug_assertions)),
    windows_subsystem = "windows"
)]

use dioxus::prelude::*;

use crate::views::Navbar;

#[cfg(all(feature = "desktop", not(feature = "server")))]
mod local_server;
mod build_id;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/portfolio")]
    Portfolio {},
    #[route("/market")]
    Market {},
    #[route("/screener")]
    Screener {},
    #[route("/calendar")]
    Calendar {},
    #[route("/economy")]
    Economy {},
    #[route("/learn")]
    Learn {},
    #[route("/learn/:slug")]
    Lesson { slug: String },
    #[route("/backtest")]
    Backtest {},
    #[route("/watchlist")]
    Watchlist {},
    #[route("/settings")]
    Settings {},
    #[route("/stock/:ticker")]
    Stock { ticker: String },
}

fn main() {
    #[cfg(all(feature = "desktop", not(feature = "server")))]
    dioxus::fullstack::set_server_url(local_server::start().leak());
    #[cfg(not(any(feature = "desktop", feature = "server")))]
    dioxus::fullstack::set_server_url("http://127.0.0.1:8080");
    #[cfg(all(feature = "desktop", not(feature = "server")))]
    {
        // On Wayland, WebKitGTK runs through XWayland without GPU buffer
        // sharing: stable, but slower. The native path is faster yet can
        // tear or go blank while the window is dragged on some setups, so
        // it's opt-in with AKHSAKOV_FAST_RENDERING=1.
        let fast = std::env::var("AKHSAKOV_FAST_RENDERING").is_ok_and(|v| v == "1");
        // On Hyprland (Intel + NVIDIA laptop) WebKit's GPU compositing
        // left the page undrawn until the window moved; without it pages
        // redraw normally. Set WEBKIT_DISABLE_COMPOSITING_MODE=0 to keep it.
        if fast && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
        dioxus::LaunchBuilder::new()
            .with_cfg(dioxus::desktop::Config::new().with_disable_dma_buf_on_wayland(!fast))
            .launch(App);
    }
    #[cfg(not(any(feature = "desktop", feature = "server")))]
    dioxus::launch(App);
    #[cfg(feature = "server")]
    exit_with_parent();
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use dioxus::server::axum::routing::get;
        let router = api::with_services(dioxus::server::router(App))
            .route(build_id::ROUTE, get(|| async { build_id::BUILD }));
        Ok(router)
    });
}

/// When the app started this server (see local_server.rs), it holds our
/// stdin open for as long as it runs; end-of-file means the app is gone.
#[cfg(feature = "server")]
fn exit_with_parent() {
    if std::env::var_os("AKHSAKOV_EXIT_WITH_PARENT").is_some() {
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            std::process::exit(0);
        });
    }
}

#[component]
fn App() -> Element {
    #[cfg(all(feature = "desktop", not(feature = "server")))]
    use_webview_background();
    #[cfg(all(feature = "desktop", not(feature = "server"), target_os = "linux"))]
    use_display_frame_rate();
    rsx! {
        ui::App { Router::<Route> {} }
    }
}

/// Keeps the webview's own background on the theme's base colour. Parts of
/// the page not painted yet (while it loads, or when scrolling outruns
/// painting with AKHSAKOV_FAST_RENDERING=1) show this colour, which is
/// white by default and would flash in every theme.
#[cfg(all(feature = "desktop", not(feature = "server")))]
fn use_webview_background() {
    use_effect(|| {
        let (r, g, b) = ui::theme_base_color();
        let window = dioxus::desktop::window();
        #[cfg(target_os = "linux")]
        {
            use dioxus::desktop::wry::WebViewExtUnix;
            use webkit2gtk::WebViewExt;
            let rgba = gdk::RGBA::new(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, 1.0);
            window.webview.webview().set_background_color(&rgba);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = window.webview.set_background_color((r, g, b, 0xff));
    });
}

/// Renders at the screen's refresh rate. WebKitGTK aims for about 60 fps
/// by default, so on a 144 Hz screen it only drew every second refresh
/// (72 fps) while scrolling or animating. An idle page draws nothing
/// either way. The power-saver profile still halves the rate.
/// AKHSAKOV_60FPS=1 keeps WebKit's default, to compare.
#[cfg(all(feature = "desktop", not(feature = "server"), target_os = "linux"))]
fn use_display_frame_rate() {
    use_hook(|| {
        if std::env::var("AKHSAKOV_60FPS").is_ok_and(|v| v == "1") {
            println!("[render] keeping WebKit's ~60 fps preference (AKHSAKOV_60FPS=1)");
            return;
        }
        use dioxus::desktop::wry::WebViewExtUnix;
        use std::ffi::{c_char, CStr};
        use webkit2gtk::{glib::translate::ToGlibPtr, WebViewExt};

        #[repr(C)]
        struct FeatureList([u8; 0]);
        #[repr(C)]
        struct Feature([u8; 0]);
        // WebKitGTK 2.42+; the Rust bindings stop at 2.40.
        extern "C" {
            fn webkit_settings_get_all_features() -> *mut FeatureList;
            fn webkit_feature_list_get_length(list: *mut FeatureList) -> usize;
            fn webkit_feature_list_get(list: *mut FeatureList, index: usize) -> *mut Feature;
            fn webkit_feature_list_unref(list: *mut FeatureList);
            fn webkit_feature_get_identifier(feature: *mut Feature) -> *const c_char;
            fn webkit_settings_set_feature_enabled(
                settings: *mut webkit2gtk::ffi::WebKitSettings,
                feature: *mut Feature,
                enabled: webkit2gtk::glib::ffi::gboolean,
            );
        }

        let webview = dioxus::desktop::window().webview.webview();
        let Some(settings) = webview.settings() else {
            return;
        };
        unsafe {
            let features = webkit_settings_get_all_features();
            for i in 0..webkit_feature_list_get_length(features) {
                let feature = webkit_feature_list_get(features, i);
                let id = CStr::from_ptr(webkit_feature_get_identifier(feature));
                if id.to_bytes() == b"PreferPageRenderingUpdatesNear60FPS" {
                    webkit_settings_set_feature_enabled(settings.to_glib_none().0, feature, 0);
                    println!("[render] rendering at the display's refresh rate");
                }
            }
            webkit_feature_list_unref(features);
        }
        webview.set_settings(&settings);
    });
}

#[component]
fn Home() -> Element {
    rsx! {
        ui::Home {}
    }
}

#[component]
fn Portfolio() -> Element {
    rsx! {
        ui::Dashboard {}
    }
}

#[component]
fn Settings() -> Element {
    rsx! {
        ui::SettingsPage {}
    }
}

#[component]
fn Market() -> Element {
    rsx! {
        ui::MarketPage {}
    }
}

#[component]
fn Screener() -> Element {
    rsx! {
        ui::ScreenerPage {}
    }
}

#[component]
fn Calendar() -> Element {
    rsx! {
        ui::CalendarPage {}
    }
}

#[component]
fn Economy() -> Element {
    rsx! {
        ui::EconomyPage {}
    }
}

#[component]
fn Learn() -> Element {
    rsx! {
        ui::LearnPage {}
    }
}

#[component]
fn Lesson(slug: String) -> Element {
    rsx! {
        ui::LessonPage { slug }
    }
}

#[component]
fn Backtest() -> Element {
    rsx! {
        ui::BacktestPage {}
    }
}

#[component]
fn Watchlist() -> Element {
    rsx! {
        ui::WatchlistPage {}
    }
}

#[component]
fn Stock(ticker: String) -> Element {
    rsx! {
        ui::StockPage { ticker }
    }
}
