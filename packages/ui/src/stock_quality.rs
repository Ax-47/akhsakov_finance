//! The Financials tab's Piotroski F-score: nine yes-or-no checks of the
//! latest fiscal year against the one before, as a checklist. The checks
//! themselves are in `dtos::quality`.

use crate::{
    components::card::Card,
    i18n::{tr, tr_str, trf},
};
use dioxus::prelude::*;
use dtos::{fundamentals::PeriodFinancials, quality::FScore};

/// Eight or nine passes is strong, two or fewer weak (Piotroski's cut-offs).
fn verdict(passed: usize) -> &'static str {
    match passed {
        8.. => tr("Strong"),
        3..=7 => tr("Mixed"),
        _ => tr("Weak"),
    }
}

/// `years` newest first.
#[component]
pub fn FScoreCard(years: Vec<PeriodFinancials>) -> Element {
    let Some(score) = FScore::compute(&years) else {
        return rsx! {};
    };
    let (passed, checked) = (score.passed(), score.checked());
    let mut groups: Vec<(&'static str, Vec<(&'static str, Option<bool>)>)> = vec![];
    for (check, result) in &score.results {
        let group = check.group().label();
        match groups.last_mut() {
            Some((g, rows)) if *g == group => rows.push((check.label(), *result)),
            _ => groups.push((group, vec![(check.label(), *result)])),
        }
    }

    rsx! {
        Card {
            title: tr("Financial strength (Piotroski F-score)"),
            subtitle: tr("The latest fiscal year against the one before").to_string(),
            div { class: "grid gap-6 md:grid-cols-[12rem_1fr]",
                div {
                    div { class: "text-4xl font-semibold tabular-nums text-ctp-text", "{passed}"
                        span { class: "text-xl text-ctp-subtext0", " / {checked}" }
                    }
                    div { class: "mt-1 text-sm font-medium text-ctp-subtext1", {verdict(passed)} }
                    p { class: "mt-2 text-xs text-ctp-overlay1",
                        {trf("{} of {} checks pass.", &[&passed, &checked])}
                        if checked < score.results.len() {
                            " "
                            {trf("{} couldn't be checked: Yahoo didn't report the figures.", &[&(score.results.len() - checked)])}
                        }
                    }
                }
                div { class: "grid gap-4 sm:grid-cols-2 lg:grid-cols-3",
                    for (group, rows) in groups {
                        div { key: "{group}",
                            div { class: "mb-1.5 text-xs font-semibold text-ctp-subtext0", {tr_str(group)} }
                            ul { class: "grid gap-1.5",
                                for (label, result) in rows {
                                    CheckRow { key: "{label}", label, result }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One check: an icon and colour for the result, always with words, so
/// colour is never the only cue.
#[component]
fn CheckRow(label: &'static str, result: Option<bool>) -> Element {
    let (icon, tone, meaning) = match result {
        Some(true) => ("✓", "bg-ctp-green/15 text-ctp-green", tr("Passed")),
        Some(false) => ("✗", "bg-ctp-red/15 text-ctp-red", tr("Failed")),
        None => ("–", "bg-ctp-surface0 text-ctp-overlay1", tr("Not reported")),
    };
    rsx! {
        li { class: "flex items-start gap-2 text-sm text-ctp-subtext1",
            span { class: "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-xs font-bold {tone}", title: meaning, "aria-label": meaning, "{icon}" }
            span { {tr_str(label)} }
        }
    }
}

#[cfg(test)]
mod tests {
    use dtos::quality::{Check, CheckGroup};

    /// Check and group labels are English keys translated where they're
    /// shown, so each needs a Thai entry.
    #[test]
    fn every_check_label_has_a_thai_entry() {
        let groups = [CheckGroup::Profitability, CheckGroup::BalanceSheet, CheckGroup::Efficiency].map(|g| g.label());
        let labels = Check::ALL.iter().map(|c| c.label()).chain(groups);
        let missing: Vec<&str> = labels.filter(|l| !crate::i18n::has_thai(l)).collect();
        assert!(missing.is_empty(), "add to i18n/thai.rs: {missing:?}");
    }
}
