use crate::{Transaction, asset::get_asset_response::GetAssetResponse};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GetPortfolioResponse {
    pub id: Uuid,
    pub name: String,
    pub assets: Vec<GetAssetResponse>,
    /// An AI assistant manages it with paper money (see `ai_portfolio`).
    #[serde(default)]
    pub ai: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, Deserialize)]
pub struct GetDashBoardResponse {
    pub portfolios: Vec<GetPortfolioResponse>,
    pub transactions: Vec<Transaction>,
}

impl GetDashBoardResponse {
    /// Ids of the AI-managed paper portfolios.
    pub fn ai_ids(&self) -> Vec<Uuid> {
        self.portfolios.iter().filter(|p| p.ai).map(|p| p.id).collect()
    }

    /// Only your own money: AI paper portfolios and their
    /// transactions left out, for "all holdings" totals, tax and reports.
    pub fn without_ai(&self) -> GetDashBoardResponse {
        let ai = self.ai_ids();
        GetDashBoardResponse {
            portfolios: self.portfolios.iter().filter(|p| !p.ai).cloned().collect(),
            transactions: self
                .transactions
                .iter()
                .filter(|t| !ai.contains(&t.portfolio_id))
                .cloned()
                .collect(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PortfolioHistoryPoint {
    pub label: String,
    pub value: Decimal,
    pub pct: Decimal,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GetPortfolioHistoryResponse {
    pub points: Vec<(String, Vec<PortfolioHistoryPoint>)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};

    fn tx(portfolio_id: Uuid) -> Transaction {
        Transaction {
            id: Uuid::new_v4(),
            portfolio_id,
            ticker: TickerSymbol::new("NVDA").unwrap(),
            transaction_type: TransactionType::Buy,
            shares: Decimal::ONE,
            price: Decimal::TEN,
            date: "2026-01-05".into(),
            fee: Decimal::ZERO,
            currency: "USD".into(),
            fx_to_usd: Decimal::ONE,
        }
    }

    #[test]
    fn without_ai_keeps_only_your_money() {
        let (mine, ai) = (Uuid::new_v4(), Uuid::new_v4());
        let data = GetDashBoardResponse {
            portfolios: vec![
                GetPortfolioResponse { id: mine, name: "Main".into(), ..Default::default() },
                GetPortfolioResponse { id: ai, name: "AI".into(), ai: true, ..Default::default() },
            ],
            transactions: vec![tx(mine), tx(ai), tx(ai)],
        };
        assert_eq!(data.ai_ids(), vec![ai]);
        let yours = data.without_ai();
        assert_eq!(yours.portfolios.len(), 1);
        assert_eq!(yours.portfolios[0].id, mine);
        assert!(yours.transactions.iter().all(|t| t.portfolio_id == mine));
        assert_eq!(yours.transactions.len(), 1);
    }
}
