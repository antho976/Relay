---
name: perf-rust
description: Rust performance - build profiles for measuring and shipping (opt-level, LTO, codegen-units, panic=abort, debug line tables, frame pointers, PGO and BOLT with cargo-pgo, linkers), allocators (mimalloc, jemalloc), allocation reduction (with_capacity, reuse, clone_from, Cow, SmallVec), hashing (FxHash vs SipHash, clippy disallowed-types), type sizes, bounds checks, strings and formatting, iterators and collections, buffered I/O, Mutex and lock hold times, async blocking (spawn_blocking, rayon), serde costs, subprocess spawning (posix_spawn vs fork), and the Rust tools - gungraun (formerly iai-callgrind), criterion, divan, dhat, cargo-flamegraph and samply. Use for any Rust hot path, benchmark, profile or build-configuration question.
---

# Rust performance

The Rust Performance Book (Nicholas Nethercote) is the canonical reference; this skill is the
part that pays off most often. Measure each change on its own — several of these make things
slower in the wrong place.

## 1. Build profiles

Know which profile you are measuring. A project's dev profile may be partly optimized (for
example `opt-level = 1` for the workspace, `3` for dependencies) — say so in every number.

```toml
[profile.release]
debug = "line-tables-only"   # symbols and lines for profilers, small cost
# strip = false               # keep symbols if a later step strips

[profile.profiling]           # cargo build --profile profiling
inherits = "release"
debug = true

[profile.bench]
debug = true                  # gungraun / callgrind attribution
```

- **Frame pointers** for reliable stacks under `perf`/`samply`: `RUSTFLAGS="-C force-frame-pointers=yes"`
  (std ships without debug info; DWARF unwinding is the fallback: `perf record --call-graph dwarf`).
  With lld (the Linux default since Rust 1.90) or mold, add `-C link-arg=-Wl,--no-rosegment` for
  accurate perf stacks.
- **Max speed for shipping**: `codegen-units = 1`, `lto = "fat"` (often 10–20 %), `panic = "abort"`
  where unwinding is not needed, a faster allocator. `target-cpu=native` only when the binary
  never leaves that machine class.
- **PGO and BOLT** (`cargo pgo build` → run a representative workload → `cargo pgo optimize`,
  then `cargo pgo bolt build --with-pgo` → `cargo pgo bolt optimize`): often 10 %+, but an
  unrepresentative training run can make the binary slower. Do not strip before BOLT.
- **Faster builds** (developer time is performance too): mold or lld; `debug = "line-tables-only"`
  or `false` in dev (20–40 % faster compiles); `cargo build --timings` to find the critical path.

## 2. Allocation

Allocation-heavy profiles show `malloc`, `free`, `realloc`, `memcpy`, `memset` near the top. Count
allocations per operation (a counting `#[global_allocator]`, `divan::AllocProfiler`, dhat) and
compare with the size of the result.

- `Vec::with_capacity` / `String::with_capacity` when the size is known (a Vec grows 0→4→8→16…: 20
  pushes are 4 allocations instead of 1). `reserve` before an `extend` of a known length.
- **Reuse a workhorse buffer** across iterations: hoist the `Vec`/`String` out of the loop and
  `clear()` it. `read_line` into one `String` rather than `lines()`. `clone_from` reuses the
  destination's allocation.
- `format!` always allocates; `write!` into a reused `String`. `to_string()` on a `&str` to compare
  or look up is an allocation for nothing — borrow (`HashMap<String, _>::get(&str)` works).
- `Cow<'static, str>` (or `&'static str`) for values that are usually literals.
- `SmallVec<[T; N]>` keeps small collections inline (every operation slightly slower);
  `ArrayVec` when the bound is hard. `Box<[T]>` instead of a `Vec` that never grows (two words,
  not three). `Arc<str>` for shared immutable strings cloned often.
- Avoid collecting into a `Vec` only to iterate it once — pass the iterator.
- `mimalloc` or `tikv-jemallocator` as `#[global_allocator]` can win broadly on allocation-heavy,
  multi-threaded programs; measure, and check RSS too.

## 3. Hashing and collections

- `std` `HashMap` uses SipHash 1-3 (HashDoS-resistant, slow for small keys). For keys that do not
  come from an attacker, `rustc-hash` (`FxHashMap`) or `ahash`; in rustc, FxHash beat the default
  by 4–84 % depending on the benchmark. Enforce with `clippy.toml`
  `disallowed-types = ["std::collections::HashMap", "std::collections::HashSet"]` where it applies.
- `BTreeMap` for ordered iteration and small maps; a sorted `Vec` + binary search for read-mostly
  data; a dense `Vec` indexed by id beats a map when ids are small integers.
- `contains` on a `Vec` inside a loop is O(n²) — build a set once.
- `entry()` API instead of `get` then `insert` (one hash, one probe).
- Types over 128 bytes are copied with `memcpy` on every move: box rare large enum variants,
  use `u32` indices instead of `usize` or pointers in big tables, check with
  `static_assertions::assert_eq_size!`.

