//! Checks active price alerts against live quotes while the app is open.

use crate::{app::DataRefresh, format::fmt_usd, hooks::use_price_stream, notify::Toasts};
use dioxus::prelude::*;
use dtos::{
    portfolio::GetDashBoardResponse,
    position::{compute_positions, portfolio_summary},
};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use types::{interval::Interval, range::Range, ticker_symbol::TickerSymbol};
use uuid::Uuid;

/// Mounted once, in `App`. Fires each alert at most once.
#[component]
pub fn AlertWatcher() -> Element {
    let refresh = use_context::<DataRefresh>();
    let toasts = use_context::<Toasts>();
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let mut fired = use_signal(HashSet::<Uuid>::new);

    let alerts = use_resource(move || async move {
        let _reload = refresh.0();
        api::get_alerts().await.unwrap_or_default()
    });
    let active = use_memo(move || {
        alerts
            .read()
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.is_active())
            .collect::<Vec<_>>()
    });

    // Alert tickers, plus holdings when a weight alert needs the total.
    let tickers = use_memo(move || {
        let mut set: HashSet<TickerSymbol> =
            active.read().iter().map(|a| a.ticker.clone()).collect();
        if active
            .read()
            .iter()
            .any(|a| a.kind == dtos::watch::AlertKind::WeightAbove)
        {
            set.extend(
                data.read()
                    .transactions
                    .iter()
                    .filter(|t| !t.is_cash())
                    .map(|t| t.ticker.clone()),
            );
        }
        set.into_iter().collect::<Vec<_>>()
    });
    let (quotes, _) = use_price_stream(tickers, Range::D1, Interval::I2m, false);

    use_effect(move || {
        let quotes = quotes.read();
        if active.read().is_empty() || quotes.is_empty() {
            return;
        }
        let prices: HashMap<TickerSymbol, (Decimal, Decimal)> = quotes
            .iter()
            .map(|(t, q)| {
                let day = if q.previous_close_price.is_zero() {
                    Decimal::ZERO
                } else {
                    (q.current_price / q.previous_close_price - Decimal::ONE) * Decimal::ONE_HUNDRED
                };
                (t.clone(), (q.current_price, day))
            })
            .collect();
        let positions = compute_positions(&data.read(), &prices);
        let (total, ..) = portfolio_summary(&positions);
        let weight = |t: &TickerSymbol| {
            let p = positions.iter().find(|p| &p.ticker == t)?;
            (total > Decimal::ZERO).then(|| p.market_value() / total * Decimal::ONE_HUNDRED)
        };

        for alert in active.read().iter() {
            if fired.peek().contains(&alert.id) {
                continue;
            }
            let (price, day) = prices
                .get(&alert.ticker)
                .map(|(p, d)| (Some(*p), Some(*d)))
                .unwrap_or((None, None));
            if !alert.is_met(price, day, weight(&alert.ticker)) {
                continue;
            }
            fired.write().insert(alert.id);
            let now = price
                .map(|p| format!("Now {}", fmt_usd(p, 2)))
                .unwrap_or_default();
            toasts.show(format!("🔔 {}", alert.describe()), now);
            let id = alert.id;
            spawn(async move {
                if api::mark_alert_triggered(id).await.is_ok() {
                    refresh.reload();
                }
            });
        }
    });

    rsx! {}
}
