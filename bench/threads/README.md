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

then `compare` it with the baseline line of `RESULTS.md`.

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

## Files

- `bench.py`: the CLI.
- `relaybench/engine.py`: the throwaway engine and op calls. `fixtures.py`: the world, its
  fingerprint, snapshot and restore. `model.py`: the GGUF, the wrapper, the GPU lock and the
  free-memory check, the capture proxy. `runner.py`: tasks × runs, threads, scheduling, token usage.
  `grade.py`: the checks. `report.py`: statistics, `compare`, `RESULTS.md`. `seen.py`: what the
  model sees, in its tokens. `truths.py`: the answer key.
- `tasks/*.json`: the suite. `--suite all` (the default) is every file but `smoke.json`, a 10-task
  starter that exercises every kind of check and runs only when named (`--suite smoke`).
- `variants.json`, `results/`, `tests/`.
