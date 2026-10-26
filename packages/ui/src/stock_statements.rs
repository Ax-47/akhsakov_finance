//! The Financials tab's statements: each line by period, beside its last
//! twelve months, its change on a year earlier and, by year, its compound
//! growth. The figures come from `dtos::statements`; this only lays them out.

use crate::{
    format::{fmt_compact, or_dash},
    i18n::{tr, tr_str, trf},
};
use dioxus::prelude::*;
use dtos::{
    fundamentals::PeriodFinancials,
    statements::{Frequency, Statement},
};

/// The line names stay put while the figures scroll sideways on a phone.
/// The background is the card's own (mantle at 60% over the page), so
/// nothing shows through as the figures pass under it.
const STICKY_LABEL: &str = "sticky left-0 z-10 w-40 md:w-56 whitespace-normal \
    bg-[color-mix(in_oklab,var(--catppuccin-color-mantle)_60%,var(--catppuccin-color-base))]";

/// One statement's lines over `periods` (newest first, `labels` matching).
/// `quarters` are the latest quarters whatever the view, for the last
/// twelve months.
#[component]
pub fn StatementTable(
    statement: Statement,
    periods: Vec<PeriodFinancials>,
    labels: Vec<String>,
    quarters: Vec<PeriodFinancials>,
    frequency: Frequency,
) -> Element {
    // Every table in a view has the same columns, at fixed widths, so a
    // period sits in one column down the page; the balance sheet leaves
    // the last-twelve-months cell empty since it doesn't add up over time.
    let quarterly = frequency == Frequency::Quarterly;
    let adds_up = statement.adds_up_over_time();
    let percent = |v: f64| format!("{:+.1}%", v * 100.0);
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "w-full table-fixed text-sm whitespace-nowrap",
                thead {
                    tr { class: "text-xs text-ctp-subtext0",
                        th { class: "{STICKY_LABEL} pl-6 pr-4 py-2.5 text-left text-sm font-semibold text-ctp-text", {tr_str(statement.title())} }
                        if quarterly {
                            th { class: "w-28 px-4 py-2.5 text-right font-medium", title: tr("Last four quarters added up"),
                                if adds_up { {tr("TTM")} }
                            }
                        } else {
                            th { class: "w-28 px-4 py-2.5 text-right font-medium", title: tr("Compound growth a year, from the oldest year shown"), {tr("Yearly growth")} }
                        }
                        th { class: "w-28 px-4 py-2.5 text-right font-medium", title: tr("Latest period against the same period a year earlier"), {tr("vs a year ago")} }
                        for label in labels.iter() {
                            th { class: "w-28 px-4 py-2.5 text-right font-medium", "{label}" }
                        }
                    }
                }
                tbody {
                    for line in statement.lines() {
                        tr { key: "{line.label}", class: "border-t border-ctp-surface0/60 hover:bg-ctp-surface0/30 transition-colors",
                            td { class: "{STICKY_LABEL} pl-6 pr-4 py-2.5 text-ctp-subtext1", {tr_str(line.label)} }
                            if quarterly {
                                td { class: "px-4 py-2.5 text-right tabular-nums font-semibold text-ctp-text",
                                    if adds_up { {or_dash(line.trailing_year(&quarters), fmt_compact)} }
                                }
                            } else {
                                {
                                    let growth = line.compound_growth(&periods);
                                    rsx! {
                                        td {
                                            class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext1",
                                            title: growth.map(|(_, years)| trf("Over {} years", &[&years])).unwrap_or_default(),
                                            {or_dash(growth.map(|(rate, _)| rate), percent)}
                                        }
                                    }
                                }
                            }
                            td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext1",
                                {or_dash(line.year_on_year(&periods, frequency), percent)}
                            }
                            for p in periods.iter() {
                                td { class: "px-4 py-2.5 text-right tabular-nums text-ctp-subtext0",
                                    {or_dash((line.get)(p), fmt_compact)}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// All three statements, with the period toggle and CSV export above them.
#[component]
pub fn Statements(
    ticker: String,
    periods: Vec<PeriodFinancials>,
    labels: Vec<String>,
    quarters: Vec<PeriodFinancials>,
    frequency: Frequency,
    toggle: Element,
) -> Element {
    let filename = format!(
        "{ticker}-statements-{}.csv",
        match frequency {
            Frequency::Quarterly => "quarterly",
            Frequency::Annual => "annual",
        }
    );
    let rows = periods.clone();
    rsx! {
        crate::components::card::Card {
            title: tr("Statements"),
            subtitle: tr("Newest first, in USD").to_string(),
            actions: rsx! {
                div { class: "flex flex-wrap items-center gap-2",
                    {toggle}
                    crate::files::ExportButtons { filename, csv: move || dtos::csv_export::statements_csv(&rows) }
                }
            },
            flush: true,
            div { class: "grid gap-4",
                for statement in Statement::ALL {
                    StatementTable {
                        key: "{statement:?}",
                        statement,
                        periods: periods.clone(),
                        labels: labels.clone(),
                        quarters: quarters.clone(),
                        frequency,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use dtos::statements::{Statement, LINES};

    /// Statement titles and line labels are English keys translated where
    /// they're shown, so each needs a Thai entry.
    #[test]
    fn every_statement_label_has_a_thai_entry() {
        let labels = Statement::ALL.iter().map(|s| s.title()).chain(LINES.iter().map(|l| l.label));
        let missing: Vec<&str> = labels.filter(|l| !crate::i18n::has_thai(l)).collect();
        assert!(missing.is_empty(), "add to i18n/thai.rs: {missing:?}");
    }
}
