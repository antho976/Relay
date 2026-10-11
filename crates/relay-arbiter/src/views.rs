//! What the book answers with: the shapes the `arbiter.*` bus ops return. Apart from the book so
//! the bus contract does not pull in SQLite.

use crate::backtest::Report;
use crate::model::{Balance, Candle, Fees, Granularity, Limits, Mode, OrderStatus, Permissions, Rule, RunState, Side, Source, Venue};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The exchange connection as last checked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterConnection")]
pub struct Connection {
    /// `none` (no key saved), `ok`, or `error` (the last check failed; see `error`).
    pub state: String,
    pub exchange: String,
    /// The key's name with its middle hidden: `organizations/…/apiKeys/9a2e…`.
    pub key: Option<String>,
    pub permissions: Option<Permissions>,
    /// The account's fee rates; the entry tier until a key reads the real ones.
    pub fees: Fees,
    /// RFC 3339.
    pub checked_at: Option<String>,
    pub error: Option<String>,
}

/// One venue's money: paper's simulated account or the exchange's real one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterAccount")]
pub struct AccountView {
    pub venue: Venue,
    /// Cash plus holdings at the last price, home currency.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub cash: Decimal,
    /// Strategies' profit today, realized plus open, after fees.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub pnl_today: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub pnl_total: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub fees_month: Decimal,
    /// The exchange's balances (live only; paper holds cash and strategies' positions).
    pub balances: Vec<Balance>,
    /// `[unix seconds, value]`, oldest first.
    pub equity: Vec<(i64, f64)>,
}

/// A strategy as a list row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterStrategyRow")]
pub struct StrategyRow {
    pub id: i64,
    pub name: String,
    pub product: String,
    pub granularity: Granularity,
    pub mode: Mode,
    pub venue: Venue,
    pub state: RunState,
    /// Why it is halted.
    pub halt_reason: Option<String>,
    pub version: i64,
    /// The rule in plain sentences.
    pub sentences: Vec<String>,
    /// What it is doing now: "Waiting for RSI(14) below 30", "Holding: sells at +4%".
    pub doing: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub held_base: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub held_cost: Decimal,
    /// Realized profit after fees, quote currency.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub pnl_realized: Decimal,
    /// What the holding is up or down at the last price, after the fee to sell it.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub pnl_open: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub pnl_today: Decimal,
    pub trades: u32,
    pub last_price: Option<f64>,
    /// The quote currency its amounts are in.
    pub currency: String,
}

