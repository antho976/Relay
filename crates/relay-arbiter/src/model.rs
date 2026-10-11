//! Arbiter's words: strategies, their rules and limits, orders, fills and proposals.
//!
//! Money and sizes are [`Decimal`], carried on the bus as strings ("50.00"), because an exchange
//! takes and gives them as strings and a float would round a size into a refusal. Prices that only
//! feed indicators and charts are `f64`: a signal is a comparison, not an amount.

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Paper simulates fills on real prices; live sends orders to the exchange.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterVenue")]
pub enum Venue {
    #[default]
    Paper,
    Live,
}

impl Venue {
    pub fn as_str(self) -> &'static str {
        match self {
            Venue::Paper => "paper",
            Venue::Live => "live",
        }
    }
    pub fn parse(s: &str) -> Option<Venue> {
        match s {
            "paper" => Some(Venue::Paper),
            "live" => Some(Venue::Live),
            _ => None,
        }
    }
}

/// Who decides, per strategy (docs/ARBITER.md, Autonomy).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterMode")]
pub enum Mode {
    /// The rules place orders within the limits; an agent may only propose.
    #[default]
    Rules,
    /// An agent may place orders itself, within the limits. Needs limits.
    Agent,
    /// Every order, from the rules or an agent, waits for the person.
    Ask,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Rules => "rules",
            Mode::Agent => "agent",
            Mode::Ask => "ask",
        }
    }
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "rules" => Some(Mode::Rules),
            "agent" => Some(Mode::Agent),
            "ask" => Some(Mode::Ask),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Rules => "Rules run, AI proposes",
            Mode::Agent => "AI trades within limits",
            Mode::Ask => "Every order asks",
        }
    }
}

/// A candle's width, as the exchange names it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[schemars(rename = "ArbiterGranularity")]
pub enum Granularity {
    OneMinute,
    FiveMinute,
    FifteenMinute,
    ThirtyMinute,
    #[default]
    OneHour,
    TwoHour,
    SixHour,
    OneDay,
}

impl Granularity {
    pub const ALL: [Granularity; 8] = [
        Granularity::OneMinute,
        Granularity::FiveMinute,
        Granularity::FifteenMinute,
        Granularity::ThirtyMinute,
        Granularity::OneHour,
        Granularity::TwoHour,
        Granularity::SixHour,
        Granularity::OneDay,
    ];
    pub fn seconds(self) -> i64 {
        match self {
            Granularity::OneMinute => 60,
            Granularity::FiveMinute => 300,
            Granularity::FifteenMinute => 900,
            Granularity::ThirtyMinute => 1800,
            Granularity::OneHour => 3600,
            Granularity::TwoHour => 7200,
            Granularity::SixHour => 21600,
            Granularity::OneDay => 86400,
        }
    }
    /// The exchange's name, `ONE_HOUR`.
    pub fn as_str(self) -> &'static str {
        match self {
            Granularity::OneMinute => "ONE_MINUTE",
            Granularity::FiveMinute => "FIVE_MINUTE",
            Granularity::FifteenMinute => "FIFTEEN_MINUTE",
            Granularity::ThirtyMinute => "THIRTY_MINUTE",
            Granularity::OneHour => "ONE_HOUR",
            Granularity::TwoHour => "TWO_HOUR",
            Granularity::SixHour => "SIX_HOUR",
            Granularity::OneDay => "ONE_DAY",
        }
    }
    pub fn parse(s: &str) -> Option<Granularity> {
        Granularity::ALL.into_iter().find(|g| g.as_str() == s)
    }
    /// "1h", for a sentence.
    pub fn short(self) -> &'static str {
        match self {
            Granularity::OneMinute => "1m",
            Granularity::FiveMinute => "5m",
            Granularity::FifteenMinute => "15m",
            Granularity::ThirtyMinute => "30m",
            Granularity::OneHour => "1h",
            Granularity::TwoHour => "2h",
            Granularity::SixHour => "6h",
            Granularity::OneDay => "1d",
        }
    }
}

