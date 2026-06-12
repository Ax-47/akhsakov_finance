use serde::{Deserialize, Serialize};
use types::quote::QuoteUpdate;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum QuoteUpdateEvent {
    QuoteUpdate(QuoteUpdate),
}
