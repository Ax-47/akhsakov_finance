use std::fmt::Display;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
/// Logical lookback range presets.
pub enum Range {
    /// 1 minute
    I1m,
    /// 2 minutes
    I2m,
    /// 5 minutes
    I5m,
    /// 10 minutes
    I10m,
    /// 15 minutes
    I15m,
    /// 30 minutes
    I30m,
    /// 1 hour
    I1h,
    /// 4 hours
    I4h,
    /// 6 hours
    I6h,
    /// 8 hours
    I8h,
    /// 12 hours
    I12h,
    /// 1 day
    #[default]
    D1,
    /// 5 days
    D5,
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
    /// Year to date
    Ytd,
    /// Maximum available
    Max,
}

impl Range {
    /// Returns the stable wire-format code for this range.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::I1m => "1m",
            Self::I2m => "2m",
            Self::I5m => "5m",
            Self::I10m => "10m",
            Self::I15m => "15m",
            Self::I30m => "30m",
            Self::I1h => "1h",
            Self::I4h => "4h",
            Self::I6h => "6h",
            Self::I8h => "8h",
            Self::I12h => "12h",
            Self::D1 => "1d",
            Self::D5 => "5d",
            Self::M1 => "1mo",
            Self::M3 => "3mo",
            Self::M6 => "6mo",
            Self::Y1 => "1y",
            Self::Y2 => "2y",
            Self::Y5 => "5y",
            Self::Y10 => "10y",
            Self::Ytd => "ytd",
            Self::Max => "max",
        }
    }

    fn from_code(value: &str) -> Option<Self> {
        match value {
            "1m" => Some(Self::I1m),
            "2m" => Some(Self::I2m),
            "5m" => Some(Self::I5m),
            "10m" => Some(Self::I10m),
            "15m" => Some(Self::I15m),
            "30m" => Some(Self::I30m),
            "1h" => Some(Self::I1h),
            "4h" => Some(Self::I4h),
            "6h" => Some(Self::I6h),
            "8h" => Some(Self::I8h),
            "12h" => Some(Self::I12h),
            "1d" => Some(Self::D1),
            "5d" => Some(Self::D5),
            "1mo" => Some(Self::M1),
            "3mo" => Some(Self::M3),
            "6mo" => Some(Self::M6),
            "1y" => Some(Self::Y1),
            "2y" => Some(Self::Y2),
            "5y" => Some(Self::Y5),
            "10y" => Some(Self::Y10),
            "ytd" => Some(Self::Ytd),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).code())
    }
}

impl Serialize for Range {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str((*self).code())
    }
}

impl<'de> Deserialize<'de> for Range {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_code(&value).ok_or_else(|| {
            serde::de::Error::unknown_variant(
                &value,
                &[
                    "1m", "2m", "5m", "10m", "15m", "30m", "1h", "4h", "6h", "8h", "12h", "1d",
                    "5d", "1mo", "3mo", "6mo", "1y", "2y", "5y", "10y", "ytd", "max",
                ],
            )
        })
    }
}
