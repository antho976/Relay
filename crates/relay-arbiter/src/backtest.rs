//! The backtester, honest by construction (docs/ARBITER.md, Honest backtests):
//!
//! - A signal on bar *t*'s close fills at bar *t+1*'s open; nothing reads a bar it has not
//!   reached.
//! - Market orders pay the taker fee and slippage; limit orders pay the maker fee and fill only
//!   when the next bar trades below them.
//! - A stop and a take-profit touched in the same bar count as the stop.
//! - The range is split: the first part is where the rule was tuned, the last where it is judged.
//!   Both are reported, beside buying and holding over the same bars.
//!
//! Returns are on the most the rule ever had in the market at once: a rule that buys 50 at a time
//! and makes 5 has made 10%, however long the range.

use crate::model::{Candle, Entry, Fees, Pricing, Rule};
use crate::rules::{self, Evaluator, ExitReason};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub struct Config {
    pub fees: Fees,
    /// Percent.
    pub slippage_pct: f64,
    /// The fraction of the range the rule was tuned on; the rest is judged. 2/3 by default.
    pub tuned_fraction: f64,
    /// The first bar of the range proper; the bars before it only warm the indicators.
    pub first: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterBacktestTrade")]
pub struct Trade {
    /// Unix seconds.
    #[schemars(extend("x-unit" = "unix"))]
    pub buy_at: i64,
    pub buy_price: f64,
    #[schemars(extend("x-unit" = "unix"))]
    pub sell_at: Option<i64>,
    pub sell_price: Option<f64>,
    /// Why it sold; absent while still held at the end.
    pub reason: Option<String>,
    /// After fees, in the quote currency; open trades are marked at the last close.
    pub pnl: f64,
    pub fees: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterBacktestStats")]
pub struct Stats {
    /// Unix seconds, the first and last bar.
    #[schemars(extend("x-unit" = "unix"))]
    pub start: i64,
    #[schemars(extend("x-unit" = "unix"))]
    pub end: i64,
    /// Percent, after fees, on the most in the market at once.
    pub return_pct: f64,
    /// The largest fall from a high, percent.
    pub max_drawdown_pct: f64,
    pub trades: u32,
    /// Percent of closed trades that made money; absent with none closed.
    pub won_pct: Option<f64>,
    pub time_in_market_pct: f64,
    pub fees: f64,
    /// Buying at the first bar's open and holding to the last close, percent, after one taker
    /// fee each way.
    pub buy_hold_pct: f64,
    /// The most in the market at once, quote currency.
    pub deployed: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterBacktest")]
pub struct Report {
    pub tuned: Stats,
    pub judged: Stats,
    pub whole: Stats,
    pub trades: Vec<Trade>,
    /// Profit after fees over time, `[unix seconds, quote currency]`, at most 400 points. For the
    /// chart; a model is not given it.
    #[schemars(extend("x-unit" = "omit"))]
    pub equity: Vec<(i64, f64)>,
    /// What a round trip costs in fees and slippage, percent: the move a trade needs to break
    /// even.
    pub break_even_pct: f64,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct Position {
    base: f64,
    cost: f64,
    peak: f64,
    bought_bar: usize,
    buy_at: i64,
    fees: f64,
}

struct Sim<'a> {
    rule: &'a Rule,
    taker: f64,
    maker: f64,
    slip: f64,
    realized: f64,
    pos: Option<Position>,
    trades: Vec<Trade>,
    deployed: f64,
}

impl Sim<'_> {
    fn buy(&mut self, at: i64, bar: usize, price: f64, market: bool) {
        let spend = self.rule.buy.to_f64().unwrap_or(0.0);
        let fill = if market { price * (1.0 + self.slip) } else { price };
        let fee = spend * if market { self.taker } else { self.maker };
        let base = (spend - fee) / fill;
        let p = self.pos.get_or_insert_with(|| Position { bought_bar: bar, buy_at: at, peak: fill, ..Default::default() });
        p.base += base;
        p.cost += spend;
        p.fees += fee;
        p.peak = p.peak.max(fill);
        self.deployed = self.deployed.max(p.cost);
    }

    fn sell(&mut self, at: i64, price: f64, reason: ExitReason) {
        let Some(p) = self.pos.take() else { return };
        let fill = price * (1.0 - self.slip);
        let gross = p.base * fill;
        let fee = gross * self.taker;
        let pnl = gross - fee - p.cost;
        self.realized += pnl;
        self.trades.push(Trade {
            buy_at: p.buy_at,
            buy_price: p.cost / p.base.max(f64::MIN_POSITIVE),
            sell_at: Some(at),
            sell_price: Some(fill),
            reason: Some(reason.as_str().into()),
            pnl,
            fees: p.fees + fee,
        });
    }

    fn equity(&self, price: f64) -> f64 {
        self.realized + self.pos.as_ref().map_or(0.0, |p| p.base * price * (1.0 - self.taker) - p.cost)
    }
}

/// Runs `rule` over `candles` (oldest first, `cfg.first` bars of warm-up before the range).
pub fn run(rule: &Rule, candles: &[Candle], cfg: &Config) -> Report {
    let eval = Evaluator::for_rule(candles, rule);
    let mut sim = Sim {
        rule,
        taker: cfg.fees.taker.to_f64().unwrap_or(0.006),
        maker: cfg.fees.maker.to_f64().unwrap_or(0.004),
        slip: cfg.slippage_pct / 100.0,
        realized: 0.0,
        pos: None,
        trades: Vec::new(),
        deployed: 0.0,
    };
    let market = rule.pricing == Pricing::Market;
    let first = cfg.first.min(candles.len());
    let width = if candles.len() >= 2 { candles[1].start - candles[0].start } else { 3600 };
    let mut curve: Vec<(i64, f64)> = Vec::with_capacity(candles.len());
    let mut held = vec![false; candles.len()];
    // An order decided on the previous bar's close, to fill at this bar's open.
    let mut pending_buy = false;
    let mut pending_sell: Option<ExitReason> = None;

    for i in first..candles.len() {
        let c = candles[i];
        // 1. What the previous close decided fills at this open.
        if let Some(reason) = pending_sell.take() {
            sim.sell(c.start, c.open, reason);
        }
        if pending_buy {
            pending_buy = false;
            if market {
                sim.buy(c.start, i, c.open, true);
            } else if c.low < c.open {
                // A post-only bid at the open fills only if the price trades below it.
                sim.buy(c.start, i, c.open, false);
            }
        }
        // 2. A schedule due inside this bar buys at its open.
        if let Entry::Schedule { .. } = rule.entry {
            if rules::schedule_due(&rule.entry, c.start - 1, c.start + width - 1) {
                sim.buy(c.start, i, c.open, market);
            }
        }
        // 3. Price exits inside the bar: the stop first, then the trailing stop, then the target.
        if let Some(p) = sim.pos.as_ref() {
            let cost = p.cost / p.base.max(f64::MIN_POSITIVE);
            let x = &rule.exit;
            let sl = x.stop_loss_pct.and_then(|d| d.to_f64()).map(|s| cost * (1.0 - s / 100.0));
            let tr = x.trailing_pct.and_then(|d| d.to_f64()).map(|t| p.peak * (1.0 - t / 100.0));
            let tp = x.take_profit_pct.and_then(|d| d.to_f64()).map(|t| cost * (1.0 + t / 100.0));
            let mut hit: Option<(f64, ExitReason)> = None;
            if let Some(level) = sl.filter(|l| c.low <= *l) {
                hit = Some((level.min(c.open), ExitReason::StopLoss));
            } else if let Some(level) = tr.filter(|l| c.low <= *l) {
                hit = Some((level.min(c.open), ExitReason::Trailing));
            } else if let Some(level) = tp.filter(|l| c.high >= *l) {
                hit = Some((level.max(c.open), ExitReason::TakeProfit));
            }
            if let Some((price, reason)) = hit {
                sim.sell(c.start, price, reason);
            }
        }
        if let Some(p) = sim.pos.as_mut() {
            p.peak = p.peak.max(c.high);
        }
        // 4. The close decides the next bar's orders.
        if let Some(p) = sim.pos.as_ref() {
            let bars = (i - p.bought_bar) as u32;
            pending_sell = rules::bar_exit(&rule.exit, &eval, i, bars);
        } else if let Entry::Signal { when } = &rule.entry {
            pending_buy = eval.holds(when, i);
        }
        held[i] = sim.pos.is_some();
        curve.push((c.start, sim.equity(c.close)));
    }

    // Still held at the end: marked at the last close, reported as open.
    if let (Some(p), Some(last)) = (sim.pos.as_ref(), candles.last()) {
        sim.trades.push(Trade {
            buy_at: p.buy_at,
            buy_price: p.cost / p.base.max(f64::MIN_POSITIVE),
            sell_at: None,
            sell_price: None,
            reason: None,
            pnl: p.base * last.close * (1.0 - sim.taker) - p.cost,
            fees: p.fees,
        });
    }

    let range = &candles[first..];
    let deployed = sim.deployed.max(rule.buy.to_f64().unwrap_or(1.0));
    let n = range.len();
    let cut = ((n as f64) * cfg.tuned_fraction.clamp(0.0, 1.0)).round() as usize;
    let taker = sim.taker;
    let stats = |from: usize, to: usize| -> Stats {
        if from >= to {
            return Stats::default();
        }
        let bars = &range[from..to];
        let eq = &curve[from..to];
        let before = if from == 0 { 0.0 } else { curve[from - 1].1 };
        let (start, end) = (bars[0].start, bars[bars.len() - 1].start);
        let closed: Vec<&Trade> = sim.trades.iter().filter(|t| t.buy_at >= start && t.buy_at <= end).collect();
        let done: Vec<&&Trade> = closed.iter().filter(|t| t.sell_at.is_some()).collect();
        let mut peak = deployed + before;
        let mut dd: f64 = 0.0;
        for &(_, e) in eq {
            let v = deployed + e;
            peak = peak.max(v);
            if peak > 0.0 {
                dd = dd.max((peak - v) / peak * 100.0);
            }
        }
        let in_market = held[first + from..first + to].iter().filter(|h| **h).count();
        let bh = bars[bars.len() - 1].close * (1.0 - taker) / (bars[0].open * (1.0 + taker)) - 1.0;
        Stats {
            start,
            end,
            return_pct: (eq[eq.len() - 1].1 - before) / deployed * 100.0,
            max_drawdown_pct: dd,
            trades: closed.len() as u32,
            won_pct: (!done.is_empty()).then(|| done.iter().filter(|t| t.pnl > 0.0).count() as f64 / done.len() as f64 * 100.0),
            time_in_market_pct: in_market as f64 / bars.len() as f64 * 100.0,
            fees: closed.iter().map(|t| t.fees).sum(),
            buy_hold_pct: bh * 100.0,
            deployed,
        }
    };
    let tuned = stats(0, cut);
    let judged = stats(cut, n);
    let whole = stats(0, n);

    let round_trip = if market { 2.0 * sim.taker + 2.0 * sim.slip } else { sim.maker + sim.taker + sim.slip };
    let break_even_pct = round_trip * 100.0;
    let mut warnings = Vec::new();
    if n == 0 {
        warnings.push("There were no bars in that range.".into());
    }
    if judged.trades < 10 && matches!(rule.entry, Entry::Signal { .. }) {
        warnings.push(format!("Only {} trades in the judged part: too few to tell skill from luck.", judged.trades));
    }
    if tuned.return_pct > 0.0 && judged.return_pct < tuned.return_pct / 2.0 {
        warnings.push("It did much worse on the judged part than where it was tuned: a sign it fits the past more than it predicts.".into());
    }
    if judged.return_pct < judged.buy_hold_pct && judged.max_drawdown_pct >= judged.buy_hold_pct.abs() {
        warnings.push("Holding would have done better over the judged part, with no more risk.".into());
    }

    let step = curve.len().div_ceil(400).max(1);
    let mut equity: Vec<(i64, f64)> = curve.iter().step_by(step).copied().collect();
    if let Some(last) = curve.last() {
        if equity.last() != Some(last) {
            equity.push(*last);
        }
    }
    Report { tuned, judged, whole, trades: sim.trades, equity, break_even_pct, warnings }
}

/// What one round trip at `fees` costs, percent, for a rule's pricing.
pub fn break_even_pct(fees: &Fees, pricing: Pricing, slippage_pct: Decimal) -> f64 {
    let t = fees.taker.to_f64().unwrap_or(0.006);
    let m = fees.maker.to_f64().unwrap_or(0.004);
    let s = slippage_pct.to_f64().unwrap_or(0.0) / 100.0;
    100.0 * match pricing {
        Pricing::Market => 2.0 * t + 2.0 * s,
        Pricing::Limit => m + t + s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Compare, Condition, Every, Exit, Operand};

    fn bars(prices: &[(f64, f64, f64, f64)]) -> Vec<Candle> {
        prices.iter().enumerate().map(|(i, &(o, h, l, c))| Candle { start: i as i64 * 3600, open: o, high: h, low: l, close: c, volume: 1.0 }).collect()
    }

    fn cfg() -> Config {
        Config { fees: Fees { maker: Decimal::ZERO, taker: Decimal::ZERO, tier: None }, slippage_pct: 0.0, tuned_fraction: 0.5, first: 0 }
    }

    fn below(v: f64) -> Rule {
        Rule {
            entry: Entry::Signal { when: Condition::Compare { left: Operand::Price, op: Compare::Below, right: Operand::Number { value: v } } },
            buy: Decimal::new(100, 0),
            pricing: Pricing::Market,
            exit: Exit { take_profit_pct: Some(Decimal::new(10, 0)), stop_loss_pct: Some(Decimal::new(5, 0)), ..Default::default() },
        }
    }

    #[test]
    fn a_signal_fills_at_the_next_open_never_its_own_close() {
        // Bar 0 closes at 90 (< 95): the buy fills at bar 1's open, 100, not at 90.
        let c = bars(&[(100.0, 100.0, 90.0, 90.0), (100.0, 100.0, 99.0, 100.0), (100.0, 100.0, 99.0, 100.0)]);
        let r = run(&below(95.0), &c, &cfg());
        assert_eq!(r.trades.len(), 1);
        assert_eq!(r.trades[0].buy_at, 3600);
        assert!((r.trades[0].buy_price - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_bar_touching_both_stop_and_target_counts_as_the_stop() {
        let c = bars(&[(90.0, 90.0, 90.0, 90.0), (100.0, 100.0, 100.0, 100.0), (100.0, 120.0, 80.0, 100.0)]);
        let r = run(&below(95.0), &c, &cfg());
        assert_eq!(r.trades[0].reason.as_deref(), Some("stop loss"));
        assert!((r.trades[0].pnl + 5.0).abs() < 1e-9, "{}", r.trades[0].pnl);
    }

    #[test]
    fn fees_and_slippage_cost_what_they_say() {
        let c = bars(&[(90.0, 90.0, 90.0, 90.0), (100.0, 100.0, 100.0, 100.0), (100.0, 111.0, 100.0, 110.0)]);
        let mut k = cfg();
        k.fees.taker = Decimal::new(1, 2); // 1%
        let r = run(&below(95.0), &c, &k);
        let t = &r.trades[0];
        // Spend 100, fee 1, 0.99 base at 100; sells at the 110 target: 108.9 gross, fee 1.089.
        assert!((t.pnl - (108.9 - 1.089 - 100.0)).abs() < 1e-6, "{}", t.pnl);
        assert!((r.break_even_pct - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_weekly_buy_buys_once_a_week() {
        // 21 days of daily bars from Monday 2026-10-05: three Mondays.
        let monday = jiff::civil::date(2026, 10, 5).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp().as_second();
        let c: Vec<Candle> = (0..21).map(|d| Candle { start: monday + d * 86400, open: 100.0, high: 100.0, low: 100.0, close: 100.0, volume: 1.0 }).collect();
        let rule = Rule { entry: Entry::Schedule { every: Every::Week, hour: 9, weekday: Some(1), day: None }, buy: Decimal::new(25, 0), pricing: Pricing::Market, exit: Exit::default() };
        let r = run(&rule, &c, &cfg());
        // One position, three lots: 75 deployed.
        assert_eq!(r.whole.deployed, 75.0);
        assert_eq!(r.trades.len(), 1);
        assert!(r.trades[0].sell_at.is_none());
    }

    #[test]
    fn the_split_reports_both_parts_and_buy_and_hold() {
        let c: Vec<Candle> = (0..20).map(|i| { let p = 100.0 + i as f64; Candle { start: i * 3600, open: p, high: p, low: p, close: p, volume: 1.0 } }).collect();
        let r = run(&below(0.0), &c, &cfg());
        assert_eq!(r.tuned.trades + r.judged.trades, 0);
        assert!(r.whole.buy_hold_pct > 18.0);
        assert_eq!(r.tuned.end + 3600, r.judged.start);
    }
}
