---
name: perf-perceived
description: Perceived performance - what a person sees and feels, on any platform - response-time thresholds (100 ms / 1 s / 10 s), input-to-photon latency, frame budgets at 60/120 Hz, layout shift after the page is usable, streaming text smoothly, interactive-before-ready shells, optimistic updates with rollback, prefetch on hover or intent, skeletons and when to delay them, progress for long work, and keeping the thing being typed into stable. Use when a change is visible on screen, when something "feels slow" or janky but the numbers look fine, when designing loading, streaming or transition behaviour, or when measuring layout stability and frame pacing.
---

# Perceived performance

People do not feel your p50. They feel the keystroke that lagged, the row that jumped under their
cursor, the spinner that flashed for 80 ms, the reply that stuttered while streaming. Measure those
directly — coarse aggregates hide them (claude.ai's sidebar shifts each scored ~0.008 CLS, far
under the 0.1 "good" line, yet 31 % of page loads moved something after the page was usable).

## 1. Thresholds

| Budget | Meaning | Source |
|---|---|---|
| ≤ 100 ms | feels instantaneous; respond to input within it (RAIL: process input in < 50 ms so the frame lands in 100) | Nielsen; RAIL |
| ≤ 1 s | flow of thought kept; no progress indicator needed | Nielsen |
| ≤ 10 s | attention kept; beyond it show progress and let people do something else | Nielsen |
| 16.7 ms / 8.33 ms | one frame at 60 / 120 Hz, including the compositor's share — your work gets less | display |
| 200 ms | web INP "good" at p75 (interaction to next paint) | Core Web Vitals |
| 2.5 s | web LCP "good" at p75 | Core Web Vitals |
| 0.1 | web CLS "good" at p75 — too coarse for single regions; see §4 | Core Web Vitals |

At 60 Hz the platform needs part of each 16.7 ms frame, so plan on ~10 ms for your own work
(RAIL); at 120 Hz, ~4–5 ms.

Typing is the most sensitive. Dan Luu measured keypress-to-pixel at 30 ms on a 1983 Apple IIe and
70–200 ms on typical 2017 PCs; terminal emulators on one laptop ranged from 6 ms to 44 ms median,
and every one got worse at p99.9 under a background compile. Throughput does not predict latency,
and Ctrl-C must stay responsive under a flood of output. Treat a keystroke path as a hard budget
with a tail (p99.9 close to p50), not a percentile target.

## 2. Measure input to photon

- **In process**: timestamp the input event (the toolkit's event time, not your handler's entry),
  then the presentation time of the frame that shows the result (GTK frame clock / `gdk_frame_timings`,
  Android `FrameMetrics`/Choreographer, `requestAnimationFrame` + `PerformanceEventTiming` on the
  web). The difference is what the person waited for, minus display scan-out.
- **End to end**: a high-speed camera (240 fps phone video) or a hardware rig (Typometer-style for
  editors and terminals) — the only measurement that includes the compositor and display.
- **Per frame**: drive frames deterministically when you can. Chromium's DevTools begin-frame
  control stepped a 120 Hz rig exactly (240 frames for 240 begin-frames), turning "did this frame
  fit in 8.33 ms" into an exact read instead of a noisy one. On other toolkits, record per-frame
  timings and count frames over budget, not average fps.

## 3. Interactive before ready

The first moment the thing someone came to use **accepts input** matters more than when the page
is "done". Patterns:

- **Static shell**: render the input surface (composer, search box, terminal) from static markup
  or a cached snapshot so it is typeable while the app initializes; hand over without moving it or
  dropping a key. This is brittle by design — guard it: generate the static markup from the real
  component, test they never drift, compare the two across viewport sizes within 1 px, and type
  straight through the handoff in a test that fails on a lost or reordered key.
- **Keep it mounted**: do not tear down and rebuild the input or the main surface when switching
  between items; swap the content.
- **Restore last state from disk first**, then reconcile with the authority (optimistic replica).
- **Defer the rest**: anything not visible in the first frame loads after it (lazy pages, idle
  callbacks, low-priority sources).

## 4. Stability: nothing moves after the page is usable

Layout shift after the person can act is a correctness bug, not a polish item.

- **Measure per region and phase**: map each shift to a named region (sidebar, transcript, header)
  and a phase (before first paint, after interactive). On the web, use the Layout Instability API's
  `layout-shift` entries and their `sources`; on native toolkits, record widget allocations of the
  named containers per frame and diff them.
- **Test it**: hold the data for one region until after first paint and fail on any movement in
  any named region. A real fix flips such a test from fails-every-run to passes-every-run.
- **Usual causes**: content arriving late without reserved space (rows, avatars, names, a header
  row), a scrollbar appearing and narrowing the content, fonts swapping metrics, images without
  dimensions, a container sized to its content whose content changes, layout as a percentage of a
  height that changes (browser UI retracting after a prerender moved a composer 10 px).
- **Fixes**: reserve space (fixed row heights, placeholders the same size as the content), pin
  scrollbars or overlay them, size by the final content, animate only transform/opacity, pin layout
  across a known resize.

## 5. Streaming and incremental updates

- **Touch only what changes.** A stream that re-renders or re-parses the whole message per chunk is
  O(length) per chunk and O(length²) overall. Memoize finished blocks; only the tail block is live.
- **Move tokenizing/highlighting of the growing block off the UI thread** (worker, background
  thread), and highlight a finished block once.
- **Reveal at a steady cadence** aligned to frames, not to network packet arrival. Decide the unit
  (character, word, cell, row) with the owner — it is a taste call with a frame cost.
- **Measure blocking per reply**, not average fps: total main-thread time over budget during a
  long reply, worst single frame, frames dropped.

## 6. Waiting well

- **Skeletons** for content with a known shape, spinners for a single component. Under ~1 s,
  show neither (NN/g) — **delay them** (≈ 300–500 ms) or a flash of skeleton makes a fast load
  feel slower; avoid frame-only skeletons and heavy shimmer. Whether to delay is the owner's call
  — show both.
- **Optimistic updates** for actions that almost always succeed: show the result immediately,
  reconcile with the authority, roll back visibly on failure. Keep a queue so offline actions
  survive.
- **Prefetch on intent**: hover, focus, pointer-down, the next item in a list, the page a person
  usually opens next. Chrome's speculation rules are a good scale of eagerness: pointer-down
  (conservative), ~200 ms hover (moderate), ~10 ms hover (eager), on sight (immediate), each
  capped. Cancel when intent changes; cap concurrency; weigh the bandwidth and memory.
- **Progress** for anything past ~1 s: determinate when the total is known, with what is happening
  in words; cancelable past ~10 s.
- **Never block input to show a spinner.**

## 7. Evidence for the owner

Every user-visible change ships with before-and-after evidence the human owner can judge without
running it: a frame log or frame-time chart, screenshots at the same moment, or a short recording.
Taste calls (fill a table cell by cell or row by row; skeleton now or later; is a fade worth a
fifth of the frame budget) belong to the owner. Present the options with their cost in ms.
