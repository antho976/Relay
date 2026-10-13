//! `arbiter.*` — Arbiter, rule-based crypto trading and AI trading research (docs/ARBITER.md).
//!
//! The book is its own SQLite file (`arbiter.db`); rules, the backtester and the risk gate are
//! `relay_arbiter`'s. Money and sizes are decimal strings in the product's quote currency. Every
//! change emits `arbiter.changed`.
//!
//! A thread's agent reaches the reads, `arbiter.backtest`, `arbiter.propose`, `arbiter.order.place`
//! and `arbiter.halt` (`threads::AGENT_OPS`); everything that saves a key, edits a strategy,
//! changes a limit, restarts after a halt or answers a proposal is the person's alone. An agent's
//! order goes through the same risk gate as a rule's, and outside "AI trades within limits" it
//! becomes a proposal instead of an order.
use crate::registry::{Actors, Audit, OpMeta, Scope};
use crate::{op, Empty};
use relay_arbiter::backtest::Report;
use relay_arbiter::gate::Refusal;
use relay_arbiter::model::{Granularity, GlobalLimits, Limits, Mode, Product, Side, Venue};
use relay_arbiter::views::{Connection, DecisionView, Draft, OrderView, ProposalView, Series, StrategyDetail, Summary};

op!(SummaryOp, "arbiter.summary", Empty => Summary,
    OpMeta::query(Scope::Global, 12, "The Arbiter overview: the exchange connection, paper and live accounts, every strategy and what it is doing, today's use of the limits, what waits for approval, open orders, recent fills and prices"));

payload!(#[schemars(rename = "ArbiterIdIn")] IdIn { pub id: i64 });
op!(StrategyGet, "arbiter.strategy.get", IdIn => StrategyDetail,
    OpMeta::query(Scope::Global, 12, "One strategy: its rule in sentences, limits, latest backtest, paper and live records, decision log, orders and fills"));

payload!(#[schemars(rename = "ArbiterListIn")] ListIn {
    pub strategy_id: Option<i64>,
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "ArbiterOrdersOut")] OrdersOut { pub orders: Vec<OrderView> });
op!(OrderList, "arbiter.order.list", ListIn => OrdersOut, OpMeta::query(Scope::Global, 12, "Orders, newest first, each with what decided it"));
result!(#[schemars(rename = "ArbiterDecisionsOut")] DecisionsOut { pub decisions: Vec<DecisionView> });
op!(DecisionList, "arbiter.decision.list", ListIn => DecisionsOut, OpMeta::query(Scope::Global, 12, "The decision log, newest first: every buy, sell, skip, refusal, halt and approval, and why"));

payload!(#[schemars(rename = "ArbiterProductsIn")] ProductsIn {
    /// Only products quoted in this currency, e.g. CAD.
    pub quote: Option<String>,
    /// Matches the product id or base currency.
    pub query: Option<String>,
});
result!(#[schemars(rename = "ArbiterProductsOut")] ProductsOut { pub products: Vec<Product> });
op!(Products, "arbiter.products", ProductsIn => ProductsOut,
    OpMeta::query(Scope::Global, 12, "The exchange's spot products with their sizes, increments, price and 24-hour change. Only these exist: never trade or backtest a product not listed here"));

payload!(#[schemars(rename = "ArbiterSeriesIn")] SeriesIn {
    /// `ETH-CAD`.
    pub product: String,
    pub granularity: Option<Granularity>,
    /// How many bars back from now, up to 1500. 300 by default.
    pub bars: Option<u32>,
    /// Mark this strategy's paper and live trades.
    pub strategy_id: Option<i64>,
});
op!(SeriesOp, "arbiter.series", SeriesIn => Series, OpMeta::query(Scope::Global, 12, "A chart's numbers: candles for a product, with a strategy's trades marked"));

payload!(#[schemars(rename = "ArbiterBacktestIn")] BacktestIn {
    /// Backtest this strategy's rule; or give `draft` to try a rule that is not saved.
    pub strategy_id: Option<i64>,
    pub draft: Option<Draft>,
    /// How many days back, up to 730. 180 by default.
    pub days: Option<u32>,
    /// The fraction of the range the rule was tuned on; the rest is judged. 0.67 by default.
    pub tuned_fraction: Option<f64>,
});
op!(Backtest, "arbiter.backtest", BacktestIn => Report,
    OpMeta::query(Scope::Global, 12, "Backtest a strategy or a draft on the exchange's candles: next-bar fills, the account's fees and slippage, a tuned part and a judged part, beside buying and holding. Counts toward the strategy's versions tried"));

