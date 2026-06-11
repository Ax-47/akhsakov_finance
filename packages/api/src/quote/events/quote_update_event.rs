use serde::{Deserialize, Serialize};
use types::quote::Quote;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum QuoteUpdateEvent {
    Quote(Quote),
}
