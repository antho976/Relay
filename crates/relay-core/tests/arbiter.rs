//! Arbiter across the bus, on a fake exchange: strategies on paper and live, the runner's
//! decisions, the risk gate, proposals and approvals, the kill switch, and the key check.

mod common;

use common::{call, code, engine, ok, refused, wait_until};
use relay_arbiter::exchange::{Account, ExchangeError, Market, Result};
use relay_arbiter::model::*;
use relay_arbiter::Decimal;
use relay_core::arbiter::{pass, set_fake_exchange, set_fake_key};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const KEY: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEICr3hFJQLruuvjzXzQvrZyWUlBfa0SdE9HtDCMhP7NYqoAoGCCqGSM49
AwEHoUQDQgAEIgIeCM0SMcTdKeIUVa5s6GrENsIc3sFPBcefipt5Ciw6PtUG6nMV
TsGghZLrhr8kSV5nZX8i1b+Q+3+QzuY0wQ==
-----END EC PRIVATE KEY-----";

/// An exchange with one product whose price the test sets.
#[derive(Default)]
struct Fake {
    price: Mutex<f64>,
    can_transfer: Mutex<bool>,
    placed: Mutex<Vec<OrderRequest>>,
    /// Live orders rest unfilled instead of filling at once.
    rest: Mutex<bool>,
    /// The exchange refuses every sell.
    refuse_sells: Mutex<bool>,
    /// Resting orders cancelled, by exchange id: they read back as cancelled.
    cancelled: Mutex<Vec<String>>,
    /// Create takes the order, then its answer is lost to this error (a timeout, a body that
    /// cannot be read).
    lost: Mutex<Option<ExchangeError>>,
    /// Create never reaches the exchange: it times out and nothing is placed.
    drop_orders: Mutex<bool>,
    /// Looking an order up by its client_order_id fails.
    lookup_fails: Mutex<bool>,
    /// The next this many reads of an order's fills answer none yet, though the order reads as
    /// filled: fills lag the order's status.
    fills_lag: Mutex<u32>,
    /// While shut, create waits at the door before it reaches the exchange: a request in flight.
    door: (Mutex<bool>, Condvar),
    at_door: AtomicBool,
}

impl Fake {
    fn shut_door(&self) {
        *self.door.0.lock().unwrap() = true;
    }
    fn open_door(&self) {
        *self.door.0.lock().unwrap() = false;
        self.door.1.notify_all();
    }
    fn new(price: f64) -> Arc<Fake> {
        let f = Fake::default();
        *f.price.lock().unwrap() = price;
        Arc::new(f)
    }
    fn set(&self, price: f64) {
        *self.price.lock().unwrap() = price;
    }
    fn now() -> i64 {
        jiff::Timestamp::now().as_second()
    }
}

fn eth() -> Product {
    Product {
        id: "ETH-CAD".into(),
        base: "ETH".into(),
        quote: "CAD".into(),
        base_increment: Decimal::new(1, 8),
        quote_increment: Decimal::new(1, 2),
        price_increment: Decimal::new(1, 2),
        base_min_size: Decimal::new(1, 8),
        quote_min_size: Decimal::ONE,
        price: Some(100.0),
        change_24h: Some(1.5),
        tradable: true,
        limit_only: false,
    }
}

/// A second product, for a strategy that changes what it trades.
fn sol() -> Product {
    Product { id: "SOL-CAD".into(), base: "SOL".into(), ..eth() }
}

impl Market for Fake {
    fn products(&self) -> Result<Vec<Product>> {
        Ok(vec![eth(), sol()])
    }
    fn product(&self, id: &str) -> Result<Product> {
        if id == "ETH-CAD" { Ok(eth()) } else { Err(ExchangeError::Refused(format!("no {id}"))) }
    }
    fn candles(&self, _: &str, g: Granularity, start: i64, end: i64) -> Result<Vec<Candle>> {
        let p = *self.price.lock().unwrap();
        let s = g.seconds();
        let first = start.div_euclid(s) * s;
        Ok((0..).map(|i| first + i * s).take_while(|t| *t < end && *t <= Fake::now()).filter(|t| *t >= start)
            .map(|t| Candle { start: t, open: p, high: p, low: p, close: p, volume: 1.0 }).collect())
    }
    fn quote(&self, _: &str) -> Result<Quote> {
        let p = *self.price.lock().unwrap();
        Ok(Quote { bid: p - 0.05, ask: p + 0.05, at: Fake::now() })
    }
}

