use crate::money::Money;

pub enum CashFlow {
    In(Money),
    Out(Money),
}
