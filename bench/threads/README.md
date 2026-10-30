# Threads model bench

How well does a small local model answer questions about Tally (the budget, entries, bills,
accounts, goals, investments) and Avex (the gym history) through Relay's Threads? This bench asks
it a fixed set of questions in a fixed world, grades every answer against values computed from that
world, and keeps the result, so a change to what the model is shown, or a new model, can be measured
against the same yardstick. Arbiter is out of scope.

Everything the model meets is the product's own path: a real thread (`thread.create`), Relay's
Claude Code agent with its arguments and appended prompt (`crates/relay-core/src/threads.rs`), the
MCP door with its decimal amounts (`crates/relay-cli/src/mcp.rs`, `units.rs`), and the engine's own
local-model start (`crates/relay-core/src/local_models.rs`). The bench only chooses which GGUF that
start loads, how many slots it gets, and the date.

Python 3, standard library only. No GPU is needed for the unit tests.

## A new small model, in short

```fish
cargo build -p relay-cli
python3 bench/threads/bench.py run --gguf ~/models/NewModel.gguf --runs 3 --append
python3 bench/threads/bench.py compare bench/threads/results/2026-10-15-qwen35-4b-after.json bench/threads/results/<the new file>.json
```

A Qwen3.5 GGUF keeps Relay's patched template; any other renders with its own, and one whose template
refuses Claude Code's second system message needs `--template` (see "Any model"). Three runs of 111
tasks take about 20 minutes on the 4070 with a 4B. To use the model in Relay afterwards it still has
to be added to `local_models::MODELS`.

## Where it stands (Qwen3.5 4B Q8_0, RTX 4070, 111 tasks × 3 runs)

| Build | Pass [95% CI] | Dev | Holdout | Median s |
|---|---|---|---|---|
| As Threads first shipped (`ff599ba`) | 74.5% [69.5, 78.9] | 73.2% | 78.2% | 16.6 |
| Small-model profile, ask first, Avex/Tally rework (`a24ed13`) | 89.8% [86.1, 92.6] | 90.7% | 87.4% | 3.0 |
| The same build on Qwen3.5 9B Q5_K_M (one slot) | 95.8% [93.1, 97.5] | 96.3% | 94.3% | 2.5 |
| Relay-side fixes: names for ids, nothing logged, no double-counted buys (`6d3e804`) | 95.2% [92.3, 97.0] | 95.1% | 95.4% | 3.5 |
| Over-budget yes/no, accounts named in the tool (`0e5caa7`), no thinking | 95.2% [92.3, 97.0] | 95.5% | 94.3% | 3.0 |
| The same, thinking within 512 tokens (the default from here) | 98.2% [96.1, 99.2] | 98.8% | 96.6% | 7.0 |
| The same, thinking with no limit | 98.5% [96.5, 99.4] | 98.8% | 97.7% | 7.0 |

Δ +15.3 points, bootstrap 95% over tasks [+8.7, +22.5]. What changed is in `docs/THREADS.md`
("Small models"). The holdout split was looked at twice while the profile was built (once for the
profile, once to find the write problem the category hold fixes), so its number is a little less
blind than its name. Still weakest: writes (70.8%: a wrong account or category, a French decimal
comma read as thousands, an activity added to a Wealthsimple-filled account) and charts (72.2%).

### v2, the harder suite (84 tasks × 3 runs, its own world; see "v2")

| Model and setting | Pass [95% CI] | Dev | Holdout | Median s |
|---|---|---|---|---|
| Qwen3.5 4B, no thinking | 57.9% [51.8, 63.9] | 55.6% | 65.1% | 6.0 |
| Qwen3.5 4B, thinking ≤512 (the default) | 66.3% [60.2, 71.8] | 63.5% | 74.6% | 15.6 |
| Qwen3.5 9B, thinking ≤512 (one slot) | 72.2% [66.4, 77.4] | 67.7% | 85.7% | 12.1 |

v1 is saturated (98.2%) and stays as the regression guard; v2 is where a change or a new model now
shows. Weakest for the 4B: dates and periods from the 15th (36%), questions on both apps (20%),
refusals around the note that gives orders (44%), totals (57%).

