# Coach v3 + Engine — Plan Review (verification pass)

> Review of `COACH_V3_PLAN.md`, `ENGINE_PLAN.md`, and `ACADEMY_LESSONS.md`, verified
> against the codebase at **0.8.8.3 / Room schema v31** (post-"Watch app" commit
> `8f75145`, 2026-07-23). Every factual claim the plans make about v2 code was checked
> against source. This doc is the Coach/Engine equivalent of the Wear plan's "rev 2
> verified" pass.

**Verdict:** the plans are structurally excellent and their diagnosis of v2 is accurate —
every one of the 17 verifiable claims about the current code checked out true. But they
need a revision pass before building: one factual error, meaningful staleness (the entire
Wear plan W0–W6 shipped since these were written), about a dozen underspecified mechanics
that will bite during implementation, and a handful of genuinely missing features — the
biggest being that the coach has no story for illness, injury, or coming back from a
layoff.

---

## 1. What's verified and solid

Audited claim-by-claim against source; **all confirmed**:

| Plan claim | Evidence |
|---|---|
| `ExerciseGoal`/`ExtendedGoal` never read by coach | zero refs in `domain/coach` + `domain/adapt`; `userGoal` only reshapes rep ranges (`GoalProfiles.kt:17-29`) |
| Mesocycle is copy-only | `WeeklyReview.mesocycleFocus()` returns a `String?` → `focusLine`; `mesocycleWeeks=5` consumed nowhere else |
| `SessionOpinion` cosmetic | sole call site `DaySessionHandlers.kt:141`, display string only |
| Moods loaded, read by zero advisors | `AdaptationSnapshot.moods` populated, no advisor consumes it |
| Bodyweight not in snapshot | no field on `AdaptationSnapshot`/`HealthSnap` |
| `toFailure`/`setType`/`difficultyTag` advisor-unread | on `LoggedSet`, consumed by UI/importers/stats only |
| Cardio `effort`/`hrZone` advisor-unread | advisors read cardio via `restReason` + `durationMin` only |
| HC sleep/HR → 14-day deload score only, readiness HC-blind | `ReadinessAdvisor.evaluate` takes no `HealthSnap`; sleep/RHR only in `DeloadAdvisor.instrument()` |
| `AdviceEvent` calibration never built | used only as a 14-day cooldown mute-set (`AdaptationRepository.kt:269-282`) |
| Tier-5 insights feed nothing | `InsightEngine` referenced only by `AdaptationRepository` insights list + Stats UI |
| `VolumeModel.weeklyCap` hard-coded | per-muscle `Map` at `VolumeModel.kt:27-39` |
| Caps 2/wk, ±1 set, ±2 drift; never trust-scaled | `AutoCoachPlanner.kt:213-214` (experience-keyed), `:249/:278`, `VOLUME_DRIFT_CAP=2`; planner never imports `TrustLedger` |
| No unwatched writes | `OutcomeWatcher.evaluate` `else → windowClosed → ok` catches every applied type |
| `CoachGenBias.from(decisions)` idempotent recompute | confirmed (`CoachGenBias.kt:55-87`) |

Also solid: phase decomposition, fail-soft invariants, recompute-from-ledger pattern,
Academy framing. `ACADEMY_LESSONS.md` is internally consistent (33 lessons; tracks match
plan phases; count math checks out).

**None of the 20 planned v3 concepts exist yet** (GoalPortfolio, TodayDirective,
BlockPlanner, TrainingBlock, ProjectScanner, CoachProject, SessionAdaptor,
PersonalProfile, CoachSignal/SignalRegistry, AcademyRegistry, Lesson/LessonEvent,
CheckinEntry, CoachGoal, ReadinessV2, PreSessionBrief, PostSessionDebrief,
NextSessionAdjustments, WeightPhase, TrustLadder) — all greenfield as the plans assume.
Nearest analogs: `TrustLedger` (per-type streaks, no tiers), `ReadinessAdvisor` (v1),
`SessionOpinion`, `ExerciseGoal`/`ExtendedGoal`.

