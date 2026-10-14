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

New leads, none of them fixed here; each is worth an estimate before it is worth a change.

1. **`settings.set` spends 53 % of its 391 k instructions in `settings::get`**, which it calls
   twice (before, for undo; after, for the event). `subtree` starts every read with
   `defaults().clone()`, the whole defaults tree, to read back one path, and allocation is 47 %
   of the op. Cloning only the default subtree on the path would leave the get at a fraction
   of its cost. `settings.set` is every audited setting change and every `perf.soak` round.
2. **`app.status` spends 51 % in `providers::spawn_profile`** (27 % in `Codex::args` alone) and
   19.5 % compiling SQL (`sqlite3_prepare`). It builds every provider's launch profile to report
   status, and somewhere a statement is not cached. The shell calls it on connect and on
   changes; 436 k instructions.
3. **`audit.list` is 53 % allocation and 26 % `serde_json::from_str`.** It parses each stored
   payload back into a `Value` to serialize it again (`AuditRow::serialize` 40 %). Writing the
   stored JSON through as raw text would skip both; 8.8 M instructions per page.
4. **`task.list` (4.6 M) and `dashboard.get` (2.0 M) spend 29 % in `task::hydrate`** and 25 %
   / 16 % building `serde_json::Value` maps (`IndexMap::push_entry` is the hottest Relay-side
   self frame). That is typed structs serialized through `Value` instead of directly.
5. **`session.input` (6.1 k instructions, about 1 µs) spends 11 % in `RandomState::hash_one`**
   (SipHash on the PTY name lookup, `Engine::pty_named`) and 21 % deserializing the payload
   through a `Map`. Small in absolute terms; it is the request a person feels.
6. **`path.engine.startup` is 30.8 M instructions**, 9.5 % compiling SQL and 31 % in `std`/
   collection code under `Vec::from_iter` (skills and recovery). Measured cold-store startup
   was the audit's open item; this is where its CPU goes.

The `git` ops are mostly `git`: `session.create` is 66 M instructions, 91 % of them in child
processes, and `file.tree`'s one level is 0.7 M in Relay and 4.7 M in `git status` for the
badges (PLAYBOOK lead 6).

## Rerun

```fish
python3 scripts/perf/baseline.py                                   # everything; picks perf where Valgrind cannot run
python3 scripts/perf/baseline.py --out-dir docs/perf/runs/<run> --attribution-only   # just this pass, into a run
zcat docs/perf/runs/<run>/sampled/folded/op.task.list.folded.gz | inferno-flamegraph > task-list.svg
```

Not measured: kernel-side attribution (`instructions:u` only; the counters record kernel
instructions per scenario but nothing breaks them down), and the native client, whose pages the
audit profiled with `perf record` separately (`runs/2026-10-14/native-after/`).