/// A limit and how much of it today has used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterLimitUse")]
pub struct LimitUse {
    pub label: String,
    pub used: f64,
    pub limit: Option<f64>,
    /// How to show the numbers: `money`, `count` or `percent`.
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterOrder")]
pub struct OrderView {
    pub id: i64,
    pub strategy_id: Option<i64>,
    pub strategy: Option<String>,
    pub venue: Venue,
    pub source: Source,
    pub side: Side,
    pub product: String,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub quote_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub base_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub limit_price: Option<Decimal>,
    pub status: OrderStatus,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub filled_base: Decimal,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub average_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub fees: Decimal,
    /// What decided it, in words.
    pub why: String,
    /// The exchange's reason, when it failed.
    pub error: Option<String>,
    pub client_order_id: String,
    pub exchange_order_id: Option<String>,
    /// RFC 3339.
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterFill")]
pub struct FillView {
    pub id: i64,
    pub order_id: i64,
    pub strategy_id: Option<i64>,
    pub strategy: Option<String>,
    pub venue: Venue,
    pub side: Side,
    pub product: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub price: Decimal,
    /// Base amount.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub size: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub fee: Decimal,
    /// Sells: the profit it realized after fees.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub pnl: Option<Decimal>,
    /// RFC 3339.
    pub at: String,
}

/// One line of the decision log: what a strategy decided, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterDecision")]
pub struct DecisionView {
    pub id: i64,
    pub strategy_id: Option<i64>,
    /// `buy`, `sell`, `skip`, `refused`, `halt`, `restart`, `proposal`, `approved`, `dismissed`,
    /// `error`, `setting`.
    pub kind: String,
    pub text: String,
    pub order_id: Option<i64>,
    /// RFC 3339.
    pub at: String,
}

/// A strategy's name, product, bars, rule and limits: what a proposal would make it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterDraft")]
pub struct Draft {
    pub name: String,
    pub product: String,
    pub granularity: Granularity,
    pub rule: Rule,
    #[serde(default)]
    pub limits: Limits,
}

/// Something waiting for the person: an agent's suggestion, or an order in "every order asks".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterProposal")]
pub struct ProposalView {
    pub id: i64,
    /// `order`, `new_strategy` or `change`.
    pub kind: String,
    pub strategy_id: Option<i64>,
    pub strategy: Option<String>,
    /// One line: "Change the take-profit", "Buy 50 CAD of ETH".
    pub title: String,
    /// The proposer's reason.
    pub why: String,
    pub source: Source,
    /// `pending`, `approved`, `dismissed`, `expired` or `failed`.
    pub status: String,
    /// What approving it did, or why it failed.
    pub outcome: Option<String>,
    /// Orders: the side, amount and pricing.
    pub side: Option<Side>,
    pub product: Option<String>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub quote: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub base: Option<Decimal>,
    pub limit: bool,
    pub venue: Option<Venue>,
    /// Strategies: what it would be, and its changes from now as sentences.
    pub draft: Option<Draft>,
    pub changes: Vec<String>,
    /// The fingerprint approval binds to: the draft approved is the draft shown.
    pub draft_hash: Option<String>,
    /// RFC 3339.
    pub created_at: String,
    pub expires_at: Option<String>,
}

/// A price to show beside the strategies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterPrice")]
pub struct PriceRow {
    pub product: String,
    pub price: f64,
    pub change_24h: Option<f64>,
}

/// The Arbiter overview: the panel and the page draw from this one reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterSummary")]
pub struct Summary {
    pub connection: Connection,
    /// The currency totals are in.
    pub home: String,
    pub live: Option<AccountView>,
    pub paper: AccountView,
    pub strategies: Vec<StrategyRow>,
    /// Today's use of the Arbiter-wide limits.
    pub limits: Vec<LimitUse>,
    pub pending: Vec<ProposalView>,
    pub open_orders: Vec<OrderView>,
    pub fills: Vec<FillView>,
    pub prices: Vec<PriceRow>,
    /// The kill switch is on.
    pub halted: bool,
    pub halt_reason: Option<String>,
    /// The runner's last pass, RFC 3339, and what went wrong in it.
    pub checked_at: Option<String>,
    pub runner_error: Option<String>,
}

/// A strategy's record on one venue.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterRecord")]
pub struct Record {
    /// Days since its first order there.
    pub days: f64,
    pub trades: u32,
    pub won_pct: Option<f64>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub fees: Decimal,
    /// Profit over the most it held at once, percent.
    pub return_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterStrategy")]
pub struct StrategyDetail {
    pub row: StrategyRow,
    pub rule: Rule,
    pub limits: Limits,
    pub rule_hash: String,
    /// The move a round trip needs to break even at the account's fees, percent.
    pub break_even_pct: f64,
    /// How many versions of this rule have been backtested.
    pub variants_tried: u32,
    /// The latest backtest of this version.
    pub backtest: Option<Report>,
    pub paper: Record,
    pub live: Record,
    pub decisions: Vec<DecisionView>,
    pub orders: Vec<OrderView>,
    pub fills: Vec<FillView>,
}

/// A buy or sell on a chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterMarker")]
pub struct Marker {
    #[schemars(extend("x-unit" = "unix"))]
    pub at: i64,
    pub side: Side,
    pub price: f64,
    /// `paper`, `live` or `backtest`.
    pub kind: String,
}

/// A chart's numbers: candles and the trades made on them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterSeries")]
pub struct Series {
    pub product: String,
    pub granularity: Granularity,
    pub candles: Vec<Candle>,
    pub markers: Vec<Marker>,
}