---

## 2. Staleness — both plans need re-baselining

Plans written 2026-07-15. The "Watch app" commit (2026-07-23) landed **the entire Wear
plan W0–W6 in one shot**; the app is now **0.8.8.3 / schema v31**, not the Engine plan's
"0.8.8.2 / schema v23" (8 schema versions behind).

| Plan says (future work) | Reality now |
|---|---|
| Coach A: "add HC steps to snapshot"; Coach F: "HC HRV read" | `HealthSnap.hrv` + `HealthSnap.dailySteps` already exist and are populated. Remaining work is **advisor consumption only** — no snapshot plumbing |
| Coach A: "full cardio fields" | `CardioEntry` already gained `inclinePct`, `laps`, `elevationM`, `conditions` (v27–v28) — richer than the plan knows |
| Engine E-A: "retro-enrich past entries with HC session HR" | W5 already matches watch HC sessions to cardio entries and shows HR on the hub (`sessionHr`/`sessionWatch`). E-A's remaining work is zone attribution, not matching |
| Engine E-C: "reuse W3's HR storage — one shared table, `sessionId` XOR `cardioEntryId`" | **Incompatible with shipped schema.** `SessionHrSample` has composite PK `(session_id, at_ms)` + NOT-NULL FK CASCADE to `Session`. Cardio live-HR needs its own table or a real migration |
| Wear plan: sleep read "duration-only" | Sleep stages (deep/REM) already read into `SleepNight` |

Also: both docs still carry planning-session ground-rules headers, branch names, and
"Next step after approval" sections — replace with per-phase status blocks (the Wear
plan's rev-2 treatment is the model).

---

## 3. Factual error (Engine plan)

**"Max HR … else age default (Tanaka; birthday already known from profile)" — there is no
birthday, age, or DOB anywhere in the repo.** Only `USER_SEX` and bodyweight exist.
`ConditioningProfile` needs a new age/DOB capture added to E-A scope, or a max-HR-only
setting with an explicit "age unknown" default path.

---

## 4. Missing features (the real gaps)

### 4a. Illness, injury, layoffs — the biggest hole ⚠️

A real coach's most common adjustment is "you were sick / hurt / away — here's how we
come back." Neither plan addresses it:

- **No sick flag anywhere.** The Academy doc's F6 lesson references a "sick-day flag"
  that no machinery creates. The check-in (sleep/soreness/stress/motivation) has no
  illness input; a flu week reads as unexplained stalls and fatigue.
- **No return-from-break ramp.** `VacationPeriod` and `SessionBreak` entities already
  exist, and `ReadinessAdvisor` already takes `onVacation` — the plan never mentions
  them. After 2+ weeks off: suppress stall/watcher verdicts across the gap, re-ramp
  loads (~-10%), restart or extend the block. `SessionType.FIRST_BACK` exists in the
  enum for exactly this and has no writer.
- **No injury model.** Per-muscle soreness gating covers acute soreness; a persistent
  restriction ("shoulder tweaked for 3 weeks") has no home. Minimal fix: a
  "restricted muscle/movement until date" flag consumed by generator + directive.

### 4b. Missing prerequisite: the session-type picker

Phase A wants technique/test sessions filtered from stall series — but TEST/TECHNIQUE/
FIRST_BACK **have no writer** (no session-type picker UI exists), and `ExerciseBout`
doesn't carry `sessionType`, so the filter is structurally impossible today. Phase A
needs two added items: the picker (or auto-tagging) and `sessionType` on the bout.

### 4c. Directive's calendar substrate underspecified

`WeeklySchedule` has two modes: **weekday mode** (each weekday → program day/rest) and
legacy **sequence mode** ("next up" only). The plans never mention this. TodayDirective
works in both, but the Engine's placement rules ("hard intervals never <24h before a
lower-body day") are **only computable in weekday mode** — in sequence mode the app
doesn't know Thursday is leg day. Define the sequence-mode degraded behavior (place
relative to next-up only), or have the coach propose adopting weekday mode as an early
project.

