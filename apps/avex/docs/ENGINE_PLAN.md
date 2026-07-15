# Engine — The Conditioning Coach ("One Brain, Two Disciplines")

> Session ground rules: planning only — no app code edits in this session. Deliverable is
> this plan plus `docs/ROADMAP.md`, committed on `claude/planning-session-a0yqrb`.
> Baseline: 0.8.8.2, Room schema v23. Sibling plans: `COACH_V3_PLAN.md` (coach brain),
> `WEAR_OS_PLAN.md` (wrist + live HR). Repo is a few small PRs behind — none touch cardio.

---

## Context

Cardio today is **tracked, not coached**. `CardioEntry` is a standalone manual log (type,
minutes, distance, `effort` easy/moderate/hard, manual `hrZone` "1"–"5", `intervalCount`)
— and the Coach v3 review already flagged that `effort`/`hrZone` are **read by zero
advisors**. The domain layer (`domain/cardio/`) computes pace, week aggregates, calories,
HC step/route matching — descriptive stats only. Meanwhile:

- Coach v3 declares a **conditioning goal type** (weekly cardio minutes / zone-2 base) and
  **cardio interference** as a ReadinessV2 input — both currently vaporware waiting for a
  real conditioning model.
- Wear plan W3 delivers **live HR during sessions** — a sensor with no consumer yet beyond
  post-hoc stats.
- The market gap: running apps coach you like a runner; lifting apps ignore conditioning.
  Nobody programs conditioning **as a lifter** — placed around training days,
  interference-aware, minimum effective dose. That's the flagship.

Engine makes conditioning a first-class coached discipline inside the existing coach — not
a second app bolted on.

---

## Governing principles

### Principle 1 — One coach, two disciplines
There is no "cardio coach." Conditioning prescriptions flow through the same machinery as
lifting suggestions: pure planners over a snapshot, human reasons (+ `lessonId`),
watcher-judged outcomes, trust-gated ambition, one-tap undo. The Week Brief and Today
Directive speak about both disciplines in one voice. A user should never sense two brains.

### Principle 2 — Minimum effective dose, lifter-first
Engine's job is an aerobic base that serves training, recovery, and health — never a
running program that colonizes the week. Prescriptions have floors AND ceilings; interval
work is rationed; more cardio is never the default answer. When lifting and conditioning
collide, lifting wins unless the user's goal portfolio says otherwise. Zone-2 dogma is
explained (Academy), never enforced.

### Principle 3 — The sensor ladder (fail-soft at every rung)
Every Engine feature defines behavior at three rungs, and degrades between them without
ever going silent-wrong:
1. **No sensors** — RPE + talk-test proxy zones (effort tags map to zone bands).
2. **Health Connect after-the-fact** — HC exercise sessions/HR enrich completed entries.
3. **Watch live HR (W3)** — real zones, live drift coaching, HR-anchored adaptation.

Absent sensors ⇒ simpler prescriptions, wider confidence bands, same surfaces. All
thresholds live in `AdaptThresholds` style constants; all estimators are data-gated.

---

## New domain concepts

**ConditioningProfile** (`domain/engine/ConditioningProfile.kt`, pure; prefs-backed):
personal HR zone model. Max HR from user override, else age default (Tanaka 208 − 0.7×age;
birthday already known from profile), refined upward by observed session maxes. Zones via
HR-reserve (Karvonen) using the HC resting-HR baseline already read for recovery. Exposes
`zoneFor(bpm)`, `bandFor(zone) → bpm range`, and the RPE↔zone mapping for rung-1 users
(easy≈Z2, moderate≈Z3, hard≈Z4+ — the existing `CardioEffort` codes become zone proxies).

**ConditioningLoad** (`domain/engine/ConditioningLoad.kt`, pure): TRIMP-lite session load =
duration × zone weight (manual zone, effort proxy, or HR-derived when available). Weekly
acute load + ramp rate. This is the number that finally makes `effort`/`hrZone` consumed
data: it feeds ReadinessV2's **interference** term and the deload drivers. (Coach v3
Phase B consumes it; until then it renders on the Cardio hub only.)

