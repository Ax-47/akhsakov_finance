//! Portfolio use cases: the dashboard read model, portfolio management,
//! recording trades and importing broker CSVs.

use crate::{
    portfolio::repositories::{PortfolioRecord, PortfolioRepository},
    shared::{FxRates, ServiceError},
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
    fx: Arc<dyn FxRates>,
}

impl PortfolioService {
    pub fn new(repo: Arc<dyn PortfolioRepository>, fx: Arc<dyn FxRates>) -> Self {
        Self { repo, fx }
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

    /// Adds a transaction, or updates the one with the same id. A trade in
    /// another currency without a rate gets the trade date's USD rate.
    pub async fn save_transaction(&self, tx: Transaction) -> Result<(), ServiceError> {
        self.ensure_portfolio(tx.portfolio_id)?;
        validate(&tx)?;
        let tx = self.priced(tx).await?;
        Ok(self.repo.save_transaction(&tx)?)
    }

    /// Fills in `fx_to_usd` when it's unknown (zero).
    async fn priced(&self, mut tx: Transaction) -> Result<Transaction, ServiceError> {
        if tx.is_usd() {
            tx.fx_to_usd = Decimal::ONE;
        } else if tx.fx_to_usd <= Decimal::ZERO {
            tx.fx_to_usd = self
                .fx
                .usd_per_unit(&tx.currency, &tx.date)
                .await
                .map_err(|e| {
                    ServiceError::Upstream(format!(
                        "Couldn't get the {} rate for {}: {e}",
                        tx.currency, tx.date
                    ))
                })?;
        }
        Ok(tx)
    }

    pub fn delete_transaction(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_transaction(id)?)
    }

    /// Imports every valid line; invalid lines are reported, not fatal.
    pub async fn import_csv(
        &self,
        portfolio_id: Uuid,
        csv: &str,
    ) -> Result<ImportResult, ServiceError> {
        self.ensure_portfolio(portfolio_id)?;
        let (parsed, mut errors) = parse_transactions_csv(csv, portfolio_id);
        let mut valid = Vec::with_capacity(parsed.len());
        for tx in parsed {
            let (date, ticker) = (tx.date.clone(), tx.ticker.clone());
            let checked = match validate(&tx) {
                Ok(()) => self.priced(tx).await,
                Err(e) => Err(e),
            };
            match checked {
                Ok(tx) => valid.push(tx),
                Err(e) => errors.push(format!("{date} {ticker}: {e}")),
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
    if !(tx.currency.len() == 3 && tx.currency.chars().all(|c| c.is_ascii_uppercase())) {
        return invalid("Currency must be a 3-letter code, e.g. USD");
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

    /// THB is 0.03 USD on every date; anything else is unavailable.
    struct FakeFx;

    #[async_trait::async_trait]
    impl FxRates for FakeFx {
        async fn usd_per_unit(&self, currency: &str, _: &str) -> Result<Decimal, String> {
            self.usd_per_unit_now(currency).await
        }
        async fn usd_per_unit_now(&self, currency: &str) -> Result<Decimal, String> {
            match currency {
                "THB" => Ok(dec!(0.03)),
                _ => Err("no rate".into()),
            }
        }
        async fn currency_of(&self, _: &TickerSymbol) -> Result<String, String> {
            Ok("USD".into())
        }
    }

    fn service() -> PortfolioService {
        PortfolioService::new(
            Arc::new(SqlitePortfolioRepository::new(Database::in_memory().unwrap())),
            Arc::new(FakeFx),
        )
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
            currency: "USD".into(),
            fx_to_usd: Decimal::ONE,
        }
    }

    #[tokio::test]
    async fn portfolio_lifecycle() {
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

        s.save_transaction(buy(id, dec!(2), dec!(100))).await.unwrap();
        s.delete_portfolio(id).unwrap();
        let d = s.dashboard().unwrap();
        assert!(
            d.portfolios.is_empty() && d.transactions.is_empty(),
            "transactions cascade"
        );
    }

    #[tokio::test]
    async fn trades_round_trip_and_validate() {
        let s = service();
        let id = s.create_portfolio("Main").unwrap();
        let mut tx = buy(id, dec!(2), dec!(100.25));
        s.save_transaction(tx.clone()).await.unwrap();
        tx.shares = dec!(3); // edit in place
        s.save_transaction(tx.clone()).await.unwrap();

        let d = s.dashboard().unwrap();
        assert_eq!(d.transactions, vec![tx.clone()]);
        let asset = &d.portfolios[0].assets[0];
        assert_eq!(asset.quantity.value(), dec!(3));
        // (3 × 100.25 + 1 fee) / 3
        assert_eq!(asset.cost.amount().round_dp(4), dec!(100.5833));

        assert!(matches!(
            s.save_transaction(buy(id, dec!(0), dec!(1))).await,
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.save_transaction(Transaction {
                date: "20/05/2026".into(),
                ..buy(id, dec!(1), dec!(1))
            }).await,
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.save_transaction(buy(Uuid::new_v4(), dec!(1), dec!(1))).await,
            Err(ServiceError::NotFound(_))
        ));

        s.delete_transaction(tx.id).unwrap();
        assert!(s.dashboard().unwrap().transactions.is_empty());
    }

    #[tokio::test]
    async fn csv_import_keeps_good_lines() {
        let s = service();
        let id = s.create_portfolio("Imported").unwrap();
        let csv = "date,symbol,action,quantity,price,fee\n\
                   2026-01-05,AAPL,buy,10,150,1\n\
                   2026-02-05,AAPL,sell,0,160,1\n\
                   2026-03-05,MSFT,buy,2,400,0\n";
        let result = s.import_csv(id, csv).await.unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.errors.len(), 1, "zero-quantity sale is rejected");
        assert_eq!(s.dashboard().unwrap().transactions.len(), 2);
    }

    #[tokio::test]
    async fn foreign_trades_get_the_trade_date_rate() {
        let s = service();
        let id = s.create_portfolio("Thai").unwrap();
        let tx = Transaction {
            ticker: TickerSymbol::new("PTT.BK").unwrap(),
            currency: "THB".into(),
            fx_to_usd: Decimal::ZERO,
            ..buy(id, dec!(100), dec!(35))
        };
        s.save_transaction(tx.clone()).await.unwrap();
        assert_eq!(s.dashboard().unwrap().transactions[0].fx_to_usd, dec!(0.03));

        let unknown = Transaction { currency: "XYZ".into(), ..tx.clone() };
        assert!(matches!(
            s.save_transaction(unknown).await,
            Err(ServiceError::Upstream(_))
        ));
        let bad = Transaction { currency: "baht".into(), ..tx };
        assert!(matches!(
            s.save_transaction(bad).await,
            Err(ServiceError::Validation(_))
        ));

        let csv = "date,symbol,action,quantity,price,currency\n2026-01-05,PTT.BK,buy,10,34,THB\n";
        s.import_csv(id, csv).await.unwrap();
        let imported = &s.dashboard().unwrap().transactions[1];
        assert_eq!((imported.currency.as_str(), imported.fx_to_usd), ("THB", dec!(0.03)));
    }
}
