//! Dialogs that change stored data: portfolios, transactions and CSV import.
//! Each calls a server function and reloads the app's data on success.

use crate::i18n::tr;
use crate::{
    app::DataRefresh,
    components::card::{ActionButton, ButtonTone, Field, Modal, Segmented, ToggleButton, INPUT},
};
use dioxus::prelude::*;
use dtos::{
    csv_import::ImportResult, planning::Goal, portfolio::GetDashBoardResponse,
    transaction::CASH_TICKER, watch::AlertKind, Transaction,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

/// Human-readable message from a server function error.
fn message(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message,
        other => other.to_string(),
    }
}

fn portfolios() -> Vec<(Uuid, String)> {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let list = data
        .read()
        .portfolios
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    list
}

// ─── Portfolios ───────────────────────────────────────────────────────────────

/// Create a portfolio (`rename: None`) or rename one.
#[component]
pub fn PortfolioDialog(
    #[props(default)] rename: Option<(Uuid, String)>,
    on_close: EventHandler<()>,
) -> Element {
    let refresh = use_context::<DataRefresh>();
    let initial = rename.as_ref().map(|r| r.1.clone()).unwrap_or_default();
    let mut name = use_signal(move || initial);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let target = rename.as_ref().map(|r| r.0);

    let submit = move |_| async move {
        busy.set(true);
        let result = match target {
            Some(id) => api::rename_portfolio(id, name()).await,
            None => api::create_portfolio(name()).await.map(|_| ()),
        };
        busy.set(false);
        match result {
            Ok(()) => {
                refresh.reload();
                on_close.call(());
            }
            Err(e) => error.set(Some(message(e))),
        }
    };

    rsx! {
        Modal { title: if target.is_some() { tr("Rename portfolio") } else { tr("New portfolio") }, on_close,
            form {
                class: "grid gap-4",
                onsubmit: move |e| e.prevent_default(),
                Field { label: tr("Name"),
                    input { class: INPUT, autofocus: true, placeholder: tr("e.g. Retirement"), value: "{name}", oninput: move |e| name.set(e.value()) }
                }
                ErrorLine { error: error() }
                div { class: "flex justify-end gap-2",
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                    ActionButton { label: if target.is_some() { tr("Save") } else { tr("Create") }, disabled: busy(), onclick: submit }
                }
            }
        }
    }
}

#[component]
pub fn DeletePortfolioDialog(id: Uuid, name: String, on_close: EventHandler<()>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let count = data
        .read()
        .transactions
        .iter()
        .filter(|t| t.portfolio_id == id)
        .count();
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let confirm = move |_| async move {
        busy.set(true);
        match api::delete_portfolio(id).await {
            Ok(()) => {
                refresh.reload();
                on_close.call(());
            }
            Err(e) => {
                busy.set(false);
                error.set(Some(message(e)));
            }
        }
    };
    rsx! {
        Modal { title: tr("Delete {name}?").replace("{name}", &name), on_close,
            p { class: "text-sm text-ctp-subtext0",
                {tr("This deletes the portfolio and its {count} transactions. It can't be undone.").replace("{count}", &count.to_string())}
            }
            ErrorLine { error: error() }
            div { class: "mt-6 flex justify-end gap-2",
                ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                ActionButton { label: tr("Delete portfolio"), tone: ButtonTone::Danger, disabled: busy(), onclick: confirm }
            }
        }
    }
}

// ─── Transactions ─────────────────────────────────────────────────────────────

const KINDS: [(TransactionType, &str); 6] = [
    (TransactionType::Buy, "Buy"),
    (TransactionType::Sell, "Sell"),
    (TransactionType::Dividend, "Dividend"),
    (TransactionType::Deposit, "Deposit"),
    (TransactionType::Withdrawal, "Withdraw"),
    (TransactionType::Split, "Split"),
];

fn is_cash_kind(kind: &TransactionType) -> bool {
    matches!(kind, TransactionType::Deposit | TransactionType::Withdrawal)
}