result!(#[schemars(rename = "ArbiterProposalsOut")] ProposalsOut { pub proposals: Vec<ProposalView> });
payload!(#[schemars(rename = "ArbiterProposalListIn")] ProposalListIn {
    /// Only pending ones when true.
    pub pending: Option<bool>,
    pub limit: Option<u32>,
});
op!(ProposalList, "arbiter.proposal.list", ProposalListIn => ProposalsOut, OpMeta::query(Scope::Global, 12, "Proposals, pending first: agents' suggestions and orders waiting for approval"));

payload!(#[schemars(rename = "ArbiterKeySetIn")] KeySetIn {
    /// `organizations/{org}/apiKeys/{id}`.
    pub key_name: String,
    /// The ECDSA private key, PEM.
    pub private_key: String,
});
op!(KeySet, "arbiter.key.set", KeySetIn => Connection,
    OpMeta::mutation(Scope::Global, 12, "Check a Coinbase key with Coinbase and keep it in the system keyring. A key that can transfer money out is refused")
        // The payload is the private key itself: an audit row would keep it in the store and
        // every backup of it, outside the keyring ARBITER.md promises it stays in.
        .audit(Audit::Never).actors(Actors::UserOnly).emits(&["arbiter.changed"]));
op!(KeyRemove, "arbiter.key.remove", Empty => Connection,
    OpMeta::mutation(Scope::Global, 12, "Forget the Coinbase key; live strategies stop").actors(Actors::UserOnly).emits(&["arbiter.changed"]));
op!(Refresh, "arbiter.refresh", Empty => Connection,
    OpMeta::mutation(Scope::Global, 12, "Read balances, fees and the key's permissions from the exchange now").emits(&["arbiter.changed"]));

payload!(#[schemars(rename = "ArbiterStrategySaveIn")] StrategySaveIn {
    /// Absent creates a strategy (stopped, on paper, rules mode).
    pub id: Option<i64>,
    pub draft: Draft,
});
op!(StrategySave, "arbiter.strategy.save", StrategySaveIn => StrategyDetail,
    OpMeta::mutation(Scope::Global, 12, "Create a strategy or save a new version of one").actors(Actors::UserOnly).emits(&["arbiter.changed"]));
payload!(#[schemars(rename = "ArbiterStrategySetIn")] StrategySetIn {
    pub id: i64,
    pub mode: Option<Mode>,
    pub venue: Option<Venue>,
    /// Start or stop it. A halted strategy needs `arbiter.restart`.
    pub running: Option<bool>,
});
op!(StrategySet, "arbiter.strategy.set", StrategySetIn => StrategyDetail,
    OpMeta::mutation(Scope::Global, 12, "Change who decides, paper or live, or start and stop a strategy. Live needs a key that can trade; AI trading needs limits on order size, position and daily loss").actors(Actors::UserOnly).emits(&["arbiter.changed"]));
op!(StrategyDelete, "arbiter.strategy.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Delete a stopped strategy with nothing held; its history stays in the log").actors(Actors::UserOnly).emits(&["arbiter.changed"]));

payload!(#[schemars(rename = "ArbiterLimitsSetIn")] LimitsSetIn {
    /// A strategy's limits; absent sets Arbiter-wide ones from `global`.
    pub strategy_id: Option<i64>,
    pub limits: Option<Limits>,
    pub global: Option<GlobalLimits>,
});
op!(LimitsSet, "arbiter.limits.set", LimitsSetIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Set a strategy's limits or Arbiter's. Agents cannot").actors(Actors::UserOnly).emits(&["arbiter.changed"]));

payload!(#[schemars(rename = "ArbiterSettingsIn")] SettingsIn {
    /// The currency totals are shown in.
    pub home: Option<String>,
    /// What paper trading starts with, in the home currency.
    pub paper_cash: Option<String>,
    /// Erase every paper order, fill and record, and start paper over.
    pub reset_paper: Option<bool>,
});
result!(#[schemars(rename = "ArbiterSettingsOut")] SettingsOut { pub home: String, pub paper_cash: String, pub global: GlobalLimits });
op!(SettingsSet, "arbiter.settings.set", SettingsIn => SettingsOut,
    OpMeta::mutation(Scope::Global, 12, "Arbiter's own settings: the home currency and the paper account").actors(Actors::UserOnly).emits(&["arbiter.changed"]));
op!(SettingsGet, "arbiter.settings.get", Empty => SettingsOut, OpMeta::query(Scope::Global, 12, "Arbiter's settings and its Arbiter-wide limits"));

payload!(#[schemars(rename = "ArbiterHaltIn")] HaltIn {
    /// One strategy; absent halts everything (the kill switch).
    pub strategy_id: Option<i64>,
    pub reason: Option<String>,
});
result!(#[schemars(rename = "ArbiterHaltOut")] HaltOut {
    /// Open orders cancelled.
    pub cancelled: u32,
    /// Orders that could not be cancelled, and why.
    pub failed: Vec<String>,
});
op!(Halt, "arbiter.halt", HaltIn => HaltOut,
    OpMeta::mutation(Scope::Global, 12, "Halt a strategy, or everything: cancel open orders and place no new ones until the person restarts. Holdings are kept").emits(&["arbiter.changed"]));
