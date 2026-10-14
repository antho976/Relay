---
name: perf-audit
description: Run a whole-application performance audit - inventory the journeys people use and the paths behind them, define each measurement from a person's action to the result on screen, take a baseline with a reproducible harness, attribute every cost (instructions, allocations, syscalls, SQL, frames, locks held), separate the work from overhead, rank fixes by milliseconds saved times frequency, write the reading as a BASELINE document, then fix in measured, guarded steps and record before-and-after in a FIXES document. Use when asked to audit, baseline, sweep or speed up an app as a whole, or to plan a performance sprint.
---

# Performance audit

An audit answers four questions for every path a person can feel: **what does it cost, where does
the cost go, is that the work or overhead, and what is it worth fixing first.** It ends in a
document someone can act on and a harness someone can rerun. Load `perf-fundamentals` first.

## 0. Before measuring anything

1. **Find what already exists.** Look for a perf folder (`docs/perf/`, `perf/`, `benches/`),
   harness scripts (`scripts/perf/`), example binaries (`examples/perf.rs`), CI benchmark jobs,
   telemetry dashboards and earlier readings. Extend the existing harness; compare against the
   existing baseline. A second harness that disagrees with the first is worse than none.
2. **Read the invariants.** `CLAUDE.md`, architecture and decision docs name the lock everything
   waits on, the thread a person feels, the request whose latency matters most. Those are the
   first things to check, because breaking them is a cliff, not a slope.
3. **Know the machine.** Record CPU model, core count, governor, RAM, disk, OS, kernel, toolchain
   versions and build profile at the top of every reading. Absolute times belong to the machine;
   instruction counts and ratios travel.

## 1. Inventory journeys and paths

**Journeys** are what people do: launch, open a page, type, switch, search, save, sync. List them
with how often they happen (from telemetry if it exists, from the product and code if not). Expect
a handful of journeys to be most of the activity — the claude.ai sprint found four journeys were
95 %. For each journey list the surfaces (desktop, phone, web, CLI) — each surface × journey is
one measurement.

**Paths** are what the code does for them. Enumerate systematically so nothing is skipped:

| Kind | How to enumerate |
|---|---|
| Request handlers / ops / endpoints | the router or registry (every registered op) |
| Startup | process start → first frame → first useful frame → interactive |
| Background work | timers, watchers, pollers, sync loops, schedulers — and what they cost at rest |
| Event fan-out | who subscribes to what, and what each event triggers downstream (refetch storms) |
| Rendering | each page or screen: first open, revisit, update on an event, scroll, resize |
| Input | keystroke, click, drag: input → handler → frame presented |
| Data growth | every list and query at today's size and at 10× rows |
| Process level | idle CPU, wakeups per second, RSS over time, threads, file descriptors |

## 2. Define each measurement

- **Start at the person's action and end when the result is on screen** (or the response is
  fully delivered for a headless path). Mark the end where a person would say "done", not where
  the code returns.
- **Split the sides**: client/UI thread vs engine/server, in-process vs through the socket or
  network, CPU vs waiting. A number without a split cannot be attributed.
- **Report distributions**: p50, p95 (and p99 when samples allow), CPU per iteration,
  allocations and bytes, syscalls, response size, instruction count. Never just a mean.
- **Shape payloads like real calls**, use a fixture shaped like real data (repository size, row
  counts, history depth), and prepare consumed rows outside the measured region.
- **Measure concurrency as a scenario**: the hot request *while* the slow one runs. A slow path
  that holds a shared lock shows up only there.
- **Measure at rest**: N quiet sessions/windows/tabs for 30 s — CPU, wakeups, allocations. Idle
  should be zero, not small.
- **Measure growth**: every list at 1× and 10× rows. Linear is a finding when it should be
  constant; quadratic is always a finding.

## 3. Take the baseline — four lenses per path

