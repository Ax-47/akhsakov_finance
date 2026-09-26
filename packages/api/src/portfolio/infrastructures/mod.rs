//! SQLite adapter for [`PortfolioRepository`].

use crate::{
    database::Database,
    portfolio::repositories::{PortfolioRecord, PortfolioRepository},
    shared::RepositoryError,
};
use dtos::Transaction;
use rusqlite::{params, Row};
use rust_decimal::Decimal;
use std::str::FromStr;
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

pub struct SqlitePortfolioRepository {
    db: Database,
}

impl SqlitePortfolioRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

const INSERT_TX: &str =
    "INSERT OR REPLACE INTO transactions (id, portfolio_id, ticker, kind, shares, price, fee, date)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

fn tx_params(tx: &Transaction) -> [String; 8] {
    [
        tx.id.to_string(),
        tx.portfolio_id.to_string(),
        tx.ticker.to_string(),
        tx.transaction_type.to_string(),
        tx.shares.to_string(),
        tx.price.to_string(),
        tx.fee.to_string(),
        tx.date.clone(),
    ]
}

/// Raw row; converted after the query so parse errors become `Corrupt`.
type TxRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

fn read_tx_row(r: &Row) -> rusqlite::Result<TxRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
    ))
}

fn to_transaction(
    (id, portfolio_id, ticker, kind, shares, price, fee, date): TxRow,
) -> Result<Transaction, RepositoryError> {
    let bad = |what: &str, value: &str| {
        RepositoryError::Corrupt(format!("transaction {id}: {what} \"{value}\""))
    };
    Ok(Transaction {
        id: Uuid::parse_str(&id).map_err(|_| bad("id", &id))?,
        portfolio_id: Uuid::parse_str(&portfolio_id)
            .map_err(|_| bad("portfolio id", &portfolio_id))?,
        ticker: TickerSymbol::new(&ticker).map_err(|_| bad("ticker", &ticker))?,
        transaction_type: TransactionType::from_str(&kind).map_err(|_| bad("type", &kind))?,
        shares: Decimal::from_str(&shares).map_err(|_| bad("shares", &shares))?,
        price: Decimal::from_str(&price).map_err(|_| bad("price", &price))?,
        fee: Decimal::from_str(&fee).map_err(|_| bad("fee", &fee))?,
        date,
    })
}

impl PortfolioRepository for SqlitePortfolioRepository {
    fn portfolios(&self) -> Result<Vec<PortfolioRecord>, RepositoryError> {
        let rows: Vec<(String, String)> = self.db.with(|c| {
            c.prepare("SELECT id, name FROM portfolios ORDER BY created_at, name")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?;
        rows.into_iter()
            .map(|(id, name)| {
                let id = Uuid::parse_str(&id)
                    .map_err(|_| RepositoryError::Corrupt(format!("portfolio id \"{id}\"")))?;
                Ok(PortfolioRecord { id, name })
            })
            .collect()
    }

    fn create_portfolio(&self, record: &PortfolioRecord) -> Result<(), RepositoryError> {
        self.db.with(|c| {
            c.execute(
                "INSERT INTO portfolios (id, name) VALUES (?1, ?2)",
                params![record.id.to_string(), record.name],
            )
        })?;
        Ok(())
    }

    fn rename_portfolio(&self, id: Uuid, name: &str) -> Result<(), RepositoryError> {
        let changed = self.db.with(|c| {
            c.execute(
                "UPDATE portfolios SET name = ?2 WHERE id = ?1",
                params![id.to_string(), name],
            )
        })?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(format!("portfolio {id}")));
        }
        Ok(())
    }

    fn delete_portfolio(&self, id: Uuid) -> Result<(), RepositoryError> {
        let changed = self
            .db
            .with(|c| c.execute("DELETE FROM portfolios WHERE id = ?1", [id.to_string()]))?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(format!("portfolio {id}")));
        }
        Ok(())
    }

    fn transactions(&self) -> Result<Vec<Transaction>, RepositoryError> {
        let rows: Vec<TxRow> = self.db.with(|c| {
            c.prepare("SELECT id, portfolio_id, ticker, kind, shares, price, fee, date FROM transactions ORDER BY date, rowid")?
                .query_map([], read_tx_row)?
                .collect()
        })?;
        rows.into_iter().map(to_transaction).collect()
    }

    fn save_transaction(&self, tx: &Transaction) -> Result<(), RepositoryError> {
        self.db.with(|c| c.execute(INSERT_TX, tx_params(tx)))?;
        Ok(())
    }

    fn insert_transactions(&self, txs: &[Transaction]) -> Result<(), RepositoryError> {
        self.db.transaction(|t| {
            let mut stmt = t.prepare(INSERT_TX)?;
            for tx in txs {
                stmt.execute(tx_params(tx))?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn delete_transaction(&self, id: Uuid) -> Result<(), RepositoryError> {
        let changed = self
            .db
            .with(|c| c.execute("DELETE FROM transactions WHERE id = ?1", [id.to_string()]))?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(format!("transaction {id}")));
        }
        Ok(())
    }
}
