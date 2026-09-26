//! Settings: display currency, analysis assumptions, and data export.

use crate::{
    app::{AppSettings, DataRefresh},
    components::card::{ActionButton, Card, Segmented, Stepper, ToggleButton},
    editors::{Dialog, Dialogs},
    files::{print_report, ExportButtons},
    hooks::{use_portfolio, PortfolioState},
    page::{GhostButton, Page},
};
use dioxus::prelude::*;
use dtos::{
    csv_export::{holdings_csv, transactions_csv},
    portfolio::GetDashBoardResponse,
    settings::{Settings, CURRENCIES},
};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::ticker_symbol::TickerSymbol;

const BENCHMARKS: [(&str, &str); 3] = [
    ("^GSPC", "S&P 500"),
    ("^NDX", "Nasdaq 100"),
    ("^DJI", "Dow Jones"),
];

#[component]
pub fn SettingsPage() -> Element {
    let AppSettings(saved) = use_context::<AppSettings>();
    let refresh = use_context::<DataRefresh>();
    let dialogs = use_context::<Dialogs>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let PortfolioState { positions, .. } = use_portfolio(None);

    let mut draft = use_signal(move || saved.peek().clone());
    let mut risk_free = use_signal(move || saved.peek().risk_free.normalize().to_string());
    let mut assumed = use_signal(move || saved.peek().assumed_return.normalize().to_string());
    let mut custom_benchmark = use_signal(String::new);
    let mut status = use_signal(|| None::<Result<(), String>>);
    let mut switching = use_signal(|| false);
    let mut currency_error = use_signal(|| None::<String>);

    let save = move |_| async move {
        let mut s: Settings = draft();
        let (Ok(rf), Ok(ar)) = (
            Decimal::from_str(risk_free().trim()),
            Decimal::from_str(assumed().trim()),
        ) else {
            return status.set(Some(Err("Enter numbers for the rates".into())));
        };
        s.risk_free = rf;
        s.assumed_return = ar;
        // The currency is saved as soon as it's picked.
        s.currency = saved.peek().currency.clone();
        match api::save_settings(s).await {
            Ok(()) => {
                status.set(Some(Ok(())));
                refresh.reload();
            }
            Err(ServerFnError::ServerError { message, .. }) => status.set(Some(Err(message))),
            Err(e) => status.set(Some(Err(e.to_string()))),
        }
    };

    let names: Vec<(uuid::Uuid, String)> = data
        .read()
        .portfolios
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    let tx_csv = transactions_csv(&data.read().transactions, |t| {
        names
            .iter()
            .find(|(id, _)| *id == t.portfolio_id)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    });
    let pos_csv = holdings_csv(&positions);
    let benchmark = draft.read().benchmark.clone();
    let is_preset = BENCHMARKS.iter().any(|(t, _)| *t == benchmark.as_str());

    rsx! {
        Page {
            header { class: "motion-safe:animate-rise",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight pb-1 bg-gradient-to-r from-ctp-pink via-ctp-mauve to-ctp-sky bg-clip-text text-transparent",
                    "Settings"
                }
                p { class: "mt-2 text-sm text-ctp-overlay1", "How amounts are shown, and the assumptions behind the analysis." }
            }
            div { class: "mt-10 grid gap-5 motion-safe:animate-rise",
                Card { title: "Display currency", subtitle: "Applies instantly, converted at today's rate. What you enter and store stays in USD.".to_string(),
                    div { class: "flex flex-wrap gap-2",
                        for (code, symbol) in CURRENCIES {
                            button {
                                key: "{code}",
                                class: if saved.read().currency == code {
                                    "rounded-full border border-ctp-mauve bg-ctp-mauve/15 px-3.5 py-1.5 text-sm font-medium text-ctp-text cursor-pointer"
                                } else {
                                    "rounded-full border border-ctp-surface0 px-3.5 py-1.5 text-sm text-ctp-subtext0 cursor-pointer hover:border-ctp-surface1 hover:text-ctp-text"
                                },
                                disabled: switching(),
                                onclick: move |_| async move {
                                    switching.set(true);
                                    currency_error.set(AppSettings(saved).change_currency(code).await.err());
                                    switching.set(false);
                                },
                                "{symbol.trim()} {code}"
                            }
                        }
                    }
                    if let Some(message) = currency_error() {
                        p { class: "mt-3 text-sm text-ctp-red", "{message}" }
                    }
                }
                Card { title: "Analysis",
                    div { class: "grid gap-5 md:grid-cols-3",
                        div {
                            div { class: "mb-2 text-xs text-ctp-overlay1", "Risk-free rate" }
                            Stepper { aria_label: "Risk-free rate", value: risk_free(), step: 0.25, suffix: "%", width: "w-14", on_change: move |v| risk_free.set(v) }
                            p { class: "mt-2 text-[0.7rem] text-ctp-overlay0", "Used for Sharpe, alpha and CAPM. Roughly the 3-month T-bill yield." }
                        }
                        div {
                            div { class: "mb-2 text-xs text-ctp-overlay1", "Benchmark" }
                            Segmented {
                                for (ticker, name) in BENCHMARKS {
                                    ToggleButton {
                                        key: "{ticker}",
                                        label: name,
                                        active: benchmark.as_str() == ticker,
                                        onclick: move |_| {
                                            if let Ok(t) = TickerSymbol::new(ticker) {
                                                draft.write().benchmark = t;
                                            }
                                        },
                                    }
                                }
                            }
                            form {
                                class: "mt-2",
                                onsubmit: move |e| {
                                    e.prevent_default();
                                    if let Ok(t) = TickerSymbol::new(&custom_benchmark()) {
                                        draft.write().benchmark = t;
                                    }
                                },
                                input {
                                    class: "w-full rounded-full border border-ctp-surface0 bg-ctp-crust/40 px-3.5 py-1.5 text-sm uppercase text-ctp-text placeholder:normal-case placeholder:text-ctp-overlay0 outline-none focus:border-ctp-mauve",
                                    placeholder: if is_preset { "…or any ticker, e.g. VT ↵".to_string() } else { format!("Using {benchmark}") },
                                    value: "{custom_benchmark}",
                                    oninput: move |e| custom_benchmark.set(e.value()),
                                }
                            }
                            p { class: "mt-2 text-[0.7rem] text-ctp-overlay0", "What your portfolio is compared against in Performance and Risk." }
                        }
                        div {
                            div { class: "mb-2 text-xs text-ctp-overlay1", "Assumed yearly return" }
                            Stepper { aria_label: "Assumed return", value: assumed(), step: 0.5, suffix: "%", width: "w-14", on_change: move |v| assumed.set(v) }
                            p { class: "mt-2 text-[0.7rem] text-ctp-overlay0", "Used to project your goals." }
                        }
                    }
                }
                div { class: "flex items-center justify-end gap-3",
                    if let Some(result) = status() {
                        span { class: if result.is_ok() { "text-sm text-ctp-green" } else { "text-sm text-ctp-red" },
                            {result.as_ref().err().cloned().unwrap_or_else(|| "Saved.".into())}
                        }
                    }
                    ActionButton { label: "Save settings", onclick: save }
                }
                Card { title: "Your data",
                    div { class: "grid gap-4 text-sm",
                        DataRow { label: "Transactions", hint: "Every trade, dividend and cash movement. Re-importable.",
                            ExportButtons { filename: "akhsakov-transactions.csv", csv: tx_csv }
                        }
                        DataRow { label: "Holdings", hint: "Current positions at live prices.",
                            ExportButtons { filename: "akhsakov-holdings.csv", csv: pos_csv }
                        }
                        DataRow { label: "Import", hint: "Add transactions from a broker's CSV export.",
                            GhostButton { label: "Import CSV", onclick: move |_| dialogs.open(Dialog::Import(None)) }
                        }
                        DataRow { label: "Report", hint: "Print the current page, or save it as a PDF from the print dialog.",
                            GhostButton { label: "Print / PDF", onclick: move |_| print_report() }
                        }
                        p { class: "text-xs text-ctp-overlay0",
                            "Your data is stored in akhsakov_finance.db where the app runs. Set the AKHSAKOV_DB environment variable to keep it elsewhere."
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DataRow(label: String, hint: String, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-wrap items-center justify-between gap-3 border-t border-ctp-surface0/60 pt-4 first:border-t-0 first:pt-0",
            div {
                div { class: "font-medium text-ctp-text", "{label}" }
                div { class: "text-xs text-ctp-overlay1", "{hint}" }
            }
            {children}
        }
    }
}
