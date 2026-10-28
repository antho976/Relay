# Aide: a personal agent that works while you are away

Aide is Relay's answer to Grok Bot and Claude Duet: one agent that keeps working when nobody is
typing to it, watches what Antho's apps are doing, messages first when something needs him, and
asks before anything that matters. It is for Antho alone and is never shared. It runs on Claude
through Antho's own Claude account (the CLI's sign-in, as threads do) and on models Antho runs
himself, on this PC or on his own server, chosen per job. Researched and built 2026-10-27;
"What was built" at the end says where each part lives and where it differs from the plan.

Decided with Antho on 2026-10-27:

- **Aide** is the name.
- **His own account.** Claude runs through the CLI's sign-in, never an API key.
- **Both local and cloud**, per job, with a fallback. A source can be kept to his own models.
- **No virtual computers.** No cloud VM, no browser: Aide works through Relay's typed ops.
- **Ready for his own server.** "Local" is any model he runs: one Relay starts on this PC, or one
  on a server or a small farm of his, reached by its address (see "Your own server").

Also decided in the plan:

- **Single-player.** No teams, sharing, seats or marketplace. That removes most of what Grok Bot
  charges for and none of what Duet's reviewers liked.
- **The PC is the home.** Memory, history and every run's record live in Relay's files on the PC.
  Nothing goes to a server of ours. A cloud model sees what a job sends it, and nothing else.
