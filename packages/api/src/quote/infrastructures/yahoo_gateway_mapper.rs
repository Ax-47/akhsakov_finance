use types::{candle::Candle, interval::Interval, range::Range};
use yfinance_rs::{
    Candle as YCandle, Interval as YInterval, Range as YRange, StreamHandle, StreamMethod, Ticker,
};

pub fn to_yinterval(value: Interval) -> YInterval {
    match value {
        Interval::I1s => YInterval::I1s,
        Interval::I2s => YInterval::I2s,
        Interval::I3s => YInterval::I3s,
        Interval::I5s => YInterval::I5s,
        Interval::I6s => YInterval::I6s,
        Interval::I10s => YInterval::I10s,
        Interval::I15s => YInterval::I15s,
        Interval::I30s => YInterval::I30s,
        Interval::I90s => YInterval::I90s,
        Interval::I1m => YInterval::I1m,
        Interval::I2m => YInterval::I2m,
        Interval::I3m => YInterval::I3m,
        Interval::I5m => YInterval::I5m,
        Interval::I6m => YInterval::I6m,
        Interval::I10m => YInterval::I10m,
        Interval::I15m => YInterval::I15m,
        Interval::I30m => YInterval::I30m,
        Interval::I90m => YInterval::I90m,
        Interval::I1h => YInterval::I1h,
        Interval::I2h => YInterval::I2h,
        Interval::I3h => YInterval::I3h,
        Interval::I4h => YInterval::I4h,
        Interval::I6h => YInterval::I6h,
        Interval::I8h => YInterval::I8h,
        Interval::I12h => YInterval::I12h,
        Interval::D1 => YInterval::D1,
        Interval::D5 => YInterval::D5,
        Interval::W1 => YInterval::W1,
        Interval::M1 => YInterval::M1,
        Interval::M3 => YInterval::M3,
        Interval::M6 => YInterval::M6,
        Interval::Y1 => YInterval::Y1,
        Interval::Y2 => YInterval::Y2,
        Interval::Y5 => YInterval::Y5,
        Interval::Y10 => YInterval::Y10,
        _ => YInterval::I1m,
    }
}

pub fn to_yrange(value: Range) -> YRange {
    match value {
        Range::I1m => YRange::I1m,
        Range::I2m => YRange::I2m,
        Range::I5m => YRange::I5m,
        Range::I10m => YRange::I10m,
        Range::I15m => YRange::I15m,
        Range::I30m => YRange::I30m,
        Range::I1h => YRange::I1h,
        Range::I4h => YRange::I4h,
        Range::I6h => YRange::I6h,
        Range::I8h => YRange::I8h,
        Range::I12h => YRange::I12h,
        Range::D1 => YRange::D1,
        Range::D5 => YRange::D5,
        Range::M1 => YRange::M1,
        Range::M3 => YRange::M3,
        Range::M6 => YRange::M6,
        Range::Y1 => YRange::Y1,
        Range::Y2 => YRange::Y2,
        Range::Y5 => YRange::Y5,
        Range::Y10 => YRange::Y10,
        Range::Ytd => YRange::Ytd,
        Range::Max => YRange::Max,
        _ => YRange::D1,
    }
}

pub fn to_candle(c: YCandle) -> Candle {
    Candle {
        ts: c.ts,
        open: c.ohlc.open.into_inner(),
        high: c.ohlc.high.into_inner(),
        low: c.ohlc.low.into_inner(),
        close: c.ohlc.close.into_inner(),
        volume: None,
    }
}
