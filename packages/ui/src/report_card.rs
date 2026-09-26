//! Portfolio Activity tab "Monthly report" card: last month (or any month)
//! at a glance, sent to your phone or chat on the 1st if you like, and
//! saved as text or printed to PDF.

use crate::i18n::{tr, trf};
use crate::{
    components::card::{ActionButton, ButtonTone, Card, MetricTile},
    files::{download, print_report},
    format::{fmt_signed, fmt_usd, signed_color},
    notify::today,
};
use dioxus::prelude::*;
use dtos::report::{previous_month, report_text};
use uuid::Uuid;

#[component]
pub fn ReportCard(portfolio: Option<Uuid>) -> Element {
    let date = use_resource(today);
    let today = date.read().clone().flatten();
    // The last 12 full months, newest first.
    let months: Vec<String> = today
        .as_deref()
        .and_then(|t| t.get(..7))
        .map(|m| {
            std::iter::successors(previous_month(m), |m| previous_month(m)).take(12).collect()
        })
        .unwrap_or_default();
    let mut chosen = use_signal(|| None::<String>);
    let month = chosen().or_else(|| months.first().cloned());
    let t = today.clone();
    let report = use_resource(use_reactive!(|month, portfolio, t| async move {
        let (Some(month), Some(today)) = (month, t) else { return None };
        Some(api::get_monthly_report(month, portfolio, today).await.map_err(|e| e.to_string()))
    }));
    let mut schedule = use_resource(|| async { api::get_report_schedule().await.unwrap_or(false) });
    let mut sent = use_signal(|| None::<Result<(), String>>);

    let body = match report.read().clone().flatten() {
        None => rsx! { p { class: "text-sm text-ctp-subtext0", {tr("Working out the month…")} } },
        Some(Err(e)) => rsx! { p { class: "text-sm text-ctp-red", "{e}" } },
        Some(Ok(r)) => {
            let text = report_text(&r, |v| fmt_usd(v, 2));
            let file = format!("report-{}.txt", r.month);
            rsx! {
                div { class: "grid grid-cols-2 gap-3 sm:grid-cols-4",
                    MetricTile { label: tr("Value"), value: fmt_usd(r.end_value, 2), hint: trf("from {}", &[&fmt_usd(r.start_value, 2)]) }
                    MetricTile {
                        label: tr("Market gain"),
                        value: fmt_signed(r.market_gain, 2),
                        tone: signed_color(r.market_gain),
                        hint: format!("{:+}%", r.return_pct),
                    }
                    MetricTile { label: tr("Money added"), value: fmt_signed(r.net_invested, 2), hint: trf("{} buys · {} sells", &[&r.buys, &r.sells]) }
                    MetricTile { label: tr("Dividends"), value: fmt_usd(r.dividends, 2), hint: trf("tax {} · fees {}", &[&fmt_usd(r.dividend_tax, 2), &fmt_usd(r.fees, 2)]) }
                }
                div { class: "mt-4 grid gap-3 text-sm sm:grid-cols-2",
                    div {
                        span { class: "text-xs text-ctp-subtext0", {tr("Best")} }
                        for m in r.best.iter() {
                            p { key: "{m.ticker}", class: "flex justify-between", span { class: "font-semibold text-ctp-text", "{m.ticker}" } span { class: "tabular-nums text-ctp-green", "{m.pct:+}%" } }
                        }
                        if r.best.is_empty() { p { class: "text-ctp-overlay1", "—" } }
                    }
                    div {
                        span { class: "text-xs text-ctp-subtext0", {tr("Worst")} }
                        for m in r.worst.iter() {
                            p { key: "{m.ticker}", class: "flex justify-between", span { class: "font-semibold text-ctp-text", "{m.ticker}" } span { class: "tabular-nums text-ctp-red", "{m.pct:+}%" } }
                        }
                        if r.worst.is_empty() { p { class: "text-ctp-overlay1", "—" } }
                    }
                }
                if !r.unpriced.is_empty() {
                    p { class: "mt-3 text-xs text-ctp-peach", {trf("No price history for {}; valued at zero.", &[&r.unpriced.join(", ")])} }
                }
                div { class: "mt-4 flex flex-wrap gap-2 print:hidden",
                    ActionButton { label: tr("⇩ Text"), tone: ButtonTone::Quiet, onclick: move |_| download(&file, "text/plain", &text) }
                    ActionButton { label: tr("Print / PDF"), tone: ButtonTone::Quiet, onclick: move |_| print_report() }
                }
            }
        }
    };

    rsx! {
        Card {
            title: tr("Monthly report"),
            subtitle: if portfolio.is_some() { tr("This portfolio, month by month").to_string() } else { tr("All holdings, month by month").to_string() },
            actions: rsx! {
                div { class: "flex flex-wrap items-center gap-2 print:hidden",
                    select {
                        class: "rounded-full border border-ctp-surface1 bg-ctp-crust/40 px-3 py-1 text-sm text-ctp-text cursor-pointer [&_option]:bg-ctp-mantle",
                        "aria-label": "Month",
                        onchange: move |e| chosen.set(Some(e.value())),
                        for m in months.iter() {
                            option { key: "{m}", value: "{m}", selected: month.as_deref() == Some(m.as_str()), "{m}" }
                        }
                    }
                    ActionButton {
                        label: match sent() { Some(Ok(())) => tr("Sent ✓"), Some(Err(_)) => tr("Couldn't send"), None => tr("Send now") },
                        tone: ButtonTone::Quiet,
                        onclick: move |_| {
                            let (m, t) = (month.clone(), today.clone());
                            async move {
                                let (Some(m), Some(t)) = (m, t) else { return };
                                sent.set(Some(api::send_monthly_report(m, t).await.map_err(|e| e.to_string())));
                            }
                        },
                    }
                }
            },
            {body}
            label { class: "mt-4 flex items-center gap-2 text-xs text-ctp-subtext0 cursor-pointer print:hidden",
                input {
                    r#type: "checkbox",
                    checked: schedule.read().unwrap_or(false),
                    onchange: move |e| async move {
                        if api::set_report_schedule(e.checked()).await.is_ok() {
                            schedule.restart();
                        }
                    },
                }
                {tr("Send last month's report (all holdings) on the 1st to the app and your push channels (ntfy / Telegram / webhook in Settings)")}
            }
        }
    }
}
