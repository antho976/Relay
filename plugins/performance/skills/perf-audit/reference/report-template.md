# Report templates

Two documents per audit cycle, both kept in the project's performance folder (`docs/perf/` unless
the project already uses another). Raw evidence goes in `runs/<date>/` beside them.

---

## `BASELINE-<YYYY-MM-DD>.md`

```markdown
# <App> performance baseline — <date>

<What was measured, in one sentence: N ops, M non-op paths, the process at rest and under load,
memory over time, startup, each UI page.> Numbers are from `runs/<date>/` (raw JSON and a
generated report with every table); this document reads them, says where each cost goes, and
whether that is the work or overhead. The next baseline is compared against this one with
<compare command>.

In one paragraph: <is it fast where it matters; the two or three numbers that prove it; how many
items stand out as overhead; which ones block other work>.

## Headline: the paths a person feels

| path | p50 | p95 | CPU | verdict |
|---|---:|---:|---:|---|
| <launch to interactive> | | | | expected / reducible: <cause>, §1.N / the work: <what> |
| <one keystroke, end to end> | | | | |
| <open the main page> | | | | |
| <the list every change refreshes> | | | | |
| <N idle windows, 30 s> | **CPU %, wakeups/s** | | | |

## 1. What is worth fixing, in order

### 1.1 <path> <does what> <under what condition>

- **Cost:** <wall p50/p95, CPU, allocations, bytes, syscalls, instructions; how it scales (×10 rows)>.
- **Where it goes:** <inclusive shares of named functions; top self entries; syscall mix; what
  that says (e.g. wall ≫ CPU means waiting on a subprocess)>.
- **Why it matters:** <who waits: holds the lock / runs on the UI thread / on every keystroke /
  N times a day>.
- **Fix:** <concrete change, the shape of it, which existing pattern to copy>.
- **Expected:** <number after, and why>.

## 2. Everything else

<Short table or list of paths that are fine or are the work, so the next reader knows they were
looked at.>

## 3. Method

Machine: <CPU, cores, governor, RAM, disk, OS/kernel>. Build: <profile, flags>. Toolchain:
<versions>. Fixture: <size and shape>. Iterations: <n, warmup>. Instruments: <callgrind, strace,
perf, frame log>. Not measured: <list>.

Rerun: `<one command>`. Compare: `<one command>`.
```

---

## `FIXES-<YYYY-MM-DD>.md`

```markdown
# Fixes from the <date> baseline

What was changed, measured with the same harness on the same machine (<harness path>, <profile>).
Section numbers refer to `BASELINE-<date>.md`. Wall p50 per iteration unless stated.

## Before and after

| path | before | after | change | what changed |
|---|---:|---:|---:|---|
| <path> | 2.42 ms | 0.76 ms | 3.2× | §1.2: <one line> |
| <path that did not move> | 30 ms | 34 ms | – | <why: same work, now off the lock — see row N> |

<Any correction to the harness itself: scenarios that measured the wrong thing, and how they
measure now.>

## What changed, file by file

- `<path>` — <what, and why it is safe>.

## Guardrails added

- <ratchet / CI check / scenario> — fails when <condition>.

## Not done

- §1.N — <why not; estimate; ticket>.
```

---

## A thread or PR description for one fix

```markdown
**Journey:** <which, which surface>   **Owner:** <human>
**Before:** <metric> = <value> (<scenario>, <machine>, <profile>, n=<runs>)
**After:** <metric> = <value> (same)   **Wall-clock:** <before → after>
**Proxy check:** <the deterministic metric moved with wall-clock: numbers>
**Change:** <one paragraph>
**Risk / flag:** <kill switch or ramp, name, removal plan>
**Guardrail:** <ratchet or test added>
**Evidence:** <frame log, screenshots, recording for anything a person sees>
```
