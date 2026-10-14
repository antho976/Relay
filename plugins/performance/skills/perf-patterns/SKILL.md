---
name: perf-patterns
description: The performance patterns that pay off in almost any codebase, whatever the language - do less work (skip, cache, memoize, invalidate correctly), do it once (hoist setup out of loops, batch, avoid N+1 queries, compile once), do it in the right place (off the UI thread and outside shared locks, in a worker, deferred, lazily), do it in the right shape (data structures, allocation and copies, string representations, I/O buffering, streaming instead of whole-file), avoid work nobody sees (idle polling, hidden reloads, re-renders of unchanged items, refetch storms), and the database, subprocess, network and startup habits that keep paths cheap. Use when fixing a hot path a profile has already pointed at, when reviewing code for performance, or when writing new code on a path people wait on.
---

# Patterns that pay off everywhere

Apply a pattern only to a path a measurement pointed at (`perf-profiling`); each one has a cost in
complexity. Ordered roughly by how often they are the answer. The stack skills (`perf-rust`,
`perf-sqlite`, `perf-gtk`, `perf-android`) go deeper on the same ideas for their platforms.

## 1. Do less work

- **Don't do it.** The biggest wins remove work nobody needed: a reload left in from debugging,
  a status recomputed for badges nobody looks at, a validation of something already known, a
  full refresh on an event that changed one row. Ask of every expensive call: who uses the
  result?
- **Do it once.** Cache derived values keyed on what they derive from (a file's mtime and size,
  a commit id, a row's version, the input's hash). Every cache needs a clear invalidation rule
  written next to it; a cache with a vague rule is a correctness bug waiting.
- **Memoize finished parts**: a stream, a document, a list — only the part still changing is
  recomputed. O(n) work per update makes the whole O(n²).
- **Compare cheaply first**: a length, a first byte, a stat, a version number before a full
  compare, a regex, a content hash. (A first-character check before a regex: −44 % wall.)
- **Resolve once**: the same lookup three times in one pass — hash map, dictionary, DOM query,
  path canonicalization — is common and invisible in review. Resolve to a local and pass it down.

## 2. Hoist and batch

- **Hoist setup out of loops**: compiled regexes, searchers, formatters, prepared statements,
  encoders, buffers. A constructor in a profile's hot self list means it runs per item.
- **Batch round trips**: N queries for N rows → one query over the id set; N writes → one
  transaction; N small messages → one frame; N syscalls → one larger read. Round trips cost
  more than the work in them.
- **Kill N+1 patterns** everywhere they hide: per-row sub-queries in list hydration, per-item
  network fetches, per-file subprocesses, per-widget style lookups.
- **Coalesce events**: a burst of change notifications becomes one refresh at the end of the burst
  (debounce for "settle", throttle for "steady cadence", frame-aligned for UI).

## 3. Keep the hot thread free

- **UI main thread / event loop**: nothing that can take more than a few milliseconds — disk,
  network, subprocesses, large parses, image decoding, syntax highlighting, big sorts — runs on it.
  Move it to a worker and post the result back; chunk long work into frame-sized slices. Detect
  violations (Android StrictMode, long-task observers on the web, frame-time logs on desktop).
- **Shared locks**: hold a lock only to read or write shared state, never across I/O or a slow
  computation. Shape: read inputs under the lock → release → do the slow part → re-acquire →
  check nothing changed → write. Measure it with a "during" scenario (the hot request while the
  slow one runs).
- **Async runtimes**: blocking calls inside an async task stall every task on that thread; send
  them to the blocking pool. Unbounded fan-out (spawn per item) needs a concurrency cap.
- **Backpressure**: a fast producer and a slow consumer need a bounded queue and a policy (drop,
  coalesce, slow the producer), or memory and latency grow without bound.

## 4. The right shape for data

- **Data structures**: a linear scan where a set/map lookup belongs (`contains` in a loop is
  O(n²)); a map where a dense array indexed by id would do; sorted data searched linearly.
