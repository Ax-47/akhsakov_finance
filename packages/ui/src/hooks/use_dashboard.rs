use super::use_price_stream;
use dioxus::prelude::*;
use dtos::{
    portfolio::GetDashBoardResponse,
    position::{compute_positions, portfolio_summary},
    Position,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{HashMap, HashSet};
use types::{
    interval::Interval, range::Range, ticker_symbol::TickerSymbol,
    transaction_type::TransactionType,
};

pub struct DashboardState {
    pub prices: HashMap<TickerSymbol, (Decimal, Decimal)>,
    pub ticker_price_map: HashMap<TickerSymbol, Decimal>,
    pub change_map: HashMap<TickerSymbol, Decimal>,
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

    let tickers = use_memo(move || {
        data()
            .transactions
            .iter()
            .map(|tx| tx.ticker.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    });

    let (price_stream, chart_stream) = use_price_stream(
        tickers,
        Range::D1, // or expose as params
        Interval::I2m,
        true,
    );

    let prices: HashMap<TickerSymbol, (Decimal, Decimal)> = price_stream
        .read()
        .iter()
        .filter_map(|(k, u)| {
            let price = u.current_price;
            let prev = u.previous_close_price;
            let day_change_pct = if prev > Decimal::ZERO {
                (price - prev) / prev * dec!(100)
            } else {
                Decimal::ZERO
            };
            Some((k.clone(), (price, day_change_pct)))
        })
        .collect();

    let loaded = !prices.is_empty();

    let mut ticker_price_map = HashMap::new();
    let mut change_map = HashMap::new();
    for (k, (p, c)) in &prices {
        ticker_price_map.insert(k.clone(), *p);
        change_map.insert(k.clone(), *c);
    }

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
        prices,
        ticker_price_map,
        change_map,
        chart_positive: total_pnl >= Decimal::ZERO,
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
    let mut book: HashMap<TickerSymbol, (Decimal, Decimal)> = HashMap::new();
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
                    if *shares >= tx.shares {
                        let avg = *cost / *shares;
                        let sold = tx.shares.min(*shares);
                        realized += sold * (tx.price - avg);
                        *cost -= sold * avg;
                        *shares -= sold;
                    }
                }
            }
            _ => {}
        }
    }
    realized
}
