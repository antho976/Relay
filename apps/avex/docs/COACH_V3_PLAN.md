# Coach v3 + Avex Academy — "A Real Coach That Makes Itself Optional"

> **Revision 3** — verified against the codebase at **0.8.8.3, Room schema v31**. Rev 3 folds
> in `COACH_ENGINE_PLAN_AUDIT.md` (the verification pass over rev 2): five factual
> corrections, the shipped machinery rev 2 duplicated, the plumbing decisions rev 2 left to
> discovery, and a phase split so every unit is genuinely shippable. Rev-by-rev history is in
> Status; the body below is the current plan, stated plainly.
> Sibling plans: `ENGINE_PLAN.md` (conditioning), `WEAR_OS_PLAN.md` (shipped).
> **No v3 phase started.** Phase A1 is next.

---

## Context

Coach v2 is a superbly-hardened weekly suggestion auditor — idempotent passes, outcome
watcher, earned-trust autopilot, bias folding, LIFO undo — but it is not a coach. A coach:
**knows your goal → has a plan → runs you through it → watches → adjusts → explains →
teaches → remembers.** V2 only does the middle four:

- **No goal model in the coach.** `ExerciseGoal`/`ExtendedGoal` exist in Room and no advisor
  reads them; `userGoal` only reshapes rep templates at generation (`GoalProfiles.kt:17-29`).
- **No plan, only reactions.** Everything fires off stalls and fatigue. "Mesocycle" exists
  only as a copy string (`WeeklyReview.kt:160`) that changes no behavior.
- **Wrong cadence.** One weekly brain; in-session chips are disconnected; `SessionOpinion` is
  cosmetic (sole call site `DaySessionHandlers.kt:141`, display string only).
- **Massive wasted data.** Moods loaded, read by zero advisors; bodyweight not in the
  snapshot at all; `toFailure`/`setType`/`difficultyTag` advisor-unread; cardio
  `effort`/`hrZone` advisor-unread; `health.hrv` and `health.dailySteps` populated and unread;
  HC sleep/HR feed only the 14-day deload score, never daily readiness; `sessionType`
  technique/test pollutes stall detection; `AdviceEvent` never became confidence calibration
  (used only as a 14-day cooldown mute-set, `AdaptationRepository.kt:269-282`).
- **Learning loop open.** Tier-5 insights (`volumeResponse` = personal MEV/MRV, `restResponse`
  = recovery curve, `sweetSpotRepRange`, `timeOfDayPerformance`) are computed and shown as
  Stats trivia; they never feed the planner or the generator, which uses the hard-coded
  `VolumeModel.weeklyCap` map (`VolumeModel.kt:27-39`).
- **Ambition never scales with trust.** Caps (2 changes/wk, ±1 set, ±2 drift) are permanent
  even after months of earned trust; the planner never imports `TrustLedger`.
- **No knowledge layer.** V2 has decisions with reasons but teaches nothing. A user can follow
  the coach for a year and understand training no better than day one. That is dependence, and
  dependence contradicts the Avex ethos.
- **No life-events model.** Illness, injury and layoffs are the most common real-coach
  adjustment and the coach has no story for them (see the correction below for the fragments
  that *do* exist).

### What already exists (rev-3 corrections to earlier drafts)

Rev 2 overstated the greenfield. These are shipped and must be extended, not re-invented:

| Earlier claim | Reality |
|---|---|
| "No sick flag anywhere" | `CardioRestReason.SICK` is user-writable today and **already consumed** — `ReadinessAdvisor.kt:79` (−4%) and `DeloadAdvisor.kt:184`. Same for `SORE` (`:80` / `:185`) |
| "The coach ignores vacation" | `CoachRepository.kt:195` already holds the **whole weekly pass** with an explained reason while on vacation |
| "`SessionBreak` is a layoff input" | Wrong entity — `SessionBreak` is a water/rest/snack break *inside* a session, CASCADE-FK'd to `Session` (#139). Layoff inputs are `VacationPeriod` + raw gap detection, nothing else |
| "Widget deep links are stubbed" | `EXTRA_START_DAY_KEY` is read (`MainActivity.kt:125`, `:302`) and threaded into `ForgeNavHost(initialDayKey=…)` (`:420`). Only `EXTRA_RESUME_SESSION` is unread |
| "`SessionType` has no writer" | The write path exists end-to-end — `DayUiEvent.SetSessionType` → `DaySessionHandlers.kt:34` → `WorkoutRepository.setSessionType:415` → `SessionDao:126`. Only a UI control that emits it is missing (and `ExerciseBout.sessionType`, genuinely absent) |
| "Goals are just `ExerciseGoal`/`ExtendedGoal` weight targets" | `ExtendedGoal` already supports `1rm \| weekly_volume \| frequency \| monthly_prs` with `stretch_value` **and `completed_at`**; `domain/goal/CustomGoal.kt` adds `GoalMetric` × `GoalPeriod` encoded into `goal_type`. Goal *lifecycle state already exists* |

Since rev 1 the Wear plan shipped in full (W0–W6): `HealthSnap` already carries HRV and daily
steps, sleep nights include stages, `CardioEntry` already has `inclinePct`/`laps`/
`elevationM`/`conditions` (v27–v28), the wear Today tile handshake exists, and W5 already
matches watch HC sessions onto cardio entries. "Eat everything" is therefore mostly a
**wire-to-advisor** job — bodyweight is the one genuinely missing series.

User decisions: morning check-in (new capture) ✅ · trust-scaled authority up to full autonomy
✅ · phased plan, each phase shippable ✅ · declare ALL future slots now ✅ · proactive after
trust is earned ✅ · multiple user-selected goals, conflict-aware ✅ · remove every "what
should I do?" decision from the user ✅ · **Avex Academy folded in as the knowledge layer — the
coach must be a tool you can use, never a dependency that strangles you into Avex** ✅.

---

## The two governing principles

### Principle 1 — Decision Zero

The user should never have to think "what should I do?" or "what can I do to improve?" — every
question a lifter could ask, the coach answers before it's asked. Every phase is graded against
this bar.

| Question | v3 answer surface |
|---|---|
| What do I do today? | **Today Directive** — one card: "Today: Pull day" / "Rest — here's why" / "20-min zone-2 walk", with the session prepped |
| How heavy / how many reps? | **PreSessionBrief** per-exercise targets (readiness- and block-shaped) |
| Am I making progress? Toward what? | **Goal Portfolio** readouts with trajectories + ETAs |
| What's my weak point? What should I change? | **Proactive Projects** — the coach hunts the biggest lever and runs a named project on it |
| When do I deload / rest / push? | **Block state machine** + readiness — scheduled, announced in advance |
| Only have 30 minutes / bench is taken / I'm sore? | **In-session adaptivity** — instant re-plan, no thinking |
| Is this program even right for me? | **Coach owns the program at high trust** — re-shapes it and says why |
| What should I improve outside the gym? | **Signal slots** — sleep, protein (future), stress, steps |
| Why is the coach doing this? How does this work? | **Academy** — every decision links to the lesson behind it |
| I'm brand new and there's no data yet | **Academy cold-start track** — the curriculum IS the directive until data gates open |
| What weight do I start with on a new/swapped exercise? | **Cold-start prescription** — relative-strength seed from similar movements, refined by the calibrator |
| I'm sick / hurt / just back from two weeks away | **Life events** — sick flag, injury restriction, layoff detection + return ramp |
| **I don't run a program at all (freestyle)** | **Freestyle directive mode** — see "Modes" below. Decision Zero must survive the mode where the user has no plan, or it is a promise made only to the already-organized |