**Two grading fixes (2026-10-29).** The grader read a French decimal comma as two numbers ("109,15 $"
as 109 and 15), so `v2.tally.period.dining_fr` failed correct answers; and
`v2.tally.multi.recat_then_undo` wanted `category_id` where `money.tx.update` takes the category by
name too (it has since `0e5caa7`). Graded again with `bench.py regrade`, which changes only
fail → pass, every v2 row above gains: Qwen3.5 4B thinking ≤512 66.3% → **68.7%**, no thinking 57.9% →
59.1%, Qwen3.5 9B 72.2% → 73.4%, Gemma 4 E4B 62.7% → 63.9%, Qwen3.8-4B-Distill 54.4% → 56.7%, LFM3
1.7B 57.9% → 58.7%. The tables keep the numbers as first measured; compare a new run with a baseline
graded by the same checks (`regrade --out`).

### Other 4B models (2026-10-19, relay `0e5caa7`, thinking ≤512, Q8_0, 3 runs)

| Model | v1 | Δ vs Qwen3.5 4B | v2 | Δ vs Qwen3.5 4B |
|---|---|---|---|---|
| Gemma 4 E4B (`unsloth/gemma-4-E4B-it-GGUF`, one slot: 8.2 GB) | 94.9% [92.0, 96.8] | −3.3 [−6.3, −0.6] | 62.7% [56.6, 68.4] | −3.6 [−9.9, +2.8] |
| Qwen3.8-4B-Distill (`empero-ai`, a community fine-tune of Qwen3.5 4B) | 92.5% [89.2, 94.9] | −5.7 [−8.4, −3.0] | 54.4% [48.2, 60.4] | −11.9 [−18.7, −5.2] |
| Nemotron 3 Nano 4B (`unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF`) | 79.0% [74.3, 83.0] | −19.2 [−24.9, −13.8] | – | – |

Δ is the bootstrap interval over tasks (`compare`). None beats Qwen3.5 4B. Gemma is closest and ties it
on v2 overall, but loses on difficulty 3 (36.2% against 48.6%). The distill, trained on Qwen3.8 math
and reasoning traces, lost the base model's habits: on v2, asking first 26.7% (73.3%), writes 56.7%
(86.7%), difficulty 3 27.6%. Nemotron answers lookups well and fails what is not there (refusals 6/18)
and stale data; v2 was not run. Its own template renders Claude Code's environment, a system message
after the person's, as the last turn, which it then answers ("ready to help"):
`templates/nemotron-3-nano.jinja` folds later system messages into the system prompt.

### LFM3 1.7B (2026-10-28, relay `0e5caa7`, thinking ≤512, Q8_0, 3 runs, 3 slots)

| Model | v1 | Δ vs Qwen3.5 4B | v2 | Δ vs Qwen3.5 4B |
|---|---|---|---|---|
| LFM3 1.7B (`LiquidAI/LFM3-1.7B-GGUF`, 1.85 GB; 3 slots ≈ 5.5 GB) | 90.1% [86.4, 92.9] | −8.1 [−12.3, −4.2] | 57.9% [51.8, 63.9] | −8.3 [−16.7, −0.4] |

Liquid's own card puts it ahead of Qwen3.5 4B on instruction following and tool calling; here it is
not. It needs llama.cpp b11960 or newer (this ran on 0.8.0-dev `c3df4b7`; the Qwen rows on 0.6.0-dev
`23b0202`). Its own template takes the later system message and answers the person. Lookups, ranks,
times and units hold at 100% on v1; charts fall to 38.9% (no chart block, a bar for a cumulative
line, sets charted for volume), asking first to 61.9%, and on v2 writes to 53.3% (86.7%). It is also
slower: median 11.5 s (7.0) on v1 and 36.6 s (15.6) on v2, because it repeats calls it has already
made (an identical call in 51 of 333 v1 runs, Qwen3.5 4B in 3) and wanders into the other app's
tools (19 calls on "how much do I weigh"); 5 v1 runs ran out of their 32k context (Qwen 0).

## Quick start

```fish
# the 10-task smoke suite, once, on Qwen3.5 4B, against the frozen baseline binary
python3 bench/threads/bench.py run --suite smoke --relay-bin /path/to/relay

# the full suite, three runs, recorded in results/RESULTS.md
python3 bench/threads/bench.py run --runs 3 --append

# no model: start the engine, load the world, print what every task expects
python3 bench/threads/bench.py run --check

# read a result again, or compare two
python3 bench/threads/bench.py report bench/threads/results/<file>.json
python3 bench/threads/bench.py compare A.json B.json

# grade a result's saved replies again with the checks as they are now (effects keep their verdict)
python3 bench/threads/bench.py regrade bench/threads/results/<file>.json --out /tmp/regraded.json

# the unit tests (grader, statistics, wrapper, fixtures, every task file's shape)
python3 -m unittest discover bench/threads
```

