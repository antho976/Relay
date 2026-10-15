---
name: perf-profiling
description: Find where time, memory and wakeups go - choosing between sampling and instrumenting profilers, building with symbols and frame pointers, perf record/report/stat and flame graphs (reading them correctly), samply and the Firefox Profiler, callgrind attribution by subsystem and inclusive entry point, off-CPU and lock-wait analysis (offcputime, perf sched, bpftrace), strace syscall counts, allocation and heap profiling (heaptrack, dhat, massif), memory over time, idle CPU and wakeups (powertop, perf stat), Netflix's 60-second checklist, tracing spans, and the platform UI tools (GTK frame timings and sysprof, Android Perfetto, Chrome DevTools). Use when a path is slow and you need to know why before changing code.
---

# Profiling

Profile before proposing a fix, and state the attribution in one line ("61 % of the instructions
are compiling SQL", "wall is 10 ms but CPU 1.5 ms: waiting on a child process") before reading
code for solutions.

## 1. Pick the instrument

| Question | Instrument |
|---|---|
| Where does CPU time go, in the real build, cheaply? | **sampling**: `perf record`, `samply`, platform profilers |
| Exactly how many instructions each function costs, deterministically? | **instrumenting**: callgrind (≈ 20–100× slower; fine for attribution, never for timing) |
| Why is wall time ≫ CPU time? | **off-CPU**: `offcputime`, `perf sched`, `strace -T`, lock-wait stacks |
| Which syscalls, how many? | `strace -c -f`, `perf trace -s` |
| Who allocates, how much, what leaks? | heaptrack, dhat, massif, a counting allocator |
| Why does it wake up when idle? | `perf stat` at rest, powertop, `perf trace` timers |
| What happened over time in one interaction? | tracing spans → timeline (Perfetto, Tracy, Chrome trace), platform frame tools |

**Build for it**: the optimized build people run, plus line tables / symbols and frame pointers
(`-fno-omit-frame-pointer`, Rust `-C force-frame-pointers=yes`, JVM
`-XX:+PreserveFramePointer`). Without frame pointers, use DWARF (`--call-graph dwarf`) or LBR.
JIT runtimes need a perf map (`--perf-basic-prof` for Node, `perf-map-agent` for the JVM).

## 2. The first 60 seconds on a busy box (Netflix)

`uptime` · `dmesg | tail` · `vmstat 1` · `mpstat -P ALL 1` · `pidstat 1` · `iostat -xz 1` ·
`free -m` · `sar -n DEV 1` · `sar -n TCP,ETCP 1` · `top`. Errors and saturation first (USE,
`perf-fundamentals`). Tells you whether it is CPU, memory, disk or network before any profiler.

## 3. Sampling: perf and samply

```fish
perf record -F 999 -g -- ./bin args          # or: -p PID -- sleep 10
perf report --no-children --sort symbol      # self cost
perf report --children                       # inclusive
perf script | stackcollapse-perf.pl | flamegraph.pl > cpu.svg
samply record ./bin args                     # opens the Firefox Profiler: call tree, flame graph, timeline per thread
perf stat -e task-clock,cycles,instructions,cache-misses,context-switches,page-faults -r 10 -- ./bin
```

- `perf_event_paranoid` must be ≤ 1 for user profiling (`sysctl kernel.perf_event_paranoid=1`).
- Sample enough: thousands of samples on the path of interest. Loop the scenario if it is short.
- `perf stat` gives IPC (instructions/cycles): low IPC (< 1) with high cache misses is memory-bound;
  many page faults on a short path means fresh buffers being zeroed.
- VMs and containers often lack hardware counters; software events (`task-clock`) still work.

**Reading a flame graph**: the x-axis is the sorted population of stacks, **not time**; width is
the share of samples; the y-axis is depth. Look for wide plateaus (self time) and wide towers
under one caller (that caller's inclusive cost). A differential flame graph (before vs after)
shows what a change moved. A flame **chart** (time on x) is a different view — use it for
"what happened during this frame".

## 4. Instrumenting: callgrind attribution

Callgrind gives exact, repeatable instruction counts per function — ideal for "where does the cost
go" and for comparing before and after.

```fish
valgrind --tool=callgrind --collect-atstart=no --toggle-collect='*measured_entry*' \
  --callgrind-out-file=cg.%p.out ./bin
callgrind_annotate --inclusive=yes cg.*.out | head -50
callgrind_annotate cg.*.out | head -50       # self cost
kcachegrind cg.*.out                         # browse callers/callees
```

Make the result readable by **bucketing self cost by subsystem** (database, JSON, allocation,
regex, compression, hashing, VCS library, your handler logic) and listing the **inclusive share of
named entry points** (SQL compile vs execute, auth, validation, the VCS call). Inclusive shares
overlap by construction — say so. Callgrind cannot see waiting, I/O time, or page-fault cost:
pair it with a native wall/CPU number.

**Where Valgrind cannot run** — a distribution that ships x86-64-v4 packages (CachyOS) builds the
dynamic loader with AVX-512, and every program dies with SIGILL under Valgrind — get the same
two answers from the CPU:

- **Counts**: `perf_event_open` a `PERF_COUNT_HW_INSTRUCTIONS` counter per thread (user space
  only, `inherit` for threads started later, opened disabled), and switch them all with
  `prctl(PR_TASK_PERF_EVENTS_ENABLE/DISABLE)` around the measured call. That is two syscalls per
  iteration, native speed, and repeatable to about 0.1 %. A second set with `inherit` to child
  processes counts the subprocesses the call started. `perf stat -e instructions:u` gives the
  whole-process figure.
- **Attribution**: `perf record -e instructions:u -c <prime> --call-graph fp -k CLOCK_MONOTONIC`
  on a build with `-C force-frame-pointers=yes` (and `CFLAGS=-fno-omit-frame-pointer` for C
  dependencies). DWARF unwinding copies kilobytes of stack per sample and still stops short in
  Rust code. Start it with `-D -1 --control fifo:ctl,ack` and enable it only around the loop,
  write each measured region's monotonic bounds from the program, and keep only the samples
  inside a region. `perf script --no-inline` names real functions, as callgrind does. About
  1 200 samples per scenario puts a large share within a few points run to run. Keep the sample
  rate under `perf_event_max_sample_rate` (a fast core retires 15 G instructions/s, so a period
  of 400 000 rather than 20 000), or the kernel throttles and samples go missing; compare the
  sampled total with the counted one to catch it.

## 5. Off-CPU and locks

If wall ≫ user + sys, the thread is blocked. Find on what:

```fish
sudo offcputime-bpfcc -df -p PID 10 > off.stacks       # bcc; then flamegraph.pl --color=io --countname=us
sudo perf sched record -g -- ./bin; perf sched timehist -MVw
sudo bpftrace -e 'tracepoint:sched:sched_switch { @[kstack, ustack] = count(); }'
strace -f -T -e trace=%process,%file,read,write,futex -p PID    # per-call durations
```

- Idle worker threads dominate off-CPU output; filter to the thread or stacks of the request.
- A blocked stack shows where a thread waited, not who woke it (wakeup graphs answer that).
- Lock waits appear as `futex` waits; contention shows up as the hot path's latency rising while a
  slow path runs — measure that scenario directly (`perf-benchmarks` §6).
- A child process: `wait4`/`poll` with backoff in the parent; the child's own time is elsewhere —
  `strace -f` follows it.

## 6. Syscalls

`strace -c -f ./bin` (summary), `strace -f -e trace=file` (which paths), `perf trace -s` (lower
overhead). Read the mix: thousands of `newfstatat`/`statx` = a tree walk or root re-validation;
many small `read`s = unbuffered or probing reads; `openat` of the same file repeatedly = a missing
cache; `clone`/`execve`/`wait4` = subprocesses; `futex` = contention; `poll`/`epoll_wait` with short
timeouts at rest = a timer loop.

## 7. Memory

- **Allocations**: heaptrack (`heaptrack ./bin`, then `heaptrack_print` or the GUI: hotspots,
  temporary allocations, leaks), dhat (also asserts counts in tests), massif (`valgrind
  --tool=massif`, heap over time), a counting allocator for per-operation numbers.
- **Over time**: run a soak (thousands of mixed operations) and sample RSS and live heap bytes;
  flat is the goal. Growth that follows the operation count is a leak or an unbounded cache.
- **RSS vs heap**: RSS includes fragmentation, mapped files and caches; compare with live heap
  before blaming a leak. `smem`/`/proc/PID/smaps_rollup` for PSS across processes.
- UI toolkits leak through reference cycles in callbacks (closures capturing their own widget):
  check object counts after opening and closing a page N times.

## 8. Idle: CPU and wakeups

At rest, CPU should be 0 % and wakeups near zero.

```fish
perf stat -e task-clock,context-switches,cpu-migrations -p PID -- sleep 30
grep -E 'voluntary_ctxt_switches' /proc/PID/status         # before and after 30 s
sudo powertop --time=20                                     # WakeUp tab: timer sources per process
sudo perf trace -p PID -s -- sleep 10                       # which syscalls fire at rest
```

Usual sources: polling loops, animations running while hidden, cursor blink, periodic refreshes,
short `poll` timeouts, heartbeat timers. Coalesce timers (timer slack defaults to 50 µs; longer
intervals and aligned deadlines batch wake-ups) and stop timers when nothing visible needs them.

## 9. Spans and timelines

Instrument the journey with named spans (OpenTelemetry, `tracing`, Android `trace("…") {}`,
`performance.mark/measure`) and view them on a timeline (Perfetto UI, Tracy, Chrome's trace
viewer). Spans answer "what ran during this slow frame" — sampling cannot. Keep span overhead off
tight loops (Android sections cost ~10 µs each).

## 10. Platform UI tools (pointers)

- **GTK**: `GDK_DEBUG=frames` frame log; the Inspector's recorder (render nodes per frame) and
  frame-timing graph; **sysprof** captures CPU samples together with GTK's own marks (layout,
  snapshot, render, CSS) and the compositor. See `perf-gtk`.
- **Android**: Perfetto system traces (FrameTimeline shows missed frames and why), Android Studio
  CPU/memory profilers, Layout Inspector recomposition counts. See `perf-android`.
- **Browser**: DevTools Performance panel (long tasks, layout/style recalc, INP breakdown),
  `PerformanceObserver` for `longtask`, `layout-shift`, `event`; React Profiler for commits.
- **Any compositor**: input-to-photon needs the display — a high-speed camera, or a
  Typometer-style rig for text input.
