//! The Analysis tab's estimates: analysts' consensus EPS and revenue for
//! this and next quarter and fiscal year, how much they disagree, and which
//! way the estimate has been moving.

use crate::{
    components::card::Card,
    format::{fmt_compact, or_dash},
    i18n::tr,
    stock_statements::STICKY_LABEL,
};
use dioxus::prelude::*;
use dtos::fundamentals::Estimate;

fn period_name(code: &str) -> &str {
    match code {
        "0q" => tr("This quarter"),
        "+1q" => tr("Next quarter"),
        "0y" => tr("This fiscal year"),
        "+1y" => tr("Next fiscal year"),
        other => other,
    }
}

type Cell = fn(&Estimate) -> String;

#[component]
pub fn EstimatesTable(estimates: Vec<Estimate>) -> Element {
    if estimates.is_empty() {
        return rsx! {};
    }
    let rows: [(&str, Cell); 9] = [
        (tr("EPS estimate"), |e| or_dash(e.eps, |v| format!("${v:.2}"))),
        (tr("Lowest – highest"), |e| match (e.eps_low, e.eps_high) {
            (Some(lo), Some(hi)) => format!("${lo:.2} – ${hi:.2}"),
            _ => "—".into(),
        }),
        (tr("EPS a year earlier"), |e| or_dash(e.eps_year_ago, |v| format!("${v:.2}"))),
        (tr("EPS growth"), |e| or_dash(e.eps_growth, |v| format!("{:+.1}%", v * 100.0))),
        (tr("Estimate change, 90 days"), |e| {
            or_dash(e.eps.zip(e.eps_90_days_ago.filter(|v| *v != 0.0)), |(now, then)| {
                format!("{:+.1}%", (now / then.abs() - then.signum()) * 100.0)
            })
        }),
        (tr("Revisions, 30 days"), |e| match (e.revisions_up_30_days, e.revisions_down_30_days) {
            (Some(up), Some(down)) => format!("↑{up} ↓{down}"),
            _ => "—".into(),
        }),
        (tr("Revenue estimate"), |e| or_dash(e.revenue, fmt_compact)),
        (tr("Revenue growth"), |e| or_dash(e.revenue_growth, |v| format!("{:+.1}%", v * 100.0))),
        (tr("Analysts"), |e| or_dash(e.analysts, |n| n.to_string())),
    ];

    rsx! {
        Card {
            title: tr("Analyst estimates"),
            subtitle: tr("Consensus forecasts; growth is against the same period a year earlier").to_string(),
            flush: true,
            div { class: "overflow-x-auto",
                table { class: "w-full text-sm whitespace-nowrap",
                    thead {
                        tr { class: "text-xs text-ctp-subtext0",
                            th { class: "{STICKY_LABEL} pl-6 pr-4 py-2.5 text-left font-medium", "" }
                            for e in estimates.iter() {
                                th { class: "px-4 py-2.5 text-right font-medium last:pr-6", {period_name(&e.period)} }
                            }
                        }
                    }
                    tbody {
                        for (label, cell) in rows {
                            tr { key: "{label}", class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
                                td { class: "{STICKY_LABEL} pl-6 pr-4 py-2.5 text-ctp-subtext1", "{label}" }
                                for e in estimates.iter() {
                                    td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0 last:pr-6", {cell(e)} }
                                }
                            }
                        }
                    }
                }
            }
            p { class: "px-6 py-3 text-xs text-ctp-overlay1",
                {tr("Revisions count analysts who raised (↑) or cut (↓) their EPS estimate in the last 30 days.")}
            }
        }
    }
}
