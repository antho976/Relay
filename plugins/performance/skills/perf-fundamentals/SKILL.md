---
name: perf-fundamentals
description: Load first for any performance work, in any language or app - the measure-change-measure loop and the anti-methods to avoid, journeys and budgets from human perception thresholds (100 ms, 1 s, 10 s, 16.7/8.33 ms frames, RAIL, Core Web Vitals), the USE and RED methods for finding a bottleneck, percentiles and tail latency (why averages and averaged percentiles lie, coordinated omission, HdrHistogram), noise and deciding whether a change is real, geometric means for combining speedups, Amdahl's and Little's laws, latency numbers for back-of-the-envelope estimates, lab versus field data, the metric ladder and ratchets. Use before measuring, optimizing, reviewing or reporting anything about speed, responsiveness, memory, startup or battery.
---

# Performance fundamentals

## 1. The loop

1. **Name the journey** a person waits on and the number that describes it (start at their
   action, end at the result on screen).
2. **Measure it** — the current value, on a stated machine, build and scenario.
3. **Find where the time goes** with a profiler, not by reading code (`perf-profiling`).
4. **Change one thing** that removes the biggest cost.
5. **Measure again** the same way. Keep the change only if it moved beyond the noise.
6. **Lock it in** with a guardrail (`perf-benchmarks`), then return to step 3 for the next cost.

Stop when the remaining costs are the work itself or the returns no longer pay for the complexity.

**Start with a problem statement** (Gregg): what says there is a problem? Has it ever been good?
What changed — code, data, load, hardware? Can it be stated as a latency or a run time? Who is
affected? Half of all performance problems are solved by answering these honestly.

**Anti-methods** to recognise in yourself (Gregg's list): *streetlight* (running the tools you
know and looking only where they shine), *random change / drunk man* (tweaking until something
beats the baseline — the noise will oblige), *blame someone else* (handing it to the database,
the network, the OS without a measurement), *passive benchmarking* (run a tool, publish the
number, never analyse what limited it), and *traffic light* (an all-green dashboard taken as
proof of health).

**Performance mantras**, best first: don't do it · do it, but don't do it again · do it less ·
do it later · do it when nobody is looking · do it concurrently · do it cheaper.

## 2. Budgets from perception

| Budget | Why |
|---|---|
| 100 ms from input to visible response | feels instantaneous (Nielsen); RAIL: handle input in < 50 ms so the frame lands within 100 ms |
| 1 s | the flow of thought stays unbroken; no progress indicator needed |
| 10 s | the limit of attention; beyond it, progress, a way out, and the ability to do something else |
| 16.7 ms per frame (60 Hz), 8.33 ms (120 Hz) | animation, scrolling, streaming — and your code gets only part of it |
| 50 ms chunks | the longest a main thread should be busy without yielding (RAIL idle, web long tasks) |
| Web, at p75: LCP ≤ 2.5 s, INP ≤ 200 ms, CLS ≤ 0.1 | Core Web Vitals "good" thresholds |

Set a budget per journey before optimizing, written down where the team sees it. "As fast as
possible" has no end and no priority.

## 3. Finding the bottleneck

- **USE method** (resources): for every resource — CPU, memory, disk, network, locks, thread
  pools, file descriptors — check **Errors** first (quickest to read), then **Utilization** (busy
  % over an interval) and **Saturation** (queued work: run-queue length, swapping, disk wait
  queue, dropped packets, lock waiters). 100 % utilization is a bottleneck; sustained > ~70 %
  already queues; averages hide bursts at 100 %; any saturation can matter. A resource with low
  utilization, no saturation and no errors is exonerated — write that down, it narrows the search.
- **RED method** (requests): **Rate**, **Errors**, **Duration** (as a distribution) per endpoint
  or op. Google SRE's four golden signals add saturation; track the latency of errors separately
  ("a slow error is even worse than a fast error").
- **TSA method** (threads): split each important thread's time into executing, runnable (waiting
  for a CPU), paging, sleeping (I/O), lock and idle. Fix runnable or paging first if either is
  above ~10 %; the others say what to trace next.
- **On-CPU vs off-CPU**: if wall time ≫ CPU time, the path is waiting (I/O, a lock, a child
  process, the network, a timer, page faults) and a CPU profile will not show it. Profile the
  waiting (off-CPU analysis, strace, lock-wait counters).
- **CPU profile rule**: understand every function above 1 % of the profile before deciding what
  to change.
- **Drill down**: start at the journey, split into phases, take the largest phase, split again
  until the cost is one named thing.
- **Workload characterisation** first: who calls it, how often, with what sizes, how it grows.
  A fast path called 10 000 times beats a slow one called once.

## 4. Statistics that do not lie

- **Latency is a distribution.** Report p50 and p95 (p99 when there are enough samples), plus
  the max for interactive paths. A mean hides the tail; a mean of a bimodal distribution
  describes no request at all.