1. **Native wall and CPU** — optimized build, warmed, enough iterations for stable percentiles.
2. **Instruction attribution** — callgrind (or the platform's equivalent) with instrumentation on
   only around the measured call; bucket self cost by subsystem (DB, serialization, allocation,
   handler logic, VCS, compression) and record the inclusive share of known entry points.
3. **Syscalls** — `strace -c -f` counts per scenario; `newfstatat`/`openat`/`read` storms are tree
   walks and unbuffered reads.
4. **Frames and threads** (UI) — frame times against the budget, long main-loop iterations, what
   ran in them (`perf-perceived`, and the toolkit's own frame tools in `perf-profiling`).

`perf-profiling` has the commands. Keep raw output (JSON, logs, folded stacks) in
`runs/<date>/` beside the reading.

## 4. Attribute: the work or overhead?

For each path, say in one line **where the cost goes**, then classify:

- **expected** — it is the work, at a sane cost (a checkout is a checkout).
- **reducible** — overhead around the work: compiling the same SQL per row, re-validating
  something already known, a subprocess where a library call would do, a per-line object where a
  per-file one would do, an O(n) walk per event, a lock held across a slow read.
- **wrong** — work that should not happen at all: hidden reloads, idle polling, snapshots of
  unchanged data, re-renders of unchanged rows, invisible retries.

Patterns that recur in every audit:

| Smell | Typical tell in the profile |
|---|---|
| N+1 queries / per-row compilation | `prepare`/parser self cost dominating; linear in rows |
| Slow work under a shared lock or on the UI thread | the hot request's latency jumps *during* the slow one |
| Subprocess where a library would do | wall ≫ CPU, `fork`/`execve`/`wait4` in strace, poll backoff |
| Re-validating on every call | the same stat/open/parse at the top of every handler |
| Content hashing where stat would do | sha1/sha256 and memset at the top of a status walk |
| Allocation churn | `malloc`/`free`/`memcpy`/`memset` > 15 % self; allocs ≫ result size |
| Per-item setup in an inner loop | constructor (searcher, regex, formatter) in the hot self list |
| Page faults on fresh buffers | wall and CPU ≫ instructions; huge bytes allocated |
| Refetch storms | one event → many identical queries in the trace |
| Idle wakeups | timers firing with nothing to do; CPU at rest > 0 |

## 5. Rank

Score each finding by **(ms saved per occurrence × occurrences per day) and by blast radius**:
work under a shared lock or on the UI thread stalls everything else and ranks above its own
duration. Then by confidence and cost to fix. Estimate each fix in milliseconds before doing it
(the claude.ai sprint estimated ~20 projects in ms and summed them into targets); the estimate is
checked against the after-measurement and keeps the ranking honest.

## 6. Write the reading

Use `reference/report-template.md`. The shape that works:

1. One paragraph: is it fast where it matters, and what stands out.
2. **Headline table** — the paths a person feels: p50, p95, CPU, verdict (expected / reducible
   with a § link / the work).
3. **What is worth fixing, in order** — per item: *Cost* (numbers), *Where it goes* (attribution),
   *Why it matters* (who waits), *Fix* (concrete) and *Expected* (a number).
4. Everything else, briefly, so the next reader knows it was looked at.
5. Method and machine; how to rerun; the raw run directory.

## 7. Fix in measured, guarded steps

For each item, in rank order: write or find tests that pin behavior → make one change → rerun the
same scenarios → keep it only if it moved beyond noise → add the guardrail (ratchet on the
deterministic count, a "during" scenario for a lock, a frame budget test) → next. Then write
`FIXES-<date>.md`: a before/after table with the *what changed* column, then file by file. Note
anything the harness itself got wrong and how it was corrected — scenarios that were measuring the
wrong thing are a finding too.

## 8. Close

The audit is done when: every path has a number, every reducible item is fixed or ticketed with an
estimate, every fix has a guardrail, and the next person can rerun the whole thing with one
command. What was not measured (p99, very large data, a platform not available here) is listed
explicitly.
