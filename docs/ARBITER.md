# Arbiter

Arbiter is Tally's sibling in the Threads space: rule-based crypto trading and AI trading research
on Antho's own exchange account, Coinbase first. Like Tally it is a source a thread's agent works
on, with its own pages and a tab in the thread's panel. Built 2026-10-09 (all four phases below at
once); "What was built" at the end says where each part lives and where it differs from the plan.

Decided with Antho on 2026-10-09:

- **In Threads, beside Tally.** A sidebar entry and pages like Tally's, a panel tab, an `Arbiter`
  source in the message box. The PC only at first.
- **Coinbase with an API key first.** The AI side runs on Antho's Claude or Codex subscription
  through the CLI, as threads already do; no AI API key is needed.
- **How far a strategy may go on its own is chosen per strategy** (the three modes below).
- **Paper to test, live when sure.** One switch per strategy, the same code path either way.

The mockup: https://claude.ai/artifact/4timQPAGdKuTrjeeHf3YYM

## What people love, what they hate, what Arbiter does about it

From reviews and forums of 3Commas, Cryptohopper, Coinrule, Pionex, Composer, Capitalise.ai,
TradingView, Freqtrade, Hummingbot, QuantConnect and the 2025–26 LLM trading contests.

| They love | Arbiter |
|---|---|
| "If this then that" rules, productive in minutes | Rules in plain sentences, built from pieces; the agent drafts them from a sentence |
| A backtest beside the rule, then a demo mode | Every rule shows its backtest, its paper record and its live record side by side |
| Set-and-forget DCA and scheduled buys | DCA and scheduled buys are the first templates |
| Open, local, inspectable (Freqtrade) | Everything runs on the PC; every decision is logged with its inputs |

| They hate | Arbiter |
|---|---|
| A hosted bot leaking keys (3Commas, 2022: ~$27M lost) | The key stays in the OS keyring; no server of ours exists to leak it. A key that can **transfer** is refused |
| Backtests that don't survive live, or look rigged | Honest by construction: next-bar fills, real fee tier, spread and slippage, an out-of-sample split, and a count of the variants tried |
| Subscriptions and fees that eat a small account | No subscription. Each rule shows the move it needs to break even at Antho's own fee tier |
| Grid and DCA bots buying all the way down a trend | A trend warning on grid and DCA rules; a drawdown halt that needs a person to restart |
| Fragile alert → webhook chains, doubled or missed orders | No webhook in the path. Every order carries a `client_order_id` derived from its signal, so a retry cannot double it |
| "AI bots" that promise returns | The agent researches and proposes; deterministic rules and engine limits trade. No promised returns anywhere in the copy |
| The exchange going down at the worst moment | A defined "exchange unreachable" state: no new orders, a notification, reconcile on return |

What goes wrong with LLMs placing trades: invented symbols and numbers, overconfidence, overtrading,
different answers to the same question, and backtests over years the model remembers from its
training data. The guard against each is in the engine, not the prompt.

## Autonomy: three modes per strategy

1. **Rules run, AI proposes** (the default). The strategy's rules place orders on their own, within
   the limits. The agent may read, backtest and propose; a proposal (a new rule, an edit, a one-off
   trade) waits on a card until Antho approves it.
2. **AI trades within limits.** The agent may place orders for this strategy itself. Every order
   still passes the same risk gate a rule's order does. This mode cannot be switched on without
   limits, and the agent can never change limits (`Actors::UserOnly`).
3. **Every order asks.** No limits are required; each order, from a rule or the agent, becomes a
   card and a notification and waits. An unanswered card expires (default 10 minutes).

**Paper or live** is a second switch on the same strategy. Paper reads real prices and balances
and simulates fills (Coinbase has no paper trading; its sandbox returns canned replies). Going live
shows the paper record and the limits in one confirmation. Either way, the switch waits for an open
order and for what the strategy holds on its venue to be sold: the runner exits and weighs a
strategy on its own venue only, so a holding left behind would have no stop-loss, take-profit or
trailing stop, count in no limit, and could not be sold from Arbiter.

## The risk gate

Every order, whatever its source or mode, goes through one function in the engine before it reaches
the exchange. It refuses rather than shrinks (a limit that resizes an order is not a limit):

- the product exists, is tradable, and is on the strategy's list (an invented symbol stops here);
- size rounded down to the product's increments and above its minimum;
- a limit price within a band of the best bid/ask (default 2%);
- the most per order, per product position, and total exposure: what every strategy on the venue
  holds at cost, plus what other strategies' open buys will still spend, since a market buy is
  usually still pending when the next strategy's turn comes;
- the most orders per hour;
- one open order per strategy: a second beside one still resting is refused, since the caps weigh
  only what has filled;
- a cooldown after a losing trade;
- the **daily loss limit**, per strategy and Arbiter-wide: it halts the strategy, or everything,
  until Antho restarts it. Arbiter-wide, each venue's loss is weighed on its own, so paper's gains
  never cover a real loss; either venue past the limit halts everything, a day lost on paper
  stopping live trading too. A restart is refused while today's loss is still past the limit,
  which would only halt it again. (A drawdown halt is not built yet.)

