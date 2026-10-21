//! Tokens and estimated cost of AI runs, from the prices entered on each
//! model profile.

use crate::errors::error_message;
use crate::i18n::tr;
use dioxus::prelude::*;
use dtos::ai_models::AiUsageRow;

/// A small dollar amount readably: cents need more places than dollars.
pub fn cost_label(usd: f64) -> String {
    if usd > 0.0 && usd < 0.01 {
        format!("${usd:.4}")
    } else {
        format!("${usd:.2}")
    }
}

fn cell(tokens: u64, cost: f64) -> String {
    format!("{tokens} · {}", cost_label(cost))
}

/// Usage per AI portfolio, model profile and race; hidden until a run
/// has been recorded.
#[component]
pub fn AiUsageTable() -> Element {
    let mut rows = use_signal(Vec::<AiUsageRow>::new);
    let mut error = use_signal(|| None::<String>);
    use_hook(move || {
        spawn(async move {
            match api::get_ai_usage().await {
                Ok(value) => rows.set(value),
                Err(e) => error.set(Some(error_message(e))),
            }
        });
    });
    if let Some(message) = error() {
        return rsx! { p { class: "text-xs text-ctp-red break-words", "{message}" } };
    }
    if rows().is_empty() {
        return rsx! {};
    }
    let unpriced: u32 = rows().iter().filter(|r| r.kind == "profile").map(|r| r.unpriced_runs).sum();
    rsx! {
        div { class: "grid gap-2",
            div { class: "font-medium text-ctp-text", {tr("AI usage and estimated cost")} }
            div { class: "overflow-x-auto",
                table { class: "w-full min-w-[32rem] text-xs",
                    thead { tr { class: "text-left text-ctp-subtext0",
                        th { class: "p-2", {tr("Name")} }
                        th { class: "p-2", {tr("Kind")} }
                        th { class: "p-2 text-right", {tr("7 days")} }
                        th { class: "p-2 text-right", {tr("30 days")} }
                        th { class: "p-2 text-right", {tr("All time")} }
                    } }
                    tbody {
                        for (i, row) in rows().into_iter().enumerate() {
                            tr { key: "{i}", class: "border-t border-ctp-surface0/60",
                                td { class: "p-2 text-ctp-text", "{row.name}" }
                                td { class: "p-2 text-ctp-subtext0",
                                    match row.kind.as_str() {
                                        "portfolio" => tr("Portfolio"),
                                        "profile" => tr("Model connection"),
                                        _ => tr("Race"),
                                    }
                                }
                                td { class: "p-2 text-right tabular-nums", {cell(row.tokens_7d, row.cost_7d)} }
                                td { class: "p-2 text-right tabular-nums", {cell(row.tokens_30d, row.cost_30d)} }
                                td { class: "p-2 text-right tabular-nums", {cell(row.tokens_all, row.cost_all)} }
                            }
                        }
                    }
                }
            }
            p { class: "text-xs text-ctp-overlay1",
                {tr("Tokens · estimated cost from the prices you entered on each model connection.")}
                if unpriced > 0 {
                    " "
                    {crate::i18n::trf("{} runs had no prices or no token counts and are left out of the cost.", &[&unpriced])}
                }
            }
        }
    }
}
