# Case study: how claude.ai got three times faster in two weeks

Source: "How we made claude.ai faster" (claude.dev blog, August 2026). This is the method the
plugin teaches, read for what an agent can copy. The numbers are theirs.

## What happened

In a two-week sprint in August 2026, Claude (driven from one Slack channel, humans approving every
change) made the four journeys that are 95 % of activity on claude.ai and the desktop app about
3× faster at p75 — geometric mean over 13 measurements: 3.1×. More than 3 000 changes merged
with no customer-facing incident or rollback.

| Journey | Surface | Before → after (p75) | Speedup |
|---|---|---|---:|
| Launch | web, fresh load, to a typeable page | 3 085 → 550 ms | 5.6× |
| Launch | desktop cold start | 6 310 → 3 328 ms | 1.9× |
| Start a conversation | Claude Code desktop | 837 → 347 ms | 2.4× |
| Load a conversation | Cowork desktop (cloud) | 2 566 → 728 ms | 3.5× |
| Send a message (client side) | Cowork desktop | 928 → 48 ms | 19× |

## The moves worth copying

### 1. Pick journeys, then define each measurement the same way

Usage data (through the Datadog MCP server) named four journeys: launch, start a conversation,
load a conversation, send a message. Across surfaces that was 13 measurements. **Each one starts
at a user interaction and ends when the result is rendered, with client and server work
separated**, so every baseline is comparable and every millisecond is attributable.

About 20 projects were each estimated in milliseconds of expected impact; the estimates were
summed into sprint targets. 12 of 13 targets were hit by day three — and "the targets are not
the stopping point".

### 2. Make everything hill-climbable

Deploys were slower than iteration, so the lab had to stand in for the field. The rule: **once
something can be measured, it can be optimized** — so the highest-leverage work became finding
more things to measure. Deterministic proxies, a ladder of them:

- React commits per interaction
- function call counts from V8 precise coverage
- layout and style-recalculation counts
- DOM mutation counts
- for pure-JS hot paths, **CPU instruction counts under Valgrind** with `node --predictable`:
  one run, no statistics, compared against a checked-in baseline.

Every benchmark had two jobs: a lab metric to move, and a **CI guardrail with a ratchet** — a
ceiling that only goes down. CI fails a PR that raises the count on a guarded path; a daily job
lowers the ceiling whenever the count drops. **A proxy had to be proven to track wall-clock time**
or it was discarded, as were flaky ones. Wall-clock itself was too noisy to gate CI.

| Hot path | Change | Instructions | Wall | Speedup |
|---|---|---:|---:|---:|
| Message-tree assembly | the same id resolved three times through megamorphic lookups; now once | −48 % | −78 % | 4.6× |
| Status-line scanner | a cheap first-character check before the regex | −31 % | −44 % | 1.8× |

### 3. The loop

1. Someone opens a thread about one slow stretch, often with a recording.
2. Trace the flow; find or build a benchmark that reproduces it.
3. Get a lab result; open PRs sized for risk and review, user-visible changes behind a flag.
4. Watch the deploy; read field data segmented by build and platform.
5. Improved → ratchet the benchmark down. Not improved → flag off, iterate.
6. Find the next slow spot in the same journey.

### 4. Look under the coarse metrics

- **Layout shift.** CLS scored each sidebar shift at ~0.008, far under the 0.1 "good" threshold,
  so monitors missed rows popping in. Using the Layout Instability API directly, each shift was
  mapped to a named region and a phase (before first paint, after typeable). An integration test
  held the sidebar data until after first paint and failed on any shift in a named region: 20/20
  failures on main, 20/20 passes on the fix. Field data then showed **31 % of page loads moved
  something after the page was usable**.
- **Hook census.** 6 900 hooks and 900 store subscriptions re-rendered on every keystroke in the
  composer.
- **One CSS selector** (`:root:has()`) added 24 ms to every DOM change.
- **Invisible work.** A leftover `location.reload()` caused about half a million hidden reloads a
  day that no load metric saw. Idle tabs cloned identical cache snapshots into IndexedDB twice a
  minute on the main thread.
- **Representation costs.** Highlighting a code block froze the page ~1 s because a single em dash
  made V8 store the string as UTF-16, pushing every regex onto the two-byte path. Copying blocks
  into one-byte strings first: 1.0 s → 0.35 s, each later pass 100 → 40 ms. About 20 lines.

### 5. Exact frame budgets

Headless Chromium ticks at 60 Hz; a deterministic **120 Hz rig** drove frames through DevTools
begin-frame control — exactly 240 frames for 240 begin-frames at 8.33 ms — so "did this frame fit"
became an exact read. Stepping long streamed answers frame by frame: memoize finished blocks (no
O(message length) work per chunk), tokenize growing code fences in a worker, reveal tables cell by
cell. ~60 PRs: main-thread blocking for a long reply 750 → 200 ms, CPU to a third, 120 fps held
throughout. The rig became a nightly job.

### 6. Guardrails

- Automated review on every PR plus one human approval; **unit tests before optimizations**.
- ~200 short-lived flags in two weeks, each classified **kill switch** or **ramp**, more than half
  retired by the end.
- Risky changes: employees, then 1 %, then everyone.
- Brittle-by-design wins get tests. The static composer (HTML that is typeable before React
  hydrates) is generated by rendering the real component in jsdom; a test keeps them identical; an
  integration suite compares them at **14 viewport sizes within 1 px**; a keystroke test types
  through the handoff and fails on a lost or reordered key; the field reports handoff shifts to a
  tenth of a pixel.
- Lab tests cannot see everything. A 10 px shift on new tabs only came from Chrome's managed-browser
  footer (56 px) disappearing after a speculative prerender had laid the page out shorter — the
  composer sits at 18 % of the height, 0.18 × 56 ≈ 10 px. Fix: pin the layout across the resize, and
  add a test that simulates the prerender.

### 7. What the humans did

- **Ambition.** The model defaulted to caution — ticketing findings, hedging, padding estimates.
  "Please be braver" got a PR within the hour.
- **Taste.** Every thread had a named owner; any user-perceptible change came with before-and-after
  recordings for the owner to judge (fill a table cell by cell or row by row? skeleton at once or
  after 500 ms? is a word-by-word fade worth a fifth of the frame budget?).
- **Direction.** Narrow threads, one benchmark or journey each ("a hundred and fifty hammers
  seeking nails"); humans sequenced, merged conflicting threads and closed them at diminishing
  returns: "2 ms per send is not worth the complexity of maintaining this build plugin."

## Still open in their write-up

p95, journeys beyond the core four, very long conversations. Lab and ratchets hold the p75 gains;
the tail is the next sprint.
