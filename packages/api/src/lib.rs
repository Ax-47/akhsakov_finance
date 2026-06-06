//! This crate contains all shared fullstack server functions.
use dioxus::prelude::*;
pub mod asset;
pub mod portfolio;
pub mod transaction;

/// Echo the user input on the server.
#[post("/api/echo")]
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}