impl Account for Fake {
    fn permissions(&self) -> Result<Permissions> {
        Ok(Permissions { can_view: true, can_trade: true, can_transfer: *self.can_transfer.lock().unwrap(), portfolio_uuid: Some("p1".into()), portfolio_type: Some("DEFAULT".into()) })
    }
    fn balances(&self) -> Result<Vec<Balance>> {
        Ok(vec![Balance { currency: "CAD".into(), available: Decimal::new(500, 0), hold: Decimal::ZERO, value: None }])
    }
    fn fees(&self) -> Result<Fees> {
        Ok(Fees { maker: Decimal::new(4, 3), taker: Decimal::new(6, 3), tier: Some("Intro".into()) })
    }
    fn preview(&self, _: &OrderRequest) -> Result<Preview> {
        Ok(Preview { preview_id: Some("pv".into()), commission: None, quote_size: None, base_size: None, errors: vec![], warnings: vec![] })
    }
    fn place(&self, o: &OrderRequest, _: Option<&str>) -> Result<ExchangeOrder> {
        let mut shut = self.door.0.lock().unwrap();
        while *shut {
            self.at_door.store(true, Ordering::SeqCst);
            shut = self.door.1.wait(shut).unwrap();
        }
        drop(shut);
        if o.side == Side::Sell && *self.refuse_sells.lock().unwrap() {
            return Err(ExchangeError::Refused("INSUFFICIENT_FUND".into()));
        }
        if *self.drop_orders.lock().unwrap() {
            return Err(ExchangeError::Unreachable("the request timed out".into()));
        }
        // Coinbase refuses a client_order_id it already has.
        if self.placed.lock().unwrap().iter().any(|p| p.client_order_id == o.client_order_id) {
            return Err(ExchangeError::Duplicate(o.client_order_id.clone()));
        }
        self.placed.lock().unwrap().push(o.clone());
        if let Some(e) = self.lost.lock().unwrap().clone() {
            return Err(e);
        }
        Ok(ExchangeOrder { order_id: format!("x-{}", o.client_order_id), client_order_id: o.client_order_id.clone(), status: OrderStatus::Pending, filled_size: Decimal::ZERO, average_filled_price: Decimal::ZERO, total_fees: Decimal::ZERO, reason: None })
    }
    fn order(&self, id: &str) -> Result<ExchangeOrder> {
        let p = Decimal::try_from(*self.price.lock().unwrap()).unwrap();
        if self.cancelled.lock().unwrap().iter().any(|c| c == id) {
            return Ok(ExchangeOrder { order_id: id.into(), client_order_id: id.trim_start_matches("x-").into(), status: OrderStatus::Cancelled, filled_size: Decimal::ZERO, average_filled_price: Decimal::ZERO, total_fees: Decimal::ZERO, reason: None });
        }
        if *self.rest.lock().unwrap() {
            return Ok(ExchangeOrder { order_id: id.into(), client_order_id: id.trim_start_matches("x-").into(), status: OrderStatus::Open, filled_size: Decimal::ZERO, average_filled_price: Decimal::ZERO, total_fees: Decimal::ZERO, reason: None });
        }
        Ok(ExchangeOrder { order_id: id.into(), client_order_id: id.trim_start_matches("x-").into(), status: OrderStatus::Filled, filled_size: Decimal::new(5, 1), average_filled_price: p, total_fees: Decimal::new(3, 1), reason: None })
    }
    fn find_order(&self, _: &str, cid: &str, _: i64) -> Result<Option<ExchangeOrder>> {
        if *self.lookup_fails.lock().unwrap() {
            return Err(ExchangeError::Unreachable("the request timed out".into()));
        }
        if !self.placed.lock().unwrap().iter().any(|p| p.client_order_id == cid) {
            return Ok(None);
        }
        self.order(&format!("x-{cid}")).map(Some)
    }
    fn fills(&self, id: &str) -> Result<Vec<ExchangeFill>> {
        if *self.rest.lock().unwrap() {
            return Ok(vec![]);
        }
        let mut lag = self.fills_lag.lock().unwrap();
        if *lag > 0 {
            *lag -= 1;
            return Ok(vec![]);
        }
        let p = Decimal::try_from(*self.price.lock().unwrap()).unwrap();
        Ok(vec![ExchangeFill { trade_id: format!("t-{id}"), order_id: id.into(), price: p, size: Decimal::new(5, 1), fee: Decimal::new(3, 1), at: Fake::now() }])
    }
    fn cancel(&self, ids: &[String]) -> Result<()> {
        // Only a resting order can be cancelled; one that filled stays filled.
        if *self.rest.lock().unwrap() {
            self.cancelled.lock().unwrap().extend(ids.iter().cloned());
        }
        Ok(())
    }
}

/// A rule that buys on every closed bar while nothing is held, and sells on a 5% fall or 5% rise.
fn draft(name: &str, buy: &str) -> Value {
    json!({
        "name": name, "product": "ETH-CAD", "granularity": "ONE_MINUTE",
        "rule": {
            "entry": {"kind": "signal", "when": {"kind": "compare", "left": {"kind": "price"}, "op": "below", "right": {"kind": "number", "value": 1_000_000}}},
            "buy": buy, "pricing": "market",
            "exit": {"take_profit_pct": "5", "stop_loss_pct": "5"}
        },
        "limits": {"max_order": "50", "max_position": "200", "daily_loss": "40"}
    })
}

fn setup() -> (Arc<relay_core::Engine>, Arc<Fake>) {
    let engine = engine();
    let fake = Fake::new(100.0);
    set_fake_exchange(&engine, fake.clone());
    (engine, fake)
}

/// [`setup`] with the store, and so `arbiter.db`, in a folder on disk, where [`age_order`] can
/// reach the book.
fn setup_on_disk() -> (tempfile::TempDir, Arc<relay_core::Engine>, Arc<Fake>) {
    let dir = tempfile::tempdir().unwrap();
    let store = relay_core::Store::open(&dir.path().join("store/store.db"), false).unwrap();
    let engine = relay_core::Engine::new(relay_core::Instance::Test, store);
    let fake = Fake::new(100.0);
    set_fake_exchange(&engine, fake.clone());
    (dir, engine, fake)
}

/// Moves an order's time back `secs`, as if it had been sent that long ago.
fn age_order(dir: &Path, id: i64, secs: i64) {
    let db = rusqlite::Connection::open(dir.join("store/arbiter.db")).unwrap();
    db.execute("UPDATE orders SET created_at = created_at - ?2 WHERE id = ?1", rusqlite::params![id, secs]).unwrap();
}