/// One bar. `start` is Unix seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterCandle")]
pub struct Candle {
    #[schemars(extend("x-unit" = "unix"))]
    pub start: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// A value a condition compares: the price, an indicator over the strategy's bars, or a number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(rename = "ArbiterOperand")]
pub enum Operand {
    /// The bar's close.
    Price,
    /// Simple moving average of closes.
    Sma { period: u32 },
    /// Exponential moving average of closes.
    Ema { period: u32 },
    /// Wilder's relative strength index, 0 to 100.
    Rsi { period: u32 },
    /// Percent change of the close over `bars` bars: 5 is +5%.
    Change { bars: u32 },
    /// Highest high of the `bars` bars before this one.
    High { bars: u32 },
    /// Lowest low of the `bars` bars before this one.
    Low { bars: u32 },
    Number { value: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterCompare")]
pub enum Compare {
    Above,
    Below,
    /// Below or equal on the bar before, above on this one.
    CrossesAbove,
    CrossesBelow,
}

/// When to act. `all`/`any` nest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(rename = "ArbiterCondition")]
pub enum Condition {
    Compare { left: Operand, op: Compare, right: Operand },
    All { of: Vec<Condition> },
    Any { of: Vec<Condition> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterEvery")]
pub enum Every {
    Day,
    Week,
    Month,
}

/// When a strategy buys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(rename = "ArbiterEntry")]
pub enum Entry {
    /// A scheduled buy (dollar-cost averaging), in UTC: `weekday` 1 (Monday) to 7 for `week`,
    /// `day` 1 to 28 for `month`.
    Schedule { every: Every, hour: u8, weekday: Option<u8>, day: Option<u8> },
    /// Buy when the condition holds on a closed bar and nothing is held.
    Signal { when: Condition },
}

/// How an order is priced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterPricing")]
pub enum Pricing {
    /// Fills at once at the best price on the other side; pays the taker fee.
    #[default]
    Market,
    /// A post-only limit at the best price on this side; pays the maker fee, may not fill.
    Limit,
}

/// When a strategy sells what it bought. Percentages are of the average buy price.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterExit")]
pub struct Exit {
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub take_profit_pct: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub stop_loss_pct: Option<Decimal>,
    /// Sell when the price falls this far below its highest close since the buy.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub trailing_pct: Option<Decimal>,
    /// Sell after this many bars whatever the price.
    #[serde(default)]
    pub max_bars: Option<u32>,
    /// Sell when this holds on a closed bar.
    #[serde(default)]
    pub when: Option<Condition>,
}

impl Exit {
    pub fn is_empty(&self) -> bool {
        self.take_profit_pct.is_none()
            && self.stop_loss_pct.is_none()
            && self.trailing_pct.is_none()
            && self.max_bars.is_none()
            && self.when.is_none()
    }
}

/// A strategy's rule: when it buys, how much, and when it sells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterRule")]
pub struct Rule {
    pub entry: Entry,
    /// How much to buy each time, in the product's quote currency.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub buy: Decimal,
    #[serde(default)]
    pub pricing: Pricing,
    #[serde(default)]
    pub exit: Exit,
}

/// What may never be exceeded. Absent means no limit. Amounts are the quote currency.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterLimits")]
pub struct Limits {
    /// The most one order may spend.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub max_order: Option<Decimal>,
    /// The most held at once, at cost.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub max_position: Option<Decimal>,
    /// A loss today (realized, plus what open positions are down) that halts until restarted.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub daily_loss: Option<Decimal>,
    #[serde(default)]
    pub orders_per_hour: Option<u32>,
    /// Minutes to wait after a losing sell before buying again.
    #[serde(default)]
    pub cooldown_minutes: Option<u32>,
}

impl Limits {
    /// Enough to let something trade by itself: a cap per order, per position and per day.
    pub fn bounded(&self) -> bool {
        self.max_order.is_some() && self.max_position.is_some() && self.daily_loss.is_some()
    }
}

/// Arbiter-wide limits, across every strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterGlobalLimits")]
pub struct GlobalLimits {
    /// The most held across every strategy, at cost, in the home currency.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub max_exposure: Option<Decimal>,
    /// A loss today across every strategy that halts them all.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub daily_loss: Option<Decimal>,
    #[serde(default)]
    pub orders_per_hour: Option<u32>,
    /// How far from the best price a limit may be placed, percent.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub price_band_pct: Decimal,
    /// Slippage assumed for market orders in backtests and paper fills, percent.
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub slippage_pct: Decimal,
    /// What a pending approval waits before it expires, minutes.
    pub approval_minutes: u32,
}

impl Default for GlobalLimits {
    fn default() -> Self {
        GlobalLimits {
            max_exposure: None,
            daily_loss: None,
            orders_per_hour: Some(30),
            price_band_pct: Decimal::new(2, 0),
            slippage_pct: Decimal::new(1, 1),
            approval_minutes: 10,
        }
    }
}

/// Running, stopped by the person, or halted by a limit or the kill switch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterRunState")]
pub enum RunState {
    #[default]
    Stopped,
    Running,
    Halted,
}

impl RunState {
    pub fn as_str(self) -> &'static str {
        match self {
            RunState::Stopped => "stopped",
            RunState::Running => "running",
            RunState::Halted => "halted",
        }
    }
    pub fn parse(s: &str) -> Option<RunState> {
        match s {
            "stopped" => Some(RunState::Stopped),
            "running" => Some(RunState::Running),
            "halted" => Some(RunState::Halted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterSide")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }
    /// The exchange's spelling.
    pub fn upper(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }
    pub fn parse(s: &str) -> Option<Side> {
        match s.to_ascii_lowercase().as_str() {
            "buy" => Some(Side::Buy),
            "sell" => Some(Side::Sell),
            _ => None,
        }
    }
}

/// Who asked for an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterSource")]
pub enum Source {
    Rule,
    Agent,
    Person,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Rule => "rule",
            Source::Agent => "agent",
            Source::Person => "person",
        }
    }
    pub fn parse(s: &str) -> Option<Source> {
        match s {
            "rule" => Some(Source::Rule),
            "agent" => Some(Source::Agent),
            "person" => Some(Source::Person),
            _ => None,
        }
    }
}

/// What a tradable product allows, from the exchange.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterProduct")]
pub struct Product {
    /// `BTC-CAD`.
    pub id: String,
    pub base: String,
    pub quote: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub base_increment: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub quote_increment: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub price_increment: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub base_min_size: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub quote_min_size: Decimal,
    /// The last price, when the exchange gave one.
    pub price: Option<f64>,
    /// Percent change over 24 hours.
    pub change_24h: Option<f64>,
    /// False when the exchange has trading, or new orders, turned off for it.
    pub tradable: bool,
    /// Only limit orders are accepted.
    pub limit_only: bool,
}

/// The best prices now.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterQuote")]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    /// Unix seconds.
    #[schemars(extend("x-unit" = "unix"))]
    pub at: i64,
}

impl Quote {
    pub fn mid(&self) -> f64 {
        (self.bid + self.ask) / 2.0
    }
}

/// The key's permissions as the exchange reports them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterPermissions")]
pub struct Permissions {
    pub can_view: bool,
    pub can_trade: bool,
    pub can_transfer: bool,
    /// The portfolio the key is limited to.
    pub portfolio_uuid: Option<String>,
    pub portfolio_type: Option<String>,
}

/// The account's fee rates, as fractions: 0.006 is 0.6%.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterFees")]
pub struct Fees {
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub maker: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub taker: Decimal,
    /// The tier's name, when known.
    pub tier: Option<String>,
}

impl Default for Fees {
    /// The entry tier, until the account's own is read.
    fn default() -> Self {
        Fees { maker: Decimal::new(4, 3), taker: Decimal::new(6, 3), tier: None }
    }
}

/// One currency held on the exchange.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterBalance")]
pub struct Balance {
    pub currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub available: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub hold: Decimal,
    /// Its worth in the home currency, when a price for it was found.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub value: Option<Decimal>,
}

/// An order on its way to the exchange: what the gate passed, sized and priced to the
/// product's increments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterOrderRequest")]
pub struct OrderRequest {
    /// Derived from what caused the order, so a retry cannot place it twice.
    pub client_order_id: String,
    pub product: String,
    pub side: Side,
    /// Buys by market spend this much quote currency.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub quote_size: Option<Decimal>,
    /// Sells, and limit buys, give the base amount.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub base_size: Option<Decimal>,
    /// A post-only limit at this price; absent is a market order.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub limit_price: Option<Decimal>,
}

/// An order's state on the exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ArbiterOrderStatus")]
pub enum OrderStatus {
    /// Sent, not yet acknowledged as open.
    Pending,
    Open,
    Filled,
    Cancelled,
    Expired,
    Failed,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::Open => "open",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Expired => "expired",
            OrderStatus::Failed => "failed",
        }
    }
    pub fn parse(s: &str) -> Option<OrderStatus> {
        match s.to_ascii_lowercase().as_str() {
            "pending" | "queued" => Some(OrderStatus::Pending),
            "open" | "cancel_queued" => Some(OrderStatus::Open),
            "filled" => Some(OrderStatus::Filled),
            "cancelled" => Some(OrderStatus::Cancelled),
            "expired" => Some(OrderStatus::Expired),
            "failed" => Some(OrderStatus::Failed),
            _ => None,
        }
    }
    /// No further fills will come.
    pub fn is_done(self) -> bool {
        matches!(self, OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Expired | OrderStatus::Failed)
    }
}

/// A fill as the exchange reports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterExchangeFill")]
pub struct ExchangeFill {
    /// The exchange's id for the fill, unique.
    pub trade_id: String,
    pub order_id: String,
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
    /// Unix seconds.
    pub at: i64,
}

/// An order as the exchange reports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterExchangeOrder")]
pub struct ExchangeOrder {
    pub order_id: String,
    pub client_order_id: String,
    pub status: OrderStatus,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub filled_size: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub average_filled_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schemars(with = "String")]
    pub total_fees: Decimal,
    /// Why it failed or was rejected.
    pub reason: Option<String>,
}

/// What a preview says an order would do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ArbiterPreview")]
pub struct Preview {
    pub preview_id: Option<String>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub commission: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub quote_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schemars(with = "Option<String>")]
    pub base_size: Option<Decimal>,
    /// Reasons the exchange would refuse it; empty when it would not.
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
