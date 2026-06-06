use dioxus::prelude::*;
use dtos::transaction::AppData;

use ui::Navbar;
use views::{Blog, Home, Portfolio};

mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},
    #[route("/portfolio")]
    Portfolio {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut app_data: Signal<AppData> = use_signal(AppData::default);
    use_context_provider(|| app_data);

    let _ = use_resource(move || async move {
        match api::get_transactions().await {
            Ok(data) => *app_data.write() = data,
            Err(e) => eprintln!("[web] Failed to load transactions: {e}"),
        }
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn WebNavbar() -> Element {
    rsx! {
        Navbar {
            Link { to: Route::Home {},        "Dashboard" }
            Link { to: Route::Portfolio {},   "Portfolio" }
            Link { to: Route::Blog { id: 1 }, "Blog"      }
        }
        Outlet::<Route> {}
    }
}