If a decision can be made for the user, it goes on the coach roadmap — the user's only jobs are
to show up, log honestly, and veto.

### Principle 2 — The coach makes itself optional (Avex Academy)

A coach that decides everything forever creates dependence. Avex doesn't do lock-in, guilt, or
manipulation — so the coach's twin is the **Academy**: a knowledge layer that teaches the user
everything the coach knows, at the moment it becomes relevant. The end state is a user who
*chooses* their position between "coach decides everything" and "I understand everything and
use the coach as a calculator" — and can move along that line freely.

**Academy invariants:**
- **Just-in-time, not curriculum-first.** Lessons attach to coach moments, not a course index.
  First scheduled deload → "what a deload is and why you've earned one." First plateau → the
  stall lesson. The coach already generates the moments; the Academy annotates them.
- **Teach exactly what the coach does — no more.** A user who completes the Academy could
  understand or override every decision the coach makes. "Everything about training science" is
  explicitly out of scope.
- **The reason IS the doorway.** Every coach output already carries a human reason; v3 lets
  every reason link to its lesson. No lesson exists that isn't reachable from a real coach
  moment; no coach concept ships without a lesson (grep-able 1:1 audit per phase).
- **Never gates, never nags.** Lessons are pull, plus one quiet "New lesson unlocked" chip.
  Reading nothing changes zero behavior. No XP for the Academy — learning is not gamified
  engagement bait (trophies may *acknowledge* completion, never drive it).
- **Mirror of the TrustLadder.** Trust is the user delegating decisions *up*; the Academy is
  knowledge flowing *down*. Coach Lab shows both.

### The cold-start resolution (where the two principles meet)

"Silent below data gates" (correct, kept) + Decision Zero = a contradiction for a new user: no
data → silent coach → user must think → promise broken on day one. **The Academy is the
resolution.** During the data-starved window the Today Directive is curriculum-driven:
fundamentals lessons + the generated program carry "what do I do?" until advisors wake up. The
directive card never goes blank — it degrades from *personalized* ("readiness-shaped targets")
to *principled* ("week 1: learn the movements — here's the lesson and your prepped session"),
never to silence. As data gates open, lessons hand off to advisors concept-by-concept, and each
handoff is itself a teachable moment.

---

## Design invariants (carry v2's soul forward)

- Every advisor stays a **pure function of a snapshot** (`AdaptationSnapshot` grows, pattern
  stays).
- **Silent below data gates** — never confident-wrong on sparse data; all thresholds in
  `AdaptThresholds`. Cold-start silence is filled by the Academy track, not by lowering gates.
- Every output carries a **human reason**; every reason may carry a **lessonId**; every write
  goes through existing user write paths with undo-state; the **watcher judges everything
  applied**.
- Watcher verdicts are **three-valued: worked / didn't work / not followed**. "Not followed"
  (the user skipped it) feeds re-planning and dose reduction — it never demotes trust and never
  folds into bias. Only efficacy failures do. Skipping a Tuesday walk is user behavior, not bad
  advice. (Storage + call-site consequences: see Mechanics M2.)
- Multi-week acts (block plans, goal sequencing) can't be judged by 14-day windows — they get
  **per-block checkpoint verdicts** instead of being exempt or misjudged.
- **New signals are additive**: absent source ⇒ zero behavior change (the Health Connect
  precedent). Academy content is additive the same way: missing lesson ⇒ reason renders without
  a link, nothing breaks.
- All learning is **recomputed from durable ledgers** (idempotent, never compounding) — the
  `CoachGenBias.from(decisions)` pattern. Academy read-state is a ledger too (`LessonEvent`
  rows), same recompute rule.
- **No duplicate signals.** Where a signal already exists and is already consumed, v3 *replaces*
  the existing computation rather than stacking a second one on top (sick/sore: M6;
  conditioning interference: `ENGINE_PLAN.md`).

---

## Modes the coach must survive

V2 has three modes beyond "normal", none of which appeared in rev 1/rev 2. Every new surface
declares its behavior in all of them, and each phase's verification includes a mode audit.

| Mode | Today | v3 rule |
|---|---|---|
| **Freestyle** (`FREESTYLE_MODE`) — no fixed program | The coach bails before any pass runs (`CoachRepository.kt:745`) | The directive still answers, from spacing + readiness + goals only: "train (here's a freestyle template shaped by your recent muscles)" / "rest" / "cardio". No program-shaped claims, no block. Weekly pass stays off. The coach may pitch adopting a program as a project, once, never nagging |
| **Coach off** (`COACH_ENABLED=false`) | Pass runs, writes inert `STATUS_SHADOW` rows ignored by TrustLedger/GenBias/watcher (`CoachRepository.kt:185-191`) | Same pattern for every new decision type: compute, record inert, surface nothing. New surfaces (directive, brief, Academy chips) hide; Academy itself stays browsable — knowledge is never gated on the coach being on |
| **On vacation** (`VacationPeriod`) | Whole pass held with an explained reason (`CoachRepository.kt:195`) | Kept and extended by Life events: the hold reason becomes the layoff/return-ramp story on re-entry |

---

## Target architecture (end state)

```
                      ┌─ GoalPortfolio (multi-goal, conflict-aware, coach-proposed)
                      ├─ BlockPlanner (periodization state machine)
AdaptationSnapshot ───┼─ ReadinessV2 (daily, multi-signal + check-in)
 + PersonalProfile ───┼─ TodayDirective ("here's what you do today")
 + SignalRegistry ────┼─ ProactiveProjects (weakness hunter → named projects)
                      ├─ PreSessionBrief / SessionAdaptor / PostSessionDebrief
                      └─ WeeklyPass v3 (block manager + structural changes)
                              │
                TrustLadder (T0–T4: edits → initiative → autonomy)
                              │                    ▲ user delegates decisions up
                              │                    ▼ Academy sends knowledge down
                     AcademyRegistry (lessons keyed to coach moments; cold-start track)
                              │
      existing apply paths + ProgramGenerator (personalized) + proactive outreach
```

---

## New domain concepts

**GoalPortfolio** (`domain/coach/GoalPortfolio.kt` + Room entity `CoachGoal`): the coach runs a
portfolio of selected objectives, not one hidden setting. Catalogue: strength on a lift (1RM
target), build a muscle/area, lose fat / recomp, consistency habit (sessions/week), conditioning
(weekly cardio minutes / zone-2 base), fix an imbalance (push/pull, quad/ham), endurance on a
movement — each with a measurable metric, a trajectory (robust slope over
`ExerciseBout.bestE1rm()` / volume / adherence series), an ETA, and on/off-track state.