- **Allocation and copies**: reuse buffers across iterations; reserve capacity when the size is
  known; borrow or slice instead of copying; avoid building intermediate collections only to
  iterate them once; avoid boxing per element. Allocation-heavy profiles show
  `malloc`/`free`/`memcpy`/`memset` in the top self entries.
- **String representation**: the encoding and layout a runtime picks can change the algorithm
  under you (one em dash pushed a JS engine's strings to UTF-16 and every regex onto a slower
  path; normalizing to the narrow form first was ~3× faster). Know your runtime's string
  representations, interning, and lossy conversions.
- **Serialization**: parse only what you read (lazy or streaming parsers, skipping unknown
  fields); don't serialize, then parse, the same data within one process; keep payloads small
  (only the fields a screen shows).
- **Locality**: iterate in memory order; struct-of-arrays for hot numeric loops; avoid pointer
  chasing in inner loops. Matters once instructions are low and wall is still high.

## 5. I/O, processes and the network

- **Buffer I/O**; read a file in one call sized from its length; stream large files instead of
  loading them whole; avoid `stat` storms (tree walks per request — keep an index or a watcher).
- **Subprocesses are expensive** (fork/exec, dynamic loading, the child's own startup, pipes):
  prefer a library call; when you must spawn, spawn once and keep it, always with a timeout. A path
  whose wall time is far above its CPU time is usually waiting on a child or the disk.
- **Network**: reuse connections (keep-alive, pooling, HTTP/2 multiplexing); compress; cache with
  validators (ETag); fetch in parallel when independent; never on the UI thread; timeouts on every
  call; batch small requests.
- **Watch, don't poll**: file watchers, change notifications, push. A poll loop is idle CPU and
  wakeups forever.

## 6. Databases

- **Prepare once, execute many**: statement caches; never build SQL per row.
- **Indexes for every filter and join on a hot path**; check with the engine's query plan
  (`EXPLAIN QUERY PLAN`, `EXPLAIN ANALYZE`). A scan over a growing table is a slope that becomes a
  cliff.
- **Transactions batch writes**; one transaction per user action, not per row.
- **Fetch only the columns and rows shown**; paginate; count with a query, not by loading.
- **Read columns by index in hot row mappers** if name lookup shows in the profile.
- **One connection behind a lock serializes everything** — keep statements short and slow work
  outside it (§3).
- **Reactive queries** (observers, live queries, Flows) re-run on every write to their tables;
  narrow what they observe and coalesce writes, or one write triggers a storm.

## 7. Work nobody sees

Search for it on purpose — no load metric reports it:

- idle timers, animations and polls running while hidden or minimized;
- background snapshots and saves of unchanged state;
- retries without backoff; hidden reloads and duplicate initializations;
- re-renders or re-layouts of items whose inputs did not change (count them per interaction);
- logging and tracing on hot paths at a level nobody reads (format cost even when filtered).

At rest, CPU and wakeups should be zero.

## 8. Startup

- **Measure the phases**: process start → runtime init → first frame → first useful frame →
  interactive. Optimize the one a person waits on.
- **Defer** everything not needed for the first useful frame; **lazy-load** pages, modules,
  plugins; **parallelize** independent init.
- **Avoid work proportional to the world at startup**: scanning all processes, all files, all
  rows, all repositories. Record what you need to check (pids, paths) and check only those.
- **Precompute** what is the same every launch (code caches, compiled resources, profiles for the
  runtime's ahead-of-time compiler).
- **Show the last known state from disk first**, reconcile after (`perf-perceived`).

## 9. Build and runtime configuration

Before tuning code, make sure the measurement is of an optimized build with the platform's
standard release settings (optimization level, link-time optimization, ahead-of-time or
profile-guided compilation where the ecosystem supports it, minification and code shrinking for
shipped bundles, logging at release levels). These are cheap, broad wins — and a debug build's
profile points at the wrong things.

## 10. When not to

- The path is not on a journey anyone waits on, or happens rarely.
- The win is inside the noise.
- The complexity (a custom cache, a build plugin, a hand-rolled data structure) costs more to
  maintain than the milliseconds are worth. Say so and stop.
