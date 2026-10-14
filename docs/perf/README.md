# Performance baselines

`BASELINE-<date>.md` is the reading: every path in the application, what it costs, where the
cost goes, and whether that is the work or overhead. `runs/<date>/` is the evidence behind
one reading: JSON from the harness and the generated `report.md` with every table.
`FIXES-<date>.md` records what was changed after a reading, with the same scenarios measured
before and after.

`PLAYBOOK.md` is how to work on Relay's performance: the journeys and their budgets, the
invariants that are cliffs, how to measure each half, and the leads for the next audit. The
method itself is the bundled Performance plugin's (`plugins/performance/`).

## Rerun

```fish
python3 scripts/perf/baseline.py            # ~55 min: native, ×10 rows, strace, soak, process, callgrind
python3 scripts/perf/baseline.py --quick    # ~5 min: fewer iterations, no callgrind
python3 scripts/perf/compare.py docs/perf/runs/<old> docs/perf/runs/<new>
```

The engine half needs `valgrind` and `strace` on PATH for the attribution and syscall passes
(both are skipped, with a note, when absent), and builds the `perf` example of `relay-core`
plus the CLI in the dev profile, which is the profile `run.sh` launches. The callgrind pass
runs `--jobs` processes (default: cores minus one, at most four), each holding a few GB by the
end of its list; on a machine with a memory limit under 6 GB per process, pass `--jobs 2` or
rerun any scenario the pass missed with `--only` and merge its `profiles.json` by hand.

The client half runs only where `relay-native` builds (GTK 4.22, VTE 0.84):

```fish
python3 scripts/perf/native-baseline.py     # opens every page against a disposable engine
```

Its output lands beside the engine run as `runs/<date>/native/` and is folded into the next
reading by hand.

## What is measured

The harness at `crates/relay-core/examples/perf.rs` builds one disposable fixture — a 1 500
file git repository with a bare origin, a store on disk, fake providers, two live PTY sessions,
a populated board — and measures:

- **every bus op** (210 registered), each with a payload shaped like a real call, as the user
  or as the builder agent, with fresh rows prepared outside the measured region when the op
  consumes them;
- **the paths that are not ops**: store open, engine construction, crash recovery, skill
  materialization, request parse, response serialize, socket connect and round trip, event
  fan-out to a subscriber, PTY spawn to first output, a 1 MiB burst through the ring,
  reconnect catch-up, disk walks and status through the library;
- **each of those four ways**: native wall time and process CPU per iteration with allocation
  counts, instruction attribution under callgrind (one dump per scenario, instrumentation
  switched on only around the measured call), syscall counts under strace, and the list
  queries again at ten times the rows;
- **memory** over 20 000 mixed operations, with live heap bytes from the allocator wrapper;
- **the real `relay serve` process**: idle CPU and wakeups with six quiet sessions, with the
  resources panel watching, with sixty lines per second of output unattached and attached, a
  50 000 line burst to a client, restart time with live sessions to reap, and the CLI's
  process startup.

Absolute times are the measuring machine's. Instruction counts and the breakdowns are not,
and are what `compare.py` trusts.

## Reading a scenario

Each op row gives p50 and p95 wall time, CPU per iteration, allocations, bytes allocated,
syscalls, the size of the returned JSON, instructions, and the outcome. A row that ends in an
error code measured that refusal path; a fixture without an Android SDK or `gh` cannot reach
the success path of `device.*`, `avd.*` and `github.*`, and says so.

The callgrind section gives, per scenario, the share of instructions by subsystem (SQLite,
JSON, allocation, handler logic, and so on), the inclusive share of known entry points (SQL
compile against SQL execute, audit, guardrail, git), and the Relay functions below the
pipeline with the most inclusive cost. Those inclusive shares overlap by construction.
