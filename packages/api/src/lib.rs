//! This crate contains all shared fullstack server functions.
use dioxus::prelude::*;
use types::money::MoneyTHB;
/// Echo the user input on the server.
#[post("/api/echo")]
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}

#[post("/api/money")]
pub async fn money() -> Result<(), ServerFnError> {
    println!("{:?}", MoneyTHB::new(5f64));
    Ok(())
}
