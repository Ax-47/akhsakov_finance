//! Server functions for stock research. Each delegates to
//! [`ResearchService`](super::ResearchService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::{
    fundamentals::StockFundamentals,
    research::{CorporateAction, Holders, NewsItem, OptionChainView, RatingChange},
};
use types::ticker_symbol::TickerSymbol;

#[cfg(feature = "server")]
use super::ResearchService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// Profile, key statistics, statements, valuation history and analyst views.
#[post("/api/research/fundamentals", service: Extension<ResearchService>)]
pub async fn get_fundamentals(ticker: TickerSymbol) -> Result<StockFundamentals, ServerFnError> {
    Ok(service.fundamentals(&ticker).await?)
}

#[post("/api/research/news", service: Extension<ResearchService>)]
pub async fn get_news(ticker: TickerSymbol) -> Result<Vec<NewsItem>, ServerFnError> {
    Ok(service.news(&ticker).await?)
}

#[post("/api/research/ratings", service: Extension<ResearchService>)]
pub async fn get_rating_changes(ticker: TickerSymbol) -> Result<Vec<RatingChange>, ServerFnError> {
    Ok(service.rating_changes(&ticker).await?)
}

#[post("/api/research/holders", service: Extension<ResearchService>)]
pub async fn get_holders(ticker: TickerSymbol) -> Result<Holders, ServerFnError> {
    Ok(service.holders(&ticker).await?)
}

#[post("/api/research/actions", service: Extension<ResearchService>)]
pub async fn get_corporate_actions(
    ticker: TickerSymbol,
) -> Result<Vec<CorporateAction>, ServerFnError> {
    Ok(service.corporate_actions(&ticker).await?)
}

/// `expiration` in unix seconds; `None` for the nearest.
#[post("/api/research/options", service: Extension<ResearchService>)]
pub async fn get_option_chain(
    ticker: TickerSymbol,
    expiration: Option<i64>,
) -> Result<OptionChainView, ServerFnError> {
    Ok(service.option_chain(&ticker, expiration).await?)
}
