//! Live portfolio state: positions priced from the quote stream, totals,
//! allocation and concentration metrics, for all holdings or one portfolio.

use super::mpt::{compute_mpt, MptAnalysis};
use super::use_price_stream;
use dioxus::prelude::*;
use dtos::{
    portfolio::GetDashBoardResponse,
    position::{cash_balance, compute_positions, portfolio_summary, realized_pnl},
    Position,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{HashMap, HashSet};
use types::ticker_symbol::TickerSymbol;

#[derive(Clone, PartialEq)]
pub struct PortfolioState {
    /// Latest price per ticker.
    pub ticker_price_map: HashMap<TickerSymbol, Decimal>,
    /// Today's change (%) per ticker.
    pub change_map: HashMap<TickerSymbol, Decimal>,
    pub loaded: bool,
    pub positions: Vec<Position>,
    /// Realized gains incl. dividends, net of fees.
    pub realized: Decimal,
    /// Market value of holdings (excludes cash).
    pub total_value: Decimal,
    pub total_cost: Decimal,
    pub total_pnl: Decimal,
    pub day_change: Decimal,
    pub pnl_pct: Decimal,
    pub day_pct: Decimal,
    /// Uninvested cash, when deposits / withdrawals are recorded.
    pub cash: Option<Decimal>,
    /// `(ticker, weight %)`, largest first.
    pub allocation: Vec<(TickerSymbol, Decimal)>,
    pub mpt: Option<MptAnalysis>,
}

/// `scope` is a portfolio id, or `None` for all holdings. Prices come from
/// the app-wide stream, so pages and scope changes don't reconnect.
///
/// Memoised: the positions are recomputed once per price update (not on
/// every render of every caller), and callers only re-render when the
/// result actually changes.
pub fn use_portfolio(scope: Option<String>) -> PortfolioState {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let LiveQuotes(quotes) = use_context::<LiveQuotes>();
    let state = use_memo(use_reactive!(|scope| compute_portfolio(
        &data.read(),
        &quotes.read(),
        scope.as_deref()
    )));
    state()
}

/// Shares held per ticker, ignoring prices: for pages that only need to
/// know what you own, so they don't re-render on every price tick.
pub fn use_held_shares() -> Memo<HashMap<TickerSymbol, Decimal>> {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    use_memo(move || {
        compute_positions(&data.read(), &HashMap::new())
            .into_iter()
            .map(|p| (p.ticker, p.shares))
            .collect()
    })
}

fn compute_portfolio(
    data: &GetDashBoardResponse,
    quotes: &HashMap<TickerSymbol, types::quote::Quote>,
    scope: Option<&str>,
) -> PortfolioState {
    let prices: HashMap<TickerSymbol, (Decimal, Decimal)> = quotes
        .iter()
        .map(|(ticker, q)| {
            let change = day_change_pct(q.current_price, q.previous_close_price);
            (ticker.clone(), (q.current_price, change))
        })
        .collect();
    let loaded = !prices.is_empty();
    let ticker_price_map = prices.iter().map(|(t, (p, _))| (t.clone(), *p)).collect();
    let change_map = prices.iter().map(|(t, (_, c))| (t.clone(), *c)).collect();

    let scoped = scoped_data(data, scope);
    let positions = compute_positions(&scoped, &prices);
    let (total_value, total_cost, total_pnl, day_change) = portfolio_summary(&positions);
    let pct = |part: Decimal, whole: Decimal| {
        if whole > Decimal::ZERO {
            part / whole * dec!(100)
        } else {
            Decimal::ZERO
        }
    };

    let mut allocation: Vec<(TickerSymbol, Decimal)> = positions
        .iter()
        .filter(|p| p.current_price > Decimal::ZERO)
        .map(|p| (p.ticker.clone(), pct(p.market_value(), total_value)))
        .collect();
    allocation.sort_by(|a, b| b.1.cmp(&a.1));

    PortfolioState {
        ticker_price_map,
        change_map,
        loaded,
        realized: realized_pnl(&scoped.transactions),
        cash: cash_balance(&scoped.transactions),
        mpt: if loaded {
            compute_mpt(&positions, total_value)
        } else {
            None
        },
        positions,
        total_value,
        total_cost,
        total_pnl,
        day_change,
        pnl_pct: pct(total_pnl, total_cost),
        day_pct: pct(day_change, total_value),
        allocation,
    }
}

/// Live prices of every stock in any portfolio, streamed once for the
/// whole app (see [`use_live_quotes_provider`]).
#[derive(Clone, Copy)]
pub struct LiveQuotes(pub ReadSignal<HashMap<TickerSymbol, types::quote::Quote>>);

/// Starts the app-wide portfolio price stream. Call once, below the
/// portfolio data context.
pub fn use_live_quotes_provider() {
    let data = use_context::<Signal<GetDashBoardResponse>>();
    let tickers = use_memo(move || {
        let mut all: Vec<TickerSymbol> = data
            .read()
            .transactions
            .iter()
            .filter(|tx| !tx.is_cash())
            .map(|tx| tx.ticker.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        all.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        all
    });
    let quotes = use_price_stream(tickers);
    use_context_provider(|| LiveQuotes(quotes));
}

/// Percent move from the previous close to the current price.
fn day_change_pct(current: Decimal, previous_close: Decimal) -> Decimal {
    if previous_close.is_zero() {
        return Decimal::ZERO;
    }
    (current - previous_close) / previous_close * dec!(100)
}

/// Only the transactions of one portfolio, or everything for `None`.
pub fn scoped_data(data: &GetDashBoardResponse, scope: Option<&str>) -> GetDashBoardResponse {
    match scope {
        None => data.clone(),
        Some(id) => GetDashBoardResponse {
            portfolios: data
                .portfolios
                .iter()
                .filter(|p| p.id.to_string() == id)
                .cloned()
                .collect(),
            transactions: data
                .transactions
                .iter()
                .filter(|tx| tx.portfolio_id.to_string() == id)
                .cloned()
                .collect(),
        },
    }
}
