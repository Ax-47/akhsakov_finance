//! Fallback S&P 500 list for when Wikipedia can't be reached and nothing
//! is cached yet: the largest companies (about three quarters of the index
//! by value) with GICS sector. The last number is shares outstanding in
//! billions (September 2026), kept for reference; sizes come from live
//! market caps.

/// (Yahoo ticker, name, sector).
pub type Member = (&'static str, &'static str, &'static str);

const fn c(ticker: &'static str, name: &'static str, sector: &'static str, _shares_bn: f64) -> Member {
    (ticker, name, sector)
}

const TECH: &str = "Technology";
const COMM: &str = "Communication";
const DISC: &str = "Consumer Discretionary";
const STAPLES: &str = "Consumer Staples";
const HEALTH: &str = "Health Care";
const FIN: &str = "Financials";
const IND: &str = "Industrials";
const ENERGY: &str = "Energy";
const UTIL: &str = "Utilities";
const REIT: &str = "Real Estate";
const MAT: &str = "Materials";

pub const SP500: &[Member] = &[
    // Technology
    c("AAPL", "Apple", TECH, 14.9),
    c("MSFT", "Microsoft", TECH, 7.43),
    c("NVDA", "Nvidia", TECH, 24.3),
    c("AVGO", "Broadcom", TECH, 4.7),
    c("ORCL", "Oracle", TECH, 2.81),
    c("PLTR", "Palantir", TECH, 2.36),
    c("CSCO", "Cisco", TECH, 3.96),
    c("IBM", "IBM", TECH, 0.93),
    c("AMD", "AMD", TECH, 1.62),
    c("CRM", "Salesforce", TECH, 0.823),
    c("MU", "Micron", TECH, 1.12),
    c("INTC", "Intel", TECH, 4.7),
    c("LRCX", "Lam Research", TECH, 1.27),
    c("AMAT", "Applied Materials", TECH, 0.8),
    c("QCOM", "Qualcomm", TECH, 1.08),
    c("NOW", "ServiceNow", TECH, 1.04),
    c("INTU", "Intuit", TECH, 0.28),
    c("APH", "Amphenol", TECH, 2.466),
    c("KLAC", "KLA", TECH, 1.305),
    c("TXN", "Texas Instruments", TECH, 0.91),
    c("ANET", "Arista Networks", TECH, 1.26),
    c("ADBE", "Adobe", TECH, 0.42),
    c("ACN", "Accenture", TECH, 0.62),
    c("ADI", "Analog Devices", TECH, 0.49),
    c("PANW", "Palo Alto Networks", TECH, 0.818),
    c("CRWD", "CrowdStrike", TECH, 1.024),
    // Communication
    c("GOOGL", "Alphabet", COMM, 12.1),
    c("META", "Meta Platforms", COMM, 2.52),
    c("NFLX", "Netflix", COMM, 4.25),
    c("TMUS", "T-Mobile US", COMM, 1.13),
    c("T", "AT&T", COMM, 7.15),
    c("DIS", "Disney", COMM, 1.8),
    c("VZ", "Verizon", COMM, 4.22),
    c("CMCSA", "Comcast", COMM, 3.75),
    // Consumer discretionary
    c("AMZN", "Amazon", DISC, 10.65),
    c("TSLA", "Tesla", DISC, 3.95),
    c("HD", "Home Depot", DISC, 0.995),
    c("MCD", "McDonald's", DISC, 0.715),
    c("BKNG", "Booking", DISC, 0.751),
    c("TJX", "TJX", DISC, 1.12),
    c("LOW", "Lowe's", DISC, 0.56),
    c("SBUX", "Starbucks", DISC, 1.14),
    c("NKE", "Nike", DISC, 1.48),
    // Consumer staples
    c("WMT", "Walmart", STAPLES, 7.98),
    c("COST", "Costco", STAPLES, 0.443),
    c("PG", "Procter & Gamble", STAPLES, 2.34),
    c("KO", "Coca-Cola", STAPLES, 4.3),
    c("PM", "Philip Morris", STAPLES, 1.56),
    c("PEP", "PepsiCo", STAPLES, 1.37),
    c("MO", "Altria", STAPLES, 1.68),
    c("MDLZ", "Mondelez", STAPLES, 1.29),
    c("CL", "Colgate-Palmolive", STAPLES, 0.81),
    // Health care
    c("LLY", "Eli Lilly", HEALTH, 0.897),
    c("JNJ", "Johnson & Johnson", HEALTH, 2.41),
    c("ABBV", "AbbVie", HEALTH, 1.77),
    c("UNH", "UnitedHealth", HEALTH, 0.906),
    c("MRK", "Merck", HEALTH, 2.5),
    c("ABT", "Abbott", HEALTH, 1.74),
    c("TMO", "Thermo Fisher", HEALTH, 0.378),
    c("ISRG", "Intuitive Surgical", HEALTH, 0.358),
    c("AMGN", "Amgen", HEALTH, 0.538),
    c("GILD", "Gilead", HEALTH, 1.24),
    c("PFE", "Pfizer", HEALTH, 5.69),
    c("BSX", "Boston Scientific", HEALTH, 1.48),
    c("DHR", "Danaher", HEALTH, 0.716),
    c("SYK", "Stryker", HEALTH, 0.382),
    c("VRTX", "Vertex", HEALTH, 0.256),
    c("MDT", "Medtronic", HEALTH, 1.28),
    // Financials
    c("BRK-B", "Berkshire Hathaway", FIN, 2.157),
    c("JPM", "JPMorgan Chase", FIN, 2.75),
    c("V", "Visa", FIN, 1.94),
    c("MA", "Mastercard", FIN, 0.905),
    c("BAC", "Bank of America", FIN, 7.45),
    c("WFC", "Wells Fargo", FIN, 3.2),
    c("GS", "Goldman Sachs", FIN, 0.307),
    c("MS", "Morgan Stanley", FIN, 1.6),
    c("C", "Citigroup", FIN, 1.84),
    c("AXP", "American Express", FIN, 0.695),
    c("SCHW", "Charles Schwab", FIN, 1.81),
    c("BLK", "BlackRock", FIN, 0.155),
    c("COF", "Capital One", FIN, 0.64),
    c("SPGI", "S&P Global", FIN, 0.305),
    c("BX", "Blackstone", FIN, 1.22),
    c("PGR", "Progressive", FIN, 0.586),
    c("CB", "Chubb", FIN, 0.4),
    c("KKR", "KKR", FIN, 0.89),
    c("MMC", "Marsh McLennan", FIN, 0.49),
    // Industrials
    c("GE", "GE Aerospace", IND, 1.06),
    c("CAT", "Caterpillar", IND, 0.47),
    c("RTX", "RTX", IND, 1.34),
    c("UBER", "Uber", IND, 2.09),
    c("GEV", "GE Vernova", IND, 0.272),
    c("BA", "Boeing", IND, 0.756),
    c("HON", "Honeywell", IND, 0.317),
    c("UNP", "Union Pacific", IND, 0.593),
    c("ETN", "Eaton", IND, 0.39),
    c("DE", "Deere", IND, 0.271),
    c("LMT", "Lockheed Martin", IND, 0.233),
    c("ADP", "ADP", IND, 0.405),
    // Energy
    c("XOM", "Exxon Mobil", ENERGY, 4.26),
    c("CVX", "Chevron", ENERGY, 2.0),
    c("COP", "ConocoPhillips", ENERGY, 1.25),
    // Utilities
    c("NEE", "NextEra Energy", UTIL, 2.06),
    c("CEG", "Constellation Energy", UTIL, 0.313),
    c("SO", "Southern", UTIL, 1.1),
    c("DUK", "Duke Energy", UTIL, 0.777),
    // Real estate
    c("WELL", "Welltower", REIT, 0.67),
    c("PLD", "Prologis", REIT, 0.928),
    c("AMT", "American Tower", REIT, 0.468),
    c("EQIX", "Equinix", REIT, 0.098),
    // Materials
    c("LIN", "Linde", MAT, 0.47),
    c("NEM", "Newmont", MAT, 1.1),
    c("SHW", "Sherwin-Williams", MAT, 0.25),
    c("FCX", "Freeport-McMoRan", MAT, 1.44),
    c("ECL", "Ecolab", MAT, 0.283),
    c("APD", "Air Products", MAT, 0.222),
];

#[cfg(test)]
mod tests {
    use super::*;
    use types::ticker_symbol::TickerSymbol;

    #[test]
    fn constituents_are_unique_and_valid() {
        let mut tickers: Vec<&str> = SP500.iter().map(|c| c.0).collect();
        tickers.sort();
        tickers.dedup();
        assert_eq!(tickers.len(), SP500.len());
        assert!(SP500.iter().all(|c| TickerSymbol::new(c.0).is_ok()));
    }
}
