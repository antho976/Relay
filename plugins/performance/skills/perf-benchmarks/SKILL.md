---
name: perf-benchmarks
description: Build benchmarks that can be climbed and that guard against regressions, in any language - deterministic proxies (instruction counts under Valgrind callgrind/cachegrind or perf stat, allocation, syscall, query, render and layout counts), proving a proxy tracks wall-clock time, micro versus scenario harnesses, wall-clock benchmarks with statistical rigor (criterion, JMH, benchstat, pyperf, hyperfine), noise control on the machine, deciding whether two runs really differ, and CI ratchets - a checked-in baseline per guarded path that fails a change raising it and is lowered when it drops. Use when writing a benchmark, a perf regression test, a CI performance gate, or deciding whether a measured change is real.
---

# Benchmarks and ratchets

A benchmark has two jobs: **a number an agent can move**, and **a guardrail that keeps it moved.**
Wall-clock time is the truth but too noisy to gate CI on; deterministic counts are gateable but
only worth gating if they track wall-clock time. Build both, prove the link, gate on the count.

## 1. The metric ladder

From most deterministic to most real. Pick the highest rung that captures the cost, and always
keep a wall-clock reading beside it.

| Rung | What | Deterministic? | Tools |
|---|---|---|---|
| Instructions | CPU instructions retired in the measured region | yes, under an emulator; ±0.1 % with `perf stat` on hardware | callgrind, cachegrind, gungraun (formerly iai-callgrind), `perf stat -e instructions:u` |
| Work counts | allocations, bytes, syscalls, SQL statements prepared/executed, subprocesses, file opens | yes | counting allocator, `strace -c`, SQLite profile hooks, tracing counters |
| UI work counts | renders/recompositions, layouts, style recalcs, DOM mutations, widgets created, frames produced | yes, given a fixed input script | toolkit debug counters, Compose recomposition counts, React profiler |
| Frames over budget | frames that missed 16.7 / 8.33 ms on a deterministic frame driver | nearly | GTK frame timings, Macrobenchmark `FrameTimingMetric`, DevTools begin-frame |
| CPU time | process/thread CPU per iteration | noisy | `getrusage`, `/proc/<pid>/stat`, criterion |
| Wall time | what a person waits | noisiest | criterion, divan, hyperfine, harness timers, field telemetry |

Instruction counts miss things a person feels: cache misses, page faults, lock waits, I/O and
subprocess time, GPU work. That is why every guarded count keeps a wall-clock companion and why a
proxy has to be **validated** (§3).

## 2. Deterministic counts

### Instructions under Valgrind

```fish
valgrind --tool=callgrind --callgrind-out-file=cg.out --collect-atstart=no \
  --toggle-collect='*measured_fn*' ./bench-binary
callgrind_annotate --inclusive=yes cg.out | head -60
```

- Switch collection on **only around the measured call** (`--toggle-collect`, or the
  `CALLGRIND_START_INSTRUMENTATION` / `CALLGRIND_STOP_INSTRUMENTATION` client-request macros from
  `valgrind/callgrind.h`, which most languages can reach through a small binding) so setup is
  excluded.
- Use `--dump-instr=yes` when you need per-line attribution; `callgrind_annotate --auto=yes` with
  sources; `kcachegrind` for browsing.
- `cachegrind` (`--cache-sim=yes` optional) is cheaper when only totals are needed.
- Keep debug info (symbols, line tables) in the optimized build you measure, or the attribution is
  addresses.
- Pin anything nondeterministic: hash seeds (`RandomState` per process → use a fixed hasher in the
  bench or accept the jitter), thread scheduling (single thread where possible), time, ASLR-sensitive
  paths, allocator caches (warm once outside the region). A count that wobbles by > 0.5 % between
  identical runs has a nondeterminism leak — find it before gating.
- For JS hot paths the claude.ai team ran `node --predictable` under Valgrind against a checked-in
  baseline: one run, no statistics.

### Harnesses

Two shapes, whatever the language:

- **Micro**: one function, many iterations, a library harness. Good for a hot loop or a parser.
- **Scenario**: the real program driven through its real entry point (a request through the
  server, a page open, startup) with a fixture shaped like real data. Better for anything a person
  feels; measure each scenario with native timers and, in a second pass, under an instruction
  counter with collection toggled around the call.

| Ecosystem | Wall-clock harness | Deterministic option |
|---|---|---|
| Rust | criterion, divan | gungraun (formerly iai-callgrind); a counting `#[global_allocator]` |
| C / C++ | Google Benchmark, nanobench | callgrind/cachegrind; `perf stat -e instructions:u` |
| Go | `testing.B` + `benchstat` | `-benchmem` allocs/op; `perf stat` |
| JVM / Kotlin | JMH; Jetpack Microbenchmark / Macrobenchmark on Android | allocation counts; frame counts on a fixed script |
| JS / TS | tinybench, mitata; Lighthouse / WebPageTest for pages | `node --predictable` under callgrind; render/layout counts |
| Python | pytest-benchmark, pyperf | cachegrind on the interpreter; call counts from `cProfile` |
| Whole commands | hyperfine | `perf stat`, `strace -c` |

## 3. Prove the proxy tracks what people feel

Before optimizing a proxy, show it moves with wall-clock time:

1. Take 3–5 variants of the path (before/after candidate fixes, different input sizes).
2. Measure the proxy and the wall time for each, wall time with enough runs for a CI.
3. They should move in the same direction with a believable ratio (e.g. −48 % instructions,
   −78 % wall when the removed work was cache-hostile lookups; the reverse is a warning).