payload!(#[schemars(rename = "ArbiterRestartIn")] RestartIn { pub strategy_id: Option<i64> });
op!(Restart, "arbiter.restart", RestartIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Lift a halt: one strategy's, or the kill switch").actors(Actors::UserOnly).emits(&["arbiter.changed"]));
payload!(#[schemars(rename = "ArbiterFlattenIn")] FlattenIn { pub strategy_id: i64 });
op!(Flatten, "arbiter.flatten", FlattenIn => OrderView,
    OpMeta::mutation(Scope::Global, 12, "Sell everything a strategy holds at the market price, even while halted").actors(Actors::UserOnly).emits(&["arbiter.changed"]));

payload!(#[schemars(rename = "ArbiterProposeIn")] ProposeIn {
    /// A change to this strategy; absent proposes a new one.
    pub strategy_id: Option<i64>,
    pub draft: Draft,
    /// Why, in a sentence or two the person reads on the card.
    pub why: String,
    /// The thread it came from.
    pub thread_id: Option<i64>,
});
op!(Propose, "arbiter.propose", ProposeIn => ProposalView,
    OpMeta::mutation(Scope::Global, 12, "Suggest a new strategy or a change to one. It waits on a card until the person approves it; nothing changes before").emits(&["arbiter.changed"]));

payload!(#[schemars(rename = "ArbiterOrderPlaceIn")] OrderPlaceIn {
    pub strategy_id: i64,
    pub side: Side,
    /// Buys: how much quote currency to spend.
    pub quote: Option<String>,
    /// Sells: how much base to sell; absent sells what the strategy holds.
    pub base: Option<String>,
    /// A post-only limit at the best price instead of a market order.
    pub limit: Option<bool>,
    pub why: String,
    pub thread_id: Option<i64>,
});
result!(#[schemars(rename = "ArbiterOrderPlaceOut")] OrderPlaceOut {
    /// `placed` (the gate passed and it went to the venue), `proposed` (waits for the person:
    /// the strategy is not in "AI trades within limits", or every order asks) or `refused`.
    pub outcome: String,
    pub order: Option<OrderView>,
    pub proposal: Option<ProposalView>,
    pub refusal: Option<Refusal>,
});
op!(OrderPlace, "arbiter.order.place", OrderPlaceIn => OrderPlaceOut,
    OpMeta::mutation(Scope::Global, 12, "An agent's order for a strategy. It passes the risk gate like a rule's; it is placed only when the strategy lets the AI trade, and otherwise waits for the person").emits(&["arbiter.changed"]));

payload!(#[schemars(rename = "ArbiterProposalResolveIn")] ProposalResolveIn {
    pub id: i64,
    pub approve: bool,
    /// Approvals bind to what was shown: the proposal's `draft_hash`.
    pub draft_hash: Option<String>,
});
op!(ProposalResolve, "arbiter.proposal.resolve", ProposalResolveIn => ProposalView,
    OpMeta::mutation(Scope::Global, 12, "Approve or dismiss a proposal. Approving an order passes it through the gate again and places it").actors(Actors::UserOnly).emits(&["arbiter.changed"]));

entries!(SummaryOp, StrategyGet, OrderList, DecisionList, Products, SeriesOp, Backtest, ProposalList, KeySet, KeyRemove, Refresh,
    StrategySave, StrategySet, StrategyDelete, LimitsSet, SettingsSet, SettingsGet, Halt, Restart, Flatten, Propose, OrderPlace, ProposalResolve);