### 4d. One-answer directive vs. two-discipline days

TodayDirective is "train OR rest OR cardio — never a menu," but the Engine prescribes
zone-2 **post-lift on training days**. The directive model needs a primary + optional
secondary slot ("Upper day · then 20 min Z2") or the two plans contradict on any dual day.

### 4e. Adherence vs. efficacy in the watcher

Engine judges prescriptions partly on "did the user do it"; Coach demotes trust on any
failure. Combined: skipping a Tuesday walk demotes the coach for the user's behavior.
Split verdicts three ways — **worked / didn't work / not followed** — with "not followed"
feeding dose reduction and re-planning, never trust demotion or bias folding.

### 4f. Trust ladder hardening (before T3/T4)

- "Any failure demotes" is right for v2's per-type streaks, too brittle for tiers — a
  T3+ coach making many autonomous calls at a ~70–80% win-rate would oscillate
  permanently. Demote on failure *rate* or user-reverts, with hysteresis.
- **T4 should be opt-in at the moment it's reached**, not automatic. One consent card:
  "You've unlocked full autonomy — turn it on?"
- LIFO undo doesn't survive T4 structural changes (undoing a split restructure after a
  week of logged sessions on the new split is undefined). Needs undo-window expiry + a
  "revert forward" rule (regenerate old shape, keep logged data).
- Concurrent edits at T4: user manual edits are always superior — treat them as pinned
  constraints + preference signal.

### 4g. Portfolio ↔ Block arbitration unspecified

Multiple active goals, one block focus. Who decides this block serves the bench-1RM goal
while zone-2 gets maintenance volume? Needs an explicit rule (block focus = top-priority
goal, others get floors) and a `RecommendationArbiter` seat for strength-vs-conditioning
volume collisions. Related: watcher windows are 14 days — an 8-week goal-sequencing
decision can't be judged by them. Define long-horizon judgment (per-block checkpoint
verdicts) or exempt-and-explain.

### 4h. Per-muscle soreness has no input source

ReadinessV2 promises "per-muscle soreness gating" but the 4-tap check-in has one generic
soreness tap. Either an optional muscle-picker step when soreness is flagged, or infer
candidates from the last 48h of trained muscles and confirm with one tap.

### 4i. Check-in additions (while it's being designed)

- A **sick/unwell** option (feeds 4a).
- An optional **bodyweight quick-log** — morning is weigh-in time; `WeightPhase` trend
  detection is data-hungry and the sheet is its natural surface.
- **Adaptive prompting** — stop prompting users who always skip.

### 4j. Smaller but real

- **Goal lifecycle** — spec covers creation + conflict, not completion: celebrate,
  archive, propose successor at ETA/target reached.
- **New-exercise cold start** — "what weight do I start with?" after a swap/new slot is
  a Decision-Zero question with no answer surface (relative-strength seed from similar
  movements would do).
- **Load rounding** — PreSessionBrief targets must round to the shared weight-step table
  (KG 2.5 / LB 5 / plates) now in `:shared`.
- **1RM test protocol** — PEAK phase should define whether/how top singles or AMRAP test
  days happen for strength goals tracked by e1RM.