4. If the proxy is flat while wall moves (I/O, locks, page faults, GPU), it is the wrong rung:
   climb down to syscalls/lock-wait/frames, or measure wall directly with more runs.

Discard proxies that are flaky or uncorrelated. Write the validation into the benchmark's comment.

## 4. Wall-clock benchmarks done right

- Use a harness that does the statistics for you (criterion, JMH, `benchstat`, pyperf) and
  **hyperfine** for whole commands (`--warmup 3 --runs 30 --export-json out.json`, `--prepare` to
  reset state between runs, `-N` to skip the shell).
- **Warm up** (JIT, caches, lazy init), then sample; measure cold start separately and on purpose. Report median and a percentile spread or a confidence interval; never a
  single run, never a mean alone.
- **Two runs are different only if the difference beats the noise**: compare distributions
  (Mann–Whitney U with ≥ 9 repetitions, or bootstrap a CI of the ratio of medians); state the
  minimum detectable effect of the setup. Interleave A and B runs (ABAB…) so drift hits both.
  Run an **A/A comparison** (the same build twice) once per setup to learn its noise floor.
- **Benchmark actively** (Gregg): while it runs, watch what limits it — CPU, a lock, the disk,
  the load generator itself. "You benchmark A, but actually measure B, and conclude you've
  measured C." Would doubling the load change the result? Did it hit a limit or errors? Does it
  reproduce?
- **Load tests are open-loop**: fixed arrival rate, not "send the next request when the last
  returns", or stalls disappear from the results (coordinated omission, `perf-fundamentals`).
- **Combine speedups with a geometric mean**, not an arithmetic one.
- **Keep the machine quiet**: performance governor (`cpupower frequency-set -g performance`),
  turbo/boost off for comparisons (`/sys/devices/system/cpu/cpufreq/boost` or
  `intel_pstate/no_turbo`), pin to cores (`taskset -c 2`), no other heavy processes, AC power,
  same thermal state. On shared CI runners, do not gate on wall time at all.
- **Benchmark the optimized build** people run. Do not let the optimizer delete the work: route
  inputs and outputs through the harness's sink (`black_box`, JMH `Blackhole`,
  `benchmark::DoNotOptimize`).
- **Shape inputs like production**: sizes, distributions, cold vs warm caches as people see them.
  Run each list at 1× and 10× to see the complexity, not just the constant.

## 5. Ratchets in CI

A ratchet is a ceiling per guarded path that only moves down.

```
perf/ratchet.json   { "task.list/40": 6_120_000, "session.list": 412_000, ... }   # instructions
```

- **CI**: run the deterministic pass for guarded paths; fail when any count exceeds its ceiling by
  more than a small tolerance (0.5–1 % for instructions; 0 for counts like statements compiled or
  renders per keystroke). Print the delta and the top changed functions so the author sees why.
- **Lowering**: a scheduled job (or the PR that wins) rewrites the ceiling to the new count when it
  drops beyond the tolerance, so the next regression is measured against the win.
- **Raising** requires an explicit, reviewed edit of the baseline file with a reason — the work got
  bigger on purpose.
- **Tie toolchain changes to baseline refreshes**: a compiler, Valgrind or dependency bump shifts
  counts; refresh all ceilings in one reviewed commit.
- **Gate only what you trust**: a flaky gate gets ignored, then deleted. Wall-clock trends go to a
  dashboard (and nightly alerts on sustained shifts), not to a blocking check. The Rust compiler's
  own CI made instruction counts its headline metric for exactly this reason, keeping cycles and
  wall time beside them for the work instructions hide (a `rep movsb` memcpy is one instruction).

### Watching trends, the way large teams do

- **Thresholds per suite, from its noise**: Mozilla's Perfherder alerts at 2 % for most
  page-load and build suites and 0.25 % for memory, and needs ~12 points before and after a change
  to call it. Measure each suite's noise (A/A runs) and set its threshold above it.
- **Change-point detection over a series**, not run-vs-run: E-divisive means (MongoDB), CUSUM
  (Meta's FBDetect) — then filter changes that went away on their own and seasonal swings.
- **Attribute at the function level**: a 0.005 % change in a function that is 0.1 % of the profile
  is a 5 % change to that function — far easier to see than in the total (FBDetect). Watch for
  cost that merely *moved* between functions (a third of its function-level alerts were that).
- **A sheriff**: someone (or an agent on a schedule) triages alerts daily, bisects, files against
  the culprit change, and cleans the backlog. A ref build that moves at the same moment means the
  machine or the test changed, not the code.

## 6. Benchmarks for invariants

Some regressions are not "slower" but "now blocks something else". Encode them as scenarios:

- **During**: the hot request issued while the slow one is in flight — asserts the slow one does not
  hold the shared lock or the UI thread. Verify the slow one *is* still running when the hot one
  goes out, or the scenario silently measures an uncontended read.
- **At rest**: N idle sessions/windows for 30 s — asserts zero CPU and zero wakeups.
- **Per keystroke**: renders, allocations, queries per input event — asserts constants.
- **Growth**: cost at 10× rows ≤ 10× cost at 1× (or ≤ constant where it should be).

## 7. Reporting a benchmark result

Machine, build profile, toolchain, fixture, iterations, warmup; median and p95 (or CI); the proxy
and wall; before/after on the same setup; what was not measured. A number without those is an
anecdote.
