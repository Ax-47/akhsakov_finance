//! Portfolio use cases: the dashboard read model, portfolio management,
//! recording trades and importing broker CSVs.

use crate::{
    portfolio::repositories::{PortfolioRecord, PortfolioRepository},
    shared::ServiceError,
};
use dtos::{
    asset::get_asset_response::GetAssetResponse,
    csv_import::{parse_transactions_csv, ImportResult},
    portfolio::{GetDashBoardResponse, GetPortfolioResponse},
    position::compute_positions,
    Transaction,
};
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::Arc};
use types::{
    asset_class::AssetClass, currency::Currency, money::Money, quantity::Quantity,
    transaction_type::TransactionType,
};
use uuid::Uuid;

const MAX_NAME_LEN: usize = 60;

#[derive(Clone)]
pub struct PortfolioService {
    repo: Arc<dyn PortfolioRepository>,
}

impl PortfolioService {
    pub fn new(repo: Arc<dyn PortfolioRepository>) -> Self {
        Self { repo }
    }

    /// Every portfolio with its current holdings, plus all transactions.
    pub fn dashboard(&self) -> Result<GetDashBoardResponse, ServiceError> {
        let transactions = self.repo.transactions()?;
        let portfolios = self
            .repo
            .portfolios()?
            .into_iter()
            .map(|p| GetPortfolioResponse {
                assets: holdings(p.id, &transactions),
                id: p.id,
                name: p.name,
            })
            .collect();
        Ok(GetDashBoardResponse {
            portfolios,
            transactions,
        })
    }

    pub fn create_portfolio(&self, name: &str) -> Result<Uuid, ServiceError> {
        let name = self.valid_name(name, None)?;
        let id = Uuid::new_v4();
        self.repo.create_portfolio(&PortfolioRecord { id, name })?;
        Ok(id)
    }

    pub fn rename_portfolio(&self, id: Uuid, name: &str) -> Result<(), ServiceError> {
        let name = self.valid_name(name, Some(id))?;
        Ok(self.repo.rename_portfolio(id, &name)?)
    }

    pub fn delete_portfolio(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_portfolio(id)?)
    }

    /// Adds a transaction, or updates the one with the same id.
    pub fn save_transaction(&self, tx: Transaction) -> Result<(), ServiceError> {
        self.ensure_portfolio(tx.portfolio_id)?;
        validate(&tx)?;
        Ok(self.repo.save_transaction(&tx)?)
    }

    pub fn delete_transaction(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_transaction(id)?)
    }

    /// Imports every valid line; invalid lines are reported, not fatal.
    pub fn import_csv(&self, portfolio_id: Uuid, csv: &str) -> Result<ImportResult, ServiceError> {
        self.ensure_portfolio(portfolio_id)?;
        let (parsed, mut errors) = parse_transactions_csv(csv, portfolio_id);
        let mut valid = Vec::with_capacity(parsed.len());
        for tx in parsed {
            match validate(&tx) {
                Ok(()) => valid.push(tx),
                Err(e) => errors.push(format!("{} {}: {e}", tx.date, tx.ticker)),
            }
        }
        self.repo.insert_transactions(&valid)?;
        Ok(ImportResult {
            imported: valid.len(),
            errors,
        })
    }

    fn ensure_portfolio(&self, id: Uuid) -> Result<(), ServiceError> {
        if self.repo.portfolios()?.iter().any(|p| p.id == id) {
            Ok(())
        } else {
            Err(ServiceError::NotFound(format!("portfolio {id}")))
        }
    }

    /// Trimmed, non-empty, not too long and unique (ignoring `except`).
    fn valid_name(&self, name: &str, except: Option<Uuid>) -> Result<String, ServiceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ServiceError::Validation("Give the portfolio a name".into()));
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(ServiceError::Validation(format!(
                "Names can be at most {MAX_NAME_LEN} characters"
            )));
        }
        let taken = self
            .repo
            .portfolios()?
            .iter()
            .any(|p| Some(p.id) != except && p.name.eq_ignore_ascii_case(name));
        if taken {
            return Err(ServiceError::Validation(format!(
                "You already have a portfolio called “{name}”"
            )));
        }
        Ok(name.to_string())
    }
}

/// Business rules for a single transaction.
fn validate(tx: &Transaction) -> Result<(), ServiceError> {
    let invalid = |msg: &str| Err(ServiceError::Validation(msg.into()));
    if !is_iso_date(&tx.date) {
        return invalid("Date must be YYYY-MM-DD");
    }
    if tx.fee < Decimal::ZERO {
        return invalid("Fee can't be negative");
    }
    match tx.transaction_type {
        TransactionType::Dividend if tx.price <= Decimal::ZERO => {
            invalid("Enter the dividend amount received")
        }
        TransactionType::Split if tx.shares <= Decimal::ZERO => {
            invalid("Enter the split ratio, e.g. 4 for 4-for-1")
        }
        TransactionType::Buy
        | TransactionType::Sell
        | TransactionType::Deposit
        | TransactionType::Withdrawal
            if tx.shares <= Decimal::ZERO =>
        {
            invalid("Quantity must be more than zero")
        }
        TransactionType::Buy | TransactionType::Sell if tx.price <= Decimal::ZERO => {
            invalid("Price must be more than zero")
        }
        _ => Ok(()),
    }
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && s[..4].parse::<u32>().is_ok()
        && s[5..7].parse::<u32>().is_ok_and(|m| (1..=12).contains(&m))
        && s[8..].parse::<u32>().is_ok_and(|d| (1..=31).contains(&d))
}