- **Background computation for outreach** — T3 notifications ("tomorrow is your
  strongest window") need readiness computed *without an app open*: a WorkManager job
  doing HC reads + snapshot assembly (`WeeklyRecapWorker` is the precedent). Quiet hours
  already exist per-day (`QuietHoursSchedule`) and must gate every outreach category.
- **Day-key convention** — `Session.dayKey` is a program-day id ("push"); body entities
  use ISO dates. `CheckinEntry`/`TodayDirective` must standardize on calendar dates.
- **JSON/CSV export coverage** — the ZIP backup auto-covers new tables, but
  `BackupRepository.exportFullDataJson` has a hardcoded entity list that already excludes
  coach tables. Each phase's verification list should include "new entities added to the
  JSON export" (check-ins, goals, blocks, projects are exactly what users will want out).
- **Directive surfaces** — a Glance home widget already exists (next-workout-focused,
  deep links stubbed) and wear tiles/complications shipped with a directive handshake
  waiting. Name widget + tile as directive surfaces in Phase B / W4.
- **In-session swap constraint** — swap is blocked once sets are logged
  (`DaySwapHandlers.kt:23`). SessionAdaptor's soreness reroute mid-exercise needs either
  a relaxation of that rule or a "finish early + substitute" path.

### 4k. Engine-specific

- **One interference formula.** Coach B lists "cardio interference (effort × zone ×
  minutes)" as a ReadinessV2 input; Engine E-A defines `ConditioningLoad` as the
  canonical TRIMP-lite; E-D "formally" wires it. Extract the pure `ConditioningLoad`
  function whenever the first consumer ships, whichever plan gets there first — say so.
- **Health Floor double-counting.** HC steps count toward the 150 min/wk floor AND a
  logged walk is a `CardioEntry` — the same 30 minutes credits twice. Dedup rule needed
  (subtract step-minutes overlapping logged sessions, or max-of).
- **HR artifact guarding** — "max HR refined upward by observed session maxes" will
  inflate from one wrist-HR spike and silently shift every zone. Require sustained
  (≥30s) elevated readings; ingest bounds (25–240 bpm) won't catch a 205-for-3s artifact.
- **Use the `conditions` field** (HOT/COLD/RAIN/WIND, shipped v28) to exclude/flag
  confounded sessions in `AerobicBase` — heat inflates HR and poisons pace-at-HR trends
  (the plan's own E4 lesson teaches exactly this).
- **Interval structure needs warm-up/cool-down segments**, not just N×work:rest; Work
  Capacity's "recovery-between-rounds" metric is HR-only — declare a rung-1 proxy
  (completed volume only).
- **WeightPhase × conditioning** belongs in the conflict matrix (a cut may *want* more
  cardio; the "lifting wins" default flips).

### 4l. Academy consistency nits

- Plan says unlock state is "derived from the coach ledger," but half the curriculum's
  triggers are app-usage events (first rest-timer use, first readiness tap, first
  mesocycle-UI open) that write no ledger rows. Widen the definition (`LessonEvent`
  records unlock moments too) or reword the invariant.
- F6's "sick-day flag" must be reconciled with 4a.

### 4m. Candidate signal slot the "declare ALL slots now" decision missed

Menstrual-cycle-aware readiness (HC exposes cycle data; `USER_SEX` already exists). Even
as COMING_SOON it belongs in the registry — the registry's whole point is declaring the
future.

---

## 5. Sequencing sanity check

The roadmap's order still holds, with corrections:

- **Wear W0–W6 is done** — roadmap items 1, 3, 6, 9 complete; Coach B's readiness inputs
  (HRV, steps, sleep stages) are already plumbed, lowering Phase A's cost.
- **Engine E-C's shared-HR-table premise needs redesign** (see §2).
- Phase B's TodayDirective ships before Phase C's blocks — fine, but state its degraded
  computation (spacing + readiness + schedule only, no block phase).
- Coach B's rest-day cardio directives need a declared stub ("suggest, don't prescribe")
  until Engine E-B exists.

---

## 6. Recommended next actions

1. **Re-baseline both docs** — versions (0.8.8.3 / v31), shipped Wear work,
   `SessionHrSample` schema reality, the birthday error — and add per-phase status
   blocks; drop the stale planning-session headers.
2. **Promote the missing features into scope** — illness/injury/layoff handling (4a)
   into Phase B alongside the check-in (its natural capture point); session-type picker
   + `ExerciseBout.sessionType` into Phase A (4b).
3. **Fold the mechanic clarifications into the plan text now** (4c–4h, T4 opt-in,
   adherence-vs-efficacy, portfolio↔block rule) — all cheap to specify, expensive to
   discover mid-build.
