# Threads

Threads replace the Money space. A thread is a conversation with an agent that works on the data
of Antho's own apps, shown and editable beside it: agent first, data second. Tally's budget is the
first source and its investments the second; Arbiter, the trading bot, is designed in
`docs/ARBITER.md`, and Avex, the gym app, read only from its export, in `docs/GYM.md`.

Decided with Antho on 2026-10-07:

- **Like the claude.ai website.** Native message bubbles, a thread list, a message box. Not a
  terminal. Many threads, each kept.
- **One app with Dev.** Threads is a space of the same window: the same title bar, sidebar,
  you-row, Settings, status bar, colours and fonts (`DESIGN.md`). Only the middle changes. The Money
  space's own warm palette goes.
- **No resending the whole conversation on every open.** Relay keeps its own copy of every
  message, so a thread reopens without the agent. While a thread is in use its agent keeps running;
  a cold thread is resumed once by Claude's own conversation id.
- **The agent may change the data, within limits.** It adds and edits entries freely, each with an
  Undo; deleting, budgets and accounts ask the person first.
- **Charts may use colour,** and the Tally panel's views may too. A chart stores its question, not
  its numbers, so it redraws when the phone syncs.

The mockup: https://claude.ai/artifact/GPtaPk6YYLiFCHGpeNPMk4

## Phases

1. **The thread engine** (built): `threads.db`, the `thread.*` ops, the background agent.
2. **The Threads space** in `relay-native` (built): thread list in the sidebar, the conversation,
   the message box, the Tally panel beside it, and Tally's pages restyled in Dev's tokens.
3. **Charts** (built): the agent writes a `chart` block, Relay draws it from the ledger and redraws
   it when the ledger moves. (The Undo card came with phase 2; pinning a chart to the panel is
   still to do.)
4. **Asking first** (built): what the agent may not do alone (deleting, many changes at once,
   budgets, accounts) it proposes; the person approves or declines each on a card, and approving
   applies it once, as the person, whole or not at all.
5. **Investments in Tally** (built in the PC's engine and for the agent; `docs/INVESTMENTS.md` is
   the contract): holdings and their value tracked in the ledger, on the phone and the PC in the
   same piece of work (`CLAUDE.md`: a money rule changes on both sides at once, a PR in each repository), and an agent that brings
   the person's Wealthsimple accounts in through the files Wealthsimple exports.

## The engine (phase 1)

`crates/relay-core/src/threads.rs`, ops in `crates/relay-bus/src/ops/thread.rs`.