**The kill switch** (sidebar, panel and command palette): cancel every open order and halt every
strategy; flattening positions is a separate, confirmed step.

## Honest backtests

- A signal on bar *t*'s close fills at bar *t+1*'s open; look-ahead is impossible by construction.
- Fees from the real tier (`GET /transaction_summary`), plus spread and a slippage model.
- The range is split: tuned on the first part, judged on the last. Both are shown.
- Every variant run of a strategy is counted, with a warning as the count grows.
- Always beside buy-and-hold, with max drawdown and time in the market.
- **The AI's memory line.** On any chart a thread's agent reasons about, the model's training cutoff
  is marked: before it, the model may remember what happened.

## Coinbase

- **Advanced Trade API** on Antho's retail account (`api.coinbase.com/api/v3/brokerage`).
- **The key:** a CDP key, **ECDSA** (Ed25519 gets 401s), **View + Trade, never Transfer**, scoped to a
  dedicated portfolio holding only what Arbiter may risk, IP allowlist recommended. Setup reads
  `GET /key_permissions` and refuses a key that can transfer; a View-only key works for read-only
  use.
- **Auth:** an ES256 JWT per request (`iss: "cdp"`, `sub`/`kid` = key name, `uri` = `METHOD
  host/path`, two-minute life). Signed in our own code (`p256`), not a third-party Coinbase crate.
- **Numbers are strings;** they are parsed with `rust_decimal`, never `f64`.
- **Orders:** preview first (`/orders/preview`), then create with the derived `client_order_id`.
  `success: true` only means accepted; status comes from reconciling orders and fills.
- **Limits:** REST throttled to ~10 requests/s; candles come 350 at a time.
- **To check on Antho's account first:** that Advanced Trade is enabled in Québec, and which CAD
  pairs exist (`GET /products`).

## Shape in the code

Tally's shape:

- `crates/relay-arbiter`: rules, indicators, the backtester, the paper fill simulator and the risk
  gate as pure, tested functions. Also an `Exchange` trait with two implementations, Coinbase and
  Paper, so paper and live share every line above it.
- `arbiter.db` beside `money.db`: the following tables. A strategy's rules are versioned and
  hashed, and an approval binds to one version.
  - strategies;
  - orders, keyed by `client_order_id`;
  - fills;
  - proposals;
  - the decision log, append-only;
  - cached candles;
  - limits.
- `arbiter.*` bus ops. Exchange calls never run under the store mutex: queries are
  `register_unlocked`; order placement is `register_staged`.
- **The runner:** one background thread in the shape of `purge::spawn_timer`. It wakes on each live
  strategy's bar and does four things:
  - fetches prices;
  - evaluates the rules;
  - sends intents through the gate;
  - places and reconciles.
- **The key** lives in the Secret Service through `secret-tool`, as the Android signing passwords do
  (`handlers/device.rs`), with attributes `application Relay purpose coinbase`. It cannot be
  removed while a live order is open: without it the order could never be read back or cancelled.
- **Threads:**
  - The `Tally` chip becomes a real source picker stored per thread.
  - `AGENT_OPS` gains Arbiter's read ops, `arbiter.backtest` and `arbiter.propose`. `arbiter.order.place`
    is admitted only for a strategy in mode 2.
  - Proposals and mode-3 orders reuse the confirm card Threads phase 4 builds.

## Pages and panel

The same tokens, strip, cards and meters as Tally (`money_pages.rs`), shown in a sidebar entry and a
panel tab:

- **Overview:** portfolio value, today's P&L after fees, each strategy as a row (mode, paper or
  live, state, P&L), open orders, the kill switch.
- **Strategies:** a strategy's rule as sentences, its limits, and backtest / paper / live side by
  side.
- **Orders:** orders and fills, each linked to the signal and decision that made it.
- **Research:** backtests and the agent's proposals.

Colour keeps its meaning: green for money in and a live strategy; red for a loss, a refusal or a
halt; amber for paper.

## Phases

1. **Read-only.** Key setup and the permission check, balances, products, candles, the Overview page
   and panel tab, Arbiter's read ops for agents, charts from `arbiter.series`.
2. **Strategies on paper.** The rule model, DCA and scheduled templates, the backtester, the paper
   runner.
3. **Research and proposals.** The agent's backtest and propose ops, proposal cards, the source
   picker.
4. **Live.** The risk gate on the Coinbase path, the kill switch, preview-then-place,
   reconciliation, notifications for fills, refusals, halts and disconnects. Modes 2 and 3.
5. **Later.** The WebSocket user channel, Tally's Invest tab showing Arbiter's portfolio, more
   exchanges.

## What was built

**The crate, `crates/relay-arbiter`.** Pure and tested:
- `model` and `views`: the contract.
- `indicators`: SMA, EMA, Wilder's RSI, change, prior high and low.
- `rules`: evaluation, schedules, exits, validation, sentences and the hash an approval binds to.
- `backtest`: next-bar fills, fees and slippage, the stop counted before the target in the same
  bar, the tuned and judged split, and buy-and-hold.
