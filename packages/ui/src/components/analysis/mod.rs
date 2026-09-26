pub mod capm;
pub use capm::*;
pub mod mpt;
pub use mpt::*;
pub mod risk;
pub use risk::*;
pub mod compare;
pub mod stats;
pub mod stock;

/// A holding period: `2.3 yr`, or `5 mo` under a year.
pub fn held_label(years: f64) -> String {
    if years >= 1.0 {
        crate::i18n::trf("{} yr", &[&format!("{years:.1}")])
    } else {
        crate::i18n::trf("{} mo", &[&(years * 12.0).round().max(1.0)])
    }
}