/// Moves back when an order was first seen done with fills missing, as if `secs` had passed.
fn age_shortfall(dir: &Path, id: i64, secs: i64) {
    let db = rusqlite::Connection::open(dir.join("store/arbiter.db")).unwrap();
    db.execute("UPDATE orders SET short_since = short_since - ?2 WHERE id = ?1", rusqlite::params![id, secs]).unwrap();
}

/// Moves a card's expiry back `secs`, as if it had been waiting that much longer.
fn age_card(dir: &Path, id: i64, secs: i64) {
    let db = rusqlite::Connection::open(dir.join("store/arbiter.db")).unwrap();
    db.execute("UPDATE proposals SET expires_at = expires_at - ?2 WHERE id = ?1", rusqlite::params![id, secs]).unwrap();
}

/// A live strategy the AI may trade, on a connected key: what the order tests place through.
fn live_agent(engine: &relay_core::Engine) -> i64 {
    connect(engine);
    let id = ok(engine, "arbiter.strategy.save", json!({"draft": draft("Agent", "20")}))["row"]["id"].as_i64().unwrap();
    ok(engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent", "venue": "live"}));
    id
}

/// The AI buys `quote` for the strategy; each amount is its own decision.
fn agent_buy(engine: &relay_core::Engine, id: i64, quote: &str) -> Value {
    ok(engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": quote, "why": "dip"}))
}

fn last_order(engine: &relay_core::Engine, id: i64) -> Value {
    ok(engine, "arbiter.order.list", json!({"strategy_id": id}))["orders"][0].clone()
}

#[test]
fn a_paper_strategy_buys_on_its_bar_and_its_stop_sells() {
    let (engine, fake) = setup();
    let s = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Dip", "50")}));
    let id = s["row"]["id"].as_i64().unwrap();
    assert_eq!(s["row"]["venue"], "paper");
    assert_eq!(s["row"]["state"], "stopped");
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "running": true}));
    pass(&engine);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!(d["fills"].as_array().unwrap().len(), 1, "{d:#}");
    assert!(d["row"]["held_base"].as_str().unwrap().parse::<f64>().unwrap() > 0.0);
    // A second pass on the same bar decides nothing new.
    pass(&engine);
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["fills"].as_array().unwrap().len(), 1);
    // The price falls 10%: the stop sells between bars.
    fake.set(90.0);
    pass(&engine);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!(d["row"]["held_base"], "0");
    assert_eq!(d["paper"]["trades"], 1);
    assert!(d["paper"]["pnl"].as_str().unwrap().starts_with('-'));
    let kinds: Vec<String> = d["decisions"].as_array().unwrap().iter().map(|x| x["kind"].as_str().unwrap().to_string()).collect();
    assert!(kinds.contains(&"buy".to_string()) && kinds.contains(&"sell".to_string()), "{kinds:?}");
    let summary = ok(&engine, "arbiter.summary", json!({}));
    assert_eq!(summary["connection"]["state"], "none");
    assert!(summary["paper"]["value"].as_str().unwrap().parse::<f64>().unwrap() < 1000.0);
}

#[test]
fn an_agents_order_is_a_proposal_unless_the_ai_may_trade() {
    let (engine, _) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Agent", "20")}))["row"]["id"].as_i64().unwrap();
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "RSI is low"}));
    assert_eq!(out["outcome"], "proposed", "{out:#}");
    let pid = out["proposal"]["id"].as_i64().unwrap();
    let approved = ok(&engine, "arbiter.proposal.resolve", json!({"id": pid, "approve": true}));
    assert_eq!(approved["status"], "approved", "{approved:#}");
    assert_eq!(ok(&engine, "arbiter.order.list", json!({"strategy_id": id}))["orders"][0]["status"], "filled");
    // Answered once only.
    assert_eq!(code(call(&engine, "arbiter.proposal.resolve", json!({"id": pid, "approve": true}))), "arbiter.answered");

    // Let the AI trade: its orders now place, and the gate still refuses what is over a limit.
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent"}));
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "60", "why": "bigger"}));
    assert_eq!(out["outcome"], "refused");
    assert_eq!(out["refusal"]["code"], "arbiter.over_max_order");
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "sell", "why": "take it"}));
    assert_eq!(out["outcome"], "placed", "{out:#}");
}

