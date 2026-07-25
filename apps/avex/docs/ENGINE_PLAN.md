# Engine — The Conditioning Coach ("One Brain, Two Disciplines")

> **Revision 3** — verified against the codebase at **0.8.8.3, Room schema v31**. Rev 3 folds in
> `COACH_ENGINE_PLAN_AUDIT.md`: the shipped cardio machinery rev 2 planned to rebuild, the
> interference double-count, deferred build-time decisions now made, and mode behavior. Rev
> history is in Status; the body is the current plan.
> Sibling plans: `COACH_V3_PLAN.md` (rev 3), `WEAR_OS_PLAN.md` (shipped).
> **No Engine phase started** — none of the `domain/engine/` concepts exist (verified).

---

## Context

Cardio today is **tracked, not coached**. `CardioEntry` is a standalone manual log (type,
minutes, distance, `effort` easy/moderate/hard, manual `hrZone` "1"–"5", `intervalCount`, plus
`inclinePct`/`laps`/`elevationM`/`conditions` from v27–v28) — and `effort`/`hrZone` are **read by
zero advisors**. `domain/cardio/` computes pace, week aggregates, calories, HC step/route
matching — descriptive stats only. Meanwhile:

- Coach v3 declares a **conditioning goal type** and **cardio interference** as a ReadinessV2
  input — both waiting on a real conditioning model.
- Wear W3 (shipped) delivers **live HR during gym sessions** into `session_hr_sample` — a sensor
  with no cardio consumer yet; W5 (shipped) already matches watch-recorded HC sessions onto
  cardio entries and shows their HR on the hub (`CardioUiState.sessionHr`/`sessionWatch`).
- The market gap: running apps coach you like a runner; lifting apps ignore conditioning. Nobody
  programs conditioning **as a lifter** — placed around training days, interference-aware,
  minimum effective dose. That's the flagship.

Engine makes conditioning a first-class coached discipline inside the existing coach — not a
second app bolted on.

### What already exists (rev-3 corrections)

| Earlier claim | Reality |
|---|---|
| "Health Floor: 150 min/wk is new" | `WHO_WEEKLY_ACTIVITY_MIN = 150` already exists (`domain/cardio/CardioGuidelines.kt:9`), already renders as a meter (`ui/cardio/CardioComponents.kt:146`), and the user can already set their own target (`CARDIO_WEEKLY_TARGET_MIN`, `PreferencesDataStore.kt:252`). Health Floor **adopts** these — it must never become a third competing target |
| "Interference is a new readiness input" | `ReadinessAdvisor.kt:84-86` already deducts for non-rest cardio in the last 24 h. `ConditioningLoad` **replaces** that block (Coach M6); stacking both would double-count |
| "Retro-enrich past entries with HC session HR" | W5 already does the matching. The remaining work is zone **attribution** over the matched series |
| "Reuse W3's HR table (`sessionId` XOR `cardioEntryId`)" | Incompatible with the shipped schema — `SessionHrSample` has composite PK `(session_id, at_ms)` and a NOT-NULL CASCADE FK to `Session`. Cardio live-HR gets its own sibling table (E-C) |
| "Age is known from the profile" | No age, DOB or birthday exists anywhere — only `USER_SEX`. E-A captures it |

---

## Governing principles

### Principle 1 — One coach, two disciplines
There is no "cardio coach." Conditioning prescriptions flow through the same machinery as lifting
suggestions: pure planners over a snapshot, human reasons (+ `lessonId`), watcher-judged outcomes,
trust-gated ambition, one-tap undo. The Week Brief and Today Directive speak about both
disciplines in one voice. A user should never sense two brains.

### Principle 2 — Minimum effective dose, lifter-first
Engine's job is an aerobic base that serves training, recovery and health — never a running
program that colonizes the week. Prescriptions have floors AND ceilings; interval work is
rationed; more cardio is never the default answer. When lifting and conditioning collide, lifting
wins unless the goal portfolio says otherwise. Zone-2 dogma is explained (Academy), never enforced.