- **Multi-select with conflict detection**: a pure `conflicts(a, b)` matrix — compatible goals
  run in parallel (bench 1RM + consistency + zone-2 base); conflicting ones (max strength +
  aggressive cut) are flagged and the coach proposes sequencing, never silent degradation.
- **Coach-proposed goals**: the coach scans for candidates it can pitch ("your pull is 40%
  behind your push — want me to make fixing your back a goal?").
- **Lifecycle**: reaching a target/ETA is a first-class moment — celebrate, archive, propose a
  successor. `ExtendedGoal.completed_at` is the existing precedent to carry forward.
- **Block arbitration**: one block, many goals — block focus = the top-priority goal; every
  other active goal gets an explicit maintenance floor. Strength-vs-conditioning volume
  collisions are settled in `RecommendationArbiter`.
- Every planner intervention names the portfolio goal it serves; the Week Brief opens with
  portfolio progress, not generic numbers.
- **Migration is the hard part, not the model** — see Mechanics M1.

**ProactiveProjects** (`domain/coach/ProjectScanner.kt` + `CoachProject` rows): the "what can I
improve?" killer. The coach permanently hunts the single biggest improvement lever across every
signal (lagging muscle, imbalance, missing movement pattern, no conditioning base, chronic short
sleep, volume below personal MEV, estimate-vs-reality drift, skipped-exercise waste) and runs
**ONE active project at a time** with a name, a why, a plan, and a finish line ("Project:
Rear-delt catch-up — 4 weeks, +3 sets/wk, done when its growth matches your shoulders").
Completed/abandoned projects go in the ledger; the scanner picks the next lever. This IS the
coach roadmap the user sees. Each project type ships with its lesson.

**TodayDirective** (`domain/coach/TodayDirective.kt`, pure): the flagship Decision-Zero surface —
ONE answer at the top of Overview every day: train (which day, prepped, readiness-shaped
targets), rest (and why, and what to do instead), or cardio (type, duration, zone). Computed
from block phase, readiness, spacing/recovery curve, portfolio priorities, and the week's
remaining session budget. Never a menu — a directive, with the veto one tap away.

- **Calendar substrate**: reads the existing `WeeklySchedule`. Weekday mode gives the full week
  layout. Sequence mode only knows "next up", so placement degrades to next-up-relative: the
  directive answers train/rest/cardio and *names the next-up day*, but makes no weekday claims
  ("tomorrow is legs") and no day-swap proposals. If spacing says train and next-up would repeat
  a muscle trained inside its recovery window, the directive says rest-or-cardio and explains
  why, rather than reordering a schedule it cannot see. The coach may pitch adopting weekday
  mode as an early project.
- **Dual-discipline days**: one answer, with an optional secondary slot ("Upper day · then
  20 min Z2") so Engine's post-lift zone-2 never turns the directive into a menu.
- **Until Engine E-B ships**, rest-day cardio directives are suggestions ("a 20-min walk would
  serve recovery"), never structured prescriptions.
- **Cold-start mode**: below data gates the directive is curriculum-driven (lesson + prepped
  template session), never blank.
- **Surfaces + displacement**: the Overview card is the primary surface and it **replaces** the
  existing next-workout lead-in there rather than stacking beside it (see M7); plus the Glance
  home widget (`EXTRA_RESUME_SESSION` is the only unwired extra) and the wear Today tile
  (`PATH_GLANCE_TODAY`, handshake already shipped, waiting for a directive to exist).

**TrainingBlock** (Room entity + `domain/coach/BlockPlanner.kt`): persisted block — phase
(ACCUMULATE / INTENSIFY / PEAK / DELOAD), weekIndex, plannedWeeks, focus lifts/muscles, intent
text. The weekly pass advances it; advisors consult it: volume ramps +N sets across
accumulation, progression aggressiveness rises into peak, deloads become scheduled and earned
(the fatigue score can pull one earlier — `DeloadAdvisor` becomes the block's tripwire, not the
only path).

**ReadinessV2** (`domain/adapt/ReadinessAdvisor` rebuilt): inputs = last-night sleep (HC, stages
already read), today's resting HR vs own baseline, morning check-in, moods, acute load,
conditioning interference (the single `ConditioningLoad` function — see M6), HC steps, bodyweight
flux. Output: 0–100 with named parts and a bounded scale % (the bound widens with trust tier),
plus per-muscle soreness gating.

- **Check-in** (`CheckinEntry`): sleep quality / soreness / stress / motivation, 4 taps ≈5 s,
  skippable. Plus a **sick/unwell option** (feeds Life events), an **optional muscle picker**
  when soreness is flagged (per-muscle gates need a per-muscle source — one generic tap cannot
  provide it; fallback: infer candidates from the last 48 h of trained muscles and confirm with
  one tap), an **optional bodyweight quick-log** (morning is weigh-in time and `WeightPhase`
  trend detection is data-hungry), and **adaptive prompting** (users who always skip stop being
  prompted).
- **It replaces, not stacks**: the existing sick/sore deductions (`ReadinessAdvisor.kt:79-80`)
  and the 24 h cardio deduction (`:84-86`) are removed as ReadinessV2 subsumes them (M6).

**Life events** (`domain/coach/LifeEvents.kt` + check-in flags): the missing half of real
coaching.
- **Sick flag** — a check-in option and a quick action on the directive card. While sick:
  rest/recovery directives, no stall or watcher verdicts accrue, readiness floored. This flag
  becomes the **single source of truth** for illness; the `CardioRestReason.SICK` path is
  migrated onto it (M6).
- **Layoff detection + return ramp** — inputs are `VacationPeriod` (declared) and raw
  session-gap detection (undeclared). *Not* `SessionBreak`, which is an in-session break. After
  ≥14 days off: suppress stall/outcome verdicts across the gap, re-enter with a ramp week
  (≈−10% loads; `SessionType.FIRST_BACK` finally gets its writer), restart or extend the block.
  Builds on the existing vacation pass-hold, which already covers the declared case — the new
  work is the *return*, and undeclared gaps.
- **Injury restriction** — a "restricted muscle/movement until cleared" flag, distinct from
  acute soreness, routed around by the generator, the directive, and the SessionAdaptor.

**PersonalProfile** (`domain/coach/PersonalProfile.kt`, assembled like GenBias): the unified
"what the coach knows about YOU" — per-muscle personal volume caps (promote the `volumeResponse`
estimator, clamped to a safety band around `VolumeModel.weeklyCap`), recovery curve
(`restResponse` → preferred day spacing), per-lift sweet-spot ranges, step calibration (absorb
`SuggestionCalibrator` / `RestTuning`), time-of-day strength. Defaults = today's constants; every
estimator hard-gated.

**TrustLadder** (extends `TrustLedger`): a global trust score from accepted proposals × watcher
win-rate × weeks coached. Trust doesn't just unlock bigger edits — it unlocks **initiative**:

- T0 observe → T1 propose (v2 today) → T2 auto-apply earned types (v2 autopilot)
- **T3 — proactive**: plans/advances blocks one-tap, caps rise (±2 sets, 4 changes/wk), starts
  Proactive Projects on its own (announce-then-run), sends directive notifications.
- **T4 — full autonomy**: owns the program (frequency/split restructures, goal sequencing,
  scheduling), acts first and informs after — the weekly brief becomes "here's what I changed
  and why", everything still watcher-judged and one-tap revertible.

Hardening (all of it pre-T3 work):
- **Demotion is rate-based, not single-failure.** V2's any-failure rule stays for per-type
  auto-apply, but a T3+ coach making many autonomous calls at a real-world win-rate would
  oscillate tiers forever. Demote on failure *rate* or user-reverts, with hysteresis.
- **T4 is opt-in at the moment it's earned** — one consent card, never a silent switch. The
  user can cap the tier in Settings; every autonomous act keeps the full undo/watch machinery.
- **Structural acts get real undo semantics** — LIFO undo can't unwind a split restructure the
  user has trained under for a week. Structural changes carry an undo-window expiry plus a
  "revert forward" rule (regenerate the old shape, keep all logged data). Data model: M4.
- **Concurrent edits**: the user's manual edits always win and become pinned constraints plus a
  preference signal — never a merge conflict.

**In-session adaptivity** (`domain/coach/SessionAdaptor.kt`): the mid-workout "what now?"
eliminator — three instant re-plans, all through existing swap/reorder paths:
- **Equipment busy**: one tap on a slot → best equivalent movement right now
  (`ProgramSlotSnap.swapCandidateIds` already exists).
- **Short on time**: "I have N minutes" → session auto-triaged to its highest-value core
  (goal-serving lifts > compounds > accessories), using `SessionEstimate` + personal rest tuning.
- **Something hurts**: the per-muscle soreness gate reroutes mid-session and flags it for the
  weekly pass. Swap is blocked once sets are logged (`DaySwapHandlers.kt:13-25`) — the adaptor
  adds a "finish early + substitute" path rather than relaxing that rule.

**Proactive outreach** (extends `ForgeNotifications` / `WeeklyRecapWorker`): trust-gated,
quiet-by-default notifications the coach initiates — session-window nudges, "readiness is
unusually high", block transitions, project milestones, goal ETAs. Frequency-capped, per-category
opt-out. Directive-grade outreach requires readiness computed **with no app open** — a
WorkManager job (the `WeeklyRecapWorker` precedent) does the HC reads and snapshot assembly.
Every category is gated by the existing per-day `QuietHoursSchedule`.

**SignalRegistry** (`domain/coach/CoachSignal.kt`): declared slots with
`availability = ACTIVE | AWAITING_DATA | COMING_SOON`. Registered from day one:
`protein_nutrition` (COMING_SOON) · `stress_hrv` (AWAITING_DATA — HC HRV when granted;
subjective stress from the check-in is ACTIVE) · `hydration_supplements` (COMING_SOON) ·
`bodyweight_goal` (ACTIVE early — data already exists) · `cycle_readiness` (COMING_SOON — HC
exposes cycle data and `USER_SEX` already exists) · `conditioning` (flips ACTIVE with Engine
E-D) · `watch_hr` (COMING_SOON — the *source* already ships, `session_hr_sample` from W3; the
consumer is intra-session HR strain in the fatigue/deload drivers, declared in
`WEAR_OS_PLAN.md`). Advisors iterate the registry; absent sources contribute nothing.

**Naming**: Coach Lab already has a "Signals lens" over `RecoverySignal`
(`CoachRepository.kt:106`, `ui/coach/CoachSignalsSection.kt`). The registry renders **inside that
existing lens** as its "what I could read" section — two "signal" concepts in one screen would be
incoherent. `RecoverySignal` stays the live-input row type; `CoachSignal` is the slot declaration.

**AcademyRegistry** (`domain/academy/AcademyRegistry.kt` + `Lesson` static content and
`LessonEvent` ledger rows): the knowledge layer.
- **Content model**: short lessons (1–3 min read), plain language, keyed by `lessonId`, grouped
  into five tracks (Fundamentals, Coach Concepts, Programming, Signals, The Engine). Ships
  in-app, offline. Format and renderer: M5.
- **Wiring**: `lessonId` on coach outputs (M3); "unlocked" = the first time its moment fires.
  Unlock state derives from **durable ledgers, plural** — the coach ledger for coach moments,
  `LessonEvent` rows for app-usage moments (first rest-timer use, first readiness tap, first
  mesocycle-UI open), since half the curriculum's triggers write no coach rows. Same idempotent
  recompute rule for both.
- **Cold-start track**: Fundamentals (10 lessons) is the only track surfaced sequentially, and
  only during the data-starved window, as part of the Today Directive.
- **Surfaces**: lesson cards inline at coach moments; an Academy section listing unlocked +
  upcoming (visible-but-locked, like COMING_SOON slots); Coach Lab cross-links.
- **Authoring reality (solo dev)**: content is the real cost, not code — 33 lessons total across
  all phases, bounded by "teach exactly what the coach does". Written as each phase's machinery
  lands, never ahead of it. Curriculum: `ACADEMY_LESSONS.md`.

**Bodyweight-goal coupling** (`WeightPhase`: cut / maintain / bulk — a Settings pick plus trend
detection from `BodyweightEntry` + HC weight): reinterprets signals per phase — a held e1RM while
cutting is a WIN (suppress stall escalation, celebrate retention); a slow bulk with stalls → an
under-eating callout; readiness reads weight flux.

### Three cadences

- **PreSessionBrief** (day-screen open): pure fn of snapshot + ReadinessV2 + block phase +
  `NextSessionAdjustments` → per-exercise targets with intent ("3×8 @ 145 — week 3 ramp;
  readiness is low, top set only on squats"). Replaces today's disconnected chips as the delivery
  layer — chips remain but become the brief's line items. Targets always round to the shared
  weight-step table (`shared/…/weight/WeightSteps.kt`) — no unloadable prescriptions. New or
  swapped exercises get a **cold-start prescription** (relative-strength seed from similar
  movements, refined by the calibrator).
- **PostSessionDebrief** (session finish): `SessionOpinion` upgraded from cosmetic to causal —
  computes and persists `NextSessionAdjustments` (a per-lift micro-plan consumed by the next
  PreSessionBrief), so the coach reacts session-to-session, not week-to-week.
- **WeeklyPass v3**: today's pass promoted to block manager — advances phase, plans next week's
  intent, structural changes, goal progress in the Brief.

---

## Mechanics — decisions made now, not at build time

Rev 2 left these to discovery. Each one is cheap to decide here and expensive to hit mid-build.

**M1 · Goal migration and its downstream consumers.** `CoachGoal` does not simply absorb
`ExerciseGoal`/`ExtendedGoal`: those tables have live readers. `program/Trophies.kt`'s
`UnlockRule.ExerciseGoalsAchievedAtLeast` powers the `goal_crusher` and `goals_5` trophies via
`exerciseGoalsAchieved` (`domain/trophy/TrophyEvaluator.kt:46`), and `ui/goals/`, `ui/cardio/`
render both tables directly. Rule: **`CoachGoal` is additive in A2 and the old tables keep
working**; the portfolio *reads* them as inputs and offers the user a one-tap "manage this as a
coach goal" promotion. No destructive migration, no trophy regression. A later phase may retire
the old editors once the portfolio covers every metric — that's a separate, explicitly-scoped
decision, not a side effect of A2. `GoalMetric`/`GoalPeriod` (`domain/goal/CustomGoal.kt`) are
the vocabulary the catalogue reuses rather than reinvents.

**M2 · Three-valued verdict storage.** `coach_decision.outcome` is `"pending" | "ok" | "failed"`
(`CoachPass.kt:53`) — a TEXT column, so a fourth value needs **no migration**; the whole change is
in code, and it touches four places at once: `OutcomeWatcher` (currently only emits ok/failed —
`:64`, `:100`, `:104`),
`CoachGenBias.from` (filters `outcome != "failed"` — `CoachGenBias.kt:59`; must also exclude
`not_followed` from bias), `TrustLedger.assess` (must count it as neither win nor loss), and the
Coach Lab / Journey rendering. Value: `"not_followed"`. Ships in B1 with the first
adherence-judgeable decision type, not later.

**M3 · `lessonId` plumbing.** `reason` is a plain `String` on `Recommendation`
(`Recommendation.kt:25`, 8 implementors) and a Room column on `CoachDecision`
(`CoachPass.kt:45`) — there is no `Reason` object to hang a field on. Decision: **add a nullable
sibling** — `val lessonId: String? = null` on the `Recommendation` interface (default null, so
no implementor changes) and a nullable `lesson_id` column on `coach_decision`. One migration, no
refactor. A `Reason` value type is explicitly rejected: it would touch every advisor and every UI
consumer for no behavioral gain.

**M4 · Ledger shape for the new cadences.** `CoachPass` is PK'd by `week_id` and every pass
writes a row — but PostSessionDebrief, TodayDirective and SessionAdaptor acts are daily/session
cadence, and "no unwatched writes, ever" applies to them too. Decision: keep one
`coach_decision` table and add **`scope`** (`"week" | "day" | "session"`) plus **`scope_key`**
(ISO week id / ISO date / session id) alongside the existing `week_id`, which stays populated
for week-scoped rows so the Journey lens keeps working unchanged. New columns in the same
migration as M2/M3. Structural undo adds **`undo_expires_at`** (nullable) — past it, undo becomes
revert-forward: regenerate the prior shape as a new change, keep every logged session.
All new daily entities key on **ISO calendar dates** (`yyyy-MM-dd`), never program-day keys —
`Session.dayKey` is a program-day id ("push"), a known foot-gun.

**M5 · Academy content format and renderer.** There is no markdown library or markdown code
anywhere in the app (zero hits in `app/` and `gradle/libs.versions.toml`), so "ships as markdown"
is undefined work on B3's critical path. Decision: **no new dependency** — lessons are structured
Kotlin/asset data, a `List<LessonBlock>` where `LessonBlock` is `Heading | Paragraph | Bullets |
Callout | Example`, rendered by one Compose component built to `.claude/DESIGN.md`. Rationale:
33 short lessons don't justify a markdown parser, the block model gives the design system real
control, and "Example" blocks can interpolate the user's live numbers — which is the whole point
of the "your numbers" track.

**M6 · One signal, one computation.** Where v3 adds a signal that already exists, the old
computation is deleted in the same change, never left to stack:
- sick → the Life-events flag subsumes `restReason == "sick"` (`ReadinessAdvisor.kt:79`,
  `DeloadAdvisor.kt:184`); the rest-day reason keeps its UI meaning and writes the flag.
- sore → per-muscle gates subsume the generic deduction (`:80`, `:185`).
- cardio interference → `ConditioningLoad` (from `ENGINE_PLAN.md`, whichever plan ships it
  first) replaces the 24 h cardio deduction (`ReadinessAdvisor.kt:84-86`).
Each removal is asserted in a test: the old and new paths must never both fire.

**M7 · Directive displacement.** The Overview already leads with next-workout and coach content.
The directive **takes that slot** — the existing next-workout lead-in is removed, not stacked
beside it, and the coach banner keeps only what the directive doesn't answer (pending proposals).
Shipping a third card that says a similar thing is the failure mode to avoid.

**M8 · Per-muscle taxonomy.** The check-in muscle picker uses `MuscleGroup` (`program/Types.kt:8`)
— the same vocabulary as `VolumeModel`, so gates, caps and projects all speak one language. The
"last 48 h trained muscles" candidate set comes from `AdaptationSnapshot.exerciseHistory` joined
to the library muscle map.

**M9 · Schema versions.** Room is at **v31** (`ForgeDatabase.kt:131`). Planned bumps, one per
shipping phase, each with schema JSON + a `MigrationTest` case: **v32** (A1: `ExerciseBout`
support columns / session-type plumbing if persisted, bodyweight read path), **v33** (A2:
`CoachGoal`, `Lesson`, `LessonEvent`, plus the M3/M4 columns on `coach_decision`), **v34** (B1:
`CheckinEntry`, life-event flags), **v35** (C: `TrainingBlock`), **v36** (D: `CoachProject`),
**v37** (E: `NextSessionAdjustments`). B2, B3 and F are expected to need none, and M2's fourth
verdict value needs none either (TEXT column). Phases that turn out migration-free simply skip
their number — the point is that no phase discovers its migration late.

**M10 · Export coverage.** The ZIP backup covers new tables automatically; the JSON export does
not — `BackupRepository.exportFullDataJson:149` is hand-rolled and already omits every coach
table. Each phase adds its new entities to the JSON export *and* asserts the ZIP restore path in
its migration test. Check-ins, goals, blocks and projects are exactly the data users will want
out.

---

## Phases (each independently shippable)

Rev 2's Phase A and Phase B were each three phases wearing a trench coat. Split below; the
sequencing rationale lives in `ROADMAP.md`.

### Phase A1 — v3.0 "Eat everything" (data foundation, no new surfaces)

- Extend `AdaptationSnapshot` / `SnapshotAssembler` / `AdaptationRepository` with the bodyweight
  series (`BodyweightDao` + HC weight) — the one genuinely missing series.
- Make loaded-but-unread data actually consumed: moods (readiness + deload driver),
  `toFailure`/`setType`/`difficultyTag` into the effort model (proximity-to-failure beside RPE),
  `health.hrv` and `health.dailySteps` into the deload/readiness drivers.
- **Session-type tagging**: add the UI control that emits the existing
  `DayUiEvent.SetSessionType` (the write path already works end-to-end), plus `sessionType` on
  `ExerciseBout` so TEST/TECHNIQUE/FIRST_BACK can finally be filtered out of e1RM stall series.
- Schema: **none** — `ExerciseBout` is a domain type and `session.session_type` already exists, so
  A1 shipped migration-free.
- **Done when**: an advisor-visible diff exists for every consumed input (a test asserting each one
  changes at least one advisor's output). The bodyweight series is plumbing only in A1 — its
  consumer is A2's `bodyweight_goal` / `WeightPhase` wiring.
- **Shipped** (see Status): `EffortModel`, `ExerciseBout.sessionType` + `countsForProgression`,
  bodyweight in the snapshot, mood/HRV/steps drivers, and the session-type picker on session
  **detail** (not the day screen — `ui/gym/train` is untouchable, §14 of `.claude/DESIGN.md`, and
  retro-tagging is the better moment since the engine only reads finished sessions).

### Phase A2 — v3.0.1 "Goal Portfolio + registry contracts"

- **GoalPortfolio**: `CoachGoal` entity, goal catalogue (reusing `GoalMetric`/`GoalPeriod`
  vocabulary), multi-select picker UI, conflict matrix, sequencing proposals, trajectories/ETAs.
  Additive to the existing goal tables per M1 — the portfolio reads them and offers promotion;
  no destructive migration, no trophy regression.
- Planner reasons become goal-referenced; the Week Brief opens with portfolio progress.
- **SignalRegistry** skeleton (all six slots) rendered inside Coach Lab's existing Signals lens;
  wire `bodyweight_goal` (`WeightPhase` + trend + phase-aware stall interpretation) since its
  data already exists.
- **AcademyRegistry** skeleton: `Lesson`/`LessonEvent` entities, `lessonId` plumbing per M3
  (nullable, unused-yet = zero behavior change), Academy section scaffold.
- Ships lesson **C3** (`coach.strength_on_a_cut`), because `WeightPhase` stall-suppression is a
  live coach concept the moment this phase lands and the audit rule is "no shipped concept
  without a lesson". Requires the M5 renderer, so B3's renderer work moves here in miniature —
  one component, one lesson.
- Schema: **v32** (A1 shipped migration-free, so A2 took the next number) — `coach_goal`,
  `lesson_event`, plus the M3 `lesson_id` and M4 `scope` / `scope_key` / `undo_expires_at`
  columns on `coach_decision` (added here even though their consumers land in B1–E: one
  migration beats four).
- **Done when**: a user can select 2+ goals, see a conflict flagged with a sequencing proposal,
  and every coach reason in the Week Brief names the goal it serves; trophies still unlock.
- **Shipped** (see Status): `CoachGoalKind` catalogue, `GoalPortfolio` (readings, weekly slope,
  ETAs, conflict matrix), `WeightPhase` + phase-aware stall suppression on the plateau ladder,
  `SignalRegistry` (11 slots) rendered as the Signals lens's slot rail, `AcademyRegistry` +
  block-model lesson content + the ledger, `CoachGoalRepository` / `AcademyRepository`, the
  Goals section + picker on the Coach page, and coach goals in the JSON export.
  **Not yet done in A2:** the goal-reference pass over planner reasons (every reason naming its
  goal) — the portfolio exists and renders, but `AutoCoachPlanner`'s reason strings still speak
  in v2's vocabulary. That is the remaining item before A2's "done when" is fully met.

### Phase B1 — v3.1 "Readiness v2 + check-in + life events"

- New `CheckinEntry` entity + 5-second sheet (design per `.claude/DESIGN.md` — load the
  forge-design skill before any UI work); prompted at first app-open of a day, always skippable;
  includes the sick option, the optional muscle picker (M8), the optional bodyweight quick-log,
  and adaptive prompting.
- Rebuild `ReadinessAdvisor` with the full input set (sleep, HR, check-in, moods, interference,
  steps, weight flux); named parts, bounded output, per-muscle soreness gates. Old duplicate
  deductions removed per M6.
- **Life events**: sick flag + directive quick action; layoff detection (`VacationPeriod` + raw
  gap detection — *not* `SessionBreak`) with verdict suppression and a FIRST_BACK return-ramp
  week; injury restriction routed around by generator and directive.
- Three-valued verdicts land here (M2), since the return ramp is the first decision type a user
  can visibly not-follow.
- Schema: **v33** (numbering follows A1 shipping migration-free).
- **Done when**: a seeded 3-week gap produces a ramp week and zero stall/watcher verdicts across
  the gap; a sick-flagged week never demotes trust; readiness renders its named parts.
- **Shipped** (see Status): `CheckinEntry` + `InjuryRestriction` + their DAOs and
  `CheckinRepository` (including adaptive prompting), `LifeEvents` (sick / layoff + ramp /
  restrictions), `ReadinessAdvisor` rebuilt as ReadinessV2 with the full input set and
  `assess()` returning soreness gates, the three-valued verdict end to end
  (`OutcomeWatcher` → `CoachDecision.OUTCOME_NOT_FOLLOWED` → neutral in `TrustLedger` and
  excluded from `CoachGenBias`), the `SessionType.FIRST_BACK` writer in
  `WorkoutRepository.startOrResumeSession`, restricted movements excluded from the swap pool,
  and the check-in sheet hosted at the app root.
  **Not yet done in B1:** the sick quick-action lives only in the sheet (its directive-card home
  is B2), and injury restrictions have no management UI yet — they are enforced but can only be
  created programmatically.

### Phase B2 — v3.1.1 "Today Directive + PreSessionBrief"

- **TodayDirective** card on Overview — the one-answer surface, replacing the existing
  next-workout lead-in per M7. Degraded modes are explicit: no block yet (Phase C) ⇒ computed
  from spacing + readiness + schedule; sequence-mode schedule ⇒ next-up-relative; cardio
  directives are suggestions until Engine E-B; freestyle ⇒ the freestyle rule in "Modes".
- Basic PreSessionBrief on the day screen, rounding to `WeightSteps`, with cold-start
  prescriptions for new/swapped exercises.
- Wire the widget's `EXTRA_RESUME_SESSION` and the wear Today tile (`PATH_GLANCE_TODAY`) as
  directive surfaces.
- **Done when**: the directive renders a non-blank, correct answer in all four modes (normal,
  freestyle, coach-off, cold-start) on a fresh install and on a seeded veteran history.

### Phase B3 — v3.1.2 "Academy foundation"

- Lesson renderer per M5 (promoted from A2's single-lesson version to the full block set) +
  Academy section + unlocked/upcoming list.
- **Fundamentals track** (F1–F10) + **C1** (readiness tap-through) + **C2** (goal conflict).
- **Cold-start directive mode**: the curriculum-driven directive below data gates — this is what
  makes Decision Zero true for a day-one user.
- **Done when**: a fresh install never shows a blank directive, every Fundamentals lesson is
  reachable from a real moment, and the 1:1 audit passes (no orphan lessons, no unlessoned
  concepts).

### Phase C — v3.2 "Block periodization"

- `TrainingBlock` entity + `BlockPlanner` state machine; the weekly pass becomes block-aware
  (advance / plan / schedule deload); phase modulates progression, volume ramp, readiness bounds
  and the directive.
- Coach screen mesocycle UI (week-in-block, phase intent, next deload date);
  `WeeklyReview.mesocycleFocus` copy replaced by real block state.
- PEAK defines the **test protocol** — scheduled top-single / AMRAP test days feeding e1RM,
  announced by the directive and tagged via A1's session-type control. A peak phase that never
  tests is a promise without a payoff.
- **Academy**: P1–P4, each wired to its transition moment.
- Schema: **v35**. **Done when**: a seeded history produces a full block cycle with a scheduled
  deload, and the fatigue tripwire can pull it earlier but never later.

### Phase D — v3.3 "Close the learning loop + Proactive Projects"

- **PersonalProfile** estimators promoted from `InsightEngine`; personal volume caps into
  `VolumeModel.allocate` (replacing hard-coded caps within safety bands); recovery curve into
  block/day scheduling and directive spacing; sweet-spot ranges into rep prescriptions; unified
  step/rest calibration.
- **ProjectScanner** + `CoachProject`: the weakness hunter goes live — one active named project
  at a time, visible roadmap (now / next / done); propose-only at this phase.
- `AdviceEvent`-driven per-signal confidence calibration (the promise in its docstring).
- **Academy**: P5–P8 + C4 — the "your numbers" lessons that only make sense once the profile
  exists.
- Schema: **v36**. **Done when**: at least one hard-coded constant per estimator is provably
  replaced by a personal value on a seeded history, and every replacement is watcher-judged.

### Phase E — v3.4 "Initiative: cadences + trust ladder + outreach + adaptivity"

- Full PreSessionBrief / PostSessionDebrief with persisted `NextSessionAdjustments`.
- **SessionAdaptor**: equipment-busy one-tap swap, "I have N minutes" auto-triage, mid-session
  soreness reroute (via finish-early + substitute).
- **TrustLadder T3/T4**: proactive projects self-start (announce-then-run), directive
  notifications, T4 acts-first-informs-after program ownership; Settings tier cap; caps scale
  with tier; rate-based demotion with hysteresis; T4 opt-in consent card; structural undo expiry
  and revert-forward (M4); user-edits-win arbitration.
- **Proactive outreach**: trust-gated categories, frequency-capped, per-category opt-out, backed
  by a WorkManager readiness job and gated by `QuietHoursSchedule`.
- **Coach voice pass**: every reason names the goal/block/project it serves and links its lesson.
- **Academy**: C5–C6, shipped exactly when autonomy ships.
- Schema: **v37**. **Done when**: a T4 structural change can be reverted forward after its undo
  window with zero logged-data loss, and tier oscillation is impossible on a seeded 70%-win-rate
  history.

### Phase F — v3.5 "Future slots go live"

- `stress_hrv`: HC HRV → readiness + deload drivers (the data is already in the snapshot).
- `protein_nutrition`: once nutrition logging exists — targets vs bodyweight+phase, under-fueling
  × stall correlation, and a protein goal type joins the catalogue.
- `hydration_supplements`: consistency tracking → small readiness/insight contributions.
- Slots flip COMING_SOON → ACTIVE with zero rearchitecting (the A2 registry contract).
- **Academy**: S1–S3, one per slot as it activates.
- **Done when**: each activated slot changes at least one advisor output and its absence still
  changes nothing.

---

## Critical files

- **Brain**: `domain/coach/` (`AutoCoachPlanner`, `TrustLedger`, `OutcomeWatcher`, `WeeklyReview`,
  `SessionOpinion`, `CoachGenBias`, `SuggestionCalibrator`) + new `GoalPortfolio` /
  `TodayDirective` / `BlockPlanner` / `ProjectScanner` / `SessionAdaptor` / `PersonalProfile` /
  `CoachSignal` / `LifeEvents`.
- **Academy**: `domain/academy/` (`AcademyRegistry`, lesson block content, `Lesson`/`LessonEvent`),
  lesson-card + block-renderer UI (M5).
- **Engine**: `domain/adapt/` (`AdaptationSnapshot`, `SnapshotAssembler`, `ReadinessAdvisor`,
  `DeloadAdvisor`, `ProgressionAdvisor`, `InsightEngine`, `AdaptThresholds`,
  `RecommendationArbiter`).
- **Orchestration**: `data/repo/CoachRepository.kt` (mode gates live here),
  `data/repo/AdaptationRepository.kt`.
- **Data**: `data/db/` (`Migrations`, `ForgeDatabase`, `CoachDao` + new `CoachGoal` /
  `CheckinEntry` / `TrainingBlock` / `CoachProject` / `NextSessionAdjustments` / `Lesson` /
  `LessonEvent`), `data/health/HealthConnectManager.kt`.
- **Existing goal surfaces (M1 consumers)**: `data/db/entities/ExerciseGoal.kt`,
  `ExtendedGoal.kt`, `domain/goal/CustomGoal.kt`, `ui/goals/`, `program/Trophies.kt`,
  `domain/trophy/TrophyEvaluator.kt`.
- **Generation link**: `program/VolumeModel.kt`, `program/ProgramGenerator.kt`,
  `program/GoalProfiles.kt`, `program/Types.kt` (`MuscleGroup`).
- **UI**: `ui/coach/*` (Coach Lab lenses), `ui/overview/OverviewScreen.kt` (directive slot),
  day screen brief, Settings coach page, Academy section, `widget/ForgeWidget.kt`.

## Verification

- Every pure module gets the existing test treatment (`app/src/test/.../domain/coach/`,
  `domain/adapt/` corpus is the template) — snapshot-built fakes, threshold-edge cases,
  determinism. AcademyRegistry unlock derivation is pure → same treatment.
- Room migrations: schema JSON + `MigrationTest` per version bump (M9), including the ZIP restore
  path and the JSON export list (M10).
- End-to-end per phase: seed a fake history (existing test builders), run the weekly pass → assert
  goal-aware / block-aware decisions; manual run of the app for UI phases (`/run`).
- **Watcher regression**: every new decision type must be judgeable (worked / didn't work / not
  followed) inside a window — **no unwatched writes, ever.**
- **Mode audit per phase**: every new surface declares and tests its behavior in freestyle,
  coach-off and vacation modes (see "Modes"). A surface with no declared freestyle behavior is
  not done.
- **Duplicate-signal audit per phase** (M6): assert the removed computation no longer fires.
- **Academy audit per release series** (v3.0 = A1+A2, v3.1 = B1+B2+B3, then each phase alone):
  grep-able 1:1 — every shipped coach concept has a lesson, every lesson is reachable from a live
  coach moment, cold-start never renders blank. Within a series a concept may ship one phase
  ahead of its lesson; no series ends unlessoned (`ACADEMY_LESSONS.md`).
- **Life-events regression**: a seeded 3-week gap produces a ramp week and zero stall/watcher
  verdicts across the gap; a sick-flagged week never demotes trust.

---

## Appendix — v2 claims verified against source

Re-verified at 0.8.8.3 / v31 (rev 3). Every claim below is true today and is what the plan is
built on:

| Claim | Evidence |
|---|---|
| `ExerciseGoal`/`ExtendedGoal` never read by the coach | zero refs in `domain/coach` + `domain/adapt`; `userGoal` only reshapes rep ranges (`GoalProfiles.kt:17-29`) |
| Mesocycle is copy-only | `WeeklyReview.kt:160` returns a `String?`; `mesocycleWeeks` consumed nowhere else |
| `SessionOpinion` cosmetic | sole call site `DaySessionHandlers.kt:141`, display string only |
| Moods loaded, read by zero advisors | `AdaptationSnapshot.moods` populated, no advisor consumes it |
| Bodyweight not in the snapshot | no field on `AdaptationSnapshot`/`HealthSnap` |
| `toFailure`/`setType`/`difficultyTag` advisor-unread | on `LoggedSet`, consumed by UI/importers/stats only |
| Cardio `effort`/`hrZone` advisor-unread | advisors read cardio via `restReason` + `durationMin` only |
| `health.hrv` / `health.dailySteps` unread | populated by W6, zero advisor refs |
| HC sleep/HR → deload score only; readiness HC-blind | `ReadinessAdvisor.evaluate` takes no `HealthSnap` (`:33-41`); sleep/RHR only in `DeloadAdvisor.instrument()` |
| `AdviceEvent` calibration never built | used only as a 14-day cooldown mute-set (`AdaptationRepository.kt:269-282`) |
| Tier-5 insights feed nothing | `InsightEngine` referenced only by `AdaptationRepository` + Stats UI |
| `VolumeModel.weeklyCap` hard-coded | per-muscle `Map` at `VolumeModel.kt:27-39` |
| Caps 2/wk, ±1 set, ±2 drift, never trust-scaled | `AutoCoachPlanner.kt:213`, `:237`, `:260`, `VOLUME_DRIFT_CAP=2` (`:100`); planner never imports `TrustLedger` |
| No unwatched writes | `OutcomeWatcher` `else → windowClosed → ok` catches every applied type (`:104`) |
| `CoachGenBias.from(decisions)` idempotent recompute | `CoachGenBias.kt:55-87` |
| `ExerciseBout` carries no `sessionType` | `AdaptationSnapshot.kt:111-118` |
| No age/DOB anywhere | only `USER_SEX` (`PreferencesDataStore.kt:201`) |
| Swap blocked once sets are logged | `DaySwapHandlers.kt:13-25` |
| Coach bails entirely in freestyle | `CoachRepository.kt:745` |

## Status

- **Rev 1** (2026-07-15): plan authored with the Academy curriculum.
- **Rev 2** (2026-07-24): re-baselined at 0.8.8.3 / schema v31 — Wear W0–W6 shipped, Life events
  added, trust ladder hardened, watcher verdicts three-valued.
- **Rev 3** (2026-07-24): audit applied (`COACH_ENGINE_PLAN_AUDIT.md`) — five factual corrections
  (SessionBreak, existing sick path, vacation hold, widget deep links, session-type writer);
  shipped machinery folded in (goal system richness + trophy coupling, Coach Lab Signals lens,
  freestyle and coach-off modes); Mechanics M1–M10 decided; Phase A split into A1/A2 and Phase B
  into B1/B2/B3 with per-phase "done when" and schema targets; rev-commentary flattened; the
  superseded `COACH_ENGINE_PLAN_REVIEW.md` deleted, its verified-claims table kept as the
  appendix above.
- **A1 built** (2026-07-24): `EffortModel` (toFailure / setType / difficultyTag folded into one
  proximity-to-failure read), `ExerciseBout.sessionType` + `countsForProgression` filtering
  test/technique/first-back out of progression and fatigue reads, bodyweight series into the
  snapshot, mood + HRV + daily-steps drivers on `DeloadAdvisor`, mood on `ReadinessAdvisor`, and
  the session-type picker on session detail. Migration-free. +40 unit tests.
- **A2 built** (2026-07-24): Goal Portfolio (`CoachGoalKind`, `GoalPortfolio`, `coach_goal`),
  `WeightPhase` + phase-aware stall suppression, `SignalRegistry` + its Coach Lab slot rail,
  `AcademyRegistry` + `lesson_event` ledger + lesson C3, both repositories, the Coach page's
  Goals section + picker dialog, coach goals in the JSON export. Schema **v32** with its
  migration test. +31 unit tests. Outstanding: goal-referenced planner reasons (see Phase A2).
- **B1 built** (2026-07-24): the morning check-in (entity, DAO, repository, root-hosted sheet,
  adaptive prompting), `LifeEvents` (illness, layoff detection + return ramp, injury
  restrictions), ReadinessV2 with check-in / measured sleep / resting-HR / steps / life inputs
  and per-muscle soreness gates, three-valued watcher verdicts wired through trust and bias, the
  FIRST_BACK writer, and restricted movements excluded from swap candidates. Schema **v33** with
  its migration test. +34 unit tests.
- **B2 built** (2026-07-24): `TodayDirective` (train / rest / cardio / learn, with declared
  degraded modes for no-block, sequence-mode, freestyle and cold start), `PreSessionBrief`
  (readiness- and soreness-shaped targets, floor-rounded to loadable steps, cold-start seeds from
  similar lifts), `DirectiveRepository`, the Overview hero rebuilt around the directive (M7), and
  the same answer published to the wear tile via `GlanceTodayDto`.
- **B3 built** (2026-07-24): the lesson-block renderer (M5), the Academy screen and route, the
  Fundamentals track F1–F10 plus C1 and C2, ledger-derived unlocks driven from the snapshot, and
  the cold-start directive carrying its lesson.
- **C built** (2026-07-24): `TrainingBlock` + `BlockPlanner` (four phases, idempotent weekly
  advance, fatigue can pull the deload forward), block-aware weekly pass and plateau ladder, the
  block card on the Coach page, and lessons P1–P4. Schema **v34**.
- **D built** (2026-07-24): `PersonalProfile` (personal volume caps clamped to a safety band,
  recovery spacing, sweet-spot reps, strongest hour) feeding `VolumeModel.allocate` through
  generation, `ProjectScanner` + `CoachProject` (one lever at a time, each with a finish line),
  `ProjectRepository`, the project card, and lessons P5–P8 + C4. Schema **v35**.
- **E built** (2026-07-24, partial): `TrustLadder` (five tiers, rate-based demotion with
  hysteresis, T4 opt-in, user cap) scaling the weekly change cap, `SessionAdaptor` (time triage,
  equipment swap, soreness reroute), and lessons C5–C6.
- **F built** (2026-07-24): HRV live as a readiness input against the athlete's own baseline, its
  registry slot ACTIVE, and lesson S1 (`signals.stress_hrv`). Protein and hydration stay
  COMING_SOON because the app still logs no food — teaching a signal the coach can't read would
  break the plan's own bound.
- Suite at time of writing: **847 unit tests, 0 failures**; `:app:assembleDebug`,
  `:wear:compileDebugKotlin` and the androidTest sources all green.
- **Remaining:** E's persisted `NextSessionAdjustments` / PostSessionDebrief, proactive outreach
  notifications, and the SessionAdaptor's in-session UI (the domain is done and tested; the live
  session screen is untouchable per `.claude/DESIGN.md` §14, so its surface needs a design
  decision first).