#[test]
fn the_ai_may_not_trade_without_limits_and_cannot_invent_products() {
    let (engine, _) = setup();
    let mut d = draft("Bare", "20");
    d["limits"] = json!({});
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": d}))["row"]["id"].as_i64().unwrap();
    assert_eq!(code(call(&engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent"}))), "arbiter.invalid");
    let mut moon = draft("Moon", "20");
    moon["product"] = json!("MOON-CAD");
    assert_eq!(code(call(&engine, "arbiter.propose", json!({"draft": moon, "why": "to the moon"}))), "arbiter.unknown_product");
    assert_eq!(code(call(&engine, "arbiter.strategy.save", json!({"draft": moon}))), "arbiter.unknown_product");
}

#[test]
fn a_proposed_change_applies_exactly_as_shown() {
    let (engine, _) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Dip", "50")}))["row"]["id"].as_i64().unwrap();
    let mut next = draft("Dip", "50");
    next["rule"]["exit"]["take_profit_pct"] = json!("3");
    let p = ok(&engine, "arbiter.propose", json!({"strategy_id": id, "draft": next, "why": "A smaller target won more often in the backtest"}));
    assert_eq!(p["kind"], "change");
    assert!(p["changes"][0].as_str().unwrap().contains("up 3%"), "{p:#}");
    assert_eq!(code(call(&engine, "arbiter.proposal.resolve", json!({"id": p["id"], "approve": true, "draft_hash": "not-it"}))), "arbiter.changed_since");
    ok(&engine, "arbiter.proposal.resolve", json!({"id": p["id"], "approve": true, "draft_hash": p["draft_hash"]}));
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!(d["row"]["version"], 2);
    assert_eq!(d["rule"]["exit"]["take_profit_pct"], "3");
}

#[test]
fn the_kill_switch_stops_everything_until_restarted() {
    let (engine, _) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Dip", "50")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "running": true}));
    ok(&engine, "arbiter.halt", json!({"reason": "testing"}));
    pass(&engine);
    assert!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["fills"].as_array().unwrap().is_empty());
    assert_eq!(ok(&engine, "arbiter.summary", json!({}))["halted"], true);
    ok(&engine, "arbiter.restart", json!({}));
    pass(&engine);
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["fills"].as_array().unwrap().len(), 1);
}

#[test]
fn a_key_that_can_transfer_is_refused_and_live_needs_a_key() {
    let (engine, fake) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Live", "50")}))["row"]["id"].as_i64().unwrap();
    assert_eq!(code(call(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "live"}))), "arbiter.not_connected");
    *fake.can_transfer.lock().unwrap() = true;
    assert_eq!(code(call(&engine, "arbiter.key.set", json!({"key_name": "organizations/o/apiKeys/k1", "private_key": KEY}))), "arbiter.key_can_transfer");
    *fake.can_transfer.lock().unwrap() = false;
    let c = ok(&engine, "arbiter.key.set", json!({"key_name": "organizations/o/apiKeys/k1234", "private_key": KEY}));
    assert_eq!(c["state"], "ok");
    assert_eq!(c["key"], "organizations/…/apiKeys/k123…");
    assert_eq!(c["fees"]["taker"], "0.006");

    // Live: preview, place, then the fills come back from the exchange.
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "live", "running": true}));
    pass(&engine);
    assert_eq!(fake.placed.lock().unwrap().len(), 1);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!(d["orders"][0]["status"], "filled", "{d:#}");
    assert_eq!(d["live"]["trades"], 0);
    assert_eq!(d["row"]["held_base"], "0.5");
    let summary = ok(&engine, "arbiter.summary", json!({}));
    assert_eq!(summary["live"]["cash"], "500");
}

#[test]
fn a_backtest_reports_both_parts_and_counts_versions() {
    let (engine, _) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Dip", "50")}))["row"]["id"].as_i64().unwrap();
    let r = ok(&engine, "arbiter.backtest", json!({"strategy_id": id, "days": 1}));
    assert!(r["judged"]["start"].as_i64().unwrap() > r["tuned"]["start"].as_i64().unwrap());
    let mut other = draft("Dip", "50");
    other["rule"]["exit"]["take_profit_pct"] = json!("2");
    let r = ok(&engine, "arbiter.backtest", json!({"strategy_id": id, "draft": other, "days": 1}));
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().starts_with("2 versions")), "{r:#}");
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["variants_tried"], 2);
}

/// Connects the fake exchange's key, as `arbiter.key.set` does.
fn connect(engine: &relay_core::Engine) {
    ok(engine, "arbiter.key.set", json!({"key_name": "organizations/o/apiKeys/k1234", "private_key": KEY}));
}

#[test]
fn the_private_key_is_never_kept_in_the_audit_log() {
    let (engine, _) = setup();
    connect(&engine);
    let rows = ok(&engine, "audit.list", json!({"op_prefix": "arbiter.key", "limit": 10}));
    assert!(rows["rows"].as_array().unwrap().is_empty(), "{rows:#}");
}

#[test]
fn a_card_asked_on_paper_is_not_placed_once_the_strategy_is_live() {
    let (engine, fake) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Ask", "20")}))["row"]["id"].as_i64().unwrap();
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "RSI is low"}));
    assert_eq!(out["outcome"], "proposed", "{out:#}");
    connect(&engine);
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "live"}));
    let p = ok(&engine, "arbiter.proposal.resolve", json!({"id": out["proposal"]["id"], "approve": true}));
    assert_eq!(p["status"], "failed", "{p:#}");
    assert!(fake.placed.lock().unwrap().is_empty());
}

#[test]
fn a_strategy_with_an_order_resting_places_no_second_beside_it() {
    let (engine, fake) = setup();
    connect(&engine);
    *fake.rest.lock().unwrap() = true;
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Agent", "20")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent", "venue": "live"}));
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "dip"}));
    assert_eq!(out["outcome"], "placed", "{out:#}");
    // The same call again is the same order; a different one waits for it.
    assert_eq!(ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "dip"}))["outcome"], "placed");
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "30", "why": "more"}));
    assert_eq!(out["refusal"]["code"], "arbiter.order_open", "{out:#}");
    assert_eq!(fake.placed.lock().unwrap().len(), 1);
}

