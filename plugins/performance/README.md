# Performance plugin for Relay

Turn this plugin on for a project and every agent Relay launches in it works as a performance
engineer: it measures before it changes anything, optimizes numbers that can be climbed, and locks
every win in with a guardrail. It is off by default; switch it on per project from the **Plugins**
page or from the plugin key on the project's row in the sidebar. It is meant for any project — the
core skills name no language, framework or app — and goes deeper on the stacks Relay itself is
built from (Rust, SQLite, GTK 4, Android with Compose and Room).

The method is the one that made claude.ai about three times faster in two weeks in August 2026
(`docs/case-study-claude-ai.md`), cross-checked against the standard practice of performance
teams and the primary sources in `docs/sources.md`.

## What an agent gets

| Piece | Where it lands | When |
| --- | --- | --- |
| 10 performance skills | `.claude/skills/perf-*/` and `.agents/skills/perf-*/` in the project root and in every agent worktree (git-excluded), plus the provider homes for Codex | The moment you switch the plugin on, including for agents already running |
| Working rules (`instructions.md`) | The agent's session brief, injected into its system prompt | Every start, wake and resume |

There is no MCP server: the tools it relies on (`perf`, `valgrind`, `strace`, `hyperfine`, the
platform profilers and the project's own harness) are run from the shell.

## Skills

| Skill | Covers |
| --- | --- |
| `perf-fundamentals` | The loop, anti-methods, USE/RED, perception thresholds and budgets, percentiles and tail latency, noise and significance, Amdahl and Little, lab vs field, the metric ladder — load first |
| `perf-audit` | A whole-app sweep: journeys and paths, measurement definitions, a four-lens baseline, work vs overhead, ranking, the BASELINE and FIXES documents |
| `perf-benchmarks` | Deterministic proxies, proving they track wall time, micro vs scenario harnesses, statistical rigor, noise control, CI ratchets, invariant scenarios |
| `perf-profiling` | Sampling and instrumenting profilers, flame graphs, off-CPU and lock analysis, syscalls, allocations, memory, idle wakeups, UI frame tools |
| `perf-perceived` | Input-to-photon latency, frame budgets, interactive-before-ready shells, layout stability, streaming, skeletons, optimistic updates, prefetch |
| `perf-patterns` | Fixes that pay off in any codebase: do less, hoist and batch, keep the hot thread free, data shape, I/O and processes, databases, invisible work, startup |
| `perf-rust` | Rust: build profiles, allocation, hashing, strings, collections, locks and async, serde, I/O, profiling Rust specifically |
| `perf-sqlite` | SQLite: WAL and pragmas, statement caching, indexes and query plans, transactions, one connection behind a mutex, measuring SQLite |
| `perf-gtk` | GTK 4 desktop clients: the frame clock, jank sources, list views and recycling, CSS cost, async on the main loop, startup, idle wakeups, VTE |
| `perf-android` | Android: startup (TTID/TTFD), Baseline Profiles, Macrobenchmark, jank and Android vitals, Compose stability and recomposition, Room, battery |

## Project-specific depth

The skills tell an agent to read the project's own performance folder first. A project goes deeper
by keeping one: its journeys and budgets, its invariants (which lock, which thread), its harness
and how to rerun it, and its readings over time. Relay's is `docs/perf/` in the Relay repository.
