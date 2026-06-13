use std::collections::HashMap;

use dioxus::prelude::*;
use dtos::{asset::get_asset_response::GetAssetResponse, position::Position};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ─── Flash state ──────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Default)]
enum FlashDir {
    #[default]
    None,
    Up,
    Down,
}

impl FlashDir {
    fn class(&self) -> &'static str {
        match self {
            FlashDir::Up => "price-flash-up",
            FlashDir::Down => "price-flash-down",
            FlashDir::None => "",
        }
    }
}

#[derive(Clone, Default)]
struct FlashEntry {
    dir: FlashDir,
    /// Incremented each time the price changes so the `key` prop changes,
    /// forcing Dioxus to remount the <span> and retrigger the CSS animation.
    gen: u32,
}

// Catppuccin-compatible flash colors:
//   green = ctp-green  (#a6e3a1)  with 25 % opacity background
//   red   = ctp-red    (#f38ba8)  with 25 % opacity background
const FLASH_CSS: &str = r#"
@keyframes price-flash-up {
  0%   { background-color: transparent; color: inherit; }
  20%  { background-color: rgba(166, 227, 161, 0.28); color: #a6e3a1; }
  100% { background-color: transparent; color: inherit; }
}
@keyframes price-flash-down {
  0%   { background-color: transparent; color: inherit; }
  20%  { background-color: rgba(243, 139, 168, 0.28); color: #f38ba8; }
  100% { background-color: transparent; color: inherit; }
}
.price-flash-up {
  border-radius: 3px;
  animation: price-flash-up 1.4s ease-out forwards;
}
.price-flash-down {
  border-radius: 3px;
  animation: price-flash-down 1.4s ease-out forwards;
}
"#;

// ─── Component ────────────────────────────────────────────────────────────────

#[component]
pub fn HoldingsTable(positions: Vec<Position>, loaded: bool) -> Element {
    let mut prev_prices: Signal<HashMap<String, Decimal>> = use_signal(HashMap::new);
    let mut flash_map: Signal<HashMap<String, FlashEntry>> = use_signal(HashMap::new);
    let positions_memo = use_memo(move || positions.clone());

    use_effect(move || {
        let positions = positions_memo();
        let prev = prev_prices.peek().clone();
        let mut new_flash = flash_map.peek().clone();
        let mut new_prev = prev.clone();

        for pos in &positions {
            if pos.current_price > Decimal::ZERO {
                let ticker = pos.ticker.to_string();

                if let Some(&old_price) = prev.get(&ticker) {
                    if pos.current_price != old_price {
                        let entry = new_flash.entry(ticker.clone()).or_default();
                        entry.dir = if pos.current_price > old_price {
                            FlashDir::Up
                        } else {
                            FlashDir::Down
                        };
                        entry.gen = entry.gen.wrapping_add(1);
                    }
                }

                new_prev.insert(ticker, pos.current_price);
            }
        }

        flash_map.set(new_flash);
        prev_prices.set(new_prev);
    });

    let flashes = flash_map.read();

    rsx! {
         style { {FLASH_CSS} }
        table { class: "w-full text-xs mt-4",
            thead {
                tr { class: "text-ctp-overlay0 border-b border-ctp-surface1",
                    th { class: "py-2 pr-6 text-left  font-semibold uppercase tracking-wider",
                        "Ticker"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Shares"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Avg Cost"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Total Cost"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Value"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "Market Price"
                    }
                    th { class: "py-2 pr-6 text-right font-semibold uppercase tracking-wider",
                        "P&L"
                    }
                    th { class: "py-2 text-right      font-semibold uppercase tracking-wider",
                        "Day"
                    }
                }
            }
            tbody {
                for pos in positions_memo.iter() {
                    {
                        let ticker_str = pos.ticker.to_string();
                        let flash = flashes.get(&ticker_str).cloned().unwrap_or_default();
                        let cls   = flash.dir.class();
                        let gen   = flash.gen;
                        let has_price = pos.current_price > Decimal::ZERO;

                        rsx! {
                            tr {
                                key: "{ticker_str}",
                                class: "border-b border-ctp-surface1 hover:bg-ctp-surface0 transition-colors",

                                td { class: "py-3 pr-6",
                                    span { class: "font-bold text-ctp-blue tracking-wide",
                                        "{pos.ticker}"
                                    }
                                }
                                td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0",
                                    "{pos.shares:.4}"
                                }
                                td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0",
                                    "{fmt_usd(pos.avg_cost, 2)}"
                                }
                                td { class: "py-3 pr-6 text-right tabular-nums text-ctp-subtext0",
                                    "{fmt_usd(pos.cost_basis(), 2)}"
                                }

                                // ── Market Price ──────────────────────────────
                                td { class: "py-3 pr-6 text-right tabular-nums",
                                    if has_price {
                                        span {
                                            // key change → remount → animation restarts
                                            key: "mp-{gen}",
                                            class: "inline-block px-1 {cls}",
                                            "{fmt_usd(pos.current_price, 2)}"
                                        }
                                    } else {
                                        "—"
                                    }
                                }

                                // ── Market Value ──────────────────────────────
                                td { class: "py-3 pr-6 text-right tabular-nums font-medium",
                                    if has_price {
                                        span {
                                            key: "mv-{gen}",
                                            class: "inline-block px-1 {cls}",
                                            "{fmt_usd(pos.market_value(), 2)}"
                                        }
                                    } else {
                                        "—"
                                    }
                                }

                                // ── P&L ──────────────────────────────────────
                                td {
                                    class: if pos.unrealized_pnl() >= Decimal::ZERO {
                                        "py-3 pr-6 text-right tabular-nums text-ctp-green"
                                    } else {
                                        "py-3 pr-6 text-right tabular-nums text-ctp-red"
                                    },
                                    if has_price {
                                        span {
                                            key: "pnl-{gen}",
                                            class: "inline-block px-1 {cls}",
                                            "{fmt_signed(pos.unrealized_pnl(), 2)} ({pos.unrealized_pnl_pct():+.1}%)"
                                        }
                                    } else {
                                        "—"
                                    }
                                }

                                // ── Day change ────────────────────────────────
                                td {
                                    class: if pos.daily_change_pct >= Decimal::ZERO {
                                        "py-3 text-right tabular-nums text-ctp-green"
                                    } else {
                                        "py-3 text-right tabular-nums text-ctp-red"
                                    },
                                    if has_price {
                                        span {
                                            key: "day-{gen}",
                                            class: "inline-block px-1 {cls}",
                                            if pos.daily_change_pct >= Decimal::ZERO {
                                                "▲ {pos.daily_change_pct:.2}%"
                                            } else {
                                                "▼ {pos.daily_change_pct.abs():.2}%"
                                            }
                                        }
                                    } else {
                                        "—"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn portfolio_stats(
    assets: &[GetAssetResponse],
    prices: &HashMap<String, Decimal>,
    changes: &HashMap<String, Decimal>,
) -> (usize, Decimal, Decimal, Decimal, Decimal) {
    let pos_count = assets.len();

    let total_cost: Decimal = assets
        .iter()
        .map(|a| a.quantity.value() * a.cost.amount())
        .sum();

    let total_value: Decimal = assets
        .iter()
        .map(|a| {
            let price = prices
                .get(a.ticker_symbol.as_str())
                .copied()
                .unwrap_or(Decimal::ZERO);
            a.quantity.value() * price
        })
        .sum();

    let day_change: Decimal = assets
        .iter()
        .map(|a| {
            let price = prices
                .get(a.ticker_symbol.as_str())
                .copied()
                .unwrap_or(Decimal::ZERO);
            let chg_pct = changes
                .get(a.ticker_symbol.as_str())
                .copied()
                .unwrap_or(Decimal::ZERO);
            a.quantity.value() * price * chg_pct / dec!(100)
        })
        .sum();

    let total_pnl = total_value - total_cost;
    (pos_count, total_cost, total_value, day_change, total_pnl)
}

/// "$1,234.56"  (ไม่มี sign)
fn fmt_usd(value: Decimal, decimals: u32) -> String {
    let neg = value.is_sign_negative();
    let abs = value.abs().round_dp(decimals);
    let whole = abs.trunc();
    let frac = ((abs - whole) * Decimal::from(10u64.pow(decimals)))
        .round()
        .to_string();

    let whole_str = whole.to_string();
    let mut out = String::new();
    for (i, c) in whole_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let whole_fmt: String = out.chars().rev().collect();

    let sign = if neg { "-" } else { "" };
    if decimals == 0 {
        format!("{sign}${whole_fmt}")
    } else {
        format!(
            "{sign}${whole_fmt}.{frac:0>width$}",
            width = decimals as usize
        )
    }
}

/// "+$1,234.56" / "-$1,234.56"
fn fmt_signed(value: Decimal, decimals: u32) -> String {
    let sign = if value >= Decimal::ZERO { "+" } else { "" };
    format!("{sign}{}", fmt_usd(value, decimals))
}