#[test]
fn a_refused_stop_is_recorded_and_waits_before_trying_again() {
    let (engine, fake) = setup();
    connect(&engine);
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Live", "50")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "live", "running": true}));
    pass(&engine);
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["row"]["held_base"], "0.5");
    // The stop fires and the exchange refuses the sell.
    *fake.refuse_sells.lock().unwrap() = true;
    fake.set(90.0);
    pass(&engine);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    let failed: Vec<&Value> = d["orders"].as_array().unwrap().iter().filter(|o| o["side"] == "sell").collect();
    assert_eq!(failed.len(), 1, "{d:#}");
    assert_eq!(failed[0]["status"], "failed");
    // Straight after, it waits rather than hammering the exchange.
    *fake.refuse_sells.lock().unwrap() = false;
    pass(&engine);
    assert_eq!(fake.placed.lock().unwrap().iter().filter(|o| o.side == Side::Sell).count(), 0);
}

#[test]
fn an_edit_cannot_drop_the_limits_the_ai_or_live_trading_needs() {
    let (engine, _) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Agent", "20")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent"}));
    let mut bare = draft("Agent", "20");
    bare["limits"] = json!({});
    assert_eq!(code(call(&engine, "arbiter.strategy.save", json!({"id": id, "draft": bare}))), "arbiter.invalid");
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["row"]["version"], 1);
}

#[test]
fn a_fill_that_races_the_kill_switch_is_still_booked() {
    let (engine, fake) = setup();
    connect(&engine);
    *fake.rest.lock().unwrap() = true;
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Agent", "20")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent", "venue": "live"}));
    ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "dip"}));
    // It fills on the exchange between passes, and then everything is halted.
    *fake.rest.lock().unwrap() = false;
    ok(&engine, "arbiter.halt", json!({"reason": "testing"}));
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!(d["row"]["held_base"], "0.5", "{d:#}");
    assert_eq!(d["orders"][0]["status"], "filled");
}

#[test]
fn the_key_is_kept_while_a_live_order_is_open() {
    let (engine, fake) = setup();
    connect(&engine);
    *fake.rest.lock().unwrap() = true;
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Agent", "20")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "mode": "agent", "venue": "live"}));
    ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "dip"}));
    // Without the key the order could never be read back or cancelled.
    assert_eq!(code(call(&engine, "arbiter.key.remove", json!({}))), "arbiter.order_open");
    assert_eq!(ok(&engine, "arbiter.summary", json!({}))["connection"]["state"], "ok");
    // Halting cancels it on the exchange; then the key may go.
    ok(&engine, "arbiter.halt", json!({"reason": "testing"}));
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["orders"][0]["status"], "cancelled");
    assert_eq!(ok(&engine, "arbiter.key.remove", json!({}))["state"], "none");
}

#[test]
fn a_restart_waits_while_todays_loss_is_past_the_daily_limit() {
    let (engine, fake) = setup();
    let global = |loss: &str| json!({"global": {"daily_loss": loss, "price_band_pct": "2", "slippage_pct": "0.1", "approval_minutes": 10}});
    ok(&engine, "arbiter.limits.set", global("1"));
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Dip", "50")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "running": true}));
    pass(&engine);
    // The stop sells at a loss of about 5, past Arbiter's daily limit of 1: everything halts.
    fake.set(90.0);
    pass(&engine);
    assert_eq!(ok(&engine, "arbiter.summary", json!({}))["halted"], true);
    // Restarted, the next pass would only halt it again.
    assert_eq!(code(call(&engine, "arbiter.restart", json!({}))), "arbiter.daily_loss");
    assert_eq!(ok(&engine, "arbiter.summary", json!({}))["halted"], true);
    ok(&engine, "arbiter.limits.set", global("100"));
    ok(&engine, "arbiter.restart", json!({}));
    assert_eq!(ok(&engine, "arbiter.summary", json!({}))["halted"], false);
}

#[test]
fn a_strategy_past_its_own_daily_loss_waits_to_restart() {
    let (engine, fake) = setup();
    let mut d = draft("Dip", "50");
    d["limits"]["daily_loss"] = json!("1");
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": d}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "running": true}));
    pass(&engine);
    // Down 3%, short of the stop: the open loss of about 1.8 is past its limit of 1.
    fake.set(97.0);
    pass(&engine);
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["row"]["state"], "halted");
    assert_eq!(code(call(&engine, "arbiter.restart", json!({"strategy_id": id}))), "arbiter.daily_loss");
    ok(&engine, "arbiter.limits.set", json!({"strategy_id": id, "limits": {"max_order": "50", "max_position": "200", "daily_loss": "40"}}));
    ok(&engine, "arbiter.restart", json!({"strategy_id": id}));
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["row"]["state"], "running");
}

#[test]
fn an_order_whose_answer_was_lost_is_found_by_its_id_and_booked() {
    for lost in [ExchangeError::Unreachable("the request timed out".into()), ExchangeError::Malformed("expected value at line 1".into())] {
        let (dir, engine, fake) = setup_on_disk();
        let id = live_agent(&engine);
        *fake.lost.lock().unwrap() = Some(lost.clone());
        *fake.lookup_fails.lock().unwrap() = true;
        // Coinbase took it, but no answer could be read: it stays pending, not failed.
        let out = agent_buy(&engine, id, "20");
        assert_eq!(out["order"]["status"], "pending", "{lost:?}: {out:#}");
        assert!(out["order"]["exchange_order_id"].is_null());
        // A pass once create can no longer be waiting finds it by its client_order_id and books
        // its fill; it is not sent twice.
        *fake.lost.lock().unwrap() = None;
        *fake.lookup_fails.lock().unwrap() = false;
        age_order(dir.path(), out["order"]["id"].as_i64().unwrap(), 40);
        pass(&engine);
        let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
        assert_eq!(d["orders"][0]["status"], "filled", "{lost:?}: {d:#}");
        assert!(d["orders"][0]["exchange_order_id"].as_str().unwrap().starts_with("x-"));
        assert_eq!(d["row"]["held_base"], "0.5");
        assert_eq!(fake.placed.lock().unwrap().len(), 1);
    }
}

