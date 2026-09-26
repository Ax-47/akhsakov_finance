//! Claude's own portfolio: paper money you give it, which it trades through
//! the connector at live prices. Orders only ever go into that portfolio;
//! your other portfolios stay read-only to the AI.

use crate::{
    mcp::repositories::{AiPortfolioRepository, LivePrices, LiveQuote},
    portfolio::PortfolioService,
    quote::services::quote::is_convertible,
    shared::ServiceError,
};
use dtos::{
    ai_portfolio::{AiPortfolioInfo, AI_PORTFOLIO_NAME},
    compute_positions,
    portfolio::GetDashBoardResponse,
    position::cash_balance,
    transaction::CASH_TICKER,
    Position, Transaction,
};
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal_macros::dec;
use std::{collections::HashMap, sync::Arc};
use types::{ticker_symbol::TickerSymbol, transaction_type::TransactionType};
use uuid::Uuid;

/// Most you can fund it with at once, in USD.
const MAX_FUNDING: Decimal = dec!(1000000000);
/// Decimal places kept on share counts (fractional shares are allowed).
const SHARE_DP: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

/// How much to trade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Shares(Decimal),
    /// This many dollars' worth, rounded down to whole millionths of a share.
    Usd(Decimal),
    /// Everything held (sells only).
    All,
}

/// A filled order.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub tx: Transaction,
    /// What it cost or raised, in USD.
    pub usd_total: Decimal,
    pub cash_after: Decimal,
}

/// Claude's portfolio as it stands: its transactions and live prices.
pub struct Book {
    pub id: Uuid,
    pub name: String,
    pub transactions: Vec<Transaction>,
}

impl Book {
    pub fn cash(&self) -> Decimal {
        cash_balance(&self.transactions).unwrap_or_default()
    }

    /// Deposits less withdrawals.
    pub fn funded(&self) -> Decimal {
        self.transactions
            .iter()
            .filter(|t| t.is_cash())
            .map(|t| match t.transaction_type {
                TransactionType::Deposit => t.shares * t.usd_price() - t.usd_fee(),
                TransactionType::Withdrawal => -(t.shares * t.usd_price()) - t.usd_fee(),
                _ => Decimal::ZERO,
            })
            .sum()
    }

    pub fn trades(&self) -> impl DoubleEndedIterator<Item = &Transaction> {
        self.transactions
            .iter()
            .filter(|t| matches!(t.transaction_type, TransactionType::Buy | TransactionType::Sell))
    }

    /// Holdings at cost; `current_price` is zero.
    pub fn positions(&self) -> Vec<Position> {
        let scoped = GetDashBoardResponse {
            portfolios: vec![],
            transactions: self.transactions.clone(),
        };
        compute_positions(&scoped, &HashMap::new())
    }

    fn held(&self, ticker: &TickerSymbol) -> Decimal {
        self.positions()
            .into_iter()
            .find(|p| p.ticker == *ticker)
            .map_or(Decimal::ZERO, |p| p.shares)
    }

    fn info(&self) -> AiPortfolioInfo {
        AiPortfolioInfo {
            portfolio_id: self.id,
            name: self.name.clone(),
            funded: self.funded(),
            cash: self.cash(),
            trades: self.trades().count(),
        }
    }
}

#[derive(Clone)]
pub struct Trading {
    repo: Arc<dyn AiPortfolioRepository>,
    portfolios: PortfolioService,
    prices: Arc<dyn LivePrices>,
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn invalid(msg: impl Into<String>) -> ServiceError {
    ServiceError::Validation(msg.into())
}

fn cash_tx(portfolio_id: Uuid, amount: Decimal) -> Transaction {
    Transaction {
        id: Uuid::new_v4(),
        portfolio_id,
        ticker: TickerSymbol::new(CASH_TICKER).expect("the cash ticker is valid"),
        transaction_type: TransactionType::Deposit,
        shares: amount,
        price: Decimal::ONE,
        date: today(),
        fee: Decimal::ZERO,
        currency: "USD".into(),
        fx_to_usd: Decimal::ONE,
    }
}

impl Trading {
    pub fn new(
        repo: Arc<dyn AiPortfolioRepository>,
        portfolios: PortfolioService,
        prices: Arc<dyn LivePrices>,
    ) -> Self {
        Self {
            repo,
            portfolios,
            prices,
        }
    }

    /// Claude's portfolio, or `None` if you haven't given it one.
    pub fn book(&self) -> Result<Option<Book>, ServiceError> {
        let Some(id) = self.repo.ai_portfolio()? else {
            return Ok(None);
        };
        let dash = self.portfolios.dashboard()?;
        let Some(p) = dash.portfolios.iter().find(|p| p.id == id) else {
            return Ok(None);
        };
        Ok(Some(Book {
            id,
            name: p.name.clone(),
            transactions: dash
                .transactions
                .into_iter()
                .filter(|t| t.portfolio_id == id)
                .collect(),
        }))
    }

    fn required_book(&self) -> Result<Book, ServiceError> {
        self.book()?.ok_or_else(|| {
            invalid(
                "You don't have a portfolio of your own yet. The user can give you one, with starting \
                 cash, in the app under Settings → Connect Claude.",
            )
        })
    }

