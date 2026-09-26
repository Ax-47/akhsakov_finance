//! The economy snapshot: which gauges and series to show, how to derive
//! them (e.g. inflation from CPI levels), and caching.

use crate::{
    economy::repositories::{GaugeSource, MacroSource},
    shared::ServiceError,
};
use chrono::{Duration as Days, NaiveDate};
use futures::StreamExt;
use dtos::economy::{
    monthly_last, value_on_or_before, year_over_year, CurvePoint, EconomySnapshot, Gauge,
    Indicator,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const GAUGE_TTL: Duration = Duration::from_secs(60);
/// Macro data is published monthly at most; FRED is asked every few hours.
const MACRO_TTL: Duration = Duration::from_secs(6 * 3600);
const HISTORY_YEARS: i64 = 5;
const FRED_CONCURRENCY: usize = 3;
const FRED_RETRY_DELAY: Duration = Duration::from_secs(1);

/// (Yahoo symbol, label, unit).
const GAUGES: [(&str, &str, &str); 11] = [
    ("^GSPC", "S&P 500", ""),
    ("^NDX", "Nasdaq-100", ""),
    ("^DJI", "Dow Jones", ""),
    ("^SET.BK", "SET Index", ""),
    ("^VIX", "VIX · fear gauge", ""),
    ("^TNX", "10-year Treasury", "%"),
    ("DX-Y.NYB", "US dollar index", ""),
    ("CL=F", "Crude oil · WTI", "$"),
    ("GC=F", "Gold", "$"),
    ("BTC-USD", "Bitcoin", "$"),
    ("THB=X", "USD / THB", ""),
];

#[derive(Clone, Copy)]
enum Derive {
    /// As published, one value per month.
    Monthly,
    /// Year-over-year % change of an index level.
    YearOverYear,
}

/// (FRED id, label, description, derivation).
const INDICATORS: [(&str, &str, &str, Derive); 7] = [
    ("CPIAUCSL", "Inflation", "Consumer prices vs a year earlier (CPI).", Derive::YearOverYear),
    ("CPILFESL", "Core inflation", "CPI without food and energy, vs a year earlier.", Derive::YearOverYear),
    ("FEDFUNDS", "Fed funds rate", "The Federal Reserve's policy rate.", Derive::Monthly),
    ("UNRATE", "Unemployment", "Share of the US labour force out of work.", Derive::Monthly),
    ("A191RL1Q225SBEA", "GDP growth", "Real US GDP growth, quarterly, annualised.", Derive::Monthly),
    ("T10Y2Y", "10y − 2y spread", "Below zero (inverted) has often come before recessions.", Derive::Monthly),
    ("MORTGAGE30US", "30-year mortgage", "Average US 30-year fixed mortgage rate.", Derive::Monthly),
];

/// (label, years, FRED id) of Treasury yields.
const CURVE: [(&str, f64, &str); 8] = [
    ("1M", 1.0 / 12.0, "DGS1MO"),
    ("3M", 0.25, "DGS3MO"),
    ("6M", 0.5, "DGS6MO"),
    ("1Y", 1.0, "DGS1"),
    ("2Y", 2.0, "DGS2"),
    ("5Y", 5.0, "DGS5"),
    ("10Y", 10.0, "DGS10"),
    ("30Y", 30.0, "DGS30"),
];

type Macro = (Vec<Indicator>, Vec<CurvePoint>);

#[derive(Clone)]
pub struct EconomyService {
    source: Arc<dyn MacroSource>,
    gauges: Arc<dyn GaugeSource>,
    gauge_cache: Arc<Mutex<Option<(Instant, Vec<Gauge>)>>>,
    macro_cache: Arc<Mutex<Option<(Instant, Macro)>>>,
}

impl EconomyService {
    pub fn new(source: Arc<dyn MacroSource>, gauges: Arc<dyn GaugeSource>) -> Self {
        Self {
            source,
            gauges,
            gauge_cache: Default::default(),
            macro_cache: Default::default(),
        }
    }

    /// Gauges, indicators and the yield curve. A series that can't be
    /// fetched is left out rather than failing the whole snapshot.
    pub async fn snapshot(&self) -> Result<EconomySnapshot, ServiceError> {
        let today = chrono::Utc::now().date_naive();
        let (gauges, (indicators, yield_curve)) =
            tokio::join!(self.gauges(), self.macro_data(today));
        let gauges = gauges?;
        if gauges.is_empty() && indicators.is_empty() {
            return Err(ServiceError::Upstream("No economic data available right now".into()));
        }
        Ok(EconomySnapshot {
            gauges,
            indicators,
            yield_curve,
        })
    }

    async fn gauges(&self) -> Result<Vec<Gauge>, ServiceError> {
        if let Some((at, g)) = self.gauge_cache.lock().await.as_ref() {
            if at.elapsed() < GAUGE_TTL {
                return Ok(g.clone());
            }
        }
        let symbols: Vec<String> = GAUGES.iter().map(|(s, ..)| s.to_string()).collect();
        let levels = match self.gauges.levels(&symbols).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("economy gauges: {e}");
                vec![]
            }
        };
        let gauges: Vec<Gauge> = GAUGES
            .iter()
            .filter_map(|(symbol, label, unit)| {
                let level = levels.iter().find(|l| l.symbol == *symbol)?;
                Some(Gauge {
                    label: label.to_string(),
                    symbol: symbol.to_string(),
                    value: level.value,
                    change_pct: level.change_pct,
                    unit: unit.to_string(),
                })
            })
            .collect();
        if !gauges.is_empty() {
            *self.gauge_cache.lock().await = Some((Instant::now(), gauges.clone()));
        }
        Ok(gauges)
    }

    async fn macro_data(&self, today: NaiveDate) -> Macro {
        if let Some((at, m)) = self.macro_cache.lock().await.as_ref() {
            if at.elapsed() < MACRO_TTL {
                return m.clone();
            }
        }
        // A year more than shown, for year-over-year changes.
        let start = (today - Days::days(365 * (HISTORY_YEARS + 1))).to_string();
        let history_from = (today - Days::days(365 * HISTORY_YEARS)).to_string();
        let ids: Vec<&str> = INDICATORS
            .iter()
            .map(|(id, ..)| *id)
            .chain(CURVE.iter().map(|(_, _, id)| *id))
            .collect();
        // FRED drops connections when many requests arrive at once.
        let fetched: Vec<(&str, Result<Vec<(String, f64)>, String>)> =
            futures::stream::iter(ids.iter().map(|id| {
                let start = start.clone();
                async move {
                    let mut result = self.source.series(id, &start).await;
                    if result.is_err() {
                        tokio::time::sleep(FRED_RETRY_DELAY).await;
                        result = self.source.series(id, &start).await;
                    }
                    (*id, result)
                }
            }))
            .buffer_unordered(FRED_CONCURRENCY)
            .collect()
            .await;
        let series = |id: &str| -> Option<Vec<(String, f64)>> {
            match fetched.iter().find(|(i, _)| *i == id)?.1.as_ref() {
                Ok(s) if !s.is_empty() => Some(s.clone()),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!("FRED {id}: {e}");
                    None
                }
            }
        };

        let indicators: Vec<Indicator> = INDICATORS
            .iter()
            .filter_map(|(id, label, description, derive)| {
                let raw = series(id)?;
                let values = match derive {
                    Derive::Monthly => monthly_last(&raw),
                    Derive::YearOverYear => year_over_year(&monthly_last(&raw)),
                };
                Some(Indicator {
                    id: id.to_string(),
                    label: label.to_string(),
                    description: description.to_string(),
                    unit: "%".into(),
                    history: values.into_iter().filter(|(d, _)| *d >= history_from).collect(),
                })
            })
            .collect();

        let year_ago = (today - Days::days(365)).to_string();
        let curve: Vec<CurvePoint> = CURVE
            .iter()
            .map(|(label, years, id)| {
                let s = series(id).unwrap_or_default();
                CurvePoint {
                    label: label.to_string(),
                    years: *years,
                    now: s.last().map(|(_, v)| *v),
                    year_ago: value_on_or_before(&s, &year_ago),
                }
            })
            .collect();

        let result = (indicators, curve);
        if !result.0.is_empty() {
            *self.macro_cache.lock().await = Some((Instant::now(), result.clone()));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::repositories::Level;
    use async_trait::async_trait;

    /// Monthly CPI rising 0.5 a month from 100; yields at 4%; UNRATE fails.
    struct FakeFred;

    #[async_trait]
    impl MacroSource for FakeFred {
        async fn series(&self, id: &str, start: &str) -> Result<Vec<(String, f64)>, String> {
            let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
            let months = |f: &dyn Fn(usize) -> f64| {
                (0..84)
                    .map(|m| {
                        let d = start + Days::days(31 * m as i64);
                        (d.to_string(), f(m))
                    })
                    .collect::<Vec<_>>()
            };
            match id {
                "UNRATE" => Err("down".into()),
                "CPIAUCSL" => Ok(months(&|m| 100.0 + m as f64 * 0.5)),
                _ => Ok(months(&|_| 4.0)),
            }
        }
    }

    struct FakeGauges;

    #[async_trait]
    impl GaugeSource for FakeGauges {
        async fn levels(&self, symbols: &[String]) -> Result<Vec<Level>, String> {
            Ok(symbols
                .iter()
                .filter(|s| *s == "^VIX")
                .map(|s| Level {
                    symbol: s.clone(),
                    value: 15.0,
                    change_pct: Some(-2.0),
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn builds_the_snapshot_and_skips_failed_series() {
        let s = EconomyService::new(Arc::new(FakeFred), Arc::new(FakeGauges));
        let snap = s.snapshot().await.unwrap();
        assert_eq!(snap.gauges.len(), 1);
        assert_eq!(snap.gauges[0].label, "VIX · fear gauge");
        assert!(!snap.indicators.iter().any(|i| i.id == "UNRATE"));
        let cpi = snap.indicators.iter().find(|i| i.id == "CPIAUCSL").unwrap();
        // 12 months × 0.5 over a base near 130 → about 4.6% a year.
        let (_, latest) = cpi.latest().unwrap();
        assert!(*latest > 4.0 && *latest < 6.5, "{latest}");
        assert_eq!(snap.yield_curve.len(), 8);
        assert_eq!(snap.yield_curve[6].now, Some(4.0));
        assert!(snap.yield_curve[6].year_ago.is_some());
    }
}