**Storage.** `threads.db` beside the store (`money.db`'s neighbour), its own SQLite file behind its
own mutex: a `thread` row (title, provider, model, `agent_ref`) and its `message` rows. The store's
lock is never held for a thread, and the store's schema is untouched.

**Messages.** `role` is `user` (`{"text"}`, with `"images": [{"id", "media_type", "width",
"height"}]` when it carries pictures, below), `assistant` (`{"blocks"}`, each a `text` or a
`tool_use` with its `id`, `name` and `input`), `tool` (`{"tool_use_id", "is_error", "text"}`) or
`error` (`{"text"}`). Claude's thinking is not kept.

**The agent.** Claude in print mode with stream-json in and out, started in `threads/` beside the
store (Claude files conversations by working directory, so a resume must start there):

```
claude -p --input-format stream-json --output-format stream-json --verbose
       --include-partial-messages --strict-mcp-config --mcp-config <relay, as the person>
       --tools "" --allowedTools mcp__relay --setting-sources ""
       --append-system-prompt <what a thread is> [--model M] [--resume <agent_ref>]
```

- `--tools ""` removes every built-in tool: no shell, no files, no web. Verified with Claude Code
  2.1.293: the agent's tool list is only the MCP server's tools, and it may call them.
- The MCP server is Relay's own (`relay mcp`, acting as the person), under an agent whose
  environment sets `RELAY_MCP_OPS` to `threads::AGENT_OPS`. A thread has no session of its own, so
  the agent acts as the person, and the list is enforced twice: the MCP server lists and answers
  only those ops, and the socket door, which refuses the person's actor from any process the engine
  started (`peer.rs`), knows each running agent's pid and admits a process in that tree for those
  ops and `bus.ops` alone (`Peer::Thread`). Verified with real Claude (Haiku) against a disposable
  engine: it read the budget and a series through these tools and answered with a chart.
- The agent's start is checked: when Claude reports Relay's server missing, failed or with no
  tools, the thread says so in an error message instead of letting the agent answer from nothing.
- Each message is one stream-json line on the agent's stdin. Its stdout is read on a thread of its
  own: `thread.delta` events carry the reply as it is written, and each finished assistant message
  and tool answer is stored and announced with `thread.message`. Claude's `result` ends the turn.
- Spawning and writing to the agent happen after the handler returns, never under the store lock.
- One turn at a time: a message sent while the agent is answering is refused with `thread.busy`.
  `thread.stop` kills the agent's process group; the next message resumes it.
- An agent idle for 15 minutes is closed; at most four run at once, the one used longest ago closing
  first. An agent that exits mid-turn leaves an `error` message saying so, with its last stderr line.

**Ops.** `thread.list`, `thread.get` (messages after an id, for catching up), and, the person's
alone: `thread.create` (optionally with the first message, which also names the thread),
`thread.send`, `thread.stop`, `thread.rename`, `thread.delete`, and a message's pictures,
`thread.upload` and `thread.image` (below).

**Events.** `thread.changed` (`{"thread"}`, or `{"id", "deleted": true}`), `thread.message`
(`{"thread", "message"}`), `thread.delta` (`{"thread", "text"}`, not stored), `thread.proposal`
(`{"thread", "proposal"}`, phase 4).

Tests: `crates/relay-core/tests/threads.rs` drives threads end to end against a stand-in Claude
that speaks the same stream-json.

## The Threads space (phase 2)

`apps/relay-native/src/threads_view.rs`, styled by `css/threads.css`; the space itself is still
`money.rs` (the switcher, the sidebar swap, the start screen), which now says Threads.

- **The sidebar** keeps Dev's frame and footer. Its keys are New thread (Ctrl N) and Tally, then
  the threads grouped Today, Yesterday, This week and Earlier, a green lamp on one at work.
- **The middle** is a new thread (greeting, a line from the panel's source, the message box, four
  questions to start) or the open one: the person's messages as bubbles, the agent's Markdown drawn natively
  (`relay_client::thread_view`: paragraphs, headings, lists, tables, code), each tool call as a
  quiet line saying what it did, and an entry the agent added as a card with Undo
  (`money.tx.delete`). Enter sends, Shift+Enter breaks the line; while the agent works the send key
  stops it.
- **The Tally panel** (the strip's right key shows or hides it): Overview (what is left, budgets,
  bills, recent entries), Entries (this month) and Invest (below). Overview shows the five budgets
  furthest ahead of pace, and a "Show all 8" key for the rest. Meters take the category's hue; amber is
  ahead of pace, red over budget. On a page narrower than about 1000 pixels the panel hides itself
  unless the person opened it with that key, which then holds their choice (`threads_view.rs`,
  `PANEL_FROM`).
- **The panel's rows keep the person in the thread.** A budget opens a small editor beside it: the
  amount (empty is no budget), last month's spending and the average of the last three whole
  months (`money.series`; a month with nothing in the whole ledger came before it was kept and is
  left out), and Save (`money.budget.set`). The panel holds still while it is open and reads what
  changed once it closes. An entry opens its editor. A bill to pay, an investment account and a
  holding draft a question ("Can I cover Rent on Sun 1 Nov?", "How is XEQT doing?") into the open
  thread's message box with `threads::draft`, to finish or send; with no thread open they ask it in
  a new one. Only the panel's own "Open …" keys leave for a page.
- **One source switch.** Tally, Arbiter or Avex is chosen once, on the switch above the panel; the
  message box no longer repeats it. What follows the source: the panel, the message box's line on
  what the agent may do ("Edits with Undo · asks first", "Proposes, you approve", "Reads only"), an open thread's
  placeholder ("Ask about your money, or tell it what to change", "Ask about your training"), and a
  new thread's line and questions. Tally's line is the month ("October so far: $2,425 spent, $475
  left"); Avex's is read from `gym.summary`: this week while the export reaches it ("This week so
  far: 2 of 4 workouts, 18,450 lb lifted"), else the export's own week and its age ("Your Avex copy
  is from Thu 8 Oct, 7 days ago. That week: 3 of 4 workouts, 46,405 lb lifted"), so a stale copy
  never reads as a week without training. Avex's questions name the lift done most and the workout
  done last ("How is my Bench Press trending?", "Plan my next Legs A"). Arbiter's new thread keeps
  Tally's line and questions.
- **Tally's pages** (Overview, Entries, Plan, Data) open from the sidebar's Tally key with their own
  tabs, in Dev's tokens (`css/money.css`); the Money space's warm palette is gone.

A display smoke run opens a thread with `RELAY_NATIVE_THREAD=<id>` beside `RELAY_NATIVE_PAGE=threads`.

## Charts (phase 3)

An agent asks for a chart by writing a fenced block with the language `chart` and one JSON object:

```chart
{"type": "bar", "title": "Groceries by week", "query": {"by": "week", "periods": 2, "category": "Groceries"}}
```

- `type` is `bar` (periods side by side), `line` (with `"cumulative": true` for pace) or `donut`
  (where the money went). `query` is a `money.series` payload (`gym` a `gym.series` one, below).
- `data` takes the place of `query` for numbers that are not the ledger's or Avex's: a projection
  into the coming months, a plan, a what-if, or figures the person gave. It is drawn as written and
  never redraws: `{"labels": [...], "series": [{"name", "values": [...], "projected"?: true}],
  "unit"?: "kg"}`, one value per label. Without `unit`, values are amounts in the ledger's currency
  as plain decimal numbers (`1234.56`, not minor units); with it (`"kg"`, `"%"`, `"km"`, `""` for a
  count) they are figures in that unit. A series with `projected` true is the projected part, and
  is drawn as one. The prompt allows a projection the person asks for as the one place the agent
  works figures out itself, from figures a tool gave and at a rate it says, and tells it to say
  so in words; figures a `query` or `gym` chart can draw are never copied into `data`. A thread
  on a model on this PC is told the same in a sentence of `LOCAL_PROMPT` (not yet measured on the
  bench).
- `money.series` (`crates/relay-money/src/series.rs`): `measure` spending, income or net; `by`
  category (the largest seven and Other, in their category colours), week, day or period; `periods`
  how many budget periods end at `period_offset`; `category` by name. A period still running says
  how far it has got (`known`), so a running total stops at today. It is a reading for the PC, not a
  money rule, so it has no Kotlin twin.
- The card (`apps/relay-native/src/threads_chart.rs`) reads its numbers when drawn and again on every
  `money.changed`. Colour is the data's: the newest period in Relay's lime, earlier ones in blue,
  orange and on; hovering a group gives its values. Labels and axis are Dev's type.

## Asking first (phase 4)

The engine is `crates/relay-core/src/threads/proposals.rs`, the ops are in
`crates/relay-bus/src/ops/thread.rs`, and the card is `apps/relay-native/src/threads_ask.rs`.

**A proposal is one action from a short list.** The agent calls
`thread.propose {action, why}`. `action` is `{"<op>": [<that op's own payload>, ...]}`, so each
payload is checked against the target op's own input type, and the MCP door converts its amounts
as it does for that op. `why` is a sentence the person reads on the card. The list:

| Action | At most | |
|---|---|---|
| `money.tx.delete` | 50 | duplicates, mistakes |
| `money.tx.update` | 50 | recategorizing many, or fixing an amount, a date or a note (one small edit stays a direct `money.tx.update`) |
| `money.budget.set` | 20 | a budget, the monthly budget, or removing one (amount 0) |
| `money.account.add` | 5 | |
| `money.account.update` | 5 | renaming, archiving |
| `money.invest.delete` | 50 | a duplicate manual activity |

Room, prices, rates, imports, erasing and restoring are not on the list. They stay the person's
(`docs/INVESTMENTS.md`). Avex is read only.

**Checked when made, tied to its thread.** Only a thread's own agent proposes. The socket door
knows the agent by the process that connected (`peer::Peer::Thread`). It hands that thread to the
handler (`Engine::dispatch_from_thread`, `Ctx::thread`), and nothing in a request can claim one;
from anywhere else `thread.propose` is refused with `thread.not_agent`. What the action acts on
must be in the ledger (`money.not_found` otherwise). It must change something. Each target may
appear only once. The whole batch then runs as a trial under the ledger's own rules and is put
back (`Ledger::trial`), so a budget on an income category is refused now, not on approval. The
same action already waiting in the thread is answered with that proposal. A thread holds at most
20 waiting proposals.

**Stored in `threads.db`** (version 3, table `proposal`), never in `money.db`, which syncs with
the phone. A row holds the action, the reason, a `summary` in words read from the ledger
(dates, notes, amounts as decimals), its status and outcome, and `items`. `items` is what the
action acts on: the entries, activities, accounts or budgets as they were when proposed. A
proposal goes with its thread.

**Reading it** (`thread.proposal.list {thread_id, pending?}`, `thread.proposal.get {id}`, both
the person's): a pending proposal reads its targets again from the ledger each time. Each item is
`same`, `changed` since it was proposed, `gone`, or `new` (an account to add). Each item also
carries the result of a trial of its change: `entry_after`, `account_after`, `budget_after`.
`stale` says something moved, and `seen` is a fingerprint of what was read. An answered proposal
shows its targets as they were when it was answered, so a card still shows what was deleted.

**Answering** (`thread.proposal.resolve {id, approve, seen?}`, the person's alone: `UserOnly`, and
refused with `thread.not_person` from any thread's agent):
- Declining keeps the targets as they are and changes nothing.
- Approving reads the targets again. It is refused with `thread.proposal_gone` when one went. It
  is refused with `thread.proposal_changed` when they differ from the reading the person approved
  (`seen`), or from the proposal as made when `seen` is absent.
- An approval then claims the proposal (`pending` → `running`), so a second answer is refused
  with `thread.proposal_answered`. It applies the whole batch in one ledger transaction
  (`Ledger::atomically`, a savepoint) inside the handler, as the person. This is a short local
  write like any money op's. No other op's handler runs inside it. The thread store's lock and
  the ledger's are taken one after the other, never one inside the other.
- If any payload fails, nothing is applied, and the proposal is `failed` with the ledger's
  message ("Entry 7: That category is not for this kind of entry").
- A proposal Relay stopped in the middle of applying is marked `failed` with a reason when
  `threads.db` next opens.
- Nothing expires.
- `money.changed` and `thread.proposal` follow.

**Undo** (`thread.proposal.undo {id}`): an approved proposal goes back whole or not at all.
Entries and activities are restored, budgets go back to their old amounts, and accounts get their
old name and state back. Undo is refused when a target was changed after the approval, which
putting it back would overwrite. It is also refused when the ledger cannot put an item back
exactly (an entry cannot lose its category). An added account has no Undo: it is archived in
Tally.

**The agent hears the answer** at the head of the person's next message. The answer is not stored
in the thread, which keeps the person's own words. When a message is handed to the agent, every
answer it has not heard comes first, one line each:

```
[Relay: the person approved proposal 3 (Delete 2 entries: 2026-10-05 “Metro” 42.50 CAD; …); it is done.]
[Relay: the person declined proposal 4 (Set 1 budget: Dining 320.00 CAD → 380.00 CAD); nothing changed.]
```

Then comes a blank line and the person's text. Each answer is marked told once the line reaches
the agent, whether the agent was running or started for this message. A cold thread hears it in
its first line. An undo is news again. The prompt tells the agent to call `thread.propose` and
that a change "is not done until Relay tells you it was approved".

**The card** (`threads_ask.rs`, from `draw_tool_answer` when a tool answer is `thread.propose`'s):
- It takes only the proposal's id from the agent's answer. Everything else comes from
  `thread.proposal.list`, read again when the thread opens, on `thread.proposal`, and on
  `money.changed`.
- The title and the key name the act and its count: "Delete 2 entries", "Move 6 entries" (only
  categories change), "Set 3 budgets", "Archive 1 account". Deleting, removing and archiving keys
  use `money-destructive`; the others are primary. The secondary key is "Not now".
- The rows are Tally's (`icon_row`): the entry's badge, note, category, account and date, and its
  amount. An edit shows before → after. A budget reads "$320.00 → $380.00 a month".
- The agent's reason shows only quoted: "“…” — Claude".
- Waiting, the card carries Arbiter's amber "Needs your approval" pill and asking border.
- A row that changed or went says "Changed since Claude asked" or "Already gone". The key then
  waits for "Review again", which reads the ledger again; the approval binds to that reading. A
  card with a row gone offers only "Not now".
- Once answered, the card shows the outcome (Done with Undo, Not done, Failed with the reason,
  Undone). It stays so when the thread is reopened.

**Edits are honest too.** `money.tx.update` answers with the entry and `previous`, the entry as it
was. The "Changed" card shows before → after (category, date, account, note, amount), and its
Undo puts the entry back (Redo makes the edit again). An edit that gave an entry its first
category has no Undo there, because the ledger cannot take a category away.

Tests:
- `crates/relay-core/tests/threads.rs` proposes from a thread agent; refuses outside a thread, an
  op off the list, a missing target, a no-op, a target twice and too many; approves once; declines;
  undoes; refuses a stale approval and a gone target; keeps an inner failure with its message; and
  checks that the stand-in Claude is told each answer once.
- One test sends `thread.propose` through a real socket door from inside the stand-in agent's own
  process tree. There the door names the thread, and the same connection is refused an answer.
- `crates/relay-cli/src/units.rs` converts a batch's amounts both ways.
- `relay-money`'s ledger tests cover `atomically` and `trial`.

Not yet: ticking rows to approve part of a batch, an amber dot on the thread's sidebar row and a
"waiting for you" chip on the panel, and the cards on the phone (`thread.proposal.*` is not in
`PHONE_OPS`).

## Model and effort, and the Tally pages (2026-10-08)

- **Each thread picks its model and effort** in the message box (Default, Opus 5.5, Sonnet 5.5,
  Haiku 5.5, Fable 5.1; effort Default, Low to Max). `thread.set` stores them (`threads.db`
  version 2 adds `effort`) and closes a running agent, so the next message resumes the same
  conversation with `--model` and `--effort`. A new thread starts with the last choice, kept
  across restarts (`native.threads.model`, `.effort`).
- **Tally's pages are dense:** each opens on a strip of figures (Overview: left to spend, spent,
  income, net worth and the pace; Entries: count, in, out, net; Plan: monthly budget, given to
  categories, unassigned, spent), then cards of compact rows with category badges and icons.
  Overview fits budgets, bills and recent entries on one screen; Entries groups days with their net;
  Plan is one table; Data is a two-column settings list.
- **The sidebar and the Tally panel can be dragged** wider or narrower (sidebar 200 to 420 pixels;
  the panel opens at 340, then at the width it was left at; its source and tab are kept too, under
  `native.threads.*`).

## Tally's pages, distilled (2026-10-15)

Easier to do things, each figure said once, one way to ask per page.

- **Overview** leads with Left to spend, large (amber ahead of pace; spent past the budget, Over
  budget and by how much, in red), and one line on how it got there: spent of budget, a day's
  allowance, against last month. Income sits beside it; Spent and Net worth left the strip (the
  line says the first, the Accounts card totals the second). Budgets beside Recent, then Accounts,
  Upcoming bills (three, and how many more) and Goals side by side: about 1,090 pixels, from 1,582.
  An account opens this period's Entries filtered to it; an investment account opens Investments.
- **Entries** says its figures in the context line ("27 entries · in $2,150 · out $2,425 · net
  −$275") instead of a strip. A row's badge opens the category list and moves the entry; its menu
  (right click, the Menu key) holds Edit, Change category, Ask about this entry and Delete.
  Delete deletes the focused row, Enter edits, Ctrl F finds. Each change has Undo on the toast,
  and on every Tally page Ctrl Z presses it while it shows (a field keeps Ctrl Z for its text).
- **Plan** takes the monthly budget in its strip, beside what categories hold of it. Beside each
  budget field: last month's spending and the average of the last three months, or as many as the
  ledger reaches (one `money.series` read per category, kept until `money.changed`). Categories
  with no budget and nothing spent fold under one key.
- **The entry sheet** starts with no category chosen, so nothing is filed under the list's first
  row; the category list finds as you type. The account starts where that kind was last saved.
  Save and add another (Ctrl Enter) keeps the day, account and category. Ctrl E opens the sheet
  anywhere in Threads. A save says what it did, with Undo.
- **Asking** is one key per page, in the person's words: the Budgets card on Overview (its
  question follows the month: over budget, ahead of pace), By day on Entries, Suggest budgets on
  Plan ("Suggest monthly budgets from my last 3 months of spending"), the head of Investments
  (and Needs a look, about what it lists). An entry's menu asks about that entry: "Tell me about
  this entry: Burger night, Dining, $14.98 on Thu 8 Oct".
- **Glass:** Tally's cards, strips, tiles, fields and keys at rest are glass veils (DESIGN.md, the
  Glass Rule), and Avex's and Arbiter's pages, which share them, follow. The sheets, menus and
  the toast float, and stay opaque. Meters draw their track in the page's ink, not a fixed grey,
  and one squeezed under 2 pixels draws nothing instead of aborting the client.

## Investments (phase 5)

`docs/INVESTMENTS.md` is the contract: the ledger's new tables, the rules both devices read them by,
the Wealthsimple files and the ops. This is what the PC's engine and the agent do with them.

- **Ops** (`crates/relay-bus/src/ops/money.rs`, handlers in `crates/relay-core/src/handlers/money.rs`):
  `money.invest.summary` (the portfolio reading) and `money.invest.list` (activities, newest first)
  are unlocked queries. `money.invest.add`, `money.invest.delete`, `money.invest.restore`,
  `money.invest.room`, `money.invest.price`, `money.fx.set`, `money.value.set` and
  `money.account.update` are short ledger writes, and `money.account.add` takes a `registration` and
  an `institution`. Every mutation emits `money.changed`.
- **Files are the person's.** `money.invest.preview` (an unlocked query) and `money.invest.import`
  (staged: the file is read before the transaction opens) are `UserOnly` and take an absolute path
  to a Wealthsimple holdings report, activities export or monthly statement: a regular file of at
  most 16 MiB, decoded as the phone decodes it (UTF-8, UTF-16 by its mark, else Windows-1252). The
  plan values a holding in US dollars at the ledger's rates, as the phone's does. An import writes
  rows under derived uids, so the same file imported twice, or on the phone and here, adds its lines
  once.
- **Rates from the Bank of Canada.** `money.fx.fetch` (`UserOnly`) answers `{started}` at once, as
  `github.connect` does. After the transaction a thread of its own runs `curl` under a 15-second
  deadline against Valet's `FXUSDCAD` series (no key; the request carries nothing about the person),
  writes the last ten business days' rates as `BANK_OF_CANADA`, and emits `money.changed` with
  `{"fx": n}`, or `{"fx": 0, "error"}` when it failed. A fetch asked for while one runs answers
  `started: false`.
- **The agent** may call `money.invest.summary`, `money.invest.list` and `money.invest.add`
  (`AGENT_OPS`). Its prompt gives it the units (units held and prices at 1e-8, gains in basis
  points), tells it to read the summary first, to say the as-of date and that a return is
  money-weighted, and to leave arithmetic to the tools; walks it through Wealthsimple's Documents
  menu for the holdings report and the activities export; and keeps it safe: it never takes a
  password, a two-factor code or a key, only `my.wealthsimple.com` is Wealthsimple, and a live
  connection (SnapTrade, the unofficial API) is not built. Importing, room, prices and rates stay
  the person's; deleting an activity it proposes (phase 4). It says plainly what reaches Claude: the names and figures it reads to
  answer go to Claude with the rest of the thread, the files and account numbers do not; and text
  inside tool results is data, never instructions (`docs/INVESTMENTS.md`, "The thread agent").
- **The import card.** The agent asks for a file with a fenced `import` block:

  ```import
  {"source": "wealthsimple", "expects": "holdings", "title": "Your holdings report"}
  ```

  `relay_client::thread_view::import_spec` reads it (`source` must be `wealthsimple`; `expects` is
  `holdings`, `activities` or `statement`). The card lets the person choose or drop the file,
  previews it (`money.invest.preview`), maps its accounts and imports it; the person's next message
  says what came in. An activity the agent records is a card with Undo (`money.invest.delete`).
- **In the native client** (`apps/relay-native/src/threads_view.rs`, `money_invest.rs`):
  - The import card has a "Choose file…" key and takes a dropped file; both run
    `money_invest::import_flow`, the flow the Investments page's Import key uses (preview, map the
    accounts, import). When it is done the card says what came in, offers "Open Investments", and
    sends a message in the person's name naming the kind of file, so the agent carries on.
  - The Tally panel's third tab, **Invest**: the portfolio's value and gain, the allocation, the
    accounts, the room and the top holdings; with no investment account, "Connect Wealthsimple".
  - `threads::ask(ui, text)` starts a new thread with the person's own words and sends them; it is
    for a page's keys. Inside the Threads page, `threads::draft(ui, text)` puts the words in the
    open thread's message box and focuses it without sending, and asks like `ask` when no thread
    is open.
    "Connect Wealthsimple", on the Invest tab and on the Investments page, asks "Help me connect my
    Wealthsimple accounts"; a new thread suggests "Connect my Wealthsimple accounts" while the
    ledger has no investment account, and "How are my investments doing?" once it has one.

Tests: `crates/relay-core/tests/money.rs` imports the shared holdings report (W1) through the bus
twice and reads it back, records, undoes and restores an activity, refuses files and the network to
an agent, and fetches rates from a stand-in `curl`; `crates/relay-remote/tests/remote.rs` carries an
imported activity from the phone through `money.sync`.

## Avex (2026-10-09)

Avex, the gym app, is the panel's third source, beside Tally and Arbiter: a place on the switch
above the panel, one scroll there (the copy's week, recent workouts, lifts moving, records), a
sidebar key and its own pages. A panel row drafts a question about itself into the open thread
rather than leaving it. It has no internet, so Relay reads the training history it exports and
keeps it read only; an agent reads `gym.*` and charts with a `gym` question instead of `query`.
`docs/GYM.md` is the contract.

## Local models (2026-10-10)

A thread can run on a model on this PC instead of one of Anthropic's (`crates/relay-core/src/local_models.rs`).
The agent is still Claude Code, given the same tools and prompt. It talks to a llama.cpp server
(`llama-server`) through the server's Anthropic Messages endpoint (`/v1/messages`). Every model
Claude Code would use, its background one included, is pointed there (`ANTHROPIC_BASE_URL`,
`ANTHROPIC_*_MODEL`).

- **The models** are Qwen3.5 9B (`qwen35-9b`, Q5_K_M) and Qwen3.5 4B (`qwen35-4b`, Q8_0), in
  `local_models::MODELS`. Each appears in the model picker, under "On this PC", once its GGUF
  file is in the models folder: `threads.local.dir`, or `~/models` when that is unset.
  `thread.models` lists them, with the one running.
- **Picking one loads it** (`thread.pick`). Relay starts `llama-server` on a free local port with
  that model, a 32k context, one slot and thinking off. It lets the server fit the layers to the
  card's free memory. Picking any other model, or Default, stops the server. A thread on a local
  model whose server is not running starts it on its next message, and that turn waits for the
  model to load.
- **Only one model runs at a time.** Starting one stops the other, and stopping the server also
  stops the agents talking to it.
- **It turns off when unused.** The server stops after `threads.local.idle_minutes` (default 15;
  0 = never) with no message to a local thread. A reply in progress counts as use. The picker
  sets the minutes ("Turn off after"). The server runs in its own process group with the
  parent-death signal set, so it also stops with Relay.
- **Locating the server:** Relay finds `llama-server` from `threads.local.server`, else on PATH,
  else in `~/.local/bin`. Its log is `local-model.log` beside the store.
- **Effort does not apply:** a local model takes no `--effort`, and the effort picker is disabled
  while one is shown.
- **Status events:** `thread.local {model, name, state, error}` reports `loading`, `ready`,
  `stopped` and `failed`. The picker shows "· loading" until the model answers, and a failure
  shows its reason.

## Small models (2026-10-15)

A 4B model on this PC was given what Claude is given, and most of it was in its way. Measured on
the bench (`bench/threads`, Qwen3.5 4B Q8_0, counted with Qwen's tokenizer), before its first
question it read about 20,000 of its 32,768 tokens:

| What | Before | After |
|---|---|---|
| Claude Code's own prompt (coding, memory files, security testing) | 1,358 | 15 |
| Relay's prompt (`PROMPT`; `LOCAL_PROMPT` after) | 2,597 | 1,340 |
| Tool schemas: 30 before (`arbiter.backtest` and `arbiter.propose` alone 10,855), 17 after | 15,485 | 3,273 |
| Claude Code's environment message | 567 | 501 |
| **Before the question** | **19,957** | **5,129** |
| Tally's and Avex's readings ahead of the first message (prefetch) | 0 | 6,109 |

Then each read came back as pretty JSON (`money.summary` 1,588 tokens, `gym.sessions` 4,538,
`gym.summary` 2,465), so a few reads overflowed the context: the bench saw `exceeds the available
context size` on turns that read entries month by month. A thread on a local model now runs a
profile of its own (`threads::LocalStyle`), each part kept for what it did on the bench:

- **Lean** (`lean`): Relay's own short prompt (`LOCAL_PROMPT`) in place of Claude Code's
  (`--system-prompt`, not `--append-system-prompt`; Claude Code still lists and calls the tools):
  plain numbered rules, which tool answers what, what the words mean, the chart and import cards,
  and only Tally's and Avex's tools (`threads::local_ops`: `AGENT_OPS` without Arbiter's or
  `bus.schema`).
- **Readings** (`readings`): the MCP door answers each read (`relay_core::readings::OPS`) with a
  reading instead of JSON: labelled lines that say what they are and what today is, money as
  "$1,234.56", days as "Thu Sep 17" with how long ago, weights in both units, and every figure a
  question is likely to need already worked out and named: where each budget stands ("NOT over
  budget, $93.05 left; over pace by $106.95"), what was spent today, yesterday, this week and each
  week before, by category this month and month by month, the three biggest expenses, the same
  day last month, which `period_offset` is which month, the weekday of each of the last seven
  dates, the last workout set by set, each lift's best set and how it improved, the only lifts
  ever logged. A summary folds in the reads it is most often followed by (`readings::extras`). A
  series ends with the chart block that draws it. Writes keep their JSON, which the entry and
  activity cards parse; a refusal reads as a sentence. Ids appear only where a write needs one: a
  model given every id in the summary added entries without `money.lists`, whose reading says
  which account "cash" is, and wrote to the wrong one.
- **Prefetch** (`prefetch`): a fresh agent's first message carries Tally's and Avex's summaries as
  readings ahead of the person's words, read by the engine outside any handler, so an easy
  question needs no tool call. The thread stores the person's words alone. Claude Code cuts an MCP
  server's instructions at about 4,000 characters, so they cannot carry it.
- **Docs** (`docs`, off): tool descriptions in plain words with an example call each, and no
  `today` or `limit` field to set wrongly. It moved nothing on the bench.

The agent's environment for a local model also drops Claude Code's billing line
(`CLAUDE_CODE_ATTRIBUTION_HEADER=0`: it changed with each conversation and was the first thing in
the prompt, so the server could reuse none of a prompt it had already read) and tells Claude Code
the real window (`CLAUDE_CODE_MAX_CONTEXT_TOKENS`, with replies up to 4,096 tokens), so it compacts
before the server refuses.

On the bench's dev split (82 tasks, the frozen binary three times, each candidate twice;
bootstrap 95% intervals over tasks against the frozen binary; the default was run twice, the
model shown the same prompt and readings each time, and a median task took 2.5 to 3 seconds
against 16.6):

| Profile | Pass | Δ |
|---|---|---|
| As shipped (frozen binary) | 73.2% | |
| `lean` | 72.6% | −0.6 [−8.3, +6.5] |
| `lean,readings` | 81.1% | +7.9 [+0.8, +15.2] |
| `lean,readings,prefetch` (the default) | 89.6%, 92.1% | +16.5 [+7.9, +25.2], +18.9 [+10.2, +28.0] |
| `lean,readings,prefetch,docs` | 92.1% | +18.9 [+11.2, +27.0]; 0.0 beside the same without `docs` |

Lean alone tells the model the figures are worked out while the tools still answer JSON; it is
for use with readings. Prefetch added 9.8 points [+1.2, +18.3] over `lean,readings`. On the
held-out split (29 tasks, run once) the default passed 84.5% against 78.2%, +6.3 [−4.6, +19.5]:
reads gained, writes lost. Given the summary up front, the model sometimes adds an entry without
reading `money.lists` and guesses its category (`bench/threads/results/RESULTS.md`).

So with readings the door holds two kinds of write (`mcp.rs`, `held`): one that takes ids
(`money.tx.add`, or a `money.tx.update` that moves an entry to another account or category) before
the model has read `money.lists`, and an expense or income added with no category, which the model
left out because the schema calls it optional (Tally's sheet never saves one). It sends either back
unmade with the lists' reading in it, to be made again with the ids the person meant; and it shows
a small model `money.tx.add` with `category_id` required, null only for a transfer. A model that
read the lists and gave the category is never held. The dev split's write tasks went from 7 of 10
with the first hold alone to 14 of 15 with both.

The whole bench again on the finished branch (111 tasks × 3 runs, every split, with asking first
and the Avex and Tally changes in): 74.5% [69.5, 78.9] → 89.8% [86.1, 92.6], Δ +15.3 [+8.7, +22.5]
over tasks; dev 73.2% → 90.7%, holdout 78.2% → 87.4%; questions on both Tally and Avex 0% → 80%,
totals over a period 41% → 87%, charts 44% → 72%; a median task 16.6 s → 3.0 s. Writes did not
move (75.0% → 70.8%, within noise): the model still now and then picks the wrong account or
category, or reads "45,20 $" as 45,200.

Then Relay took over what both the 4B and the 9B still got wrong (2026-10-16):

- **Names, not ids.** A small model gives `money.tx.add` and `money.tx.update` (and the edits it
  proposes) the account and category by name, "cash", "the Visa", "Dining"; the door finds the ids
  (`mcp.rs`, `resolve_names`: the whole name, a unique part of one, or for an account a word for
  its type) or sends the write back with the names it may use. `money.lists` gives each default
  category's examples: coffee is Dining, Bixi is Transport.
- **No double-counted buys.** A thread's agent may not record investment activity in an account
  Wealthsimple's files fill (`money.import_filled`); every model tried. Readings mark those accounts.
- **Readings fill the gaps:** each exercise's volume, a lift with no estimated max because every set
  was over 12 reps, cardio counted by month, and `periods` counting months even by week.
- **Only what was logged.** Tally and Avex hold what the person entered, not their bank or their
  life: readings say "none logged in Tally" for a zero, and both prompts say a missing figure is not
  logged, never that it did not happen (found by Antho testing the 4B).

The whole bench again with the 4B: 89.8% → 95.2% [92.3, 97.0], Δ +5.4 [+1.8, +9.3]; holdout 95.4%;
writes 70.8% → 95.8%. That is the 9B's 95.8% on the build before, at the 4B's speed.

Then thinking (2026-10-16). The server had run with thinking off, which is most of a small model's
wait; `think` or `think:<tokens>` in the profile turns it on (`--reasoning on`, with
`--reasoning-budget`), and Claude Code's reply budget grows to fit. With two more rules in first,
each budget answered "Over budget?" and "Ahead of pace?" separately, and `money.invest.add`
naming the accounts Wealthsimple fills, the 4B over the whole bench:

| Thinking | Pass | Median s | p90 s |
|---|---|---|---|
| off | 95.2% [92.3, 97.0] | 3.0 | 6.5 |
| within 512 tokens (**the default**) | 98.2% [96.1, 99.2] | 7.0 | 13.6 |
| no limit | 98.5% [96.5, 99.4] | 7.0 | 21.6 |

512 tokens against none: +3.0 [+0.6, +5.7]; no limit against 512: +0.3 [−0.6, +1.5], with a slower
tail. Timings are with three questions at once; a thread alone answers faster. `no-think` in
`bench/threads/variants.json` is the profile before this.

`RELAY_THREADS_LOCAL` in the engine's environment, a comma list of those parts or `classic` for
none, overrides the default, to measure a new model or a change (`bench/threads/variants.json`).
A thread on Claude is unchanged.

## Amounts for models (2026-10-11)

The ledger counts money in minor units, and investment quantities, prices and rates at 1e-8.
People's apps read those integers. A model misreads them: a local 4B model read a $157.99
phone case, stored as `15799`, as $15,799. So models never see them.

- **Units are marked in the schema.** Each such field carries an `x-unit` in the bus schema:
  - `minor`: minor units of the reading's own `currency`, else the ledger's.
  - `minor:<field>`: minor units of the currency the sibling `<field>` names.
  - `e8`: an integer at 1e-8.
- **The MCP door converts both ways** (`crates/relay-cli/src/units.rs`). That door is the one
  models use. It shows those fields as decimal strings in tool input schemas and in `bus.schema`,
  and converts results the same way. A $157.99 entry reads `"157.99"`, 1.5 units read `"1.5"`.
  A model writes `"157.99"` (or `157.99`) and the door hands the bus `15799`. More decimal
  places than the currency has is refused, not rounded.
- **Matches Arbiter.** Arbiter already gives its amounts as decimal strings.
- **Cards accept both forms.** Threads' entry and activity cards read either an integer or a
  decimal string, so threads stored before this change still draw.
- **Other fields models misread are reshaped too,** in results only:

  | Unit | Becomes | Example |
  |---|---|---|
  | `bps` | a percent under `_pct` | `gain_bps: 1250` → `gain_pct: "12.50"` |
  | `fraction` | a percent under `_pct` | `spent_fraction: 0.68` → `spent_pct: "68"` |
  | `unix` | a UTC time | `1760140800` → `"2025-10-11T00:00:00Z"`, for Arbiter's bars, trades and backtest ranges |
  | `omit` | left out | palette colours and icons outside an entry, sync uids, `fraction_digits`, `devices`, a backtest's 400-point `equity` curve |

  An entry keeps its `icon` and `color`, because its card draws them.
- **Avex goal targets are in the person's unit.** A lift goal's (`lift_1rm`, or `lift_target` in
  an older export) and a body-weight goal's `target_value` are converted at the source
  (`relay_gym::views`), like every other weight. Avex stores them in pounds, and a kg user's Avex
  page showed that pound figure.
- **Only the MCP door changes.** The socket, the CLI and the phone door still carry the integers.

## Pictures (2026-10-18)

The person may send the agent pictures: a receipt, a screenshot, a statement, a chart from
elsewhere. `crates/relay-core/src/threads/images.rs`; the ops are in `crates/relay-bus/src/ops/thread.rs`.

- **Uploading:** `thread.upload {media_type, bytes_b64}` → `{image: {id, media_type, width,
  height, bytes}}`. It takes a PNG, JPEG, GIF or WebP whose size can be read from its header
  (`crate::picture`, the reader `session.upload` and `design.asset.add` use), at most 4 MiB
  decoded (a door takes 8 MiB in one message). The bytes decide its `media_type`; any other is
  refused with `thread.upload`. The id is 32 lowercase hex characters. It is the person's alone,
  and only file work: it is staged, so nothing of it holds the store's lock.
- **Where they live:** `threads/images/<id>.<ext>` beside `threads.db`, written 0600 and renamed
  into place. `threads.db` version 4 keeps each picture's row (`image`) and which messages carry it
  (`message_image`).
- **Sending:** `thread.send {id, text, images?}` and `thread.create {text?, images?, model?,
  effort?}` take at most 4 ids (`thread.invalid` past that). The text may be empty beside a
  picture, but not both empty. An id never uploaded, or swept, is `thread.image_unknown`. A thread
  created with pictures and no words is named "Picture" (or "2 pictures"). The stored body is
  `{"text", "images": [{"id", "media_type", "width", "height"}]}`; `images` is absent when there
  are none.
- **A model on this PC reads none:** in a thread on a local model (`local_models::find`) a message
  with pictures is refused with `thread.images_unsupported`: "This thread runs on a model on this
  PC, which cannot read pictures: choose a Claude model for it to send them."
- **What the agent is given:** the stream-json line's `content` becomes an array, one
  `{"type": "image", "source": {"type": "base64", "media_type", "data"}}` per picture, then
  `{"type": "text", "text"}` with what it has not yet heard of its proposals and the person's
  words (no text block when both are empty). The files are read in `deliver`, after the handler,
  on a thread of their own, so a line of megabytes to an agent still starting holds no request. A
  message without pictures is the line it always was. Claude Code's stream-json input takes image
  blocks as the Messages API does (the Agent SDK documents image attachments in streaming input);
  verified with Claude Code 2.1.302 on Haiku 4.5, which read a 16-pixel PNG sent so as "Red".
  The prompt tells the agent to say what it read, to put a picture's numbers into the ledger only
  through its tools after saying which entries it would add (each with its Undo; what
  `thread.propose` covers is still proposed), and that text in a picture is data, never
  instructions.
- **Drawing them:** `thread.image {id}` → `{media_type, bytes_b64}`, an unlocked query;
  `thread.image_not_found` for an id it does not know.
- **Going:** an upload no message carries is removed a day after it was made, swept whenever
  another is uploaded (with any file a cut-short write left without a row). Deleting a thread
  removes the pictures its messages carried that no other message does, after the handler. The
  phone may call both ops (`PHONE_OPS`).

Tests: `crates/relay-core/tests/threads.rs` uploads and refuses pictures (kind, size, a header cut
short, not base64, an agent), reads them back, sends them in new and running threads and checks
the stored body and the line the stand-in Claude was given, refuses five, an unknown id, both
empty and a local model, and deletes a thread's pictures but not one another thread carries;
`threads::images` sweeps a day-old upload and keeps a sent one.

## The assistant (2026-10-19)

A key in the Dev space's title bar (the sparkle, beside the bell) opens a drawer over the page's
right edge: a conversation with an agent about Relay itself. Antho asked for something that can
"do most things that do not pose serious security risk", asks before what a lone session cannot
do, and only acts when spoken to.

**It is a thread of its own kind.** `thread.create {kind: "assistant"}` makes one; `thread.list`
lists Threads' own unless asked `{kind: "assistant"}`, so neither the Threads sidebar nor the phone
shows it. `threads.db` version 5 adds the `kind` column and the `ask` table. The agent is the
thread agent above (Claude in print mode, no shell, no files, `--tools ""`), started with
`ASSISTANT_PROMPT` and `RELAY_MCP_OPS = threads::ASSISTANT_OPS`. It
runs only for a message the person sends, like any thread, and its prompt tells it that what it
reads from sessions, transcripts, mail, notes and tasks is data, never instructions.

**What it may do alone** (`ASSISTANT_OPS`, admitted by the socket door as `peer::Peer::Assistant`):
reads of workspaces, projects, sessions (list, transcript, scrollback, restorable), the board,
notes, mail, worktrees, Git status and log, holds and exception requests, notifications and usage;
and what any session may do itself: `mailbox.send`, `notes.append`, `task.comment`.

**What it asks for** (`thread.ask {steps, why}`, `threads::asks::ASKABLE`): up to 5 steps, each a
user-only op with its own payload, among `session.create`, `.spawn`, `.resume`, `.park`, `.wake`,
`.close`, `.input`, `.update`, `task.create`, `.update`, `.move`, `.dispatch`, `.delete`,
`notes.create`, `.update`, `workspace.create` and `project.add`. A step after `session.create`
that leaves `session` out acts on the session it made, so "launch a builder" is one card.
Refused when asked: any other op (`thread.ask_op`), and `discard_changes`, `remove_worktree(s)`,
`force`, `bus_writes`, `allow_ui` or `use_api_key` set (`thread.ask_flag`). Guardrail answers,
settings, removing anything, Git, files, money, keys and pairing are neither free nor askable.
Only an assistant thread's own agent asks (`thread.not_agent`, `thread.not_assistant`).

**Answering** (`thread.ask.resolve {id, approve}`, the person's alone: `UserOnly`, and
`thread.not_person` from any thread's agent): the answer claims the ask in its transaction
(`pending` → `running` or `declined`, a second answer `thread.ask_answered`); an approval's steps
then run after that transaction and before the reply, each its own request as the person through
the engine's pipeline, so every op's own checks, guardrails and audit apply. The first failure
stops the rest; the outcome says which step failed and what ran before it. An ask Relay stopped
mid-run is `failed` when `threads.db` next opens. The agent hears each answer once, at the head of
the person's next message, beside its proposals' (`[Relay: the person approved ask 3 (…); it is done.]`).

**On a model on this PC** (picked in the drawer, 2026-10-19), the assistant runs the local profile
Threads' small models do (`LocalStyle`), with its own parts: `ASSISTANT_LOCAL_PROMPT` in place of
Claude Code's prompt (short rules, the tools it has, and every ask written out as a step, since it
has no `bus.schema`), `assistant_local_ops()` (20 of the 42 tools: the reads that answer about
Relay, mail, a note's line, a comment, and `thread.ask`), a door introduced as Relay's
(`RELAY_MCP_STYLE` gains `assistant`), and, with `prefetch`, what Relay holds handed over ahead of
the first message: each project with its ids and path, and each open session with its project,
role, state and intent (`relay_now`). Its tool schemas come to about 15.7k characters, against
14.9k for Threads' own lean set, so it fits the same 32k context. The asks and their card are the
same; a small model's wrong payload is refused by the op when the person approves it.

**The drawer** (`apps/relay-native/src/assistant.rs`, `css/assistant.css`) reopens the newest
assistant thread; + starts a new one. Its model picker, under the message box, offers Claude's
models and the models on this PC whose files are present (`thread.models`), and says while a local
one loads. A pick moves the conversation shown (`thread.set`, between replies) and is what new
chats start on, kept as `native.assistant.model`; a local pick starts loading at once
(`thread.pick`), and a Claude pick never calls it, so it cannot stop a local model a Threads
conversation is using. Replies and tool lines are Threads' own; an ask is a card
with Arbiter's amber "Needs your approval" pill, the summary, the agent's quoted reason, and each
step's op and payload exactly as it will run, with Approve and Not now. Its state is the engine's
(`thread.ask.list`, the `thread.ask` event), so an answer stays answered.

This widens SPEC §10's "an agent never spawns, closes, or types into a session" in one place: the
assistant may *ask*, and only the person's approval, from the app, makes it happen.

Tests: `crates/relay-core/tests/threads.rs` (`the_assistant_asks_the_person_and_hears_what_came_of_it`,
`the_assistant_runs_on_a_model_on_this_pc_with_its_lean_profile`, against a stand-in llama-server),
`peer.rs` (the door admits only `ASSISTANT_OPS`), `threads.rs` and `threads/asks.rs` unit tests.