    pub fn info(&self) -> Result<Option<AiPortfolioInfo>, ServiceError> {
        Ok(self.book()?.map(|b| b.info()))
    }

    /// Makes a new portfolio for Claude with `starting_cash` USD in it.
    pub async fn start(&self, starting_cash: Decimal) -> Result<AiPortfolioInfo, ServiceError> {
        check_amount(starting_cash)?;
        if self.book()?.is_some() {
            return Err(invalid("Claude already has a portfolio. Add funds to it instead."));
        }
        let taken: Vec<String> = self
            .portfolios
            .dashboard()?
            .portfolios
            .into_iter()
            .map(|p| p.name.to_lowercase())
            .collect();
        let name = std::iter::once(AI_PORTFOLIO_NAME.to_string())
            .chain((2..).map(|n| format!("{AI_PORTFOLIO_NAME} {n}")))
            .find(|n| !taken.contains(&n.to_lowercase()))
            .expect("some name is free");
        let id = self.portfolios.create_portfolio(&name)?;
        self.portfolios.save_transaction(cash_tx(id, starting_cash)).await?;
        self.repo.set_ai_portfolio(Some(id))?;
        Ok(self.required_book()?.info())
    }

    /// Gives Claude more cash to invest.
    pub async fn add_funds(&self, amount: Decimal) -> Result<AiPortfolioInfo, ServiceError> {
        check_amount(amount)?;
        let book = self.required_book()?;
        self.portfolios.save_transaction(cash_tx(book.id, amount)).await?;
        Ok(self.required_book()?.info())
    }

    /// Takes the portfolio back: Claude can't trade in it any more. It and
    /// its history stay, as an ordinary portfolio.
    pub fn stop(&self) -> Result<(), ServiceError> {
        Ok(self.repo.set_ai_portfolio(None)?)
    }

    pub async fn quote(&self, ticker: &TickerSymbol) -> Result<LiveQuote, ServiceError> {
        self.prices.quote(ticker).await.map_err(|e| {
            ServiceError::Upstream(format!("Couldn't get a price for {ticker}: {e}"))
        })
    }

    /// Buys or sells in Claude's portfolio at the live price, with no fee.
    pub async fn order(&self, ticker: &TickerSymbol, side: Side, size: Size) -> Result<Fill, ServiceError> {
        let book = self.required_book()?;
        if ticker.as_str() == CASH_TICKER || !is_convertible(ticker.as_str()) {
            return Err(invalid("Only stocks, funds and other assets can be traded, not cash, indices or currency pairs."));
        }
        let q = self.quote(ticker).await?;
        if q.stale {
            return Err(ServiceError::Upstream(format!(
                "Only an old saved price for {ticker} is available right now; try again later."
            )));
        }
        if q.price <= Decimal::ZERO || q.usd_per_unit <= Decimal::ZERO {
            return Err(ServiceError::Upstream(format!("{ticker} has no usable price.")));
        }
        let usd_price = q.price * q.usd_per_unit;
        let held = book.held(ticker);
        let shares = match size {
            Size::Shares(s) => s.round_dp_with_strategy(SHARE_DP, RoundingStrategy::ToZero),
            Size::Usd(amount) => (amount / usd_price).round_dp_with_strategy(SHARE_DP, RoundingStrategy::ToZero),
            Size::All if side == Side::Sell => held,
            Size::All => return Err(invalid("\"all\" only works for selling; say how many shares or dollars to buy.")),
        };
        if shares <= Decimal::ZERO {
            return Err(invalid(format!(
                "That's less than a millionth of a share at ${}.",
                usd_price.round_dp(2)
            )));
        }
        let cash = book.cash();
        let usd_total = shares * usd_price;
        match side {
            Side::Buy if usd_total > cash => {
                return Err(invalid(format!(
                    "Not enough cash: {shares} {ticker} costs ${} and you have ${}.",
                    usd_total.round_dp(2),
                    cash.round_dp(2)
                )))
            }
            Side::Sell if held <= Decimal::ZERO => return Err(invalid(format!("You don't hold {ticker}."))),
            Side::Sell if shares > held => {
                return Err(invalid(format!("You hold only {} {ticker}.", held.normalize())))
            }
            _ => {}
        }
        let tx = Transaction {
            id: Uuid::new_v4(),
            portfolio_id: book.id,
            ticker: ticker.clone(),
            transaction_type: match side {
                Side::Buy => TransactionType::Buy,
                Side::Sell => TransactionType::Sell,
            },
            shares,
            price: q.price,
            date: today(),
            fee: Decimal::ZERO,
            currency: q.currency.clone(),
            fx_to_usd: q.usd_per_unit,
        };
        self.portfolios.save_transaction(tx.clone()).await?;
        let cash_after = match side {
            Side::Buy => cash - usd_total,
            Side::Sell => cash + usd_total,
        };
        Ok(Fill {
            tx,
            usd_total,
            cash_after,
        })
    }
}

fn check_amount(amount: Decimal) -> Result<(), ServiceError> {
    if amount <= Decimal::ZERO {
        return Err(invalid("Enter an amount more than zero"));
    }
    if amount > MAX_FUNDING {
        return Err(invalid("That's more than the app can hold"));
    }
    Ok(())
}