### Principle 3 — The sensor ladder (fail-soft at every rung)
Every Engine feature defines behavior at three rungs and degrades between them without ever going
silent-wrong:
1. **No sensors** — RPE + talk-test proxy zones (effort tags map to zone bands).
2. **Health Connect after the fact** — HC exercise sessions/HR enrich completed entries.
3. **Watch live HR (W3)** — real zones, live drift coaching, HR-anchored adaptation.

Absent sensors ⇒ simpler prescriptions, wider confidence bands, same surfaces. All thresholds live
in `AdaptThresholds`-style constants; all estimators are data-gated.

### Principle 4 — Same modes as the coach
Engine surfaces respect the three modes in `COACH_V3_PLAN.md` → "Modes":
**freestyle** (no program) — prescriptions still work; placement degrades to "not within 24 h of
your last hard session" since there is no upcoming leg day to place around; **coach off** —
compute and record inert, surface nothing, the cardio hub keeps its descriptive stats;
**vacation** — no prescriptions, no adherence verdicts.

---

## New domain concepts

**ConditioningProfile** (`domain/engine/ConditioningProfile.kt`, pure; prefs-backed): the personal
HR-zone model. Max HR from a user override, else the Tanaka age default (208 − 0.7×age) — which
requires the new age/DOB capture in E-A, with an explicit age-unknown path (no zone claims until
age or max HR is provided). Refinement upward from observed session maxes requires a **sustained
reading** (≥30 s above the current max), never a single wrist-HR spike — one artifact must not
silently shift every zone (ingest bounds of 25–240 bpm won't catch a 205-for-3-s artifact). Zones
via HR-reserve (Karvonen) using the HC resting-HR baseline already read for recovery. Exposes
`zoneFor(bpm)`, `bandFor(zone) → bpm range`, and the RPE↔zone mapping for rung-1 users
(easy≈Z2, moderate≈Z3, hard≈Z4+ — the existing `CardioEffort` codes become zone proxies).

**ConditioningLoad** (`domain/engine/ConditioningLoad.kt`, pure): TRIMP-lite session load =
duration × zone weight (manual zone, effort proxy, or HR-derived when available). Weekly acute
load + ramp rate. This is the number that finally makes `effort`/`hrZone` consumed data, and it is
**the one interference formula in the product**: ReadinessV2 consumes this pure function from
whichever phase ships first (Coach B1 or Engine E-A), and the existing 24 h cardio deduction
(`ReadinessAdvisor.kt:84-86`) is deleted in the same change — never a parallel
effort×zone×minutes reimplementation, never both.

**Conditioning goals** (join the Coach v3 catalogue as `CoachGoal` rows):
- **Health Floor** — the default pitch for lifters who do nothing. Metric: weekly minutes,
  trajectory, on/off-track. It **adopts the existing target**: `CARDIO_WEEKLY_TARGET_MIN` when the
  user has set one, else `WHO_WEEKLY_ACTIVITY_MIN` (150) as the reference — the same number the
  cardio meter already draws, now with a coach behind it. Setting the goal writes the pref; there
  is exactly one weekly-minutes target in the product.
  **Dedup rule**: HC steps count toward the floor as ambient base, but step-minutes overlapping a
  logged cardio entry never double-credit (subtract the overlap, or take max-of) — otherwise a
  logged 30-minute walk counts twice.
- **Base Build** — a zone-2 volume ramp toward a target (e.g. 3×40 min/wk); metric: weekly Z2
  minutes + (rung 3) pace-at-HR trend.
- **Work Capacity** — interval sessions/wk with structure; metric: completed interval volume +
  recovery-between-rounds trend. That trend is **HR-only**; the rung-1 proxy is completed interval
  volume alone, with no recovery claims.

Conflict-matrix entries ship with them (Work Capacity × aggressive strength peak = flagged and
sequenced). `WeightPhase` joins the matrix: a cut can flip the "lifting wins" default, since more
conditioning serves the deficit — flagged and sequenced, never silently resolved.

**ConditioningPlanner** (`domain/engine/ConditioningPlanner.kt`, pure): places the conditioning
week around the lifting week. Placement rules (all constants, all explained): hard intervals never
<24 h before a lower-body day; zone-2 lands post-lift or on rest days; weekly load ramp capped
(≈+10%); block deload weeks (Coach v3 Phase C) halve conditioning load too; HC steps count toward
the Health Floor as ambient base (dedup rule above). Placement needs the week's layout, so the
full rules run only in `WeeklySchedule` **weekday mode**; in sequence mode the app doesn't know
Thursday is leg day, so placement degrades to next-up-relative and the coach may pitch weekday
mode as the fix. Output: **CardioPrescription** rows — type, minutes, zone (bpm band at rung 3,
effort word at rung 1), structure (steady / N×work:rest intervals), reason + `lessonId`, and the
goal it serves.

**ZoneCoach** (`domain/engine/ZoneCoach.kt`, pure state machine + thin service): live in-session
zone coaching. Consumes the W3 HR stream; states in-zone / drifting-high / drifting-low with
hysteresis (the classic failure is zone-2 creeping into zone 3 — one quiet haptic and "ease off",
never nagging). Interval mode is **warm-up → N×work:rest → cool-down** with per-phase zone targets;
intervals without warm-up/cool-down segments are incomplete prescriptions. Surfaces: the phone
cardio-session screen; wrist screen + haptics via a new `/cardio/live` DataItem and `/cmd/cardio`
messages — an additive protocol rev (confirmed absent from the shipped v1 protocol, which is
exactly what the Wear plan's declared-future slot intended).

**AerobicBase estimator** (`domain/engine/AerobicBase.kt`, pure, hard-gated): progress without a
lab. Inputs, best-available: pace-at-HR trend (same route/type sessions), within-session HR drift
at steady pace, resting-HR trend (already read). Output: a base trend (improving / holding /
detraining) with confidence — feeds goal trajectories and the Week Brief. Silent below data gates,
like every estimator in the app. Sessions tagged with `conditions` (HOT/COLD/RAIN/WIND, shipped
v28) are excluded or down-weighted in pace-at-HR trends: heat inflates HR exactly the way lesson
E4 teaches, and the estimator should practice what the lesson preaches.

**Watcher integration**: every applied conditioning suggestion (add a session, move a session, ramp
minutes) is judged in a window — did the user do it, did readiness/interference respond, did
lifting performance hold? Verdicts are **three-valued: worked / didn't work / not followed**;
"not followed" feeds dose reduction and re-planning, never trust demotion or bias folding
(skipping a Tuesday walk is user behavior, not bad advice). Only efficacy failures fold into bias,
exactly like gym suggestions. Storage for the third value is Coach v3 M2 — Engine consumes it and
does not invent a parallel scheme. **No unwatched writes** carries over verbatim.

**Academy track — "The Engine"** (E1–E6, written per phase): why lifters need an aerobic base ·
what zone 2 actually is (and the talk test) · interference and why placement matters · reading HR:
zones, drift, resting HR · intervals: dose and recovery · how Engine measures your base without a
lab. Each keyed to its first live coach moment. Rendered with the Coach v3 M5 lesson-block
renderer — no separate content pipeline.

---

## Phases (each independently shippable)

### Phase E-A — "Zones and load" (foundation, no coach behavior change)
- `ConditioningProfile` (prefs + pure math) + a zones settings row (max-HR override).
- **Age (or DOB) capture** in profile/settings — the Tanaka default needs it and no age exists
  today; explicit age-unknown path (no zone claims until age or max HR is provided).
- `ConditioningLoad` computed for all entries (manual zone → effort proxy → HR when available);
  the cardio hub shows weekly load + zone breakdown; `effort`/`hrZone` finally consumed. Extend
  `AdaptationSnapshot` with the conditioning-load series (additive).
- Zone **attribution** over the HR series W5 already matches onto entries (read-only, fail-soft).
- **No persistence, no migration**: enrichment and load are recomputed from the entries and the
  matched series on read. `ConditioningLoad` is pure and cheap; persisting it would buy nothing and
  cost a migration plus a staleness class of bug. (Rev 2 left this "decide at build time" — it is
  decided.)
- **Done when**: every existing cardio entry renders a load and a zone (or an explicit
  "no zone — set your age or max HR"), and coach behavior is byte-identical.

### Phase E-B — "The coached week"
- Conditioning goals join the portfolio (`CoachGoal`, from Coach v3 A2), adopting the existing
  weekly-minutes target per the Health Floor rule above.
- `ConditioningPlanner` + `CardioPrescription`: the planned conditioning week renders on the
  cardio hub ("Tue · 35 min zone 2 — serving: Base Build"); prescriptions one-tap log into
  `CardioEntry` (pre-filled), through existing write paths.
- Today Directive integration once Coach v3 B2 exists (rest-day directives become real
  prescriptions, and the dual-discipline secondary slot carries post-lift zone-2); until then the
  cardio hub is the surface.
- Watcher coverage for prescription outcomes (three-valued, per Coach M2). Academy E1–E3.
- **Done when**: a seeded lifting week produces a placed conditioning week that never puts hard
  intervals <24 h before a lower-body day, in weekday mode; and produces a defensible degraded
  week in sequence mode and in freestyle.

### Phase E-C — "Live zone coaching"
- `ZoneCoach` + a phone live-session screen for cardio (started from a prescription); wrist zone
  display, out-of-zone haptic and interval timer via the `/cardio/live` + `/cmd/cardio` protocol
  rev.
- Live HR persistence: a sibling **`cardio_hr_sample`** table mirroring the shipped
  `session_hr_sample` pattern (composite PK + CASCADE FK to `CardioEntry`). Schema bump with the
  locked migration pattern.
- Rung-1 fallback: the same screen runs as a structured timer with RPE prompts — no HR, no zone
  claims. Academy E4–E5.
- **Done when**: a rung-1 device runs a complete interval session (warm-up → work:rest →
  cool-down) with zero zone claims, and a rung-3 device shows live zone + drift with no alert
  flapping across a seeded HR trace.

### Phase E-D — "The base loop"
- `AerobicBase` live: pace-at-HR + HR-drift + RHR trends → a base trend on goal readouts and the
  Week Brief, with `conditions`-confounded sessions excluded.
- Prescriptions adapt: ramp / hold / deload conditioning volume from base trend + interference +
  block phase; conditioning enters the coach's weekly pass as a first-class section.
- `ConditioningLoad` formally feeds ReadinessV2 interference (if Coach B1 hasn't already wired it)
  and the deload drivers; the SignalRegistry's `conditioning` slot flips ACTIVE. Academy E6.
- **Done when**: a seeded 12-week history moves the base trend in the right direction and the
  weekly pass ramps or holds conditioning volume because of it — watcher-judged like any other
  decision.

### Declared future slots (not built)
- `hr_max_test` — a guided field test to calibrate max HR / zones.
- `event_prep` — conditioning toward a dated event (race, meet GPP block).

*(GPS cardio recorded from the wrist is declared in `WEAR_OS_PLAN.md` and owned there — one owner
per slot.)*

---

## Dependency map

| Engine phase | Needs | Enhanced by |
|---|---|---|
| E-A | age / max-HR capture (new) | HC HR grants (W5 matching already shipped) |
| E-B | Coach v3 A2 (`CoachGoal` + catalogue) | Coach v3 B2 (directive) |
| E-C | `/cardio/live` + `/cmd/cardio` protocol rev; `cardio_hr_sample` table | rung-1 fallback works without HR |
| E-D | E-A/E-B data history | Coach v3 B1 (readiness), C (blocks) |

Sequencing lives in `docs/ROADMAP.md`.

---

## Critical files

- **New**: `domain/engine/` (`ConditioningProfile`, `ConditioningLoad`, `ConditioningPlanner`,
  `CardioPrescription`, `ZoneCoach`, `AerobicBase`) · Academy lesson blocks (track: The Engine).
- **Touched**: `domain/cardio/` (`CardioEffort` zone mapping, `CardioWeekAggregate` load,
  `CardioGuidelines` as the Health Floor reference) · `data/db/entities/CardioEntry.kt` +
  `CardioDao`/`Migrations` (E-C only) · `data/repo/CardioRepository.kt` (prescriptions,
  enrichment) · `domain/adapt/` (`AdaptationSnapshot` conditioning-load series;
  `ReadinessAdvisor.kt:84-86` deduction removed when interference lands) ·
  `data/prefs/PreferencesDataStore.kt` (`CARDIO_WEEKLY_TARGET_MIN`, new age/max-HR keys) ·
  `data/health/HealthConnectManager.kt` · `ui/cardio/` (hub: planned week, load, zones; live
  session screen) · Wear protocol in `:shared` (+ `/cardio/live`, `/cmd/cardio`).
- **UI doctrine**: load the forge-design skill before any UI; the cardio hub already exists —
  Engine adds sections, no new hub.

## Verification

- All `domain/engine/` modules are pure → a unit corpus like `domain/adapt/`: zone math (Karvonen
  edges, missing RHR, absurd max-HR guards, the ≥30 s sustained-max rule), load weighting, planner
  placement (property: never schedules hard intervals <24 h before lower-body), ZoneCoach
  hysteresis (no alert flapping), AerobicBase gates (silent below N sessions, `conditions`-tagged
  sessions excluded).
- **Sensor-ladder audit per phase**: a rung-1 device (no watch, no HC) must see complete,
  zone-claim-free surfaces; coach behavior byte-identical until the E-D wiring lands.
- **Mode audit per phase**: freestyle, coach-off and vacation behavior declared and tested
  (Principle 4).
- **Duplicate-signal audit**: when interference lands, assert the old 24 h cardio deduction no
  longer fires and the Health Floor reads the single weekly-minutes target.
- **Watcher regression**: every prescription type judgeable in a window (worked / didn't work /
  not followed) — no unwatched writes.
- Migration tests if/when schema changes (E-C); end-to-end: seed lifting + cardio history →
  planner places a sane week; manual `/run` for UI phases.

## Status

- **Rev 1** (2026-07-15): plan authored alongside `ROADMAP.md`.
- **Rev 2** (2026-07-24): re-baselined at 0.8.8.3 / schema v31 — Wear W0–W6 shipped, birthday
  claim corrected, E-C HR-storage design corrected, interference single-sourced, Health-Floor
  dedup, HR-artifact guard, `conditions` handling, interval warm-up/cool-down, `WeightPhase`
  conflicts, three-valued verdicts.
- **Rev 3** (2026-07-24): audit applied (`COACH_ENGINE_PLAN_AUDIT.md`) — Health Floor now adopts
  the shipped WHO meter and `CARDIO_WEEKLY_TARGET_MIN` instead of competing with them;
  interference explicitly replaces the existing readiness deduction; E-A's persistence question
  decided (no persistence, no migration); the dead standalone-goal hedge and the duplicated
  `outdoor_watch_cardio` slot removed; mode behavior (Principle 4) and per-phase "done when"
  added; rev-commentary flattened.
- **Built** (2026-07-24), domain-complete across E-A → E-D:
  - **E-A**: `ConditioningProfile` (Karvonen zones, Tanaka estimate, explicit age-unknown path
    making NO zone claims, sustained-reading guard against wrist-HR artifacts), `EffortZones`
    (the rung-one proxy + talk test), `ConditioningLoad` (TRIMP-lite; `effort`/`hrZone` finally
    consumed), age and max-HR capture in settings prefs, and `ConditioningRepository`.
  - **E-B**: `ConditioningPlanner` — floors and ceilings, rationed intervals that always carry a
    warm-up and cool-down, deload weeks halving conditioning, sequence-mode placement claims
    suppressed, the health-floor step dedup rule, and illness/layoff suspending it entirely.
  - **E-C**: `ZoneCoach` — a pure hysteresis state machine (one alert per drift episode, silent
    without zones, a dropped stream is not a drift) plus interval segmentation for the timer.
  - **E-D**: `AerobicBase` — pace-at-effort and resting-HR trends triangulated, with
    `conditions`-tagged sessions excluded so a hot week never reads as lost fitness.
  - **Interference is single-sourced**: `ReadinessAdvisor` consumes
    `ConditioningLoad.interferencePenalty` and its own cardio rule was deleted.
  - Academy track E1–E6 written and wired to real cardio moments. 31 unit tests, all green.
- **Remaining**: the UI surfaces — the planned week and zone breakdown on the cardio hub, the live
  zone-coaching session screen, the `/cardio/live` + `/cmd/cardio` wear protocol rev, and the
  `cardio_hr_sample` table (E-C's storage decision stands: a sibling table, not a migration of
  `session_hr_sample`).