- **Never average percentiles** (across hosts, minutes or runs) — it is mathematically
  meaningless. Merge the raw data or histograms (HdrHistogram) and take the percentile of the
  merge.
- **The tail dominates at scale** ("The Tail at Scale", Dean & Barroso): with a 10 ms typical
  and 1 s p99 per server, a request fanned out to 100 servers is slower than 1 s **63 %** of the
  time. Per-call p99 becomes journey p50. The same holds inside one app: a page that waits on ten
  queries waits on the slowest. Over a session of a couple of hundred interactions, a person
  almost surely meets something worse than your p95 — keep the max for interactive paths.
  Hedged requests (a backup sent after the p95 has elapsed) cut a p99.9 from 1 800 ms to 74 ms
  for 2 % more load in their benchmark.
- **Coordinated omission** (Gil Tene): a load generator that waits for each response before
  sending the next stops sending exactly when the system stalls, so the stall is recorded once
  instead of for every request that would have queued. Tene's example — 1 ms responses, then a
  100 s freeze — reports p99.99 = 1 ms when the truth is 100 s. Test for it by freezing the
  process (Ctrl-Z) mid-run: if the percentiles barely move, the tool is lying. Use open-loop load
  (fixed arrival rate, wrk2) or correct for it (HdrHistogram's `recordValueWithExpectedInterval`).
  A closed-loop tool measures service time, not response time.
- **HdrHistogram** records latencies allocation-free in a few ns at fixed relative precision,
  and its histograms merge across threads and intervals — the right container for latency.
- **Noise**: run enough iterations to see the spread; interleave A and B runs (environment size
  and link order alone have flipped published conclusions); a difference smaller than the
  run-to-run spread is not a result. Compare distributions — Mann–Whitney U (Google Benchmark's
  `compare.py`; at least 9 repetitions), or a bootstrap CI (criterion) — and report the effect
  size with its interval, not just a p-value. Significant is not the same as important.
- **Minimum detectable effect**: for 80 % power at α = 0.05 you need roughly 16·(σ/δ)² runs per
  side, so halving the effect you can detect costs 4× the runs. If you cannot afford the runs,
  climb down to a deterministic metric.
- **A/A tests** (Meta's ServiceLab runs them every two hours): compare a build against itself to
  measure the setup's false-positive rate and noise floor before trusting any A/B result.
- **Combine speedups with a geometric mean** (claude.ai's 3.1× is the geometric mean of 13
  measurements); an arithmetic mean lets one large ratio dominate.
- **Field data is skewed**: segment by build, platform, device class, and cold vs warm before
  comparing; a mix shift can look like a regression.

## 5. Laws for estimating

- **Amdahl's law**: speeding up a part that is fraction *p* of the time by *s* gives
  1 / ((1 − p) + p / s). A 10× win on 20 % of the time is 1.22× overall. Measure *p* first.
- **Little's law**: items in the system = arrival rate × time in system. Queues, connection pools
  and thread pools are sized with it; latency rises sharply as utilization approaches 100 %.
- **Back of the envelope** before building anything, with orders of magnitude (current hardware,
  roughly): L1 hit ~1 ns, branch mispredict ~3 ns, L2 ~4 ns, main memory ~100 ns, mutex
  uncontended ~20 ns, syscall ~100 ns–1 µs, context switch ~2–5 µs, process spawn ~0.5–2 ms, NVMe
  random read ~20–100 µs, SSD fsync ~0.1–2 ms, same-datacenter round trip ~0.5 ms, cross-continent
  ~70–150 ms. If the estimate is already within budget, the design is fine; if it is 10× over,
  no micro-optimization will save it.

## 6. Lab and field

- **Field (RUM / telemetry)** is the truth about people: real devices, data sizes and networks.
  Slow to change, noisy, segmented.
- **Lab** is where you iterate: deterministic, fast, debuggable — but only as good as its
  resemblance to the field.
- Use the lab to iterate faster than deploys allow; confirm in the field per build and platform;
  feed every field surprise back into a lab scenario. Some problems exist only in the field
  (a browser prerender, a managed-device footer, a slow disk) — the field is how you learn they
  exist.

## 7. The metric ladder and ratchets

Wall-clock is what people feel and too noisy to gate on. Climb down to deterministic proxies —
instructions, allocations, syscalls, queries, renders, layouts, frames over budget — **prove the
proxy moves with wall-clock**, optimize it, and gate CI on it with a **ratchet**: a checked-in
ceiling per guarded path that fails a change raising it and is lowered when it drops. Once
something can be measured deterministically it can be climbed; finding more things to measure is
often the highest-leverage work there is. Details in `perf-benchmarks`.

## 8. What a performance claim contains

The journey and surface; the metric and where it starts and ends; before and after; p50 and p95
(or a CI); the machine, build profile and toolchain; the scenario and data size; the number of
runs; for a proxy, its wall-clock companion; what was not measured. Anything less is an anecdote —
useful as a lead, never as a result.
