use super::use_price_stream;
use dioxus::prelude::*;
use dtos::{
    portfolio::GetDashBoardResponse,
    position::{compute_positions, portfolio_summary},
    Position,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use types::transaction_type::TransactionType;

pub struct DashboardState {
    pub prices: HashMap<String, (Decimal, Decimal)>,
    pub ticker_price_map: HashMap<String, Decimal>,
    pub change_map: HashMap<String, Decimal>,
    pub loaded: bool,
    pub positions: Vec<Position>,
    pub realized: Decimal,
    pub total_value: Decimal,
    pub total_cost: Decimal,
    pub total_pnl: Decimal,
    pub day_change: Decimal,
    pub pnl_pct: Decimal,
    pub day_pct: Decimal,
    pub chart_positive: bool,
}

pub fn use_dashboard() -> DashboardState {
    let data = use_context::<Signal<GetDashBoardResponse>>();

    let tickers: Vec<String> = data()
        .transactions
        .iter()
        .map(|tx| tx.ticker.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let price_map = use_price_stream(tickers);

    let prices: HashMap<String, (Decimal, Decimal)> = price_map
        .read()
        .iter()
        .map(|(k, u)| (k.clone(), (u.price, u.change_pct)))
        .collect();

    let loaded = !prices.is_empty();
    let positions = compute_positions(&data(), &prices);
    let realized = compute_realized_pnl(&data());
    let (total_value, total_cost, total_pnl, day_change) = portfolio_summary(&positions);

    let pnl_pct = if total_cost > Decimal::ZERO {
        total_pnl / total_cost * dec!(100)
    } else {
        Decimal::ZERO
    };
    let day_pct = if total_value > Decimal::ZERO {
        day_change / total_value * dec!(100)
    } else {
        Decimal::ZERO
    };

    DashboardState {
        ticker_price_map: prices.iter().map(|(k, (p, _))| (k.clone(), *p)).collect(),
        change_map: prices.iter().map(|(k, (_, c))| (k.clone(), *c)).collect(),
        chart_positive: total_pnl >= Decimal::ZERO,
        prices,
        loaded,
        positions,
        realized,
        total_value,
        total_cost,
        total_pnl,
        day_change,
        pnl_pct,
        day_pct,
    }
}

fn compute_realized_pnl(data: &GetDashBoardResponse) -> Decimal {
    let mut book: HashMap<String, (Decimal, Decimal)> = HashMap::new(); // (cost_basis, shares)
    let mut realized = Decimal::ZERO;

    for tx in &data.transactions {
        match tx.transaction_type {
            TransactionType::Buy => {
                let e = book.entry(tx.ticker.clone()).or_default();
                e.0 += tx.shares * tx.price;
                e.1 += tx.shares;
            }
            TransactionType::Sell => {
                if let Some((cost, shares)) = book.get_mut(&tx.ticker) {
                    if *shares > Decimal::ZERO {
                        let avg = *cost / *shares;
                        realized += tx.shares * (tx.price - avg);
                        *cost -= tx.shares * avg;
                        *shares -= tx.shares;
                    }
                }
            }
            _ => {}
        }
    }
    realized
}
