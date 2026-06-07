use dioxus::prelude::*;
use dtos::asset::get_asset_response::GetAssetResponse;
use dtos::portfolio::GetPortfolioResponse;
use rust_decimal_macros::dec;
use types::{asset_class::AssetClass, currency::Currency, money::Money, quantity::Quantity, ticker_symbol::TickerSymbol};
use uuid::Uuid;

#[post("/api/portfolios")]
pub async fn get_portfolios() -> Result<Vec<GetPortfolioResponse>, ServerFnError> {
    let pid = Uuid::new_v4();

    let assets = vec![
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("AAPL").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(10)).unwrap(),
            cost: Money::new(dec!(185.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("MSFT").unwrap(),
            asset_class: AssetClass::Stock,
            quantity: Quantity::new(dec!(8)).unwrap(),
            cost: Money::new(dec!(415.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("BTC").unwrap(),
            asset_class: AssetClass::Crypto,
            quantity: Quantity::new(dec!(0.25)).unwrap(),
            cost: Money::new(dec!(67000.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("SPY").unwrap(),
            asset_class: AssetClass::Etf,
            quantity: Quantity::new(dec!(15)).unwrap(),
            cost: Money::new(dec!(525.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("BND").unwrap(),
            asset_class: AssetClass::Bond,
            quantity: Quantity::new(dec!(30)).unwrap(),
            cost: Money::new(dec!(72.00), Currency::Usd).unwrap(),
        },
        GetAssetResponse {
            portfolio_id: pid,
            ticker_symbol: TickerSymbol::new("USD").unwrap(),
            asset_class: AssetClass::Cash,
            quantity: Quantity::new(dec!(5000)).unwrap(),
            cost: Money::new(dec!(1.00), Currency::Usd).unwrap(),
        },
    ];

    Ok(vec![GetPortfolioResponse {
        id: pid,
        name: "My Portfolio".to_string(),
        assets,
    }])
}