`--relay-bin` defaults to `target/debug/relay` of this checkout (`cargo build -p relay-cli`). Point it
at a frozen copy when others may be rebuilding that one while a run is going: the engine, and the MCP
server its agents start (through `RELAY_BIN`), are both that binary. A `COMMIT` file beside a frozen
copy names the commit it was built from; the result records it, with the binary's sha256.

## What you need

- **libfaketime**, in `~/.local/lib/faketime/` (or `/usr/lib/faketime/`, or `RELAYBENCH_FAKETIME`
  naming `libfaketime.so.1`). On Arch it is the `libfaketime` package. Without root:

  ```fish
  git clone --depth 1 --branch v0.9.13 https://github.com/wolfcw/libfaketime
  make -C libfaketime/src all PREFIX=$HOME/.local LIBDIRNAME=/lib/faketime   # `all`: the first target builds only the time64 one
  mkdir -p ~/.local/lib/faketime; install -m 0644 libfaketime/src/libfaketime*.so.1 ~/.local/lib/faketime/
  ```

- **llama-server** (llama.cpp) on PATH or in `~/.local/bin`, or `--server`.
- **Claude Code** (`claude`) on PATH; `RELAYBENCH_CLAUDE` names another.
- The model file: `~/models/Qwen3.5-4B-Q8_0.gguf` by default, `--gguf` for any other.

## The fixed world

- **The date.** The engine, Claude Code and the MCP server all run under libfaketime at
  **2026-09-18 12:00, America/Toronto** (`FAKETIME="@2026-09-18 12:00:00"`, monotonic clocks real,
  file times real), so "today", "this month" and the agent's own idea of the date agree and never
  drift. llama-server runs without it.
- **Tally**: `money.sample`, the sample household for that today and the default seed (122 entries,
  September 2026 the running period), plus the Wealthsimple holdings report of the shared tests
  (`docs/INVESTMENTS.md`, W1) imported into a new TFSA: three holdings as of **2026-05-08**, more than
  four months before today, so investment answers have to say the values are old.
- **Avex**: `crates/relay-gym/tests/avex_export.json` with every date moved back exactly three weeks
  (weekdays kept), so it was exported on 2026-09-17 21:14, the evening before today. 49 sessions,
  in pounds.
- **A throwaway engine** per run: `relay --instance test serve` with its data, socket and
  `CLAUDE_CONFIG_DIR` under a fresh `/tmp/relay-bench/<n>/`, never the user's instance, ledger or
  `~/.claude`. Every variable of the session running the bench that names Relay, Claude Code or a
  fake clock (`RELAY_*`, `CLAUDE*`, `ANTHROPIC_*`, …) is removed first, and `RELAY_BIN` set to the
  binary under test. Everything it starts carries a marker variable, and teardown stops the model
  server, quits the engine, kills whatever still carries the marker, reaps it and removes the folder
  (`--keep` keeps it, to look at `engine.log`, `local-model.log` and the threads).
- **The fingerprint** (`world.fingerprint` in a result) is a sha256 over what the agent could read
  (`money.lists`, `money.summary`, three periods of entries, the portfolio, the training summary and
  lifts) with load-time noise left out. Two runs with the same fingerprint asked their questions of
  the same world. After each task that changes the ledger, the snapshot taken after loading is
  restored (`money.import`) and the fingerprint checked again.

## Any model

The engine runs model id `qwen35-4b` from `Qwen3.5-4B-Q8_0.gguf` in `threads.local.dir`, started
by `threads.local.server`. The bench sets the folder to one holding a symlink of that name to
`--gguf`, and the server to a wrapper (`relaybench/model.py`) that runs the real llama-server with
the engine's own arguments except:

- `-np 1 -c 32768` become `-np N -c N×32768`: N slots, each with the product's 32k context, so N
  read-only tasks run side by side (`--parallel`, default 3, lowered to what fits the card's free
  memory; the 4B Q8 with 3 slots takes about 7.4 GB);