#[test]
fn an_order_coinbase_never_got_is_sent_again_then_failed_after_a_minute() {
    let (dir, engine, fake) = setup_on_disk();
    let id = live_agent(&engine);
    *fake.drop_orders.lock().unwrap() = true;
    let o = agent_buy(&engine, id, "20")["order"].clone();
    assert_eq!(o["status"], "pending");
    // While create may still be waiting for its answer, the runner leaves the order to it.
    *fake.drop_orders.lock().unwrap() = false;
    pass(&engine);
    assert!(fake.placed.lock().unwrap().is_empty());
    // After that, within the minute, it is sent again under the same id, and this time arrives.
    age_order(dir.path(), o["id"].as_i64().unwrap(), 40);
    pass(&engine);
    pass(&engine);
    assert_eq!(last_order(&engine, id)["status"], "filled");
    assert_eq!(fake.placed.lock().unwrap().len(), 1);

    // One that never arrives: a minute on, Coinbase still has no order with its id.
    *fake.drop_orders.lock().unwrap() = true;
    let o = agent_buy(&engine, id, "21")["order"].clone();
    assert_eq!(o["status"], "pending");
    age_order(dir.path(), o["id"].as_i64().unwrap(), 120);
    pass(&engine);
    let o = last_order(&engine, id);
    assert_eq!(o["status"], "failed", "{o:#}");
    assert!(o["error"].as_str().unwrap().contains("no order with this id"), "{o:#}");
    assert_eq!(fake.placed.lock().unwrap().len(), 1);
}

#[test]
fn a_halted_strategys_unanswered_order_is_not_sent_again_and_cancels_once_coinbase_answers() {
    let (dir, engine, fake) = setup_on_disk();
    let id = live_agent(&engine);
    *fake.drop_orders.lock().unwrap() = true;
    let oid = agent_buy(&engine, id, "20")["order"]["id"].as_i64().unwrap();
    let halt = || ok(&engine, "arbiter.halt", json!({"strategy_id": id, "reason": "testing"}));
    let not_cancelled = |h: &Value| (h["cancelled"].as_u64(), h["failed"].as_array().unwrap().len()) == (Some(0), 1);
    // The halt cannot ask Coinbase whether it has the order, so it does not call it cancelled.
    *fake.lookup_fails.lock().unwrap() = true;
    let h = halt();
    assert!(not_cancelled(&h), "{h:#}");
    assert_eq!(last_order(&engine, id)["status"], "pending");
    // Nor when Coinbase does not list it yet, moments after it was sent: it may still appear.
    *fake.drop_orders.lock().unwrap() = false;
    *fake.lookup_fails.lock().unwrap() = false;
    let h = halt();
    assert!(not_cancelled(&h), "{h:#}");
    assert!(h["failed"][0].as_str().unwrap().contains("does not list this order yet"), "{h:#}");
    // Halted, it is looked up but never sent again.
    age_order(dir.path(), oid, 40);
    pass(&engine);
    assert!(fake.placed.lock().unwrap().is_empty());
    assert_eq!(last_order(&engine, id)["status"], "pending");
    // A minute on, Coinbase still has no such order: only then is it cancelled here alone.
    age_order(dir.path(), oid, 30);
    let h = halt();
    assert_eq!(h["cancelled"], 1, "{h:#}");
    let o = last_order(&engine, id);
    assert_eq!(o["status"], "cancelled");
    assert!(o["error"].as_str().unwrap().contains("no record"), "{o:#}");
}

#[test]
fn a_cancel_finds_an_unanswered_order_on_coinbase_and_cancels_it_there() {
    let (engine, fake) = setup();
    let id = live_agent(&engine);
    *fake.rest.lock().unwrap() = true;
    *fake.lost.lock().unwrap() = Some(ExchangeError::Unreachable("the request timed out".into()));
    *fake.lookup_fails.lock().unwrap() = true;
    agent_buy(&engine, id, "20");
    assert!(last_order(&engine, id)["exchange_order_id"].is_null());
    *fake.lookup_fails.lock().unwrap() = false;
    ok(&engine, "arbiter.halt", json!({"reason": "testing"}));
    let o = last_order(&engine, id);
    assert_eq!(o["status"], "cancelled", "{o:#}");
    assert_eq!(fake.cancelled.lock().unwrap().as_slice(), [o["exchange_order_id"].as_str().unwrap()]);
}

#[test]
fn without_the_key_a_halt_leaves_a_live_order_open_and_says_so() {
    let (engine, fake) = setup();
    let id = live_agent(&engine);
    *fake.rest.lock().unwrap() = true;
    agent_buy(&engine, id, "20");
    set_fake_key(&engine, false);
    let h = ok(&engine, "arbiter.halt", json!({"reason": "testing"}));
    assert_eq!((h["cancelled"].as_u64(), h["failed"].as_array().unwrap().len()), (Some(0), 1), "{h:#}");
    assert_eq!(last_order(&engine, id)["status"], "open");
    assert!(fake.cancelled.lock().unwrap().is_empty());
    // With the key back, the kill switch cancels it on Coinbase.
    set_fake_key(&engine, true);
    assert_eq!(ok(&engine, "arbiter.halt", json!({"reason": "testing"}))["cancelled"], 1);
    assert_eq!(last_order(&engine, id)["status"], "cancelled");
    assert_eq!(fake.cancelled.lock().unwrap().len(), 1);
}