## 4. Strings, iterators, bounds checks

- Search with `memchr` / `memchr::memmem::Finder` built once; `str::find(&str)` constructs a
  searcher per call — per line, that is a constructor in the hot loop.
- `regex`: compile once (`LazyLock`/`OnceLock`), prefer `is_match` when you do not need captures,
  and try a cheap literal prefix check first.
- `from_utf8_lossy` allocates when the input is invalid; avoid it on hot paths over large data
  (`/proc` environments, file contents) — work on bytes.
- Iterators instead of indexing remove bounds checks; slice once before the loop
  (`let s = &v[..n];`) or assert the range up front so the optimizer can prove it.
  `get_unchecked` is a last resort with a safety comment.
- `chunks_exact`, `windows`, `zip` keep the optimizer informed; `iter().sum()` on integers
  vectorizes where an indexed loop may not.

## 5. I/O and serialization

- `File` is unbuffered: wrap in `BufReader`/`BufWriter`, and `flush()` explicitly (drop swallows
  errors). `println!` locks stdout per call — lock once (`let mut out = stdout().lock()`).
- Read a whole file in one call sized from its metadata (`fs::read` does this); avoid
  `read_to_end` into a fresh buffer per file in a walk.
- `serde_json`: deserialize into borrowed types (`&'a str`, `Cow`) where the input outlives the
  value; `#[serde(skip_serializing_if = …)]` to shrink payloads; `to_writer` into a reused buffer
  instead of `to_string`; avoid `Value` round trips (`to_value` then `from_value`) on hot paths.
  JSON encode/decode is often 5–10 % of a request path — check the profile before reaching for
  another format.

## 6. Locks, threads and async

- **Hold a lock for the state change, not the work.** Read inputs under the guard → drop it →
  do the slow part → re-acquire → validate → write. Never hold a `MutexGuard` across I/O, a
  subprocess, a channel receive or an `.await`. A function that takes the guard should be visibly
  short.
- Measure contention, not just duration: the hot request's latency *while* the slow one runs
  (`perf-benchmarks` §6), `perf lock` / off-CPU stacks in `futex_wait`.
- `parking_lot` or std `Mutex` are both fine uncontended (tens of ns); contention is the problem,
  not the lock type. Shard state, use `RwLock` only for genuinely read-heavy data (it is slower
  uncontended), or move to message passing for a single owner.
- **Async**: keep ≤ 10–100 µs between `.await` points. Blocking I/O → `spawn_blocking` (cannot be
  cancelled once started; runtime shutdown waits for it). CPU-heavy work → `rayon` + a oneshot
  channel. A forever loop → its own thread. Cap fan-out with a semaphore or `buffer_unordered(n)`.
- `std::thread::scope` and `rayon` for data parallelism; check that the work per task beats the
  scheduling cost (≫ 10 µs).

## 7. Subprocesses

- A spawn is ~0.5–2 ms plus the child's own startup; wall ≫ CPU in a profile is usually a child.
  Prefer a library (gix/git2 for git, a crate for the format) on hot paths.
- `Command::spawn` uses `posix_spawn` (vfork-like, cheap even from a big parent) unless something
  forces `fork`+`exec`: a `pre_exec` closure, uid/gid/groups/chroot, an old glibc, or a `cwd` the
  libc cannot pass to `posix_spawn`. `fork` copies page tables, so from a parent with a large heap
  (a GUI, a big engine) it costs milliseconds — tens of ms at a few GB. Keep `pre_exec` off hot paths.
- Every child gets a timeout and has its output read concurrently (a full pipe deadlocks the
  child); poll with backoff only if there is no better wake-up (pidfd).

## 8. Rust tools

| Need | Tool |
|---|---|
| Deterministic instruction counts in CI | **gungraun** (iai-callgrind renamed in 0.17): `harness = false`, runner version must equal the library's; `cargo bench -- --save-baseline=main` on main, `--baseline=main --callgrind-limits='ir=1%'` on PRs (exit code 3 on regression) |
| Statistical wall-clock benchmarks | **criterion** (bootstrap CIs, saved baselines; its default 1 % noise threshold is too tight for CI — use 3–5 %), **divan** (light, `AllocProfiler`, `args = [...]`) |
| Hosted continuous benchmarking | CodSpeed (simulated instruction counts on CI, walltime on bare metal), Bencher (thresholds per test type) |
| CPU profile | `samply record ./bin` (Firefox Profiler UI), `cargo flamegraph` (perf + flame graph; on-CPU only) |
| Allocation profile | `dhat` (`dhat::Alloc` global allocator; testing mode asserts allocation counts), `heaptrack ./target/release/bin` (never `heaptrack cargo run`) |
| Spans and timelines | `tracing` spans with `tracing-tracy` or `tracing-chrome` behind a feature flag |
| Type sizes | `-Zprint-type-sizes` (nightly), `static_assertions` |

`cargo bench` and `cargo test --release` are not the same profile; benchmarks use `[profile.bench]`.