- `--chat-template-file` keeps the product's Qwen3.5 template for a GGUF whose name says Qwen3.5,
  is replaced by `--template FILE` when given, and is dropped otherwise, so another model renders
  with the template in its own GGUF (`--keep-template` / `--no-keep-template` override the guess).

Thinking stays off, as the product sets it. To test a new model:

```fish
python3 bench/threads/bench.py run --gguf ~/models/NewModel-Q8_0.gguf --runs 3 --append
# a model whose own template cannot take Claude Code's second system message:
python3 bench/threads/bench.py run --gguf ~/models/X.gguf --template ~/templates/x-fixed.jinja
```

then `compare` it with the baseline line of `RESULTS.md`. A template that accepts a later system
message can still answer it instead of the person: run `--suite smoke` first and read a reply.
`templates/` keeps the fixed templates the results used.

## The GPU

One card, several people running benches: every run that starts a llama-server first takes an
exclusive lock on `/tmp/relay-bench-gpu.lock` and holds it until its server is gone. A second run
says `waiting for the GPU (held by pid N, since T, …)` and waits; the lock is released on exit,
Ctrl-C, `timeout`/SIGTERM and errors included (the engine also gets SIGTERM if the bench is killed
outright). `--check` takes no lock. A run holds the lock until its server has left the card, and
the next one waits up to a minute for a server still leaving it before it reads the free memory; a
llama-server that stays on the card (Relay's own local model, say) stops the run before it starts.
A CPU-only one (`-ngl 0`) does not count.

Slots are sized to the free memory when the run starts (`--parallel` is the most). Seconds per
task-run are measured with that many tasks side by side, so compare timings only between runs with
the same `parallel` (it is in the result). Pass rates should not depend on it: each slot has the
product's own 32k context, and only the batching differs.

## A task

`tasks/*.json`, each a list (the contract, `bench-contract.md`, is the reference):

```json
{
  "id": "tally.spent.this_month",
  "source": "tally",              // tally | avex | both
  "category": "lookup",           // lookup compare aggregate rank time units pace write ask_first refuse chart stale multi_turn
  "difficulty": 1,                // 1 easy … 3 hard
  "prompt": "How much have I spent this month?",
  "followups": [],                // further user turns, each sent after the reply before it
  "truth": "spent_this_month",    // relaybench/truths.py → named expected values
  "checks": [{"type": "number", "of": "spent", "tol": 0.01}, {"type": "no_error"}],
  "mutates": false                // true: runs alone, the ledger is restored after it
}
```

A truth function, `fn(q, ctx) -> dict`, reads the throwaway engine through the CLI as the person
(`q(op, payload)`, raw integers: minor units, e8, bps) and computes the answer itself where that is
cheap, so a change to what the model is shown cannot move the answer key. It runs before each
task-run. An `effect` check names a truth that runs twice: before the turn (its answer is kept) and
after it with `ctx["before"]` and `ctx["thread"]`, returning `{"ok", "why"}`.

The checks (all must pass; each is scored too):

| type | passes when | keys |
|---|---|---|
| `number` | a number in the reply is the truth's value | `of` (a key, or keys), `any_of` (keys or literals), `value`, `tol` (absolute, default 0.01), `signed` (default: the magnitude is compared, since "$50 over budget" says −50 in words) |
| `text_any` / `text_all` / `text_none` | case-insensitive substrings or regexes are (not) in the reply | `values`, `of` (the truth's strings), `regex` |
| `tool` / `no_tool` | an op was (not) called | `ops`, `args` (subset: numbers and decimal strings by value, strings without case, `{"$truth": key}`), `min`, `ok_only` |
| `effect` | the effect truth says so | `truth` |
| `chart` | a ```` ```chart ```` block holds JSON matching a subset | `match`, or `any_of` subsets |
| `no_error` | no error message and no timeout | |

Text, number and chart checks read the last reply (`"turn": "all"` or an index for another); tool
checks read every turn. Numbers are read from "$1,234.56", "1234.56", "1,234.56 CAD", "-$12.30",
"($12.30)", "$1.2k", "12.5%"; dates, clock times and the JSON of chart and import blocks are not
numbers.

Before running a model on new tasks: `bench.py run --check --tasks 'new.*'` prints each task's
expected values and fails on a missing truth or a check naming a key the truth does not return.
`python3 -m unittest discover bench/threads` checks every task file's shape.

## Splits

A task is **holdout** when `int(sha1(id).hexdigest(), 16) % 10 < 3` (about 30%), **dev** otherwise:
fixed by the id alone, forever. Work on making the data easier to read looks at dev failures only;
holdout says whether the gain is real or fitted to the tasks looked at. `--split dev|holdout|all`
runs one; reports and comparisons always show both.

## Results

`results/<time>-<name>.json`, one per run: the model (file, size, sha256 of its first 64 MiB, the
template), the variant, the relay binary (path, sha256, commit), Claude Code's and llama-server's
versions and the server's arguments, the GPU, the anchor, the world (counts, fingerprint), and per
task per run: pass, every check with why, each turn's reply, tool calls (op, input, the start of the
answer), seconds, tokens (from Claude Code's own transcript: model calls, prompt and completion
tokens, the largest prompt), errors and timeouts. Beside it, `<same>.jsonl.gz` keeps each thread's
messages whole.

The summary: pass rate overall and by split, source, category, difficulty and check type; a Wilson
95% interval over task-runs and a bootstrap 95% interval over tasks (the runs of one task are not
independent, so the task is what is resampled: 4000 resamples, seeded); each task's stability (k of
n), always / never / flaky; median and p90 seconds per task-run; timeouts; tokens.

`compare A B` gives B against A on the tasks both ran: the delta overall with a bootstrap interval
over tasks of the per-task difference, the same by split, source, category and difficulty, and the
tasks that flipped (fixed: below half the runs to at least half; broken: the reverse). An interval
that holds 0 is not a difference.

`RESULTS.md` has one line per committed run (`--append`, or `report --append FILE`).

Turn timeout is 240 s (`--turn-timeout`); a timed-out turn is stopped (`thread.stop`) and fails.
Six task-runs in a row ending in the same error with no tool call stop the run (a model that will
not load, Claude Code missing), and the result says why.

## Variants

`variants.json` names product variants: `env` for the engine (and so for Claude Code and the MCP
server it starts), `settings` set with `settings.set`, optionally `relay_bin`. `baseline` changes
nothing. `--variant NAME` picks one; the result records it.

## What the model sees

Measured 2026-10-15 on the baseline (relay `ff599ba7`, Claude Code 2.1.301, Qwen3.5 4B's own
tokenizer through the product's template), from every request a capture run recorded
(`bench.py run --capture DIR`, then `bench.py seen DIR`; `seen DIR --measure` counts a capture again on
a CPU-only llama-server, no GPU). The server's `/v1/messages/count_tokens` renders a request through
the chat template, so counting it with a part taken out gives that part's exact share; the total
matched the `input_tokens` the server reported for the same request to the token.

**The first request of every thread: 20,206 tokens**, of a 32,768-token context:

| part | tokens | |
|---|---:|---|
| the 30 tool schemas | **15,923** | 52,213 characters of JSON |
| · Arbiter's 12 | ≈12,240 | `arbiter.backtest` ≈5,470 and `arbiter.propose` ≈5,405 alone |
| · Tally's 10 (`money.*`) | ≈2,360 | `money.series` ≈520, `money.invest.add` ≈495, `money.tx.add` ≈240 |
| · Avex's 7 (`gym.*`) | ≈1,045 | `gym.series` ≈310 |
| · `bus.schema` | ≈75 | |
| · the template's tool-calling instructions | ≈197 | once |
| Relay's appended `PROMPT` | **2,597** | 10,929 characters |
| · Tally core + charts | 598 | the opening paragraph 395, charts 203 |
| · investments | 498 | |
| · Wealthsimple files (steps 1–3, the no-API note) | 499 | |
| · Avex + its charts | 453 | |
| · Arbiter + its charts | 363 | |
| · untrusted data, secrets | 175 | |
| Claude Code's own system prompt | **1,665** | with a per-conversation `x-anthropic-billing-header` block first |
| the question | ≈8–11 | nothing else: no reminders in this mode |

(Per tool, a schema counted alone includes the ≈197-token instructions; the ≈ figures above take
them out.)

**What the tools answer**, as the model receives it. Claude Code hands the model the door's
`structuredContent` re-serialized as compact JSON, not the door's pretty-printed text, so the
indentation the door writes never reaches the model. (Inferred from the captured requests, not from
Claude Code's source: a reading meant to reach the model as plain text should be checked the same way.)

| op | tokens | characters |
|---|---:|---:|
| `gym.summary` | 2,465 | 5,727 |
| `money.tx.list`, a whole month (26 entries) | 2,063 | 6,047 |
| `money.summary` | 1,588 | 4,152 |
| `gym.lift.get` (Bench Press) | 1,431 | |
| `gym.lifts` (every lift) | 1,246 | 2,804 |
| `money.invest.summary` | 1,004 | 2,549 |
| `money.tx.list`, one category's month | 734 | 2,129 |
| `money.lists` | 578 | 1,871 |
| `gym.lifts` (one lift) | 106 | 232 |
| `money.series` | 98–119 | ~260 |

**A task-run** (the smoke suite): a median of 2 model calls, each re-sending the whole conversation,
so ≈42,000 prompt tokens processed and ≈250 written; the largest single prompt was ≈24,000.

What follows from it:

- Three quarters of what the model reads before the question is tool schemas, and three quarters
  of those are Arbiter's two big rule schemas, which no Tally or Avex question needs. Taking
  Arbiter's tools out of a local thread would cut the first prompt from ≈20,200 to ≈8,000 tokens.
- After the first request, one or two readings (1,000–2,500 tokens each) fill most of what is
  left: a thread has ≈12,500 tokens for everything after its first prompt.
- The billing-header block at the top of Claude Code's system prompt differs between
  conversations (`cc_version=2.1.301.<3 hex>`), so a new thread cannot reuse the server's cache of
  the previous thread's prompt: every new thread pays the full ≈20,000-token prefill (about 3 to 7
  seconds here). Within one thread the prefix is stable and is reused.
- Claude Code does not know `qwen35-4b` and assumes a 200k context (it says so on stderr), so its
  own compaction never triggers before the server's 32k limit.
- Each request also carries `thinking: adaptive`, `output_config.effort: high` and
  `max_tokens: 32000`; the server runs with reasoning off.

## v2: the harder suite

v1 is saturated (98.2% on the 4B) and stays frozen as a regression guard: its tasks, truths and world
do not change. v2 is a second suite in a second, messier world, meant to leave room to climb.

- **The world** (`worlds/v2/`, what is in it: `worlds/v2/WORLD.md`): the same clock, a household
  ledger from 2025-08-15 to today with the **budget month starting on the 15th** ("this month" is
  Sep 15 – Oct 14), ten accounts (two savings, two cards, a line of credit, a closed card), duplicates,
  entries with no category, refunds in an expense category, a note that is an instruction, notes in
  French and English, bills that post and reminders that don't, goals, a Wealthsimple TFSA filled from
  a holdings report and an activities export as of 2026-07-31, and an RRSP at Desjardins valued by
  hand; Avex for nine months in **kilograms**, with a deload week, a two-week trip, a plateaued
  overhead press, barbell bench switched to dumbbells, untracked sessions, runs, rides and hikes.
- **Loaded through the engine's own ops**: `money.import` of `tally-backup.json`, `money.invest.import`
  of the two Wealthsimple files, `gym.import` of `avex_export.json`. Then every load compares the
  engine's readings (periods, balances, budget standings, counts) with `worlds/v2/facts.json` and
  stops on a difference.
- **Built by** `worlds/v2/generate.py` (seeded, standard library). Its output is committed, so a run
  never regenerates; after changing it, run it and commit both (`generate.py --check`, and a unit
  test, fail when the files are stale). Changing the world changes the fingerprint and every v2
  answer key's inputs: compare results only across runs with the same fingerprint.
- **Tasks**: `tasks/v2/*.json`, the same format as v1, ids `v2.<file>.<name>`, truths in
  `relaybench/truths_v2.py` named `v2_*` (registered in the same table). A task may set
  `"max_tools": N` to fail a thread that made more than N tool calls (scored as check type
  `max_tools`; v1 tasks do not use it).
- **The suite picks the world**: `--suite v2` is every `tasks/v2/*.json` but its smoke starter, in the
  v2 world; `--suite v2/<file>` one file; v1 suites load v1's world exactly as before. Tasks from both
  worlds in one run are refused.

```fish
# the v2 world and every v2 task's expected values, no model
python3 bench/threads/bench.py run --check --suite v2
# the 8-task v2 starter (it joins `--suite v2` only when --tasks names v2.smoke.*)
python3 bench/threads/bench.py run --suite v2 --tasks 'v2.smoke.*' --runs 1
python3 bench/threads/bench.py run --suite v2/smoke
# the whole v2 suite, three runs
python3 bench/threads/bench.py run --suite v2 --runs 3
```

**Adding a task**: append it to a `tasks/v2/*.json` file (a new file is picked up by `--suite v2`),
write its truth in `truths_v2.py` with `@truth` and a `v2_` name, computing from `q(...)` and checking
against the engine's own figure, then `bench.py run --check --suite v2 --tasks 'v2.<id>'` prints what it
expects and `python3 -m unittest discover bench/threads` checks its shape. Aim at what `WORLD.md`
lists, but write the truth from the engine, not from the page.

First pipeline run, 2026-10-16 (Qwen3.5 4B Q8_0, relay `0e5caa7`, the 8-task starter, one run): 6/8.
The two failures are the model's: it looked for the Sep 3 note in this period only (Sep 3 is last
month's), and it answered the overhead-press question with pounds beside kilograms and a claim that
nothing was logged since March.

## v3: harder questions in the v2 world

v2 stopped leaving room on the 4B once Relay's readings were tuned on it (69.4% → 91.7% over five
rounds, 2026-10-29/30). v3 is a third suite of questions **no single reading answers**, asked in the
v2 world (`--suite v3` loads `worlds/v2`; v2 and v3 tasks may share a run):

- **Sums across months or the whole ledger**: interest on the line of credit this year, freelance
  this year against last, GST credits in all, tax refunds, Costco this year and its trips, all
  spending this year, everything ever left without a category, Miso since the vet bill.
- **Ranking and grouping**: the weekday most spent eating out, the priciest budget month and why,
  the biggest category this year besides rent, the week with the most volume lifted.
- **What-ifs and ratios**: a year without Netflix and Spotify, months to the Japan goal at 300.00 a
  month, a 1,200.00 laptop against what is left after the bills, rent as a share of income.
- **Days named the way people name them**: since Labour Day, Q2 against Q1, last weekend, the four
  weeks before the trip, the budget month of the 10 km race.
- **False premises and missing data**: a rent that "went up" (it never did), Starbucks (none), training
  "stopped in August" (16 workouts), steps (Avex has none).
- **Both apps over the same days**: the gym's cost per workout this year, the Portugal trip's Visa and
  training, last month against the one before, last weekend.
- **Writes**: two entries in one message, cash from the ATM, three entries recategorized at once (or
  proposed), and an amount or a card corrected in a second turn.

Truths are in `relaybench/truths_v3.py` (`v3_` names; an effect check may use v2's
`v2_eff_unchanged`) and read only what relay `b413157` already served, so a run on that build and one
on a newer build grade against the same keys (`--check` on both prints the same answers).

The tasks were written after the readings' six rounds of tuning and none of them shaped those
rounds: for that build, the whole of v3 is new. Tuning on v3 follows the v2 rule: look at the dev
split's failures only, and report the holdout's number beside it.

```fish
python3 bench/threads/bench.py run --check --suite v3
python3 bench/threads/bench.py run --suite v3 --runs 3
```

## Files

- `bench.py`: the CLI.
- `relaybench/engine.py`: the throwaway engine and op calls. `fixtures.py`: the world, its
  fingerprint, snapshot and restore. `model.py`: the GGUF, the wrapper, the GPU lock and the
  free-memory check, the capture proxy. `runner.py`: tasks × runs, threads, scheduling, token usage.
  `grade.py`: the checks. `report.py`: statistics, `compare`, `RESULTS.md`. `seen.py`: what the
  model sees, in its tokens. `truths.py`: the answer key; `truths_v2.py`: v2's; `truths_v3.py`: v3's.
- `worlds/v2/`: the v2 world (`generate.py`, its output, `facts.json`, `WORLD.md`). `tasks/v2/`: its tasks;
  `tasks/v3/`: the harder suite's, in the same world.
- `tasks/*.json`: the suite. `--suite all` (the default) is every file but `smoke.json`, a 10-task
  starter that exercises every kind of check and runs only when named (`--suite smoke`).
- `variants.json`, `results/`, `tests/`, `templates/` (chat templates fixed for a model, `--template`).