/// Add a transaction (`existing: None`) or edit one.
#[component]
pub fn TransactionDialog(
    #[props(default)] existing: Option<Transaction>,
    /// Pre-selected portfolio for new transactions.
    #[props(default)]
    portfolio: Option<Uuid>,
    on_close: EventHandler<()>,
) -> Element {
    let refresh = use_context::<DataRefresh>();
    let choices = portfolios();
    let first = portfolio.or_else(|| choices.first().map(|p| p.0));
    let e = existing.clone();
    let mut form = use_signal(move || TxForm::from_existing(e.as_ref(), first));
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let id = existing.as_ref().map_or_else(Uuid::new_v4, |t| t.id);

    // New transactions default to today (the browser knows the local date).
    use_effect(move || {
        if form.peek().date.is_empty() {
            spawn(async move {
                if let Ok(today) = document::eval("return new Date().toLocaleDateString('sv-SE');")
                    .join::<String>()
                    .await
                {
                    form.write().date = today;
                }
            });
        }
    });

    let submit = move |_| async move {
        let tx = match form.read().build(id) {
            Ok(tx) => tx,
            Err(msg) => return error.set(Some(msg)),
        };
        busy.set(true);
        match api::save_transaction(tx).await {
            Ok(()) => {
                refresh.reload();
                on_close.call(());
            }
            Err(e) => {
                busy.set(false);
                error.set(Some(message(e)));
            }
        }
    };

    let f = form.read().clone();
    let (qty_label, price_label) = match f.kind {
        TransactionType::Dividend => ("", "Amount received"),
        TransactionType::Split => ("Split ratio (e.g. 4 for 4-for-1)", ""),
        TransactionType::Deposit | TransactionType::Withdrawal => ("Amount", ""),
        _ => ("Shares", "Price per share"),
    };

    rsx! {
        Modal { title: if existing.is_some() { tr("Edit transaction") } else { tr("Add transaction") }, on_close,
            form {
                class: "grid gap-4",
                onsubmit: move |e| e.prevent_default(),
                Segmented {
                    for (kind, label) in KINDS {
                        ToggleButton {
                            key: "{label}",
                            label,
                            active: f.kind == kind,
                            onclick: move |_| form.write().kind = kind.clone(),
                        }
                    }
                }
                div { class: "grid grid-cols-2 gap-3",
                    Field { label: tr("Portfolio"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| form.write().portfolio = Uuid::parse_str(&e.value()).ok(),
                            for (pid, name) in choices.iter().cloned() {
                                option { value: "{pid}", selected: f.portfolio == Some(pid), "{name}" }
                            }
                        }
                    }
                    Field { label: tr("Date"),
                        input { class: INPUT, r#type: "date", value: "{f.date}", oninput: move |e| form.write().date = e.value() }
                    }
                    if !is_cash_kind(&f.kind) {
                        Field { label: tr("Ticker"), hint: tr("Other markets too, e.g. PTT.BK, 7203.T, VOD.L"),
                            input {
                                class: "{INPUT} uppercase",
                                placeholder: "e.g. NVDA",
                                value: "{f.ticker}",
                                oninput: move |e| form.write().ticker = e.value(),
                                // Look up the trading currency (and a price to start from).
                                onchange: move |e| async move {
                                    let Ok(ticker) = TickerSymbol::new(&e.value()) else { return };
                                    if let Ok(q) = api::quote::quote::get_native_quote(ticker).await {
                                        let mut f = form.write();
                                        f.currency = q.currency;
                                        if f.price.trim().is_empty() && q.current_price > Decimal::ZERO {
                                            f.price = q.current_price.round_dp(4).normalize().to_string();
                                        }
                                    }
                                },
                            }
                        }
                    }
                    Field { label: tr("Currency"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| form.write().currency = e.value(),
                            for code in currency_choices(&f.currency) {
                                option { key: "{code}", value: "{code}", selected: f.currency == code, "{code}" }
                            }
                        }
                    }
                    if !qty_label.is_empty() {
                        Field { label: qty_label,
                            input { class: INPUT, inputmode: "decimal", value: "{f.shares}", oninput: move |e| form.write().shares = e.value() }
                        }
                    }
                    if !price_label.is_empty() {
                        Field { label: price_label,
                            input { class: INPUT, inputmode: "decimal", value: "{f.price}", oninput: move |e| form.write().price = e.value() }
                        }
                    }
                    if f.kind != TransactionType::Split {
                        Field { label: if f.kind == TransactionType::Dividend { tr("Tax withheld") } else { tr("Fee") },
                            input { class: INPUT, inputmode: "decimal", placeholder: "0", value: "{f.fee}", oninput: move |e| form.write().fee = e.value() }
                        }
                    }
                }
                if let Some(total) = f.total() {
                    p { class: "text-xs text-ctp-subtext0", "Total: {total}" }
                }
                if f.currency != "USD" {
                    p { class: "text-xs text-ctp-subtext0",
                        "Converted to USD at the {f.currency} rate on the trade date."
                    }
                }
                ErrorLine { error: error() }
                div { class: "flex justify-end gap-2",
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                    ActionButton { label: tr("Save"), disabled: busy(), onclick: submit }
                }
            }
        }
    }
}

/// Form fields as typed, before validation.
#[derive(Clone, Debug, PartialEq)]
struct TxForm {
    portfolio: Option<Uuid>,
    kind: TransactionType,
    ticker: String,
    date: String,
    shares: String,
    price: String,
    fee: String,
    /// Currency of price and fee; detected from the ticker.
    currency: String,
    /// The edited transaction's (currency, date, rate): the rate is kept
    /// unless the currency or date changes.
    saved_rate: Option<(String, String, Decimal)>,
}

impl TxForm {
    fn from_existing(tx: Option<&Transaction>, portfolio: Option<Uuid>) -> Self {
        match tx {
            Some(t) => Self {
                portfolio: Some(t.portfolio_id),
                kind: t.transaction_type.clone(),
                ticker: if t.is_cash() {
                    String::new()
                } else {
                    t.ticker.to_string()
                },
                date: t.date.clone(),
                // Cash rows store the amount in `shares` (price 1).
                shares: t.shares.normalize().to_string(),
                price: t.price.normalize().to_string(),
                fee: t.fee.normalize().to_string(),
                currency: t.currency.clone(),
                saved_rate: Some((t.currency.clone(), t.date.clone(), t.fx_to_usd)),
            },
            None => Self {
                portfolio,
                kind: TransactionType::Buy,
                ticker: String::new(),
                date: String::new(),
                shares: String::new(),
                price: String::new(),
                fee: String::new(),
                currency: "USD".into(),
                saved_rate: None,
            },
        }
    }

    /// USD per unit for the saved transaction; zero asks the server to look
    /// up the trade date's rate.
    fn fx_to_usd(&self) -> Decimal {
        match &self.saved_rate {
            _ if self.currency == "USD" => Decimal::ONE,
            Some((currency, date, rate)) if *currency == self.currency && *date == self.date => *rate,
            _ => Decimal::ZERO,
        }
    }

    fn total(&self) -> Option<String> {
        let (s, p) = (num(&self.shares)?, num(&self.price)?);
        matches!(self.kind, TransactionType::Buy | TransactionType::Sell)
            .then(|| format!("{:.2} {}", s * p, self.currency))
    }

    fn build(&self, id: Uuid) -> Result<Transaction, String> {
        let portfolio_id = self.portfolio.ok_or("Choose a portfolio")?;
        let cash = is_cash_kind(&self.kind);
        let ticker = if cash {
            TickerSymbol::new(CASH_TICKER)
        } else {
            TickerSymbol::new(&self.ticker)
        }
        .map_err(|_| "Enter a ticker, e.g. NVDA".to_string())?;
        let need =
            |field: &str, label: &str| num(field).ok_or(crate::i18n::trf("Enter a number for {}", &[&label]));
        let (shares, price) = match self.kind {
            TransactionType::Dividend => (Decimal::ZERO, need(&self.price, "the amount")?),
            TransactionType::Split => (need(&self.shares, "the ratio")?, Decimal::ZERO),
            _ if cash => (need(&self.shares, "the amount")?, Decimal::ONE),
            _ => (
                need(&self.shares, "shares")?,
                need(&self.price, "the price")?,
            ),
        };
        Ok(Transaction {
            id,
            portfolio_id,
            ticker,
            transaction_type: self.kind.clone(),
            shares,
            price,
            date: self.date.clone(),
            fee: if self.fee.trim().is_empty() {
                Decimal::ZERO
            } else {
                num(&self.fee).ok_or("Enter a number for the fee")?
            },
            currency: self.currency.clone(),
            fx_to_usd: self.fx_to_usd(),
        })
    }
}

/// Display currencies, plus `current` if it's another one (e.g. SEK).
fn currency_choices(current: &str) -> Vec<String> {
    let mut codes: Vec<String> = dtos::settings::CURRENCIES
        .iter()
        .map(|(code, _)| code.to_string())
        .collect();
    if !codes.iter().any(|c| c == current) {
        codes.push(current.to_string());
    }
    codes
}

fn num(s: &str) -> Option<Decimal> {
    Decimal::from_str(&s.trim().replace(',', "")).ok()
}

/// Delete button that asks once before deleting.
#[component]
pub fn DeleteTransactionButton(id: Uuid) -> Element {
    let refresh = use_context::<DataRefresh>();
    let mut armed = use_signal(|| false);
    let mut failed = use_signal(|| false);
    rsx! {
        button {
            class: if armed() {
                "rounded-full bg-ctp-red/15 px-2.5 py-1 text-xs font-semibold text-ctp-red cursor-pointer"
            } else {
                "inline-flex h-8 min-w-8 items-center justify-center rounded-full px-2 text-sm text-ctp-subtext0 cursor-pointer transition-colors hover:bg-ctp-surface0 hover:text-ctp-red"
            },
            title: if failed() { tr("Couldn't delete — try again") } else { tr("Delete") },
            onclick: move |e| async move {
                e.stop_propagation();
                if !armed() {
                    armed.set(true);
                    return;
                }
                match api::delete_transaction(id).await {
                    Ok(()) => refresh.reload(),
                    Err(_) => failed.set(true),
                }
            },
            onmouseleave: move |_| armed.set(false),
            if armed() { {tr("Delete?")} } else { "🗑" }
        }
    }
}

// ─── Alerts ───────────────────────────────────────────────────────────────────

#[component]
pub fn AlertDialog(
    #[props(default)] ticker: Option<TickerSymbol>,
    on_close: EventHandler<()>,
) -> Element {
    let refresh = use_context::<DataRefresh>();
    let mut symbol = use_signal(move || ticker.map(|t| t.to_string()).unwrap_or_default());
    let mut kind = use_signal(|| AlertKind::PriceAbove);
    let mut value = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |_| async move {
        let Ok(t) = TickerSymbol::new(&symbol()) else {
            return error.set(Some("Enter a ticker, e.g. NVDA".into()));
        };
        let Some(v) = num(&value()) else {
            return error.set(Some("Enter a number".into()));
        };
        busy.set(true);
        match api::create_alert(t, kind(), v).await {
            Ok(_) => {
                refresh.reload();
                on_close.call(());
            }
            Err(e) => {
                busy.set(false);
                error.set(Some(message(e)));
            }
        }
    };
    let value_label = match kind() {
        AlertKind::PriceAbove | AlertKind::PriceBelow => tr("Price ($)"),
        _ => tr("Percent (%)"),
    };

    rsx! {
        Modal { title: tr("New alert"), on_close,
            form { class: "grid gap-4", onsubmit: move |e| e.prevent_default(),
                Field { label: tr("Ticker"),
                    input { class: "{INPUT} uppercase", placeholder: "e.g. NVDA", value: "{symbol}", oninput: move |e| symbol.set(e.value()) }
                }
                Field { label: tr("When"),
                    div { class: "grid gap-1.5",
                        for k in AlertKind::ALL {
                            button {
                                key: "{k}",
                                r#type: "button",
                                class: if kind() == k {
                                    "rounded-xl border border-ctp-mauve bg-ctp-mauve/10 px-3 py-2 text-left text-sm text-ctp-text cursor-pointer"
                                } else {
                                    "rounded-xl border border-ctp-surface0 px-3 py-2 text-left text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1"
                                },
                                onclick: move |_| kind.set(k),
                                "{k.label()}"
                            }
                        }
                    }
                }
                Field { label: value_label,
                    input { class: INPUT, inputmode: "decimal", value: "{value}", oninput: move |e| value.set(e.value()) }
                }
                p { class: "text-xs text-ctp-subtext0", {tr("Alerts are checked against live prices while the app is open, and fire once.")} }
                ErrorLine { error: error() }
                div { class: "flex justify-end gap-2",
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                    ActionButton { label: tr("Create alert"), disabled: busy(), onclick: submit }
                }
            }
        }
    }
}

// ─── Goals ────────────────────────────────────────────────────────────────────

#[component]
pub fn GoalDialog(#[props(default)] goal: Option<Goal>, on_close: EventHandler<()>) -> Element {
    let refresh = use_context::<DataRefresh>();
    let choices = portfolios();
    let existing = goal.clone();
    let mut name = use_signal(move || {
        existing
            .as_ref()
            .map(|g| g.name.clone())
            .unwrap_or_default()
    });
    let existing = goal.clone();
    let mut target = use_signal(move || {
        existing
            .as_ref()
            .map(|g| g.target.normalize().to_string())
            .unwrap_or_default()
    });
    let existing = goal.clone();
    let mut date = use_signal(move || {
        existing
            .as_ref()
            .map(|g| g.date.clone())
            .unwrap_or_default()
    });
    let existing = goal.clone();
    let mut monthly = use_signal(move || {
        existing
            .as_ref()
            .map(|g| g.monthly.normalize().to_string())
            .unwrap_or_default()
    });
    let existing = goal.clone();
    let mut portfolio = use_signal(move || existing.and_then(|g| g.portfolio_id));
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let id = goal.as_ref().map_or_else(Uuid::new_v4, |g| g.id);

    let submit = move |_| async move {
        let Some(target_value) = num(&target()) else {
            return error.set(Some("Enter the target amount".into()));
        };
        let monthly_value = if monthly().trim().is_empty() {
            Some(Decimal::ZERO)
        } else {
            num(&monthly())
        };
        let Some(monthly_value) = monthly_value else {
            return error.set(Some("Enter a number for the monthly amount".into()));
        };
        busy.set(true);
        let g = Goal {
            id,
            name: name(),
            target: target_value,
            date: date(),
            monthly: monthly_value,
            portfolio_id: portfolio(),
        };
        match api::save_goal(g).await {
            Ok(()) => {
                refresh.reload();
                on_close.call(());
            }
            Err(e) => {
                busy.set(false);
                error.set(Some(message(e)));
            }
        }
    };

    rsx! {
        Modal { title: if goal.is_some() { tr("Edit goal") } else { tr("New goal") }, on_close,
            form { class: "grid gap-4", onsubmit: move |e| e.prevent_default(),
                Field { label: tr("Name"),
                    input { class: INPUT, autofocus: true, placeholder: tr("e.g. House deposit"), value: "{name}", oninput: move |e| name.set(e.value()) }
                }
                div { class: "grid grid-cols-2 gap-3",
                    Field { label: tr("Target ($)"),
                        input { class: INPUT, inputmode: "decimal", value: "{target}", oninput: move |e| target.set(e.value()) }
                    }
                    Field { label: tr("By"),
                        input { class: INPUT, r#type: "date", value: "{date}", oninput: move |e| date.set(e.value()) }
                    }
                    Field { label: tr("Adding per month ($)"),
                        input { class: INPUT, inputmode: "decimal", placeholder: "0", value: "{monthly}", oninput: move |e| monthly.set(e.value()) }
                    }
                    Field { label: tr("Counts"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| portfolio.set(Uuid::parse_str(&e.value()).ok()),
                            option { value: "", selected: portfolio().is_none(), {tr("All holdings")} }
                            for (pid, pname) in choices.iter().cloned() {
                                option { value: "{pid}", selected: portfolio() == Some(pid), "{pname}" }
                            }
                        }
                    }
                }
                ErrorLine { error: error() }
                div { class: "flex justify-end gap-2",
                    ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                    ActionButton { label: tr("Save"), disabled: busy(), onclick: submit }
                }
            }
        }
    }
}

// ─── CSV import ───────────────────────────────────────────────────────────────

#[component]
pub fn ImportDialog(
    #[props(default)] portfolio: Option<Uuid>,
    on_close: EventHandler<()>,
) -> Element {
    let refresh = use_context::<DataRefresh>();
    let choices = portfolios();
    let mut target = use_signal(move || portfolio.or_else(|| choices.first().map(|p| p.0)));
    let mut csv = use_signal(String::new);
    let mut result = use_signal(|| None::<ImportResult>);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let choices = portfolios();

    let submit = move |_| async move {
        let Some(portfolio_id) = target() else {
            return error.set(Some("Choose a portfolio".into()));
        };
        busy.set(true);
        match api::import_transactions(portfolio_id, csv()).await {
            Ok(r) => {
                refresh.reload();
                error.set(None);
                result.set(Some(r));
            }
            Err(e) => error.set(Some(message(e))),
        }
        busy.set(false);
    };

    rsx! {
        Modal { title: tr("Import transactions"), on_close,
            if let Some(r) = result() {
                p { class: "text-sm text-ctp-text", "Imported {r.imported} transactions." }
                if !r.errors.is_empty() {
                    p { class: "mt-3 text-xs text-ctp-subtext0", "Skipped {r.errors.len()} lines:" }
                    ul { class: "mt-1 max-h-48 overflow-y-auto rounded-xl bg-ctp-crust/40 p-3 text-xs text-ctp-peach",
                        for err in r.errors {
                            li { "{err}" }
                        }
                    }
                }
                div { class: "mt-6 flex justify-end",
                    ActionButton { label: tr("Done"), onclick: move |_| on_close.call(()) }
                }
            } else {
                div { class: "grid gap-4",
                    Field { label: tr("Into portfolio"),
                        select {
                            class: "{INPUT} cursor-pointer [&_option]:bg-ctp-mantle",
                            onchange: move |e| target.set(Uuid::parse_str(&e.value()).ok()),
                            for (pid, name) in choices.iter().cloned() {
                                option { value: "{pid}", selected: target() == Some(pid), "{name}" }
                            }
                        }
                    }
                    Field {
                        label: tr("CSV file"),
                        hint: tr("Needs columns for date, ticker/symbol, quantity and price; type/action and fee are optional."),
                        input {
                            class: "block w-full text-sm text-ctp-subtext0 file:mr-3 file:cursor-pointer file:rounded-full file:border-0 file:bg-ctp-surface0 file:px-4 file:py-2 file:text-sm file:text-ctp-text",
                            r#type: "file",
                            accept: ".csv,text/csv",
                            onchange: move |e| async move {
                                if let Some(file) = e.files().into_iter().next() {
                                    match file.read_string().await {
                                        Ok(text) => csv.set(text),
                                        Err(_) => error.set(Some("Couldn't read that file".into())),
                                    }
                                }
                            },
                        }
                    }
                    Field { label: tr("…or paste CSV"),
                        textarea {
                            class: "{INPUT} h-32 font-mono text-xs",
                            placeholder: "date,symbol,action,quantity,price,fee\n2026-05-20,NVDA,buy,2,219.80,1",
                            value: "{csv}",
                            oninput: move |e| csv.set(e.value()),
                        }
                    }
                    ErrorLine { error: error() }
                    div { class: "flex justify-end gap-2",
                        ActionButton { label: tr("Cancel"), tone: ButtonTone::Quiet, onclick: move |_| on_close.call(()) }
                        ActionButton { label: tr("Import"), disabled: busy() || csv.read().trim().is_empty(), onclick: submit }
                    }
                }
            }
        }
    }
}

#[component]
fn ErrorLine(error: Option<String>) -> Element {
    rsx! {
        if let Some(e) = error {
            p { class: "rounded-xl bg-ctp-red/10 px-3 py-2 text-sm text-ctp-red", "{e}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn form(kind: TransactionType) -> TxForm {
        TxForm {
            portfolio: Some(Uuid::nil()),
            kind,
            ticker: "nvda".into(),
            date: "2026-05-20".into(),
            shares: "1,000".into(),
            price: "2.5".into(),
            fee: "".into(),
            currency: "USD".into(),
            saved_rate: None,
        }
    }

    #[test]
    fn builds_each_kind() {
        let buy = form(TransactionType::Buy).build(Uuid::nil()).unwrap();
        assert_eq!(
            (buy.ticker.as_str(), buy.shares, buy.price, buy.fee),
            ("NVDA", dec!(1000), dec!(2.5), dec!(0))
        );
        let div = form(TransactionType::Dividend).build(Uuid::nil()).unwrap();
        assert_eq!((div.shares, div.price), (dec!(0), dec!(2.5)));
        let dep = form(TransactionType::Deposit).build(Uuid::nil()).unwrap();
        assert_eq!(
            (dep.ticker.as_str(), dep.shares, dep.price),
            (CASH_TICKER, dec!(1000), dec!(1))
        );
        let bad = TxForm {
            price: "abc".into(),
            ..form(TransactionType::Buy)
        };
        assert!(bad.build(Uuid::nil()).is_err());
        let round_trip = TxForm::from_existing(Some(&buy), None);
        assert_eq!(round_trip.build(buy.id).unwrap(), buy);
    }

    #[test]
    fn keeps_a_foreign_rate_until_currency_or_date_changes() {
        let thb = TxForm { currency: "THB".into(), ..form(TransactionType::Buy) };
        assert_eq!(thb.build(Uuid::nil()).unwrap().fx_to_usd, dec!(0), "server looks it up");
        let saved = Transaction { fx_to_usd: dec!(0.03), ..thb.build(Uuid::nil()).unwrap() };
        let edit = TxForm::from_existing(Some(&saved), None);
        assert_eq!(edit.build(saved.id).unwrap().fx_to_usd, dec!(0.03));
        let moved = TxForm { date: "2026-06-01".into(), ..edit.clone() };
        assert_eq!(moved.build(saved.id).unwrap().fx_to_usd, dec!(0));
        let usd = TxForm { currency: "USD".into(), ..edit };
        assert_eq!(usd.build(saved.id).unwrap().fx_to_usd, dec!(1));
    }
}

// ─── Host ─────────────────────────────────────────────────────────────────────

/// Which editor dialog is open.
#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    NewPortfolio,
    RenamePortfolio(Uuid, String),
    DeletePortfolio(Uuid, String),
    /// Optionally pre-select a portfolio.
    AddTransaction(Option<Uuid>),
    EditTransaction(Transaction),
    Import(Option<Uuid>),
    /// Optionally pre-fill the ticker.
    NewAlert(Option<TickerSymbol>),
    /// New goal (`None`) or edit one.
    Goal(Option<Goal>),
}

/// App-wide handle for opening editor dialogs.
#[derive(Clone, Copy)]
pub struct Dialogs(pub Signal<Option<Dialog>>);

impl Dialogs {
    pub fn open(mut self, dialog: Dialog) {
        self.0.set(Some(dialog));
    }
}

/// Renders the open dialog, if any. Mounted once, in `App`.
#[component]
pub fn EditorHost() -> Element {
    let Dialogs(mut open) = use_context::<Dialogs>();
    let close = move |_| open.set(None);
    match open() {
        None => rsx! {},
        Some(Dialog::NewPortfolio) => rsx! { PortfolioDialog { on_close: close } },
        Some(Dialog::RenamePortfolio(id, name)) => {
            rsx! { PortfolioDialog { rename: (id, name), on_close: close } }
        }
        Some(Dialog::DeletePortfolio(id, name)) => {
            rsx! { DeletePortfolioDialog { id, name, on_close: close } }
        }
        Some(Dialog::AddTransaction(portfolio)) => {
            rsx! { TransactionDialog { portfolio, on_close: close } }
        }
        Some(Dialog::EditTransaction(tx)) => {
            rsx! { TransactionDialog { existing: tx, on_close: close } }
        }
        Some(Dialog::Import(portfolio)) => rsx! { ImportDialog { portfolio, on_close: close } },
        Some(Dialog::NewAlert(ticker)) => rsx! { AlertDialog { ticker, on_close: close } },
        Some(Dialog::Goal(goal)) => rsx! { GoalDialog { goal, on_close: close } },
    }
}
