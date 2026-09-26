//! Storage port for portfolios and transactions.

use crate::shared::RepositoryError;
use dtos::Transaction;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioRecord {
    pub id: Uuid,
    pub name: String,
}

pub trait PortfolioRepository: Send + Sync {
    fn portfolios(&self) -> Result<Vec<PortfolioRecord>, RepositoryError>;
    fn create_portfolio(&self, record: &PortfolioRecord) -> Result<(), RepositoryError>;
    /// `NotFound` if no portfolio has `id`.
    fn rename_portfolio(&self, id: Uuid, name: &str) -> Result<(), RepositoryError>;
    /// Also deletes the portfolio's transactions.
    fn delete_portfolio(&self, id: Uuid) -> Result<(), RepositoryError>;

    fn transactions(&self) -> Result<Vec<Transaction>, RepositoryError>;
    /// Inserts, or replaces the transaction with the same id.
    fn save_transaction(&self, tx: &Transaction) -> Result<(), RepositoryError>;
    /// All-or-nothing insert.
    fn insert_transactions(&self, txs: &[Transaction]) -> Result<(), RepositoryError>;
    fn delete_transaction(&self, id: Uuid) -> Result<(), RepositoryError>;
}
