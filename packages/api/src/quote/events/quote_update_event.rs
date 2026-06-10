use serde::{Deserialize, Serialize};
use types::quote::Quote;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) enum QuoteUpdateEvent {
    Quote(Quote),
    Init,
}
