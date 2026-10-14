# How an agent works on performance

This is the loop the plugin's instructions and skills teach. It is written for agents; it is also
what a human reviewer should expect to see in every performance change.

## 1. Orient

1. Load `perf-fundamentals`, then the skill for the task (`perf-audit` for a sweep,
   `perf-profiling` and `perf-patterns` for a fix, `perf-perceived` for anything on screen), plus
   whatever the project and its ecosystem document for the stack at hand.
2. Find what exists: the project's performance folder and harness, its last baseline, CI
   benchmark jobs, telemetry. Reuse before writing.
3. Read the project's invariants — the lock everything waits on, the thread a person feels, the
   request that must stay off the database.

## 2. Pick one journey and one number

Name the journey, the surface, and the measurement: where it starts (a person's action) and where
it ends (the result on screen). Get the current number from the field if there is one, from the
harness if not, and write it down with the machine and the build profile.

## 3. Make it hill-climbable

| Step | Done when |
|---|---|
| Reproduce the slow path in a benchmark or a scenario | it runs with one command and shows the same cost |
| Find a deterministic proxy (instructions, allocations, syscalls, statements, renders, layouts, frames over budget) | it gives the same value on every run |
| Prove the proxy tracks wall-clock | moving it moves wall time in the same direction, by a believable amount |
| Check in a baseline for the proxy | CI can compare against it |

## 4. Find where it goes

Profile before reading code for fixes: sampling for wall and CPU (`perf`, `samply`, the
platform's profiler), instrumentation for exact attribution (callgrind), `strace -c` for
syscalls, frame logs for UI, allocation profilers for churn. State the attribution in one line —
"61 % of the instructions are compiling SQL" — before proposing anything.

## 5. Change one thing

Tests that pin the behaviour first. Then the smallest change that removes the cost. Rerun the
same scenario; keep the change only if the number moved beyond the noise. Risky user-visible
changes go behind a flag classified as a kill switch or a ramp.

## 6. Lock it in

Lower the ratchet for the proxy; add a scenario for any invariant the fix restored (the hot
request *while* the slow one runs); add a layout-stability or frame-budget test for anything a
person sees.

## 7. Report

Before and after, same machine, same scenario, same profile; the proxy and wall-clock both;
evidence for anything visible; what was not verified. Append to the project's FIXES document if
it keeps one. Then look for the next slow spot in the same journey — the target is not the
stopping point; diminishing returns are.

## What a reviewer should check

- Is there a before and an after, from the same machine and profile?
- Does the metric start at a person's action and end at what they see?
- Is the proxy shown to move with wall-clock time?
- Is there a guardrail that would catch a regression next week?
- Does the change hold any lock or block any UI thread for longer than before?
- Is the complexity worth the milliseconds?
