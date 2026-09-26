use crate::i18n::tr;
use crate::{
    components::{
        card::{Segmented, ToggleButton},
        charts::ChartSection,
        tables::PortfoliosCard,
    },
    dashboard::{HoldingsTable, TabPanel},
    editors::{Dialog, Dialogs},
    format::{fmt_signed, fmt_usd, signed_color},
    hooks::{use_portfolio, PortfolioState},
    page::{GhostButton, HeroStat, Page, PageHero},
};
use dioxus::prelude::*;
use dtos::portfolio::GetDashBoardResponse;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use types::ticker_symbol::TickerSymbol;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Portfolios,
    Holdings,
}

#[component]
pub fn Home() -> Element {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let PortfolioState {
        ticker_price_map,
        change_map,
        loaded,
        positions,
        realized,
        total_value,
        total_cost,
        total_pnl,
        day_change,
        pnl_pct,
        day_pct,
        cash,
        ..
    } = use_portfolio(None);
    let dialogs = use_context::<Dialogs>();
    let mut view = use_signal(|| View::Portfolios);

    // (ticker, weight %) largest first, so holding colours match the
    // portfolio page's allocation legend.
    let mut allocation: Vec<(TickerSymbol, Decimal)> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| {
            (
                p.ticker.clone(),
                p.market_value() / total_value.max(dec!(1)) * dec!(100),
            )
        })
        .collect();
    allocation.sort_by(|a, b| b.1.cmp(&a.1));

    let portfolio_count = data.read().portfolios.len();

    rsx! {
        Page {
            PageHero {
                title: tr("Dashboard"),
                loaded,
                total_value: total_value + cash.unwrap_or_default(),
                day_change,
                day_pct,
                actions: rsx! {
                    div { class: "flex flex-wrap gap-2",
                        GhostButton { label: tr("＋ Transaction"), primary: true, onclick: move |_| dialogs.open(Dialog::AddTransaction(None)) }
                        GhostButton { label: tr("Import CSV"), onclick: move |_| dialogs.open(Dialog::Import(None)) }
                        GhostButton { label: tr("＋ Portfolio"), onclick: move |_| dialogs.open(Dialog::NewPortfolio) }
                    }
                },
                HeroStat {
                    label: tr("Return"),
                    value: format!("{} ({pnl_pct:+.2}%)", fmt_signed(total_pnl, 2)),
                    color: signed_color(total_pnl),
                }
                HeroStat { label: tr("Invested"), value: fmt_usd(total_cost, 2) }
                HeroStat { label: tr("Realized"), value: fmt_signed(realized, 2), color: signed_color(realized) }
                if let Some(cash) = cash {
                    HeroStat { label: tr("Cash"), value: fmt_usd(cash, 2) }
                }
            }

            div { class: "mt-10 motion-safe:animate-rise",
                ChartSection {
                    transactions: data().transactions.clone(),
                    portfolios: data()
                        .portfolios
                        .iter()
                        .map(|p| (p.id.to_string(), p.name.clone()))
                        .collect::<Vec<_>>(),
                    height: dec!(260),
                }
            }

            div { class: "mt-5 motion-safe:animate-rise",
                crate::calendar_page::UpcomingEvents { limit: 5 }
            }

            nav { class: "flex items-center justify-between gap-4 mt-10 mb-5",
                Segmented {
                    ToggleButton { label: tr("Portfolios"), active: view() == View::Portfolios, onclick: move |_| view.set(View::Portfolios) }
                    ToggleButton { label: tr("Holdings"), active: view() == View::Holdings, onclick: move |_| view.set(View::Holdings) }
                }
                span { class: "hidden sm:block text-xs text-ctp-overlay1",
                    "{portfolio_count} portfolios · {positions.len()} holdings"
                }
            }

            match view() {
                View::Portfolios => rsx! {
                    TabPanel {
                        PortfoliosCard { data, price_map: ticker_price_map, change_map, loaded }
                    }
                },
                View::Holdings => rsx! {
                    TabPanel {
                        HoldingsTable { positions, allocation, total_value }
                    }
                },
            }
        }
    }
}