#[test]
fn an_order_filled_before_its_fills_show_stays_open_until_they_are_booked() {
    let (engine, fake) = setup();
    let id = live_agent(&engine);
    *fake.fills_lag.lock().unwrap() = 1;
    // Coinbase says filled, but lists no fill yet: it is not closed with nothing booked.
    agent_buy(&engine, id, "20");
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!((d["orders"][0]["status"].as_str(), d["row"]["held_base"].as_str()), (Some("open"), Some("0")), "{d:#}");
    pass(&engine);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!((d["orders"][0]["status"].as_str(), d["row"]["held_base"].as_str()), (Some("filled"), Some("0.5")), "{d:#}");

    // The same when the kill switch's cancel finds it filled and its fill not listed yet.
    let (engine, fake) = setup();
    let id = live_agent(&engine);
    *fake.rest.lock().unwrap() = true;
    agent_buy(&engine, id, "20");
    *fake.rest.lock().unwrap() = false;
    *fake.fills_lag.lock().unwrap() = 1;
    ok(&engine, "arbiter.halt", json!({"reason": "testing"}));
    assert_eq!(last_order(&engine, id)["status"], "open");
    pass(&engine);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!((d["orders"][0]["status"].as_str(), d["row"]["held_base"].as_str()), (Some("filled"), Some("0.5")), "{d:#}");
}

#[test]
fn a_strategy_cannot_switch_venue_while_it_holds_what_it_bought() {
    let (engine, _) = setup();
    connect(&engine);
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Live", "50")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "live", "running": true}));
    pass(&engine);
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["row"]["held_base"], "0.5");
    // Back on paper the real coins would have no stop, count in no limit and could not be sold.
    let err = refused(call(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "paper"})));
    assert_eq!(err.code, "arbiter.invalid");
    assert!(err.message.contains("sell it before switching to paper"), "{}", err.message);
    assert_eq!(ok(&engine, "arbiter.strategy.get", json!({"id": id}))["row"]["venue"], "live");
    // Sold, it may go back.
    ok(&engine, "arbiter.flatten", json!({"strategy_id": id}));
    assert_eq!(ok(&engine, "arbiter.strategy.set", json!({"id": id, "venue": "paper"}))["row"]["venue"], "paper");
}

#[test]
fn a_change_card_is_not_approved_over_a_change_made_since() {
    let (engine, _) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Dip", "50")}))["row"]["id"].as_i64().unwrap();
    let mut next = draft("Dip", "50");
    next["rule"]["exit"]["take_profit_pct"] = json!("3");
    let p = ok(&engine, "arbiter.propose", json!({"strategy_id": id, "draft": next, "why": "A smaller target won more often"}));
    // The person lowers the most held after the card was made; the card still carries 200.
    ok(&engine, "arbiter.limits.set", json!({"strategy_id": id, "limits": {"max_order": "50", "max_position": "100", "daily_loss": "40"}}));
    let r = ok(&engine, "arbiter.proposal.resolve", json!({"id": p["id"], "approve": true, "draft_hash": p["draft_hash"]}));
    assert_eq!(r["status"], "failed", "{r:#}");
    assert!(r["outcome"].as_str().unwrap().contains("changed since"), "{r:#}");
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!((d["limits"]["max_position"].as_str(), d["row"]["version"].as_i64()), (Some("100"), Some(1)), "{d:#}");
    // A card made from the strategy as it is now applies.
    let p = ok(&engine, "arbiter.propose", json!({"strategy_id": id, "draft": next, "why": "Again"}));
    assert_eq!(ok(&engine, "arbiter.proposal.resolve", json!({"id": p["id"], "approve": true}))["status"], "approved");
}

#[test]
fn an_order_card_is_not_placed_once_the_strategy_trades_another_product() {
    let (engine, fake) = setup();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Ask", "20")}))["row"]["id"].as_i64().unwrap();
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "RSI is low"}));
    assert_eq!(out["outcome"], "proposed", "{out:#}");
    let mut sol = draft("Ask", "20");
    sol["product"] = json!("SOL-CAD");
    ok(&engine, "arbiter.strategy.save", json!({"id": id, "draft": sol}));
    let p = ok(&engine, "arbiter.proposal.resolve", json!({"id": out["proposal"]["id"], "approve": true}));
    assert_eq!(p["status"], "failed", "{p:#}");
    assert_eq!(p["outcome"], "The strategy trades SOL-CAD now; nothing was placed.");
    assert!(ok(&engine, "arbiter.order.list", json!({"strategy_id": id}))["orders"].as_array().unwrap().is_empty());
    assert!(fake.placed.lock().unwrap().is_empty());
}

#[test]
fn a_card_past_its_time_is_not_approved_before_the_runner_expires_it() {
    let (dir, engine, _) = setup_on_disk();
    let id = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Ask", "20")}))["row"]["id"].as_i64().unwrap();
    let out = ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "buy", "quote": "20", "why": "RSI is low"}));
    let pid = out["proposal"]["id"].as_i64().unwrap();
    age_card(dir.path(), pid, 3600);
    assert_eq!(code(call(&engine, "arbiter.proposal.resolve", json!({"id": pid, "approve": true}))), "arbiter.answered");
    let cards = ok(&engine, "arbiter.proposal.list", json!({}));
    assert_eq!(cards["proposals"][0]["status"], "expired", "{cards:#}");
    assert!(ok(&engine, "arbiter.order.list", json!({"strategy_id": id}))["orders"].as_array().unwrap().is_empty());
}