- **The model is a setting, not the product.** Every job names its model and fallback; every run
  says which ran it. Relay never routes silently (Grok Bot's router was its most criticised part).
- **Rules wake it; the model thinks.** Clocks and data rules decide *when* to run, without a
  model. A model runs only when there is something to read or decide. That keeps it cheap on a
  subscription and small enough for a local model.

## What exists elsewhere

**Grok Bot** (SpaceXAI, formerly xAI; beta 2026-08-11; $200/month individual through Cursor
Ultra, $300 with SuperGrok Heavy). Standing "Bots" with a job each, every one on its own cloud
computer that keeps working when the laptop is closed. Bots sign in to websites and use their
screens, so they need no API. A Bot learns a routine by watching you do it once, then reruns it
and folds in your corrections. Several Bots can share a thread, hand work over, and report to a
"Chief of Staff" Bot. Memory across conversations; scheduled routines; phone and desktop on the
same conversation. Models are routed for you and cannot be chosen.

**Claude Duet** (Anthropic; research preview 2026-10-21; included in Pro and Max, US, waitlist).
A separate phone app and claude.ai page on Opus 5.5. It starts with the account's Claude memory,
skills and connectors, checks connected accounts every few hours, and messages first: a morning
brief, calendar clashes, stalled email threads. One main thread plus **channels** (one memory
across them); a **Tasks** tab of its work and what waits on you, where only you can mark a task
done; a **Make** tab of outputs it keeps current (briefs, dashboards, decks) whose figures link to
their sources. It pauses before sending, paying, signing or deleting, and the card shows the
destination, the content and the recipients while it carries on with the rest. Some actions are
refused even with approval (trading securities, moving money between accounts). A check outside
the model compares each action with the request; credentials never reach the model. Cloud
computer and browser, which you cannot watch or take over.

**Also in the field:** Meta Muse (2026-09-08; cloud VM, reaches out first, in WhatsApp; may train
on your conversations), OpenAI dots (Pro, $100+; you can watch and take over its browser; better
email drafts than Duet per Every), and OpenClaw, the open-source self-hosted one (the local
option, and the cautionary tale: "insecure by default" per Gartner, ClawJacked let websites take
over local agents, CNCERT warning 2026-03-14, and a MacStories writer's $3,600 month).

## What people love, what they hate, what Aide does about it

From launch coverage and early reviews (Every / Dan Shipper, Claire Vo, Peter Yang, Flavio Copes,
Phil Simon, VentureBeat, Unite.ai, DataCamp) and OpenClaw's security coverage. Duet is days old,
so these are early impressions, not a fair side-by-side test.

| They love | Aide |
|---|---|
| It messages first: the morning brief (Yang: "the most proactive agent he has tried") | A brief is the first job, and every job may message first |
| Zero setup: memory and connectors carried over (Vo: "zero problems getting up and running") | It starts with everything Relay already holds: sessions, tasks, Git, Tally, Arbiter, Avex |
| Channels for topics, one memory across them | Channels are threads (Relay already keeps many threads); memory is one store shared by all |
| Tasks: its work apart from yours, and only you close one | An errand list; the agent may only propose "done" |
| Outputs that stay current, figures linked to sources | Living pages that store their question, not their numbers (threads' charts already work this way) |
| Approval cards with the destination, content and recipients; it keeps working meanwhile | The same card, from the policy below; other steps carry on |
| Hard refusals no approval can override | A refused list the agent and its settings cannot change |
| Grok Bot: teach it once by showing it | Later (phase 4); a routine is first written as steps it can be shown and corrected on |
| Grok Bot: always on, even with the laptop shut | The PC runs it; "PC off" is an open question below |
| The same conversation on the phone and the desktop | Relay's phone app already mirrors threads |

| They hate | Aide |
|---|---|
| Long jobs go quiet; nothing pulls you back, so you forget them (Vo, the Warriors tickets) | Every job has a promised time. Late or stuck, it says so; a waiting card nudges once, then lands in the brief |
| Checks every few hours, so urgent things wait (Copes, Duet's docs) | Data rules fire on Relay's own events within a second; a clock is only for what has no event |
| Background checks that find nothing still burn the plan (Copes; OpenClaw's $3,600) | A rule decides first, without a model. "Nothing new" costs nothing. Each job has a budget and a monthly count |
| A polished brief silently leaves out a source or turns a suggestion into a deadline | Every brief lists each source read and each that failed. Every claim names the row it came from |
| You cannot pick the model; the router guesses (Grok Bot) | The model is chosen per job, shown on every run, never switched without saying |
| Scheduled runs cannot use custom connectors (Duet) | One op list for a run started by a message and one started by a clock |
| Disconnecting an app does not erase what it learned (Duet) | Every memory names its source. Removing a source offers to forget what came from it. Memory is a page you can read and edit |
| No desktop, cut off from your computer work (Vo) | Relay is the desktop, and already knows the sessions, tasks and branches |
| Insecure by default, untrusted skills, hijackable (OpenClaw) | No shell and no file writes: the agent has Relay's typed ops only, checked at two doors (as threads are). Text from mail or the web is data, marked as such in the prompt |
| May train on your conversations (Muse) | A job marked local-only never leaves the PC |
| Personal errands are unreliable; buying failed (Vo, Yang) | Aide does not buy. Payments stay on the refused list |
| Verbose replies (Grok) | Short by default; the brief has a fixed shape |

## The foundation

Eight parts. Each is small; together they are what every later job stands on. The order is the
build order.

### 1. Jobs

A **job** is one thing Aide does on its own: "the morning brief", "watch Tally's budgets",
"tell me when an agent has been blocked for an hour". It stores:

- **when** it wakes: a schedule, a data rule, or both (part 2);
- **what it may read and do**: a list of sources and an op list, narrower than the assistant's;
- **which model** runs it, its effort, and its fallback;
- **its budget**: at most N runs a day, at most M minutes a run;
- **its privacy tier** (part 5) and **its approval policy** (part 4);
- **its channel**: the thread its messages go to.

A job is data in the store, made from a sentence by the agent and confirmed by Antho on a card,
like an Arbiter strategy. It can be paused, and every job has an "off" that wins over everything.

### 2. Wakes, without polling

`docs/engine/SPEC.md` forbids polling timers. Aide keeps to that:

- **Schedules** reuse Arbiter's `Entry::Schedule` (`crates/relay-arbiter/src/rules.rs:95`,
  `next_scheduled`): "every weekday at 7", "every Sunday". One timer sleeps until the earliest
  due job across all jobs, then re-arms. One deadline, not a tick. SPEC.md's list of clock-driven
  work gains one line.
- **Data rules** listen on the bus (`bus.subscribe`) for events Relay already sends
  (`money.changed`, `gym.changed`, session and notification events, Arbiter's decisions) and test
  a condition without a model: "a transaction over $200", "a budget past 90%", "a session blocked
  for 60 minutes". Only a true rule starts a run. A rule's evaluation runs off the store mutex
  and reads through `prepare_cached`, as the invariants require.
- **A run may promise a follow-up** ("look again at 5 if he has not answered"). A follow-up is a
  one-shot schedule entry, so it sleeps on the same timer.
- **Missed wakes** while the PC was off run once on start, if still inside a grace window
  (Arbiter's `SCHEDULE_GRACE` is 3600 s). A brief from yesterday is not sent today.

### 3. Runs

A **run** is one wake of one job: why it woke, what was handed to the model, the model and its
effort, the time and tokens it took, what it did, and how it ended (`done`, `nothing new`,
`waiting on you`, `late`, `failed`, `over budget`). Every run is kept and shown, the way Arbiter
keeps every decision with its inputs.

It runs as a thread turn does: a `claude -p` process with Relay's MCP door as its only tools
(`threads.rs`, `agent_args`), pointed at llama-server for a local model (`local_models::agent_env`).
Two rules on top:

- **Background runs never take the person's seat.** Threads allow four live agents
  (`MAX_LIVE`). Background runs get at most one of them and queue behind a person's message.
- **Every run has a deadline.** Past it, the run is stopped and reported `late`, not left
  running quietly. That is the answer to "it went silent and I forgot about it".

### 4. Approvals by kind

Today every proposal and ask needs a click (`threads/proposals.rs`, `threads/asks.rs`); the
assistant's per-kind auto-run (coral-osprey, in progress 2026-10-27) is the first step past that,
and Aide should use it rather than add another. Aide extends Arbiter's three modes
(`docs/ARBITER.md`, "Autonomy") to every kind of action:

| Policy | Means |
|---|---|
| **Free** | Reads, drafts, notes to itself, its own errand list. Never asks |
| **Within limits** | Allowed while inside limits only Antho can set (`Actors::UserOnly`): "up to 3 messages a day", "Tally entries under $50, each with Undo" |
| **Asks** | A card with what, where, to whom, and the exact text. Expires (default a day; a brief's question expires with the brief) |
| **Refused** | Never, whatever is approved: payments, transfers, trades, deleting mail, anything signed, anything that changes guardrails or Aide's own settings |

The card goes to the phone too. `thread.ask.*` is not in `PHONE_OPS` today
(`crates/relay-remote/src/wire.rs:133`), so adding it is part of this phase. An approval is
checked again when it runs (proposals already re-check staleness), and the policy is enforced in
the engine, never in the prompt.

### 5. Privacy tiers

Each source carries a tier that the run checks before it sends anything to a model:

- **Any model**: Relay's dev data, Avex.
- **Local only**: what Antho marks, for example Tally's ledger or mail bodies. A job reading it
  runs on a local model or does not run; with no local model loaded it waits and says so.
- **Never to a model**: keys and tokens. They stay in the keyring and are used by the engine
  (the key proxy's rule, `docs/KEY-PROXY.md`); a connector's token is never in a prompt.

This is the reason to plan for local models now. The tier tells you which jobs are waiting for
local to be good enough, and moves them over one at a time.

### 6. Memory

Threads have no long-term memory today. Each conversation keeps its own history, and the agent
runs with `--setting-sources ""`. Aide gets one store of **facts**:

- each fact is a sentence with its **source** (a message, a Tally row, a mail), when it was
  learned, and when it was last used;
- the agent proposes facts; ones about Antho himself are confirmed on a card the first time;
- a run is handed the few facts that match its job and sources, not the whole store (a small
  model's context is 32k);
- a **Memory** page lists every fact, to edit, pin or forget; removing a source offers to forget
  everything learned from it.

### 7. Speaking first

Every outward message goes through one outbox, with a level:

- **Interrupt**: a phone notification now (urgent, or an approval with a short expiry);
- **Brief**: held for the next brief;
- **Quiet**: only on its page.

Quiet hours hold everything but interrupts. A cap on interrupts per day, set by Antho, keeps a
bad rule from flooding the phone. Messages land in the job's channel and as a notification in a
new `aide` category. The phone announces it (`Notifier.kt`'s `LOUD` list), and Reply answers
in the thread.

### 8. Errands and living pages

- **Errands**: a list of what Aide is doing and what waits on Antho, each with its promised time.
  Only Antho closes one. It is separate from the dev task board, which is for coding sessions.
- **Living pages**: a brief, a weekly money review, a training summary. Each stores the questions
  that build it and redraws from current data, with every number linked to its row.

## The first jobs: data Relay already has

No new connectors, so the foundation is tested on known data first:

1. **Morning brief** (weekdays 7:00): overnight sessions done or blocked, PRs waiting, tasks in
   review; yesterday's spending against budgets; Arbiter's night (orders, halts); the last
   training session. Every source is listed, each marked read or failed.
2. **Stuck agent** (rule): a session blocked or waiting for an hour while Antho is away.
   Interrupt once, then it goes in the brief.
3. **Budget watch** (rule on `money.changed`): a category past 90%, or a transaction unlike the
   usual ones. Local only.
4. **Arbiter watch** (rule): a halt, a drawdown limit near, an exchange unreachable.
5. **Weekly review** (Sunday): money, training and dev, as a living page.

Each one goes into `bench/threads` as tasks before it may run on a local model. The bench shows
how far local still is. The small-model profile scores 98.2% on the reading tasks with Qwen3.5 4B,
but the harder "v2 world" suite, multi-step and closer to these jobs, is at 66% for the 4B and
72% for the 9B (`bench/threads/results/RESULTS.md`, 2026-10-16). A job moves to local when its
own tasks pass, not before.

## Later phases

- **Phase 2, accounts.** Gmail and Google Calendar first, read-only: OAuth with the refresh token
  in the keyring, used by the engine, never in a prompt. Mail text is marked untrusted in the
  prompt. Sending and replying go on a card first, and stay there until Antho moves them to
  "within limits". Relay has no OAuth today.
- **Phase 3, many jobs, one voice.** Jobs that hand work to each other, with one brief over all
  of them (Grok Bot's Chief of Staff, without a second chat to read).
- **Phase 4, show it once.** Record a routine as steps, rerun it, and correct a step. Websites and
  a browser come last, and only once Antho can watch the browser and take it over (Duet's most
  criticised gap).

## Relay's rules this has to keep

- Handlers hold the store mutex inside one transaction (BUS.md §5.1). Runs, rule checks and model
  calls happen outside it; only reading inputs and saving results go through the store.
- Every subprocess goes through `proc::output_with_timeout`, a run's agent included (its deadline
  is that timeout).
- `session.input` stays off SQLite. Aide never sits on the path of a keystroke.
- New tables are a migration in `store::MIGRATIONS` and a `SCHEMA_VERSION` bump.
- Aide's op list is enforced at the MCP door and the socket door, like `ASSISTANT_OPS`.

## What was built

**The rules: `crates/relay-aide`** (no engine, tested alone). `model.rs`: jobs (`JobSpec`, checked),
schedules, data rules, runs, facts, errands, messages, follow-ups, living pages, settings with
privacy tiers and your servers (`Endpoint`). `schedule.rs`: the next wake in the PC's time zone,
through daylight saving, and what a PC that was off missed. `policy.rs`: which host a model is on
(Claude, this PC, your server), the model a run gets (`choose`: the job's model, its fallback,
Aide's, Aide's fallback, the first that may read every source and is available), and the ops a run
is given. `store.rs`: `aide.db` beside the store. `templates.rs`: the five first jobs.

**The engine: `crates/relay-core/src/aide*`.** `aide.rs` starts two threads from `serve`: the
scheduler (one wait until the earliest thing due, woken early by any change, capped at 15 minutes
only as a guard against a suspended PC) and the watcher (the engine's events; data rules after a
1.5 s settle). `runs.rs` queues, starts and ends runs; `rules.rs` checks data rules without a
model; `outbox.rs` is everything Aide says; `models.rs` lists and reaches the models. The ops are
`aide.*` (`crates/relay-bus/src/ops/aide.rs`, handlers in `handlers/aide.rs`), all the person's.

**A run** is a `wake` turn in its job's channel, a thread of kind `aide` (`threads::AIDE`), on the
agent every thread uses: Claude Code with Relay's MCP door as its only tools, Aide's prompt
(`runs::PROMPT`, or `LOCAL_PROMPT` for a small model on this PC), and only its sources' reads plus
Aide's own ops (`RELAY_MCP_OPS`; the socket door admits an Aide agent, `Peer::Aide`, to Aide's ops
alone). The wake carries the job's instructions, confirmed facts, open errands, what was held for a
brief (scheduled runs only), and each source read now, marked read or not. It ends when the agent
calls `aide.run.finish`, when the turn ends without it (done), or at its minutes (late: stopped,
and an interrupt says so). One run holds an agent at a time, so a person's conversation never
waits on Aide, and a channel the person is typing in keeps its run queued.

**Where it differs from the plan:**

- **Each run starts a fresh conversation** (its `agent_ref` cleared). Cost does not grow with each
  run, and what one of your own models read is never resumed on Claude's. Continuity is what Aide
  keeps: facts, errands, follow-ups and held messages. Talking in a channel after a run continues
  that run's conversation.
- **Approvals.** Aide has no outward action to approve: it cannot pay, trade, delete, send to
  anyone but you, or change settings (`policy::REFUSED`); those ops are not in any run's list. What
  needs you is a job it proposes (does nothing until turned on), a fact it learned (not given to
  runs until confirmed), and an errand it calls done (you close it). Interrupts are its limited
  rung: the daily cap and quiet hours, past which they are held for the brief. Asking for a Relay
  action (the assistant's `thread.ask`) is left for later.
- **A brief is any scheduled run**: it carries what was held, and marks it briefed.
- **Rules built**: spend over an amount, budget past a share, an agent waiting N minutes, a Relay
  notification of a category (never Aide's own), an Arbiter halt or unreachable exchange, a new
  Avex upload. A rule's first look takes what is already there as seen.
- **No connectors yet** (Gmail, Calendar): they need an OAuth client of Antho's own. Phase 2.
- **The phone** shows Aide (Now, Jobs, Memory) and rings for its interrupts; its settings stay on
  the PC, so a lost phone cannot point Aide at another server or open private data to Claude.

## The ops

All are the person's (`Actors::UserOnly`); Aide's own agent calls the ones in its run's list as
the person, and the engine tells its calls apart by their thread (`Ctx::thread`). Event:
`aide.changed`.

| op | what | from Aide's agent |
|---|---|---|
| `aide.summary` | jobs, the next wake, runs now, what waits on you, held, interrupts today | — |
| `aide.settings.get` / `.set` | paused, default model and fallback, quiet hours, the cap, tiers, your servers | — |
| `aide.models` | every model a job may name, each asked whether it can run now | — |
| `aide.job.list` / `.get` / `.templates` | jobs with what wakes them; one with its runs; the starter jobs | `list` |
| `aide.job.save` / `.set` / `.delete` | make or change a job; turn on (approving a proposal) or pause; remove | `save` only as a proposal |
| `aide.job.propose` | a job that waits for you to turn it on | yes |
| `aide.run.now` / `.stop` / `.list` / `.get` | run a job now; stop a run; runs, newest first | — |
| `aide.run.finish` | how this run ended: done, nothing, waiting, failed | its own run only |
| `aide.notify` | say something: interrupt, brief or quiet | yes |
| `aide.followup` | wake this job again in N minutes (five waiting at most) | its own job only |
| `aide.fact.list` / `.add` / `.update` / `.forget` | what it learned; forget one, or all from one source | `list`, `add` (unconfirmed) |
| `aide.errand.list` / `.add` / `.update` | what it is on and what waits on you | yes; its "done" asks you |
| `aide.page.list` / `.get` / `.write` / `.delete` | living pages | `list`, `write` |
| `aide.message.list` | what it said | — |

## Your own server

A job may name a model on a server of yours: add it in Aide's settings (`aide.settings.set`,
`endpoints`) with an id, a name, its address (`http://host:port`, no path), the model name the
server knows it by, and its context. The server must answer **Anthropic's Messages API**
(`POST /v1/messages`), because the agent is still Claude Code, pointed at that address. llama.cpp's
`llama-server` does; so does LiteLLM's proxy in front of vLLM or Ollama, for a farm of several.

For a Qwen3.5 model on llama.cpp, start it as Relay starts its own: Claude Code sends more than
one system message, which Qwen's own template refuses, so give it Relay's copy.

```fish
llama-server -m Qwen3.5-9B-Q5_K_M.gguf --alias qwen35-9b --host 0.0.0.0 --port 8080 \
    -c 32768 -np 1 --jinja --reasoning off --no-webui --no-slots \
    --chat-template-file crates/relay-core/src/local_models/qwen3.5.jinja
```

Keep it on your own network: Aide sends it whatever its jobs read, and Relay sends no key (a
server that wants one reads it from its own configuration). `aide.models` says whether each
server answers; a run whose server is down falls back to the next model allowed to read its data,
or does not run and says why.

## Checked on 2026-10-27

- `cargo test` (2,300+ tests) and `cargo clippy --all-targets -D warnings`; the engine tests in
  `crates/relay-core/tests/aide.rs` drive jobs, runs on a stand-in Claude, privacy tiers, the
  outbox, what Aide's agent may and may not do, a spend rule, and a missed schedule.
- The phone: `./gradlew :core:test :app:testDebugUnitTest :app:lintRelease`.
- One real run of a test job on a disposable engine on each host, each calling `aide.notify`,
  `aide.fact.add` and `aide.run.finish` through Relay's MCP door: Claude Haiku 5.5 through the CLI's
  sign-in (2 s), Qwen3.5 4B on this PC started by Relay (10 s with its load), and the same model
  on a llama-server Relay did not start, as a server of yours (4 s). The first Claude run found a
  bug no test had: the socket door refused Aide's MCP server the `bus.ops` it lists tools from.

## Open questions for Antho

1. **The PC off.** Nothing runs then: a schedule missed by less than an hour runs on start, an
   older one waits for its next time. Once your own server exists, the engine could run there
   with Aide on it, but the dev side (repositories, terminals) lives on the PC.
2. **Which accounts first**, once connectors come: Gmail and Calendar are the usual two.
3. **Which sources are yours only** from the start: every tier is "any model" until you set one.
4. **May it act on the dev side**: start a session on a task, for example, through an ask.

## Sources

- Grok Bot: [VentureBeat](https://venturebeat.com/orchestration/spacexais-grok-bot-turns-agents-into-persistent-digital-coworkers-that-can-operate-your-apps-for-120-per-month),
  [Unite.ai](https://www.unite.ai/xai-launches-grok-bot-always-on-ai-teammates-with-their-own-cloud-computers/),
  [Layer3 Labs](https://www.layer3labs.io/guides/what-is-grok-bot),
  [BEAM](https://beam.ai/agentic-insights/grok-bot-enterprise-ai-agents)
- Claude Duet: [DataCamp](https://www.datacamp.com/blog/claude-duet),
  [Flavio Copes](https://flaviocopes.com/claude-duet/),
  [ChatPRD / Claire Vo](https://www.chatprd.ai/how-i-ai/claude-duet-review),
  [DEV, early reviews](https://dev.to/studio1hq/claude-duet-features-pricing-early-reviews-and-limitations-4i53),
  [Lenny's Newsletter](https://www.lennysnewsletter.com/p/how-i-ai-claude-duet-honest-review),
  [SmartScope](https://smartscope.blog/en/blog/claude-duet-features-pricing-waitlist-requirements-availability-differences-2026/)
- Muse and dots: [Axios](https://axios.com/2026/09/08/meta-debuts-muse-personal-ai-agent),
  [SmartScope](https://smartscope.blog/en/blog/meta-muse-differences-chatgpt-features-pricing-comparison-claude-2026/)
- OpenClaw: [Microsoft Security](https://www.microsoft.com/en-us/security/blog/2026/02/19/running-openclaw-safely),
  [Security Boulevard](https://securityboulevard.com/2026/03/latest-openclaw-flaw-can-let-malicious-websites-hijack-local-ai-agents/),
  [TechBullion](https://techbullion.com/openclaw-has-230000-github-stars-and-a-security-problem-this-is-how-the-industry-is-responding/)
