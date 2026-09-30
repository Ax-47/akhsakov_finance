//! Funds, gold, deposits and other assets: describe what a ticker is, mark
//! it as a Thai tax-saving fund, and enter prices for assets Yahoo doesn't
//! price. Hand-priced assets then work like any stock (add transactions
//! with their ticker).

use crate::i18n::{tr, tr_str};
use crate::{
    app::DataRefresh,
    components::card::{ActionButton, ButtonTone, Card, Field, Modal, INPUT},
    hooks::use_price_stream::reprice,
    notify::today,
};
use dioxus::prelude::*;
use dtos::assets::{AssetInfo, ManualPrice, TaxWrapper};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::{asset_class::AssetClass, ticker_symbol::TickerSymbol};

fn message(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message,
        other => other.to_string(),
    }
}

/// Every described asset; reloads with the app's data.
pub fn use_assets() -> Resource<Vec<AssetInfo>> {
    let refresh = use_context::<DataRefresh>();
    use_resource(move || async move {
        let _reload = refresh.0();
        api::get_assets().await.unwrap_or_default()
    })
}

#[component]
pub fn AssetsCard() -> Element {
    let assets = use_assets();
    let mut editing = use_signal(|| None::<Option<AssetInfo>>);
    let list = assets.read().clone().unwrap_or_default();

    rsx! {
        Card {
            title: tr("Funds, gold & other assets"),
            subtitle: tr("Mutual funds, SSF / RMF, gold, deposits and bonds — enter prices yourself when there's no market price").to_string(),
            actions: rsx! {
                ActionButton { label: tr("＋ Asset"), tone: ButtonTone::Quiet, onclick: move |_| editing.set(Some(None)) }
            },
            if list.is_empty() {
                p { class: "text-sm text-ctp-subtext0",
                    {tr("Add an asset, e.g. a Thai mutual fund with ticker KFSSF, then record buys with that ticker. Crypto (BTC-USD) and gold futures (GC=F) have market prices already.")}
                }
            }
            div { class: "flex flex-col gap-2",
                for a in list {
                    AssetRow { key: "{a.ticker}", asset: a.clone(), on_edit: move |a| editing.set(Some(Some(a))) }
                }
            }
        }
        if let Some(existing) = editing() {
            AssetDialog { existing, on_close: move |_| editing.set(None) }
        }
    }
}

#[component]
fn AssetRow(asset: AssetInfo, on_edit: EventHandler<AssetInfo>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let ticker = asset.ticker.clone();
    let t = ticker.clone();
    let mut prices = use_resource(move || {
        let t = t.clone();
        async move {
            let _reload = refresh.0();
            api::get_manual_prices(t).await.unwrap_or_default()
        }
    });
    let mut new_price = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let latest = prices.read().as_ref().and_then(|p| p.first().cloned());
    let edit = asset.clone();
    let del = ticker.clone();

    let add_price = move |_| {
        let ticker = ticker.clone();
        async move {
            let Ok(price) = Decimal::from_str(new_price().trim().replace(',', "").as_str()) else {
                return error.set(Some(tr("Enter a number").into()));
            };
            let Some(date) = today().await else { return };
            match api::save_manual_price(ticker, ManualPrice { date, price }).await {
                Ok(()) => {
                    new_price.set(String::new());
                    error.set(None);
                    prices.restart();
                    reprice();
                }
                Err(e) => error.set(Some(message(e))),
            }
        }
    };

    rsx! {
        div { class: "group flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl bg-ctp-base/40 px-3 py-2 text-sm",
            span { class: "w-24 font-semibold text-ctp-text", "{asset.ticker}" }
            span { class: "rounded-full bg-ctp-surface0 px-2 py-0.5 text-xs text-ctp-subtext1", {tr_str(&asset.class.to_string()).to_string()} }
            if let Some(w) = asset.wrapper {
                span { class: "rounded-full bg-ctp-mauve/15 px-2 py-0.5 text-xs font-semibold text-ctp-mauve", "{w}" }
            }
            span { class: "min-w-0 flex-1 truncate text-ctp-subtext0", "{asset.name}" }
            if asset.manual {
                span { class: "text-xs tabular-nums text-ctp-subtext0",
                    match &latest {
                        Some(p) => rsx! { "{p.price.normalize()} {asset.currency} · {p.date}" },
                        None => rsx! { span { class: "text-ctp-peach", {tr("No price yet")} } },
                    }
                }
                form {
                    class: "flex items-center gap-1.5",
                    onsubmit: move |e| e.prevent_default(),
                    input {
                        class: "w-24 rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3 py-1 text-right text-sm tabular-nums text-ctp-text outline-none focus:border-ctp-mauve",
                        inputmode: "decimal",
                        placeholder: tr("Today's price"),
                        "aria-label": "Today's price for {asset.ticker}",
                        value: "{new_price}",
                        oninput: move |e| new_price.set(e.value()),
                    }
                    ActionButton { label: tr("Update"), tone: ButtonTone::Quiet, disabled: new_price().trim().is_empty(), onclick: add_price }
                }
            } else {
                span { class: "text-xs text-ctp-overlay1", {tr("Market price")} }
            }
            span { class: "flex gap-1 opacity-40 transition-opacity group-hover:opacity-100 focus-within:opacity-100",
                button {
                    class: "rounded-full px-2 py-0.5 text-ctp-subtext0 cursor-pointer hover:bg-ctp-surface0 hover:text-ctp-text",
                    "aria-label": "Edit {asset.ticker}",
                    onclick: move |_| on_edit.call(edit.clone()),
                    "✎"
                }
                crate::components::card::DeleteButton {
                    title: crate::i18n::trf("Forget {}", &[&asset.ticker]),
                    onconfirm: move |_| {
                        let del = del.clone();
                        spawn(async move {
                            if api::delete_asset(del).await.is_ok() {
                                refresh.reload();
                                reprice();
                            }
                        });
                    },
                }
            }
            if let Some(e) = error() {
                p { class: "w-full text-xs text-ctp-red", "{e}" }
            }
        }
    }
}

