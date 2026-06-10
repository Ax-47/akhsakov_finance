use std::fmt::Display;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
/// Supported resolution intervals.
pub enum Interval {
    /// 1 second
    I1s,
    /// 2 seconds
    I2s,
    /// 3 seconds
    I3s,
    /// 5 seconds
    I5s,
    /// 6 seconds
    I6s,
    /// 10 seconds
    I10s,
    /// 15 seconds
    I15s,
    /// 30 seconds
    I30s,
    /// 90 seconds
    I90s,
    /// 1 minute
    #[default]
    I1m,
    /// 2 minutes
    I2m,
    /// 3 minutes
    I3m,
    /// 5 minutes
    I5m,
    /// 6 minutes
    I6m,
    /// 10 minutes
    I10m,
    /// 15 minutes
    I15m,
    /// 30 minutes
    I30m,
    /// 90 minutes
    I90m,
    /// 1 hour
    I1h,
    /// 2 hours
    I2h,
    /// 3 hours
    I3h,
    /// 4 hours
    I4h,
    /// 6 hours
    I6h,
    /// 8 hours
    I8h,
    /// 12 hours
    I12h,
    /// 1 day
    D1,
    /// 5 days (provider-dependent)
    D5,
    /// 1 week
    W1,
    /// 1 month
    M1,
    /// 3 months
    M3,
    /// 6 months
    M6,
    /// 1 year
    Y1,
    /// 2 years
    Y2,
    /// 5 years
    Y5,
    /// 10 years
    Y10,
}

impl Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).code())
    }
}

impl Interval {
    /// Returns the stable wire-format code for this interval.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::I1s => "1s",
            Self::I2s => "2s",
            Self::I3s => "3s",
            Self::I5s => "5s",
            Self::I6s => "6s",
            Self::I10s => "10s",
            Self::I15s => "15s",
            Self::I30s => "30s",
            Self::I90s => "90s",
            Self::I1m => "1m",
            Self::I2m => "2m",
            Self::I3m => "3m",
            Self::I5m => "5m",
            Self::I6m => "6m",
            Self::I10m => "10m",
            Self::I15m => "15m",
            Self::I30m => "30m",
            Self::I90m => "90m",
            Self::I1h => "1h",
            Self::I2h => "2h",
            Self::I3h => "3h",
            Self::I4h => "4h",
            Self::I6h => "6h",
            Self::I8h => "8h",
            Self::I12h => "12h",
            Self::D1 => "1d",
            Self::D5 => "5d",
            Self::W1 => "1wk",
            Self::M1 => "1mo",
            Self::M3 => "3mo",
            Self::M6 => "6mo",
            Self::Y1 => "1y",
            Self::Y2 => "2y",
            Self::Y5 => "5y",
            Self::Y10 => "10y",
        }
    }

    fn from_code(value: &str) -> Option<Self> {
        match value {
            "1s" => Some(Self::I1s),
            "2s" => Some(Self::I2s),
            "3s" => Some(Self::I3s),
            "5s" => Some(Self::I5s),
            "6s" => Some(Self::I6s),
            "10s" => Some(Self::I10s),
            "15s" => Some(Self::I15s),
            "30s" => Some(Self::I30s),
            "90s" => Some(Self::I90s),
            "1m" => Some(Self::I1m),
            "2m" => Some(Self::I2m),
            "3m" => Some(Self::I3m),
            "5m" => Some(Self::I5m),
            "6m" => Some(Self::I6m),
            "10m" => Some(Self::I10m),
            "15m" => Some(Self::I15m),
            "30m" => Some(Self::I30m),
            "90m" => Some(Self::I90m),
            "1h" => Some(Self::I1h),
            "2h" => Some(Self::I2h),
            "3h" => Some(Self::I3h),
            "4h" => Some(Self::I4h),
            "6h" => Some(Self::I6h),
            "8h" => Some(Self::I8h),
            "12h" => Some(Self::I12h),
            "1d" => Some(Self::D1),
            "5d" => Some(Self::D5),
            "1wk" => Some(Self::W1),
            "1mo" => Some(Self::M1),
            "3mo" => Some(Self::M3),
            "6mo" => Some(Self::M6),
            "1y" => Some(Self::Y1),
            "2y" => Some(Self::Y2),
            "5y" => Some(Self::Y5),
            "10y" => Some(Self::Y10),
            _ => None,
        }
    }

    /// Is this an intraday interval?
    #[must_use]
    pub const fn is_intraday(self) -> bool {
        matches!(
            self,
            Self::I1s
                | Self::I2s
                | Self::I3s
                | Self::I5s
                | Self::I6s
                | Self::I10s
                | Self::I15s
                | Self::I30s
                | Self::I90s
                | Self::I1m
                | Self::I2m
                | Self::I3m
                | Self::I5m
                | Self::I6m
                | Self::I10m
                | Self::I15m
                | Self::I30m
                | Self::I90m
                | Self::I1h
                | Self::I2h
                | Self::I3h
                | Self::I4h
                | Self::I6h
                | Self::I8h
                | Self::I12h
        )
    }

    /// Returns the interval length in minutes for intraday, otherwise None.
    #[must_use]
    pub const fn minutes(self) -> Option<i64> {
        match self {
            Self::I1m => Some(1),
            Self::I2m => Some(2),
            Self::I3m => Some(3),
            Self::I5m => Some(5),
            Self::I6m => Some(6),
            Self::I10m => Some(10),
            Self::I15m => Some(15),
            Self::I30m => Some(30),
            Self::I90m => Some(90),
            Self::I1h => Some(60),
            Self::I2h => Some(120),
            Self::I3h => Some(180),
            Self::I4h => Some(240),
            Self::I6h => Some(360),
            Self::I8h => Some(480),
            Self::I12h => Some(720),
            _ => None,
        }
    }

    /// Returns the interval length in seconds for intraday, otherwise None.
    #[must_use]
    pub const fn seconds(self) -> Option<i64> {
        match self {
            Self::I1s => Some(1),
            Self::I2s => Some(2),
            Self::I3s => Some(3),
            Self::I5s => Some(5),
            Self::I6s => Some(6),
            Self::I10s => Some(10),
            Self::I15s => Some(15),
            Self::I30s => Some(30),
            Self::I90s => Some(90),
            Self::I1m => Some(60),
            Self::I2m => Some(120),
            Self::I3m => Some(180),
            Self::I5m => Some(300),
            Self::I6m => Some(360),
            Self::I10m => Some(600),
            Self::I15m => Some(900),
            Self::I30m => Some(1_800),
            Self::I90m => Some(5_400),
            Self::I1h => Some(3_600),
            Self::I2h => Some(7_200),
            Self::I3h => Some(10_800),
            Self::I4h => Some(14_400),
            Self::I6h => Some(21_600),
            Self::I8h => Some(28_800),
            Self::I12h => Some(43_200),
            _ => None,
        }
    }
}

impl Serialize for Interval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str((*self).code())
    }
}

impl<'de> Deserialize<'de> for Interval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_code(&value).ok_or_else(|| {
            serde::de::Error::unknown_variant(
                &value,
                &[
                    "1s", "2s", "3s", "5s", "6s", "10s", "15s", "30s", "90s", "1m", "2m", "3m",
                    "5m", "6m", "10m", "15m", "30m", "90m", "1h", "2h", "3h", "4h", "6h", "8h",
                    "12h", "1d", "5d", "1wk", "1mo", "3mo", "6mo", "1y", "2y", "5y", "10y",
                ],
            )
        })
    }
}