/// Current holdings of one portfolio, at average cost.
fn holdings(portfolio_id: Uuid, transactions: &[Transaction]) -> Vec<GetAssetResponse> {
    let scoped = GetDashBoardResponse {
        portfolios: vec![],
        transactions: transactions
            .iter()
            .filter(|t| t.portfolio_id == portfolio_id)
            .cloned()
            .collect(),
    };
    compute_positions(&scoped, &HashMap::new())
        .into_iter()
        .filter_map(|p| {
            Some(GetAssetResponse {
                portfolio_id,
                ticker_symbol: p.ticker,
                asset_class: AssetClass::Stock,
                quantity: Quantity::new(p.shares).ok()?,
                cost: Money::new(p.avg_cost, Currency::Usd).ok()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, portfolio::infrastructures::SqlitePortfolioRepository};
    use rust_decimal_macros::dec;
    use types::ticker_symbol::TickerSymbol;

    fn service() -> PortfolioService {
        PortfolioService::new(Arc::new(SqlitePortfolioRepository::new(
            Database::in_memory().unwrap(),
        )))
    }

    fn buy(portfolio_id: Uuid, shares: Decimal, price: Decimal) -> Transaction {
        Transaction {
            id: Uuid::new_v4(),
            portfolio_id,
            ticker: TickerSymbol::new("NVDA").unwrap(),
            transaction_type: TransactionType::Buy,
            shares,
            price,
            date: "2026-05-20".into(),
            fee: dec!(1),
        }
    }

    #[test]
    fn portfolio_lifecycle() {
        let s = service();
        let id = s.create_portfolio("  Growth ").unwrap();
        assert!(matches!(
            s.create_portfolio("growth"),
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.create_portfolio("   "),
            Err(ServiceError::Validation(_))
        ));
        s.rename_portfolio(id, "Long term").unwrap();
        assert_eq!(s.dashboard().unwrap().portfolios[0].name, "Long term");
        assert!(matches!(
            s.rename_portfolio(Uuid::new_v4(), "x"),
            Err(ServiceError::NotFound(_))
        ));

        s.save_transaction(buy(id, dec!(2), dec!(100))).unwrap();
        s.delete_portfolio(id).unwrap();
        let d = s.dashboard().unwrap();
        assert!(
            d.portfolios.is_empty() && d.transactions.is_empty(),
            "transactions cascade"
        );
    }

    #[test]
    fn trades_round_trip_and_validate() {
        let s = service();
        let id = s.create_portfolio("Main").unwrap();
        let mut tx = buy(id, dec!(2), dec!(100.25));
        s.save_transaction(tx.clone()).unwrap();
        tx.shares = dec!(3); // edit in place
        s.save_transaction(tx.clone()).unwrap();

        let d = s.dashboard().unwrap();
        assert_eq!(d.transactions, vec![tx.clone()]);
        let asset = &d.portfolios[0].assets[0];
        assert_eq!(asset.quantity.value(), dec!(3));
        // (3 × 100.25 + 1 fee) / 3
        assert_eq!(asset.cost.amount().round_dp(4), dec!(100.5833));

        assert!(matches!(
            s.save_transaction(buy(id, dec!(0), dec!(1))),
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.save_transaction(Transaction {
                date: "20/05/2026".into(),
                ..buy(id, dec!(1), dec!(1))
            }),
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.save_transaction(buy(Uuid::new_v4(), dec!(1), dec!(1))),
            Err(ServiceError::NotFound(_))
        ));

        s.delete_transaction(tx.id).unwrap();
        assert!(s.dashboard().unwrap().transactions.is_empty());
    }

    #[test]
    fn csv_import_keeps_good_lines() {
        let s = service();
        let id = s.create_portfolio("Imported").unwrap();
        let csv = "date,symbol,action,quantity,price,fee\n\
                   2026-01-05,AAPL,buy,10,150,1\n\
                   2026-02-05,AAPL,sell,0,160,1\n\
                   2026-03-05,MSFT,buy,2,400,0\n";
        let result = s.import_csv(id, csv).unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.errors.len(), 1, "zero-quantity sale is rejected");
        assert_eq!(s.dashboard().unwrap().transactions.len(), 2);
    }
}