**Conditioning goals** (join the GoalPortfolio catalogue, `CoachGoal` rows):
- **Health Floor** — 150 min/wk moderate movement; the default pitch for lifters who do
  nothing. Metric: weekly minutes; trajectory + on/off-track.
- **Base Build** — zone-2 volume ramp toward a target (e.g. 3×40 min/wk); metric: weekly
  Z2 minutes + (rung 3) pace-at-HR trend.
- **Work Capacity** — interval sessions/wk with structure; metric: completed interval
  volume + recovery-between-rounds trend.
Conflict matrix entries ship with them (Work Capacity × aggressive strength peak = flagged,
sequenced — the machinery is Coach v3 Phase A's).

**ConditioningPlanner** (`domain/engine/ConditioningPlanner.kt`, pure): places the
conditioning week around the lifting week. Placement rules (all constants, all explained):
hard intervals never <24 h before a lower-body day; zone-2 lands post-lift or on rest days;
weekly load ramp capped (~+10%); block deload weeks (Coach v3 Phase C) halve conditioning
load too; HC steps count toward the Health Floor as ambient base. Output:
**CardioPrescription** rows — type, minutes, zone (bpm band at rung 3, effort word at rung
1), structure (steady / N×work:rest intervals), reason + `lessonId`, the goal it serves.

**ZoneCoach** (`domain/engine/ZoneCoach.kt`, pure state machine + thin service): live
in-session zone coaching. Consumes the W3 HR stream; states in-zone / drifting-high /
drifting-low with hysteresis (the classic failure: zone-2 creeping into zone 3 — one quiet
haptic + "ease off", never nagging). Interval mode: work/rest timer with per-phase zone
targets. Surfaces: phone cardio-session screen; wrist screen + haptics via the Wear
protocol (new `/cardio/live` DataItem + `/cmd/cardio` messages — additive protocol rev).

**AerobicBase estimator** (`domain/engine/AerobicBase.kt`, pure, hard-gated): progress
without a lab. Inputs, best-available: pace-at-HR trend (same route/type sessions),
within-session HR drift at steady pace, resting-HR trend (already read). Output: base
trend (improving / holding / detraining) with confidence — feeds goal trajectories and the
Week Brief. Silent below data gates, like every estimator in the app.

**Watcher integration**: every applied conditioning suggestion (add a session, move a
session, ramp minutes) is judged in a window — did the user do it, did readiness/interference
respond, did lifting performance hold? Failed prescriptions fold into bias exactly like
gym suggestions (`CoachGenBias` pattern). **No unwatched writes** carries over verbatim.

**Academy track — "The Engine"** (~6 lessons, written per phase): why lifters need an
aerobic base · what zone 2 actually is (and the talk test) · interference — why cardio
placement matters · reading HR: zones, drift, resting HR · intervals: dose and recovery ·
how Engine measures your base without a lab. Each keyed to its first live coach moment
(first prescription, first drift alert, first base-trend readout).

---

## Phases (each independently shippable)

### Phase E-A — "Zones and load" (foundation, no coach behavior change)
- `ConditioningProfile` (prefs + pure math) + zones settings row (max-HR override).
- `ConditioningLoad` computed for all entries (manual zone → effort proxy → HR when
  available); Cardio hub shows weekly load + zone breakdown; `effort`/`hrZone` finally
  consumed. Extend `AdaptationSnapshot` with the conditioning-load series (additive).
- Retro-enrich: match HC exercise sessions' HR (where granted) onto past `CardioEntry`
  rows for zone attribution (read-only enrichment, fail-soft).
- No Room migration needed unless enrichment is persisted — decide at build time; if
  persisted, v-next migration follows the locked pattern.

### Phase E-B — "The coached week"
- Conditioning goals join the portfolio (needs Coach v3 Phase A's `CoachGoal`/catalogue;
  if Engine ships first, a minimal standalone goal row is acceptable and migrates in).
- `ConditioningPlanner` + `CardioPrescription`: the planned conditioning week renders on
  the Cardio hub ("Tue · 35 min zone 2 — serving: Base Build"); prescriptions one-tap log
  into `CardioEntry` (pre-filled), through existing write paths.
- Today Directive integration when Coach v3 Phase B exists (rest-day directives become
  real prescriptions); otherwise Cardio hub is the surface.
- Watcher coverage for prescription outcomes. Academy lessons 1–3.

### Phase E-C — "Live zone coaching" (wants Wear W3)
- `ZoneCoach` + phone live-session screen for cardio (started from a prescription); wrist
  zone display + out-of-zone haptic + interval timer via Wear protocol rev.
- Live HR persists to the session (reuses/extends W3's HR-sample storage for cardio
  entries — one shared table, `sessionId` XOR `cardioEntryId`).
- Rung-1 fallback: the same screen runs as a structured timer with RPE prompts — no HR, no
  zone claims. Academy lessons 4–5.

### Phase E-D — "The base loop"
- `AerobicBase` estimator live: pace-at-HR + HR-drift + RHR trends → base trend on goal
  readouts and the Week Brief.
- Prescriptions adapt: ramp/hold/deload conditioning volume from base trend + interference
  + block phase; conditioning enters the coach's weekly pass as a first-class section.
- `ConditioningLoad` formally feeds ReadinessV2 interference (Coach v3 Phase B+) and
  deload drivers. Academy lesson 6. SignalRegistry: the Coach Lab renders Engine as an
  ACTIVE signal (`conditioning`), completing the "product visibly grows" story.

### Declared future slots (not built)
- `outdoor_watch_cardio` — GPS cardio recorded from the wrist (listed in WEAR_OS_PLAN).
- `hr_max_test` — guided field test to calibrate max HR / zones.
- `event_prep` — conditioning toward a dated event (race, meet GPP block).

---

## Dependency map

| Engine phase | Needs | Enhanced by |
|---|---|---|
| E-A | nothing | HC HR grants (enrichment) |
| E-B | — (minimal goal row) | Coach v3 A (portfolio), B (directive) |
| E-C | Wear W1 protocol plumbing | Wear W3 (live HR); rung-1 fallback works without |
| E-D | E-A/E-B data history | Coach v3 B (readiness), C (blocks) |

Suggested interleave lives in `docs/ROADMAP.md`.

---

## Critical files

- **New**: `domain/engine/` (`ConditioningProfile`, `ConditioningLoad`,
  `ConditioningPlanner`, `CardioPrescription`, `ZoneCoach`, `AerobicBase`) · Academy
  lesson assets (track: The Engine).
- **Touched**: `domain/cardio/` (`CardioEffort` zone mapping, `CardioWeekAggregate` load) ·
  `data/db/entities/CardioEntry.kt` + `CardioDao`/`Migrations` (only if enrichment/HR
  persisted) · `data/repo/CardioRepository.kt` (prescriptions, enrichment) ·
  `domain/adapt/` (`AdaptationSnapshot` conditioning-load series; ReadinessV2 interference
  in its Coach v3 phase) · `data/health/HealthConnectManager.kt` (session-HR read for
  enrichment) · `ui/cardio/` (hub: planned week, load, zones; live session screen) ·
  Wear protocol in `:shared` (+ `/cardio/live`, `/cmd/cardio`) per WEAR_OS_PLAN.
- **UI doctrine**: forge-design skill before any UI; cardio hub already exists — Engine
  adds sections, no new hub.

## Verification

- All `domain/engine/` modules are pure → unit corpus like `domain/adapt/`: zone math
  (Karvonen edges, missing RHR, absurd max-HR guards), load weighting, planner placement
  rules (property: never schedules hard intervals <24 h before lower-body), ZoneCoach
  hysteresis (no alert flapping), AerobicBase gates (silent below N sessions).
- Sensor-ladder audit per phase: rung-1 device (no watch, no HC) must see complete,
  zone-claim-free surfaces; byte-identical coach behavior until E-D wiring lands.
- Watcher regression: every prescription type judgeable in a window — no unwatched writes.
- Migration tests if/when schema changes; end-to-end: seed lifting + cardio history →
  planner places a sane week; manual `/run` for UI phases.

## Next step after approval

Sequence per `docs/ROADMAP.md`. E-A is standalone and small — a good first Engine release
whenever a slot opens; nothing in it blocks or is blocked by Coach v3 A or Wear W0–W1.
