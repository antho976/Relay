# Instruction counts and attribution, 2026-10-14

The 2026-10-14 audit ran without instruction counts or callgrind attribution: Valgrind cannot run
on this machine (CachyOS's x86-64-v4 loader uses AVX-512; `BASELINE-2026-10-14.md`, "Machine and method"). This reading
fills both gaps from the CPU instead, with two new harness passes (PLAYBOOK §1), and records what
they show. Raw output: `runs/2026-10-14-attribution/` (`counters.jsonl`, `profiles.json`,
`sampled/folded/*.folded.gz`, and `report.md` with every scenario).

Machine: AMD Ryzen 7 9800X3D (Zen 5, 8 cores / 16 threads), 30.5 GiB, Linux 7.2.9-2-cachyos,
rustc 1.99.0, dev profile (`opt-level` 1, dependencies 3), `perf` 7.2.10. Fixture and scenarios
as in the audit; 260 scenarios.

## The two passes

| pass | what it gives | how good |
|---|---|---|
| `perf run --counters` | user-space instructions per iteration on every thread, child processes apart, kernel instructions apart | two runs agreed to within 0.1 % on every scenario that does not wait on a timer (`session.list` 215 015 / 215 014, `task.list` 4 619 074 / 4 618 959) |
| `perf record` sampling | self cost by subsystem, inclusive share of named entry points, the Relay functions below the pipeline, a flame graph per scenario | a large bucket repeats within 1–3 points (up to 6 on `mailbox.send`); sampled totals match the counted ones at a median of 1.02 (p5 0.76, p95 1.21) |

Two cross-checks against the counters. The share of `op.git.status`'s instructions spent in
`git` children is 78.8 % counted and 72–76 % sampled; `op.session.create`'s is 90.7 % counted and
90.3–90.7 % sampled. And the sampled total of each scenario, divided by the counted total
(`coverage`), is within 0.75–1.33 for all but 19 of the 230 scenarios with at least 300 samples;
the report flags those 19 by name.

What the passes got wrong first, and what changed:

- **DWARF call stacks stopped two or three frames above the sample** (8 KiB of stack per
  sample, 60 MB for three scenarios). A frame-pointer copy of the harness walks to `main` at
  1.5 MB for the same three. The runner builds it under `target/perf-fp`.
- **One sample per 200 003 instructions was throttled.** The kernel caps samples at 50 a
  millisecond, and Zen 5 runs `task.list` at about 16 G instructions/s, so a third of its
  samples were lost and its SQLite share moved between 31 % and 38 % from run to run. At
  400 009 its coverage is 1.02, and four runs put SQLite within 32–34 %.
- **Asking every scenario for 1 500 samples filled `/tmp`.** `session.park` costs few
  instructions but sets up a whole session per iteration, so it asked for 607 iterations; one
  shard's fixture reached 5.2 GB and the shards died. Iterations are now capped by the native
  pass's loop time (8 s) and by cost class.
- **Samples from the fixture's provider stand-ins were counted.** A pasted 4 KiB is echoed by
  the stand-in provider, a process that was running before the scenario. Its samples made
  `session.input.paste_4k` 87 % "child processes". Samples now count only for processes this
  process is or that the scenario started, as the counters do.

## What the attribution shows

Six leads. All six are now fixed, 3 and 4 in part ("Fixed since" and "Fixed since, part 2" below).

1. **`settings.set` spends 53 % of its 391 k instructions in `settings::get`**, which it calls
   twice (before, for undo; after, for the event). `subtree` starts every read with
   `defaults().clone()`, the whole defaults tree, to read back one path, and allocation is 47 %
   of the op. Cloning only the default subtree on the path would leave the get at a fraction
   of its cost. `settings.set` is every audited setting change and every `perf.soak` round.
   **Fixed**: 397 k → 230 k instructions.
2. **`app.status` spends 51 % in `providers::spawn_profile`** (27 % in `Codex::args` alone) and
   19.5 % compiling SQL (`sqlite3_prepare`). It builds every provider's launch profile to report
   status, and somewhere a statement is not cached. The shell calls it on connect and on
   changes; 436 k instructions. **Fixed**: 466 k → 178 k.
3. **`audit.list` is 53 % allocation and 26 % `serde_json::from_str`.** It parses each stored
   payload into a `Value`, and the engine's `to_value` of the typed result copies that `Value`
   node by node (`AuditRow::serialize` 40 %); 8.8 M instructions per page. Passing the stored
   text through untouched is not possible while bus results are `Value`s, so one parse stays.
   **Fixed** (the copy): 8.8 M → 6.5 M instructions.
4. **`task.list` (4.6 M) and `dashboard.get` (2.0 M) spend 29 % in `task::hydrate`** and 25 %
   / 16 % building `serde_json::Value` maps (`IndexMap::push_entry` is the hottest Relay-side
   self frame). That is typed structs serialized through `Value` instead of directly.
   **Fixed in part**: 4.63 M → 3.56 M, from reading columns by index and a statement SQLite
   re-prepared on every call; the `Value` maps stay (see "What is left").
5. **`session.input` (6.1 k instructions, about 1 µs) spends 11 % in `RandomState::hash_one`**
   (SipHash on the PTY name lookup, `Engine::pty_named`) and 21 % deserializing the payload
   through a `Map`. Small in absolute terms; it is the request a person feels.
   **Fixed**: 6.06 k → 5.17 k.
6. **`path.engine.startup` is 30.8 M instructions**, 9.5 % compiling SQL and 31 % in `std`/
   collection code under `Vec::from_iter` (skills and recovery). Measured cold-store startup
   was the audit's open item; this is where its CPU goes. Read by caller, 62 % was crash
   recovery copying the environment of every process on the machine. **Fixed**: 30.3 M → 13.0 M.

The `git` ops are mostly `git`: `session.create` is 66 M instructions, 91 % of them in child
processes, and `file.tree`'s one level is 0.7 M in Relay and 4.7 M in `git status` for the
badges (PLAYBOOK lead 6).

## Fixed since

Same machine and build; each column is two runs of the harness (`--counters` for instructions,
the native pass for the rest), raw rows and the script that prints them in
`runs/2026-10-14-attribution/fixes/`.

| scenario | instructions before → after | p50 before → after | allocations before → after |
|---|---:|---:|---:|
| `op.settings.set` | 397 k → 230 k (−42 %) | 37.0 µs → 21.6 µs | 419 → 67 |
| `op.settings.reset` | 379 k → 202 k (−47 %) | 34.9 µs → 20.3 µs | 408 → 54 |
| `op.settings.get.path` | 121 k → 54.6 k (−55 %) | 9.9 µs → 4.0 µs | 200 → 31 |
| `op.notify.settings.get` | 106 k → 39.7 k (−63 %) | 9.1 µs → 3.2 µs | 195 → 26 |
| `op.notify.settings.set` | 361 k → 277 k (−23 %) | 33.2 µs → 26.3 µs | 266 → 97 |
| `path.event.roundtrip` (a `settings.set` to its event) | 501 k → 326 k (−35 %) | 59.0 µs → 41.4 µs | 484 → 132 |
| `op.audit.list` (200 rows) | 8.78 M → 6.52 M (−26 %) | 578 µs → 410 µs | 10 228 → 7 552 |
| `op.audit.get` | 57.5 k → 52.1 k (−9 %) | 4.2 µs → 3.9 µs | 72 → 61 |
| `op.settings.get` (whole tree, unchanged path) | 242 k → 240 k | 18.3 µs → 18.3 µs | 440 → 440 |
| `op.audit.undo` (unchanged path) | 1.28 M → 1.29 M | 114 µs → 117 µs | 505 → 505 |

The two-run spread is under 0.3 % on every instruction count. The last two rows are paths the
changes do not touch, read as controls.

- **`handlers/settings.rs`**: `subtree` starts from `defaults_along(path)`, the defaults on the way
  to `path` and nothing else, instead of a clone of every default. `set_at` and `get_at` only
  touch nodes on their own path, so the read is the same; `a_path_read_matches_the_full_tree`
  checks every default path, and paths below and beside them, against the full tree, with
  stored rows that replace an ancestor with a scalar or an object.
- **`engine.rs`**: `register_unlocked_json`, `register_unlocked` for a query that builds its result
  as JSON itself. The handler promises the shape of `O::Result`, and keeps a test that holds it
  to that.
- **`audit.rs`, `handlers/audit.rs`**: `RawAudit::into_json` serializes the typed row with its
  payload, result and inverse empty, then moves the parsed values into their places.
  `audit.list` and `audit.get` answer with it. `into_json_is_the_typed_row_exactly` compares it
  with the typed row byte for byte (key order included), for full and empty columns, text that
  no longer parses, and inverses with and without `expect`.

A first version came out *slower* (11.6 M instructions): `json!({ "rows": rows })` serializes
its argument, so it copied every row once more. The rows object is now built directly.

What is left in `audit.list`: parsing the stored text (35 %), the rows' own small maps with
their owned keys (24 %), and copying the text out of SQLite (17 %).

## Fixed since, part 2

Leads 2, 4, 5 and 6. Same machine and build, two runs of each, raw rows and the script that
prints them in `runs/2026-10-14-attribution/fixes-2/`. Two-run spread under 0.5 % on every
instruction count except startup's, which reads every process on the machine and so moves with
what else is running.

| scenario | instructions before → after | p50 before → after | allocations before → after |
|---|---:|---:|---:|
| `op.app.status` | 466 k → 178 k (−62 %) | 32.5 µs → 17.7 µs | 471 → 292 |
| `op.provider.list` | 425 k → 138 k (−68 %) | 27.2 µs → 12.3 µs | 401 → 222 |
| `op.task.list` (board, 40 tasks) | 4.63 M → 3.56 M (−23 %) | 282 µs → 212 µs | 2 428 → 2 268 |
| `op.task.list.filtered` | 1.49 M → 1.05 M (−30 %) | 92.3 µs → 63.6 µs | 535 → 503 |
| `op.dashboard.get` | 1.96 M → 1.73 M (−12 %) | 142 µs → 122 µs | 856 → 817 |
| `path.socket.task_list` (through the socket) | 9.15 M → 8.08 M (−12 %) | 690 µs → 583 µs | 4 999 → 4 839 |
| `path.engine.startup` | 30.3 M → 13.0 M (−57 %) | 3.1–3.4 ms → 2.0–2.4 ms | 19 005 → 6 436 |
| `op.session.input` (a keystroke) | 6.06 k → 5.17 k (−15 %) | 0.8 µs → 0.8 µs | 5 → 3 |
| `path.socket.keystroke` | 49.0 k → 47.1 k (−4 %) | 7.1 µs → 7.2–8.2 µs | 27 → 26 |
| `op.task.get` (control: one task, unchanged path) | 254 k → 255 k | 18.2 µs → 17.7 µs | 87 → 83 |

A keystroke's wall time is under the timer's resolution here; the instructions are the
measure. Startup's wall time is noisy, its instructions are not.

- **`providers.rs`** (lead 2): `spawn_profile_of` builds each provider's launch profile once;
  everything it reads is fixed in the build (`the_kept_spawn_profile_is_the_one_built_now`).
  The two `query_row`s per provider, compiled on every call, are `prepare_cached`.
- **`handlers/task.rs`, `handlers/notify.rs`** (lead 4): `TaskCols` finds a statement's columns
  once; rusqlite finds a column by name by comparing it with every column's name, twenty times a
  row. And `task.list`'s `LIMIT ? OFFSET ?` became `LIMIT ?+0 OFFSET ?+0`: **SQLite re-prepares a
  statement whose LIMIT is a bare parameter on every run**, so its `prepare_cached` statement was
  compiled again each call. Every `LIMIT ?` in the engine is now `?+0` (ten statements: audit,
  notifications, holds, both mailbox lists, task activity and messages, file trash, purge);
  `EXPLAIN QUERY PLAN` is the same for each with a literal limit of 2 and of 201.
  `tests/store.rs` `no_limit_is_a_bare_parameter` pins SQLite's behaviour and keeps a bare one
  from coming back.
- **`pty.rs`** (lead 6): crash recovery's `relay_children` reads every process's environment
  looking for Relay's three variables; it searched owned copies of every variable of every
  process. It now searches the bytes as read and copies only a match
  (`relay_child_session_reads_the_environment_as_proc_env_did`).
- **`engine.rs`** (lead 5): the PTY registries are `BTreeMap`s, which find one of a few dozen
  short keys in a handful of compares where `HashMap` ran SipHash three times a keystroke; and
  the input fast path reads its payload borrowing the session name and text (`InputRef`), held
  to `InputIn`'s answers and error words by `input_ref_answers_as_input_in`. That test caught
  a difference in the first version: serde's error names the struct.

### What is left

The largest remaining cost on every list is building `serde_json::Value` trees: a handler's
typed result goes through `to_value` and is then written to the socket from the tree (`task.list`
25 %, `audit.list` 59 % counting the stored payloads it parses). Writing typed results straight
to the socket as text would remove both, but it changes the one path every request takes:
`Response.result` is a `Value` that the socket, the idempotency record, events, the phone door,
the CLI and the client all read. The calls it would speed up are already well inside the
100 ms a person notices (board 212 µs, audit page 410 µs in process).

## Rerun

```fish
python3 scripts/perf/baseline.py                                   # everything; picks perf where Valgrind cannot run
python3 scripts/perf/baseline.py --out-dir docs/perf/runs/<run> --attribution-only   # just this pass, into a run
zcat docs/perf/runs/<run>/sampled/folded/op.task.list.folded.gz | inferno-flamegraph > task-list.svg
```

Not measured: kernel-side attribution (`instructions:u` only; the counters record kernel
instructions per scenario but nothing breaks them down), and the native client, whose pages the
audit profiled with `perf record` separately (`runs/2026-10-14/native-after/`).