/// Describe a new asset (`existing: None`) or change one.
#[component]
pub fn AssetDialog(existing: Option<AssetInfo>, on_close: EventHandler<()>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let start = existing.clone().unwrap_or_else(|| AssetInfo {
        class: AssetClass::Fund,
        currency: "THB".into(),
        manual: true,
        ..AssetInfo::new(TickerSymbol::default())
    });
    let is_new = existing.is_none();
    let s = start.clone();
    let mut ticker = use_signal(move || s.ticker.to_string());
    let mut info = use_signal(move || start);
    let mut first_price = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |_| async move {
        let Ok(t) = TickerSymbol::new(&ticker()) else {
            return error.set(Some(tr("Enter a ticker (up to 10 letters), e.g. KFSSF").into()));
        };
        let asset = AssetInfo { ticker: t.clone(), ..info() };
        let price = first_price().trim().replace(',', "");
        let price = if asset.manual && !price.is_empty() {
            match Decimal::from_str(&price) {
                Ok(p) => Some(p),
                Err(_) => return error.set(Some(tr("Enter a number for the price").into())),
            }
        } else {
            None
        };
        busy.set(true);
        let mut result = api::save_asset(asset).await;
        if let (Ok(()), Some(price)) = (&result, price) {
            if let Some(date) = today().await {
                result = api::save_manual_price(t, ManualPrice { date, price }).await;
            }
        }
        busy.set(false);
        match result {
            Ok(()) => {
                refresh.reload();
                reprice();
                on_close.call(());
            }
            Err(e) => error.set(Some(message(e))),
        }
    };

    let f = info();
    rsx! {
        Modal { title: if is_new { tr("New asset") } else { tr("Edit asset") }, on_close,
            form { class: "grid gap-4", onsubmit: move |e| e.prevent_default(),
                div { class: "grid grid-cols-2 gap-3",
                    Field { label: tr("Ticker"), hint: tr("Your own code for it; use it in transactions"),
                        input {
                            class: "{INPUT} uppercase",
                            disabled: !is_new,
                            autofocus: is_new,
                            placeholder: "e.g. KFSSF",
                            value: "{ticker}",
                            oninput: move |e| ticker.set(e.value()),
                        }
                    }
                    Field { label: tr("Name"),
                        input { class: INPUT, placeholder: tr("e.g. Krungsri SSF fund"), value: "{f.name}", oninput: move |e| info.write().name = e.value() }
                    }
                    Field { label: tr("Kind"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| info.write().class = AssetClass::from_key(&e.value()),
                            for c in AssetClass::CHOICES {
                                option { key: "{c.key()}", value: "{c.key()}", selected: f.class == c, {tr_str(&c.to_string()).to_string()} }
                            }
                        }
                    }
                    Field { label: tr("Thai tax-saving fund"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| info.write().wrapper = TaxWrapper::from_key(&e.value()),
                            option { value: "", selected: f.wrapper.is_none(), {tr("None")} }
                            for w in TaxWrapper::ALL {
                                option { key: "{w.key()}", value: "{w.key()}", selected: f.wrapper == Some(w), "{w}" }
                            }
                        }
                    }
                }
                label { class: "flex items-center gap-2 text-sm text-ctp-text cursor-pointer",
                    input { r#type: "checkbox", checked: f.manual, onchange: move |e| info.write().manual = e.checked() }
                    {tr("I enter its price myself (no market price)")}
                }
                if f.manual {
                    div { class: "grid grid-cols-2 gap-3",
                        Field { label: tr("Currency"),
                            select {
                                class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                                onchange: move |e| info.write().currency = e.value(),
                                for (code, _) in dtos::settings::CURRENCIES {
                                    option { key: "{code}", value: "{code}", selected: f.currency == code, "{code}" }
                                }
                            }
                        }
                        Field { label: tr("Price today (optional)"), hint: tr("NAV per unit, gold price, or 1 for a deposit"),
                            input { class: INPUT, inputmode: "decimal", value: "{first_price}", oninput: move |e| first_price.set(e.value()) }
                        }
                    }
                }
                if let Some(e) = error() {
                    p { class: "rounded-xl bg-ctp-red/10 px-3 py-2 text-sm text-ctp-red", "{e}" }
                }
                div { class: "flex justify-end gap-2",
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                    ActionButton { label: tr("Save"), disabled: busy(), onclick: submit }
                }
            }
        }
    }
}