#[test]
fn arbiters_most_held_counts_what_other_strategies_open_buys_will_spend() {
    let (engine, fake) = setup();
    ok(&engine, "arbiter.limits.set", json!({"global": {"max_exposure": "60", "price_band_pct": "2", "slippage_pct": "0.1", "approval_minutes": 10}}));
    let a = live_agent(&engine);
    let b = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Other", "20")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": b, "mode": "agent", "venue": "live"}));
    // A's buy of 20 is open on Coinbase, nothing filled yet: B may add 40, not 50.
    *fake.rest.lock().unwrap() = true;
    assert_eq!(agent_buy(&engine, a, "20")["order"]["status"], "open");
    let out = agent_buy(&engine, b, "50");
    assert_eq!(out["refusal"]["code"], "arbiter.over_exposure", "{out:#}");
    assert_eq!(agent_buy(&engine, b, "40")["outcome"], "placed");
}

#[test]
fn arbiters_daily_loss_is_weighed_on_each_venue_alone() {
    let (engine, fake) = setup();
    ok(&engine, "arbiter.limits.set", json!({"global": {"daily_loss": "4", "price_band_pct": "2", "slippage_pct": "0.1", "approval_minutes": 10}}));
    let live = live_agent(&engine);
    let paper = ok(&engine, "arbiter.strategy.save", json!({"draft": draft("Paper", "50")}))["row"]["id"].as_i64().unwrap();
    ok(&engine, "arbiter.strategy.set", json!({"id": paper, "mode": "agent"}));
    let sell = |id: i64| ok(&engine, "arbiter.order.place", json!({"strategy_id": id, "side": "sell", "why": "out"}));
    // Paper makes about 11 today; live loses about 5.6, past the limit of 4.
    fake.set(80.0);
    agent_buy(&engine, paper, "50");
    fake.set(100.0);
    sell(paper);
    agent_buy(&engine, live, "20");
    fake.set(90.0);
    sell(live);
    // Paper's gain does not cover the real loss: everything halts, and stays halted.
    pass(&engine);
    let summary = ok(&engine, "arbiter.summary", json!({}));
    assert_eq!(summary["halted"], true, "{summary:#}");
    assert!(summary["halt_reason"].as_str().unwrap().ends_with("on live"), "{summary:#}");
    let err = refused(call(&engine, "arbiter.restart", json!({})));
    assert_eq!(err.code, "arbiter.daily_loss");
    assert!(err.message.starts_with("Today's loss on live is still past"), "{}", err.message);
}

#[test]
fn a_halt_waits_for_a_resend_in_flight_and_cancels_what_it_placed() {
    let (dir, engine, fake) = setup_on_disk();
    let id = live_agent(&engine);
    *fake.rest.lock().unwrap() = true;
    *fake.drop_orders.lock().unwrap() = true;
    let oid = agent_buy(&engine, id, "20")["order"]["id"].as_i64().unwrap();
    age_order(dir.path(), oid, 40);
    // The runner sends it again, and that request is still on its way when the kill switch comes.
    *fake.drop_orders.lock().unwrap() = false;
    fake.shut_door();
    let runner = std::thread::spawn({
        let engine = engine.clone();
        move || pass(&engine)
    });
    wait_until("the resend is in flight", || fake.at_door.load(Ordering::SeqCst));
    let halting = std::thread::spawn({
        let engine = engine.clone();
        move || ok(&engine, "arbiter.halt", json!({"reason": "testing"}))
    });
    std::thread::sleep(Duration::from_millis(300));
    fake.open_door();
    runner.join().unwrap();
    // The halt waited for the resend's answer, so it cancels the order it placed on Coinbase.
    let h = halting.join().unwrap();
    assert_eq!(h["cancelled"], 1, "{h:#}");
    let o = last_order(&engine, id);
    assert_eq!(o["status"], "cancelled", "{o:#}");
    assert_eq!(fake.cancelled.lock().unwrap().as_slice(), [o["exchange_order_id"].as_str().unwrap()]);
}

#[test]
fn an_order_whose_fills_never_add_up_closes_with_what_was_booked_after_ten_minutes() {
    let (dir, engine, fake) = setup_on_disk();
    let id = live_agent(&engine);
    *fake.fills_lag.lock().unwrap() = 1000;
    let oid = agent_buy(&engine, id, "20")["order"]["id"].as_i64().unwrap();
    pass(&engine);
    assert_eq!(last_order(&engine, id)["status"], "open");
    // Ten minutes after Coinbase first called it filled, its fill is still not listed: it is
    // closed with what was booked, and the shortfall is told, rather than left open for good.
    age_shortfall(dir.path(), oid, 601);
    pass(&engine);
    let d = ok(&engine, "arbiter.strategy.get", json!({"id": id}));
    assert_eq!((d["orders"][0]["status"].as_str(), d["row"]["held_base"].as_str()), (Some("filled"), Some("0")), "{d:#}");
    assert!(d["orders"][0]["error"].as_str().unwrap().contains("lists fills for only 0"), "{d:#}");
    assert!(d["decisions"].as_array().unwrap().iter().any(|x| x["text"].as_str().unwrap().starts_with("Coinbase reports 0.5 ETH filled")), "{d:#}");
}