- `gate`: the risk gate, and a `client_order_id` derived from the cause.

Behind the `engine` feature:
- `book`: `arbiter.db`. Positions are read back from fills. The decision log is append-only by
  trigger.
- `paper`: paper fills.
- `coinbase`: the Advanced Trade client. It signs ES256 JWTs with `ring`, reads market data from the
  public endpoints so paper needs no key, and throttles itself to about 10 requests a second.

**The engine, `crates/relay-core/src/arbiter.rs`.**
- **The desk.** It holds:
  - the book behind its own mutex, never held across a network call;
  - the key, cached once the keyring answers, a key or none (a keyring that could not be read,
    locked or timing out, is asked again a minute later by the runner, and at once by the kill
    switch and Refresh; a lookup that comes back empty while a key is on record is a keyring
    that could not be read, not "no key");
  - quotes;
  - the trade mutex, which serializes the gate and the order being recorded.
- **`execute`** is the only path an order takes. `route` sends an intent there, or turns it into a
  proposal when the mode says so. An intent the gate would refuse is refused at once, not
  proposed.
- **Live orders** are previewed, then placed. When no answer to create can be read (it timed out,
  came back unreadable, or Coinbase says it has that id already), the order stays pending:
  - For its first 30 seconds the runner leaves it to the call that sent it, whose create may still
    be waiting.
  - After that, each pass looks it up by its `client_order_id`: List Orders for its product,
    created from five minutes before it was sent to five after its last resend, placed through
    Advanced Trade (as the API's own orders are taken to be). This runs in the trade turn the kill
    switch also takes, so a halt never decides while a resend is on its way.
  - Found, it is reconciled like any other order. Not found, it is sent again under the same id
    for a minute, unless a halt came since; after that it is failed as never placed. A lookup
    that fails decides nothing.
  - An order Coinbase answered with "duplicate" exists, whatever its list shows: it is kept
    pending, never failed as never placed nor cancelled here alone, and the person is told to
    check the Coinbase app.
  - A row the book closed alone is reopened only by an answer from Coinbase that carries the
    order's id, which proves it exists there; one Coinbase closed never reopens.
- **An order Coinbase calls filled or cancelled** is closed only once the book has its fills: they
  can lag the status, so until they show it stays open and the next pass books the rest. That
  wait ends ten minutes after Coinbase first called it done: a shortfall that long is not lag, and
  the order closes with what was booked, saying what Coinbase reported and what its fills came to.
- **A cancel** goes to Coinbase or nowhere. Without the key, or with Coinbase not answering, the
  order stays open and the kill switch lists it as not cancelled. An order with no answer is looked
  up first. Within a minute of being sent, "not listed" says nothing (Coinbase may not list it
  yet), so it stays pending and the kill switch lists it; only one Coinbase still has no record of
  after that is cancelled in the book alone. A cancel reads the
  order and its fills back from Coinbase, so a fill that raced the kill switch is still booked.
- **A proposal** is placed on the venue and product it was asked for, or not at all: a card asked
  on paper fails once the strategy has gone live, and one asked for ETH once it trades SOL. A
  proposed change applies only to the strategy as it was when the card was made (its rule,
  limits, product and bars, hashed on the card): changed since, approving it fails rather than
  put back what was changed. A card past its time cannot be approved, whether or not the runner
  has expired it yet.
- **An exit that failed** (refused, unconfirmed, or filled in part) is tried again a minute later
  under a new cause, which names the holding as it is then; an expired card asks again, a dismissed
  one does not. The key's payload is never written to the audit log.
- **The runner** is a thread, as `purge` is. Every 20 seconds, while anything runs or waits, it:
  1. reconciles open orders;
  2. expires proposals;
  3. checks the daily loss limits;
  4. fires the price exits;
  5. buys on a schedule; a buy missed by more than an hour, while Relay was closed, is skipped,
     not caught up;
  6. decides once on each strategy's newest closed bar;
  7. cancels a limit order that has not filled within a bar.

**The handlers, `crates/relay-core/src/handlers/arbiter.rs`.** Reads are `register_unlocked`;
everything that calls the exchange or the keyring is `register_staged`. Tests on a fake exchange:
`crates/relay-core/tests/arbiter.rs`.

**Threads.** `AGENT_OPS` gains the reads, `arbiter.backtest`, `arbiter.propose`,
`arbiter.order.place` and `arbiter.halt`. The prompt tells the agent:
- only listed products exist;
- to say when a backtest covers years its training data may hold;
- that an order is placed only in "AI trades within limits" mode.

A chart block of type `price` draws `arbiter.series`, with the model's cutoff marked.

**The native client.** Arbiter's pages, its panel and the Tally/Arbiter source chips are
`apps/relay-native/src/arbiter_pages.rs`, `threads_view.rs`, `threads_chart.rs` and
`css/arbiter.css`. The source chips pick the panel on the client only; the agent always has both
Tally and Arbiter.

**Not built yet:**
- the drawdown halt;
- Coinbase's WebSocket feeds (the runner polls REST);
- regime warnings on grid and DCA rules;
- Tally's Invest tab showing Arbiter;
- the phone.
