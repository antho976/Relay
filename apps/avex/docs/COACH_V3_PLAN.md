# Coach v3 + Avex Academy — "A Real Coach That Makes Itself Optional"

> Revision 2 — re-verified against the codebase at **0.8.8.3, Room schema v31** (post
> "Watch app" commit: Wear W0–W6 shipped since rev 1, which moves several inputs this
> plan listed as future work into "already plumbed" — noted inline). Full audit trail in
> `COACH_ENGINE_PLAN_REVIEW.md`. Status: no v3 phase started; all new concepts below
> remain greenfield (verified — zero source matches for any of them).

---

## Context

Coach v2 is a superbly-hardened weekly suggestion auditor — idempotent passes, outcome
watcher, earned-trust autopilot, bias folding, LIFO undo — but it is not a coach. A coach:
**knows your goal → has a plan → runs you through it → watches → adjusts → explains →
teaches → remembers.** V2 only does the middle four. The deep review found:

- **No goal model** — `ExerciseGoal`/`ExtendedGoal` exist in Room and the coach never reads
  them; `userGoal` only touches rep templates at generation (`GoalProfiles.kt`).
- **No plan, only reactions** — everything fires off stalls/fatigue. "Mesocycle" exists only
  as a copy string (`WeeklyReview.mesocycleFocus`) that changes no behavior.
- **Wrong cadence** — one weekly brain; in-session chips are disconnected; `SessionOpinion`
  is cosmetic (never changes anything).
- **Massive wasted data** — moods loaded but read by zero advisors; bodyweight not even in
  the snapshot; `toFailure`/`setType`/`difficultyTag` unread; cardio effort/hrZone unread;
  HC sleep/HR feed only the 14-day deload score, not daily readiness; `sessionType`
  technique/test pollutes stall detection; `AdviceEvent` never became confidence calibration.
- **Learning loop open** — Tier-5 insights (`volumeResponse` = personal MEV/MRV,
  `restResponse` = recovery curve, `sweetSpotRepRange`, `timeOfDayPerformance`) are computed
  and shown as Stats trivia; they never feed the planner or generator (which uses hard-coded
  `VolumeModel.weeklyCap`).
- **Ambition never scales with trust** — caps (2 changes/wk, ±1 set, ±2 drift) are permanent
  even after months of earned trust.
- **No knowledge layer** — the original v3 vision was *advice + training knowledge*. V2 (and
  the first draft of this plan) has decisions with reasons, but teaches nothing. A user can
  follow the coach for a year and understand training no better than day one. This creates
  dependence, and dependence contradicts the Avex ethos.
- **No life-events model** *(rev 2 addition)* — nothing handles illness, injury, or a
  layoff. `VacationPeriod`/`SessionBreak` entities and `ReadinessAdvisor`'s `onVacation`
  flag already exist but the coach ignores them; `SessionType.FIRST_BACK` has no writer;
  a flu week or a two-week trip reads as unexplained stalls and fatigue.

**Rev-2 re-baseline:** every claim above was re-verified true at v31. Since rev 1 the
Wear plan shipped in full (W0–W6), which changes the ground under Phase A: `HealthSnap`
already carries HRV and daily steps (populated, advisor-unread), sleep nights include
stages, and `CardioEntry` already has `inclinePct`/`laps`/`elevationM`/`conditions`
(v27–v28). "Eat everything" is therefore mostly a **wire-to-advisor** job, not snapshot
plumbing — bodyweight remains the one genuinely missing series. Also already existing
and unmentioned in rev 1: `WeeklySchedule` (weekday vs sequence mode — see
TodayDirective) and the Glance home widget + wear Today tile as directive surfaces.

User decisions: morning check-in (new capture) ✅ · trust-scaled authority up to full
autonomy ✅ · phased plan, each phase shippable ✅ · declare ALL future slots now
(protein/nutrition, stress/HRV, hydration/supplements, bodyweight-goal coupling) ✅ ·
proactive after trust is earned ✅ · multiple user-selected goals, conflict-aware ✅ ·
remove every "what should I do?" decision from the user ✅ · **Avex Academy folded in as the
knowledge layer — the coach must be a tool you can use, never a dependency that strangles
you into Avex** ✅.

---

## The two governing principles

### Principle 1 — Decision Zero
The user should never have to think "what should I do?" or "what can I do to improve?" —
every question a lifter could ask, the coach answers before it's asked. Every phase is
graded against this bar.

| Question | v3 answer surface |
|---|---|
| What do I do today? | **Today Directive** — one card: "Today: Pull day" / "Rest — here's why" / "20-min zone-2 walk", with the exact session prepped |
| How heavy / how many reps? | **PreSessionBrief** per-exercise targets (readiness- and block-shaped) |
| Am I making progress? Toward what? | **Goal Portfolio** readouts with trajectories + ETAs |
| What's my weak point? What should I change? | **Proactive Projects** — coach continuously hunts the biggest lever and runs a named project on it |
| When do I deload / rest / push? | **Block state machine** + readiness — scheduled, announced in advance |
| Only have 30 minutes / bench is taken / I'm sore? | **In-session adaptivity** — instant re-plan, no thinking |
| Is this program even right for me? | **Coach owns the program at high trust** — re-shapes it and tells you why |
| What should I improve outside the gym? | **Signal slots**: sleep, protein (future), stress, steps — coach names the off-gym lever when it's the real bottleneck |
| *Why is the coach doing this? / How does this actually work?* | **Academy** — every decision links to the lesson behind it |
| *I'm brand new and there's no data yet — what do I do?* | **Academy cold-start track** — the curriculum IS the directive until data gates open |
| What weight do I start with on a new/swapped exercise? | **Cold-start prescription** — relative-strength seed from similar movements, refined by the calibrator |
| I'm sick / hurt / just got back from two weeks away | **Life events** — sick flag, injury restriction, layoff detection + return ramp (rev 2) |

If a decision can be made for the user, it goes on the coach roadmap — the user's only jobs
are to show up, log honestly, and veto.

### Principle 2 — The coach makes itself optional (Avex Academy)
A coach that decides everything forever creates dependence. Avex doesn't do lock-in, guilt,
or manipulation — so the coach's twin is the **Academy**: a knowledge layer that teaches the
user everything the coach knows, at the moment it becomes relevant. The end state is a user
who *chooses* their position between "coach decides everything" and "I understand everything
and use the coach as a calculator" — and can move along that line freely.

**Academy invariants:**
- **Just-in-time, not curriculum-first.** Lessons attach to coach moments, not a course
  index. First scheduled deload → "What a deload is and why you've earned one." First
  plateau → the stall lesson. Low-readiness morning → what readiness is built from. The
  coach already generates the moments; the Academy annotates them.
- **Teach exactly what the coach does — no more.** The curriculum bound is: a user who
  completes the Academy could understand or override every decision the coach makes.
  "Everything about training science" is explicitly out of scope (bottomless pit; other
  people's full-time job).
- **The reason IS the doorway.** Every coach output already carries a human reason
  (v2 invariant). v3 extends it: every reason can link to its lesson
  (`reason.lessonId: String?`). No lesson exists that isn't reachable from a real coach
  moment; no coach concept exists without a lesson (grep-able 1:1 audit).
- **Never gates, never nags.** Lessons are pull, plus one quiet "New lesson unlocked" chip
  on the relevant surface. Reading nothing changes zero behavior. No XP for the Academy —
  learning is not gamified engagement bait (trophies can *acknowledge* completion, never
  drive it).
- **Mirror of the TrustLadder.** Trust is the user delegating decisions *up*; the Academy is
  knowledge flowing *down*. Coach Lab shows both: what the coach is allowed to do, and what
  the user has learned about how it does it.

### The cold-start resolution (where the two principles meet)
"Silent below data gates" (correct, kept) + Decision Zero = contradiction for a new user:
no data → silent coach → user must think → promise broken on day one. **The Academy is the
resolution.** During the data-starved window the Today Directive is curriculum-driven:
fundamentals lessons + the generated program carry "what do I do?" until advisors wake up.
The directive card never goes blank — it degrades from *personalized* ("readiness-shaped
targets") to *principled* ("week 1: learn the movements — here's the lesson + your prepped
session") instead of to *silence*. As data gates open, lessons hand off to advisors
concept-by-concept, and each handoff is itself a teachable moment ("I have 3 weeks of your
data now — from here, your targets are yours, not templates").

---

## Design invariants (carry v2's soul forward)

- Every advisor stays a **pure function of a snapshot** (`AdaptationSnapshot` grows, pattern
  stays).
- **Silent below data gates** — never confident-wrong on sparse data; all thresholds in
  `AdaptThresholds`. (Cold-start silence is filled by the Academy track, not by lowering
  gates.)
- Every output carries a **human reason**; every reason may carry a **lessonId**; every
  write goes through existing user write paths with undo-state; the **watcher judges
  everything applied**.
- Watcher verdicts are **three-valued: worked / didn't work / not followed** *(rev 2)*.
  "Not followed" (the user skipped it) feeds re-planning and dose reduction — it never
  demotes trust and never folds into bias. Only efficacy failures do. Skipping a Tuesday
  walk is user behavior, not bad advice.
- Multi-week acts (block plans, goal sequencing) can't be judged by 14-day windows —
  they get **per-block checkpoint verdicts** instead of being exempt or misjudged.
- **New signals are additive**: absent source ⇒ zero behavior change (Health Connect
  precedent). Academy content is additive the same way: missing lesson ⇒ reason renders
  without a link, nothing breaks.
- All learning is **recomputed from durable ledgers** (idempotent, never compounding) — the
  `CoachGenBias.from(decisions)` pattern. Academy read-state is a ledger too
  (`LessonEvent` rows), same recompute rule.

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

### New domain concepts

**GoalPortfolio** (`domain/coach/GoalPortfolio.kt` + new Room entity `CoachGoal`): the coach
runs a portfolio of selected objectives, not one hidden setting. Catalogue of goal types —
strength on a lift (1RM target), build a muscle/area, lose fat / recomp, consistency habit
(sessions/week streak), conditioning (weekly cardio minutes / zone-2 base), fix an imbalance
(push/pull, quad/ham), endurance on a movement — each with a measurable metric, trajectory
(robust slope over `ExerciseBout.bestE1rm()` / volume / adherence series), ETA, and
on/off-track state. Rules:
- **Multi-select with conflict detection**: a pure `conflicts(a, b)` matrix — compatible
  goals run in parallel (bench 1RM + consistency + zone-2 base = fine); conflicting ones
  (max strength gain + aggressive cut) are flagged and the coach proposes sequencing ("cut
  first 8 weeks, then a strength block — here's the order and why"), never silent
  degradation. *Academy hook: conflict flags link to the "why these goals fight" lesson.*
- **Coach-proposed goals**: the coach continuously scans for goal candidates it can pitch
  ("your pull is 40% behind your push — want me to make fixing your back a goal?").
- Absorbs `ExerciseGoal`/`ExtendedGoal` as migration inputs.
- **Lifecycle** *(rev 2)*: reaching a target/ETA is a first-class moment — celebrate,
  archive to the ledger, propose a successor. The portfolio never silently carries dead
  goals.
- **Block arbitration** *(rev 2)*: one block, many goals — block focus = the top-priority
  goal; every other active goal gets an explicit maintenance floor. Strength-vs-
  conditioning volume collisions are settled in `RecommendationArbiter`.
- Every planner intervention names the portfolio goal it serves; the Week Brief opens with
  portfolio progress, not generic numbers.

**ProactiveProjects** (`domain/coach/ProjectScanner.kt` + `CoachProject` rows): the "what
can I improve?" killer. The coach permanently hunts the single biggest improvement lever
across every signal (lagging muscle, imbalance, missing movement pattern, no conditioning
base, chronic short sleep, volume below personal MEV, estimate-vs-reality drift,
skipped-exercise waste) and runs **ONE active project at a time** with a name, a why, a
plan, and a finish line ("Project: Rear-delt catch-up — 4 weeks, +3 sets/wk, done when its
growth matches your shoulders"). Completed/abandoned projects go in the ledger; the scanner
picks the next lever. This IS the coach roadmap the user sees. *Academy hook: each project
type ships with its lesson ("what an imbalance is, why it matters, how we fix it").*

**TodayDirective** (`domain/coach/TodayDirective.kt`, pure): the flagship Decision-Zero
surface — ONE answer at the top of Overview every day: train (which day, prepped, with
readiness-shaped targets), rest (and why, and what to do instead: walk / mobility / sleep),
or cardio (type, duration, zone — serving the conditioning goal). Computed from block phase,
readiness, spacing/recovery curve, portfolio priorities, and the calendar week's remaining
session budget. Never a menu — a directive, with the veto one tap away. **Cold-start mode:**
below data gates the directive is curriculum-driven (lesson + prepped template session),
never blank (see cold-start resolution above). *(Rev 2 additions:)* **Calendar
substrate:** the directive reads the existing `WeeklySchedule` — weekday mode gives the
full week layout; sequence mode only knows "next up", so day-placement logic degrades to
next-up-relative and the coach may pitch adopting weekday mode as an early project.
**Dual-discipline days:** one answer, but with an optional secondary slot ("Upper day ·
then 20 min Z2") so Engine's post-lift zone-2 never turns the directive into a menu.
**Until Engine E-B ships**, rest-day cardio directives are suggestions ("a 20-min walk
would serve recovery"), never structured prescriptions. **Surfaces:** the Overview card,
plus the existing Glance home widget (deep links are stubbed — wire them) and the wear
Today tile handshake (already shipped, waiting for the directive to exist).

**TrainingBlock** (new Room entity + `domain/coach/BlockPlanner.kt`): persisted block —
phase (ACCUMULATE / INTENSIFY / PEAK / DELOAD), weekIndex, plannedWeeks, focus
lifts/muscles, intent text. Weekly pass advances it; advisors consult it: volume ramps +N
sets across accumulation, progression aggressiveness rises into peak, deloads become
scheduled and earned (fatigue score can pull one earlier — `DeloadAdvisor` becomes the
block's tripwire, not the only path). *Academy hook: first block start → periodization
lesson; each phase transition → that phase's lesson.*

**ReadinessV2** (`domain/adapt/ReadinessAdvisor` rebuilt): inputs = last-night sleep (HC —
stages already read), today's resting HR vs own baseline, morning check-in (new
`CheckinEntry`: sleep quality / soreness / stress / motivation, 4 taps ≈5s, skippable),
moods, acute load, cardio interference (= Engine's `ConditioningLoad`, one formula — see
rev-2 note below), HC steps (already plumbed), bodyweight flux. Output: 0–100 score with
named parts + bounded scale % (bound widens with trust tier), plus per-muscle soreness
gating. *(Rev 2 check-in details:)* a **sick/unwell option** (feeds Life events); flagged
soreness opens an **optional muscle picker** — per-muscle gates need a per-muscle source,
one generic tap can't provide it (fallback: infer candidates from the last 48 h of trained
muscles, confirm with one tap); an **optional bodyweight quick-log** (morning is weigh-in
time and `WeightPhase` trend detection is data-hungry); **adaptive prompting** — users who
always skip stop being prompted. **Interference is defined once:** ReadinessV2 consumes
the pure `ConditioningLoad` function from whichever plan ships it first (Coach B or
Engine E-A) — never a parallel effort×zone×minutes reimplementation. *Academy hook: score
tap-through → "what readiness is built from," with the named parts as the lesson's live
example.*

**Life events** (`domain/coach/LifeEvents.kt` + check-in flags — *new in rev 2*): the
missing half of real coaching — illness, injury, layoffs. Three mechanisms:
- **Sick flag** — a check-in option and a quick action on the directive card. While
  sick: rest/recovery directives, no stall or watcher verdicts accrue, readiness floored.
- **Layoff detection + return ramp** — consume the existing `VacationPeriod` /
  `SessionBreak` entities plus raw gap detection. After ≥14 days off: suppress
  stall/outcome verdicts across the gap, re-enter with a ramp week (~-10% loads;
  `SessionType.FIRST_BACK` finally gets its writer), restart or extend the block instead
  of pretending the calendar didn't happen.
- **Injury restriction** — a "restricted muscle/movement until cleared" flag, distinct
  from acute soreness, that the generator, directive, and SessionAdaptor all route
  around.
*Academy hook: F6 (soreness vs injury) unlocks from the first sick/injury flag — the
curriculum already references this flag; this is the machinery that creates it.*

**PersonalProfile** (`domain/coach/PersonalProfile.kt`, assembled like GenBias): the unified
"what the coach knows about YOU" — per-muscle personal volume caps (promote `volumeResponse`
estimator; clamped to a safety band around `VolumeModel.weeklyCap`), recovery curve
(`restResponse` → preferred day spacing), per-lift sweet-spot ranges (bias rep
prescriptions), step calibration (absorb `SuggestionCalibrator`/`RestTuning`), time-of-day
strength. Defaults = today's constants; every estimator hard-gated. *Academy hook: the
"your numbers vs the defaults" lesson — the moment the app explains MEV/MRV using the
user's own data is the single best teaching moment in the product.*

**TrustLadder** (extends `TrustLedger`): global trust score from accepted proposals ×
watcher win-rate × weeks coached. Trust doesn't just unlock bigger edits — it unlocks
**initiative**:
- T0 observe → T1 propose (v2 today) → T2 auto-apply earned types (v2 autopilot)
- **T3 — proactive**: coach plans/advances blocks one-tap, caps rise (±2 sets, 4
  changes/wk), starts Proactive Projects on its own (announce-then-run), sends directive
  notifications ("Tomorrow morning is your strongest window — Upper day is prepped").
- **T4 — full autonomy**: coach owns the program (frequency/split restructures, goal
  sequencing, scheduling), acts first and informs after — the weekly brief becomes "here's
  what I changed and why", everything still watcher-judged and one-tap revertible.

*(Rev-2 hardening:)* **Demotion is rate-based, not single-failure** — v2's any-failure
rule stays for per-type auto-apply, but a T3+ coach making many autonomous calls at a
real-world win-rate would oscillate tiers forever; demote on failure *rate* or on
user-reverts, with hysteresis. **T4 is opt-in at the moment it's earned** — one consent
card ("You've unlocked full autonomy — turn it on?"), never a silent switch. The user can
cap the tier in Settings; every autonomous act keeps the full undo/watch machinery.
**Structural acts get real undo semantics**: LIFO undo can't unwind a split restructure
the user has trained under for a week — structural changes carry an undo-window expiry
plus a "revert forward" rule (regenerate the old shape, keep all logged data).
**Concurrent edits**: the user's manual edits always win and become pinned constraints +
preference signal — never a merge conflict. *Academy mirror: tier changes are teachable
moments ("here's what T3 lets me do, and here's how you'd do it yourself").*

**In-session adaptivity** (`domain/coach/SessionAdaptor.kt`): the mid-workout "what now?"
eliminator — three instant re-plans, all through existing swap/reorder paths:
- **Equipment busy**: one tap on a slot → best equivalent movement right now
  (swap-candidate pool already exists in `ProgramSlotSnap.swapCandidateIds`).
- **Short on time**: "I have N minutes" → session auto-triaged to its highest-value core
  (priority: goal-serving lifts > compounds > accessories), using `SessionEstimate` +
  personal rest tuning for real pacing.
- **Something hurts / too sore**: per-muscle soreness gate reroutes mid-session
  (drop/replace the aggravating movement, flag it for the weekly pass). *(Rev-2
  constraint check: swap is blocked today once sets are logged —
  `DaySwapHandlers.kt:23`. The adaptor adds a "finish early + substitute" path for
  mid-exercise reroutes rather than relaxing that rule.)*

**Proactive outreach** (extends `ForgeNotifications`/`WeeklyRecapWorker` patterns):
trust-gated, quiet-by-default notifications the coach initiates — session-window nudges
(time-of-day strength gap since last session), "readiness is unusually high, great day to
push", block transitions ("deload week starts Monday"), project milestones, goal ETAs
reached. Frequency-capped and per-category opt-out — persistent, never spammy. *(Rev 2:)*
directive-grade outreach requires readiness computed **with no app open** — a WorkManager
job (per the `WeeklyRecapWorker` precedent) does the HC reads + snapshot assembly in the
background. Every category is gated by the existing per-day `QuietHoursSchedule`.

**SignalRegistry** (`domain/coach/CoachSignal.kt`): declared slots with
`availability = ACTIVE | AWAITING_DATA | COMING_SOON`. Registered from day one:
- `protein_nutrition` (COMING_SOON — flags under-fueling vs stalls once nutrition logging
  ships)
- `stress_hrv` (AWAITING_DATA — HC HRV read when granted; subjective stress from check-in
  is ACTIVE)
- `hydration_supplements` (COMING_SOON — creatine consistency etc.)
- `bodyweight_goal` (ACTIVE early — data already exists)
- `cycle_readiness` (COMING_SOON — menstrual-cycle-aware readiness; HC exposes cycle
  data and `USER_SEX` already exists. *Rev-2 addition: the "declare ALL slots now"
  decision had missed it.*)

Advisors iterate the registry; absent sources contribute nothing; Coach Lab renders every
slot (active / forming / coming) so the product visibly grows into the architecture.

**AcademyRegistry** (`domain/academy/AcademyRegistry.kt` + Room entities `Lesson` is static
content, `LessonEvent` is the read/completed ledger): the knowledge layer.
- **Content model**: short lessons (1–3 min read), plain language, each keyed by
  `lessonId`, grouped into tracks (Fundamentals, Coach Concepts, Signals, Programming).
  Content ships in-app (offline, like everything else) as structured markdown/asset files —
  no server, no CMS.
- **Wiring**: `reason.lessonId` on coach outputs; "unlocked" = the first time its moment
  fires. *(Rev-2 correction:)* unlock state derives from **durable ledgers, plural** —
  the coach ledger for coach moments, and `LessonEvent` rows for app-moment triggers
  (first rest-timer use, first readiness tap, first mesocycle-UI open), since half the
  curriculum's triggers are app-usage events that write no coach rows. Same idempotent
  recompute rule for both; no separate progression state to corrupt.
- **Cold-start track**: Fundamentals (≈10 lessons) is the only track surfaced
  sequentially, and only during the data-starved window, as part of the Today Directive.
- **Surfaces**: lesson cards inline at coach moments; an Academy tab/section listing
  unlocked + upcoming (visible-but-locked, like SignalRegistry's COMING_SOON — the product
  visibly grows); Coach Lab cross-links ("this advisor uses concepts: readiness, MEV —
  learned/unread").
- **Authoring reality check (solo dev)**: content is the real cost, not code. Foundation
  batch ≈ 10 fundamentals + ~1 lesson per coach concept per phase (each phase ships 4–8
  lessons for the machinery it introduces). Total ≈ 30–40 short pieces across all phases —
  bounded by the "teach exactly what the coach does" rule. Write them as each phase's
  machinery lands, never ahead of it.

**Bodyweight-goal coupling** (`WeightPhase`: cut / maintain / bulk, a Settings pick + trend
detection from `BodyweightEntry`+HC weight): reinterprets signals per phase — a held e1RM
while cutting is a WIN (suppress stall escalation, celebrate retention); slow bulk with
stalls → under-eating callout; readiness reads weight-flux. *Academy hook: the
"strength on a cut" lesson fires the first time stall-suppression triggers — turning the
coach's most counterintuitive behavior into its most trust-building explanation.*

### Three cadences

- **PreSessionBrief** (day-screen open): pure fn of snapshot + ReadinessV2 + block phase +
  `NextSessionAdjustments` → per-exercise targets with intent ("3×8 @ 145 — week 3 ramp;
  readiness is low, top set only on squats"). Replaces today's disconnected chips as the
  delivery layer — chips remain but become the brief's line items. *(Rev 2:)* targets
  always round to the shared weight-step table in `:shared` (KG 2.5 / LB 5 / plates) —
  no unloadable prescriptions. New or swapped exercises get a **cold-start prescription**
  (relative-strength seed from similar movements, refined by the calibrator) — "what
  weight do I start with?" is a Decision-Zero question.
- **PostSessionDebrief** (session finish): `SessionOpinion` upgraded from cosmetic to
  causal — computes and persists `NextSessionAdjustments` (per-lift micro-plan consumed by
  the next PreSessionBrief), so the coach reacts session-to-session, not week-to-week.
- **WeeklyPass v3**: current pass promoted to block manager — advances phase, plans next
  week's intent, structural changes, goal progress report in the Brief.

---

## Phases (each independently shippable)

### Phase A — v3.0 "Eat everything + Goal Portfolio" (foundation)
- Extend `AdaptationSnapshot`/`SnapshotAssembler`/`AdaptationRepository`: bodyweight series
  (`BodyweightDao` + HC weight — the one genuinely missing series); make moods actually
  consumed (readiness + deload driver); feed `toFailure`/`setType`/`difficultyTag` into
  the effort model (proximity-to-failure beside RPE). *(Rev 2: HRV, daily steps, sleep
  stages, and the rich cardio fields are already IN the snapshot — those line items are
  now advisor-consumption work, not plumbing.)*
- Filter sessionType technique/test/first_back out of e1RM stall series — *(rev-2
  prerequisites discovered:)* TEST/TECHNIQUE/FIRST_BACK currently have **no writer** (no
  session-type picker exists) and `ExerciseBout` carries no `sessionType`, so this needs
  two added pieces: the **session-type picker** (or auto-tagging) and `sessionType` on
  the bout.
- Convention *(rev 2)*: all new daily entities key on ISO calendar dates (`yyyy-MM-dd`),
  never program-day keys — `Session.dayKey` is a program-day id ("push"), a known
  foot-gun.
- **GoalPortfolio**: `CoachGoal` entity + goal catalogue + multi-select picker UI +
  conflict matrix + sequencing proposals + trajectories/ETAs; migrate
  `ExerciseGoal`/`ExtendedGoal` in. Planner reasons become goal-referenced; Week Brief
  opens with portfolio progress.
- Declare **SignalRegistry** skeleton (all four slots); wire `bodyweight_goal`
  (`WeightPhase` + trend + phase-aware stall interpretation) since its data already exists.
- Declare **AcademyRegistry** skeleton: `Lesson`/`LessonEvent` entities, `reason.lessonId`
  plumbing on coach outputs (nullable, unused-yet = zero behavior change), Academy section
  scaffold. *No content pressure yet — the contract ships, lessons follow in B.*
- Room migration(s) — follow `Migrations.kt` + schema-JSON + `MigrationTest.kt` pattern.

### Phase B — v3.1 "Decision Zero daily layer: Today Directive + Readiness v2 + check-in + life events + Academy foundation"
- New `CheckinEntry` entity + 5-second sheet (design per `.claude/DESIGN.md` — load
  forge-design skill before any UI work); prompt at first app-open of a day, always
  skippable. *(Rev 2:)* includes the sick/unwell option, optional muscle picker on
  soreness, optional bodyweight quick-log, and adaptive prompting (see ReadinessV2).
- **Life events** *(rev 2 — promoted into this phase; the check-in is its natural capture
  point)*: sick flag + directive quick action; layoff detection (consume
  `VacationPeriod`/`SessionBreak` + gap detection) with verdict suppression and a
  FIRST_BACK return-ramp week; injury restriction flag routed around by generator and
  directive.
- Rebuild `ReadinessAdvisor` with the full input set (sleep, HR, check-in, moods,
  interference, steps, weight flux); named parts, bounded output, per-muscle soreness gates.
- **TodayDirective** card on Overview — the one-answer surface (train X / rest / cardio,
  with the why and the prepped session). Basic PreSessionBrief on the day screen.
  *(Rev 2 — degraded modes declared:)* no block yet (Phase C) ⇒ computed from spacing +
  readiness + schedule only; sequence-mode schedule ⇒ next-up-relative placement; cardio
  directives are suggestions until Engine E-B. Wire the widget deep links + wear Today
  tile handshake as directive surfaces.
- **Academy foundation**: lesson-card component + Fundamentals track (≈10 lessons: what a
  program is, sets/reps/RPE, form vs load, progressive overload, rest & recovery, soreness
  vs injury, warm-ups, how the coach works, what readiness means, how to log honestly) +
  **cold-start directive mode** (curriculum-driven directive below data gates — this is
  what makes Decision Zero true for a day-one user). Readiness tap-through lesson ships
  here too (first live coach-moment link).

### Phase C — v3.2 "Block periodization"
- `TrainingBlock` entity + `BlockPlanner` state machine; WeeklyPass becomes block-aware
  (advance/plan/schedule deload); phase modulates progression + volume ramp + readiness
  bounds + the Today Directive.
- Coach screen mesocycle UI (week-in-block, phase intent, next deload date).
- `WeeklyReview.mesocycleFocus` copy replaced by real block state.
- *(Rev 2:)* PEAK defines the **test protocol** — how strength goals actually get
  expressed: scheduled top-single / AMRAP test days feeding e1RM, announced by the
  directive. A peak phase that never tests is a promise without a payoff.
- **Academy**: periodization track (what a block is, the four phases, why deloads are
  earned not failures, reading your block card) — each wired to its transition moment.

### Phase D — v3.3 "Close the learning loop + Proactive Projects"
- **PersonalProfile** estimators promoted from `InsightEngine`; personal volume caps into
  `VolumeModel.allocate` (replacing hard-coded caps within safety bands); recovery curve
  into block/day scheduling + directive spacing; sweet-spot ranges into rep prescriptions;
  unify step/rest calibration.
- **ProjectScanner** + `CoachProject`: the weakness hunter goes live — one active named
  project at a time, visible roadmap (now / next / done) on the coach screen; propose-only
  at this phase.
- `AdviceEvent`-driven per-signal confidence calibration (the promise in its docstring).
- **Academy**: "your numbers" track (MEV/MRV with the user's own data, recovery curve,
  sweet-spot ranges, what a project is / imbalances) — the personal-data lessons that only
  make sense once the profile exists.

### Phase E — v3.4 "Initiative: three cadences + trust ladder + outreach + adaptivity"
- Full PreSessionBrief / PostSessionDebrief with persisted `NextSessionAdjustments`.
- **SessionAdaptor**: equipment-busy one-tap swap, "I have N minutes" auto-triage,
  mid-session soreness reroute.
- **TrustLadder T3/T4**: proactive projects self-start (announce-then-run), directive
  notifications, T4 acts-first-informs-after program ownership; Settings tier cap; caps
  scale with tier. *(Rev 2:)* rate-based demotion with hysteresis, T4 opt-in consent
  card, structural undo expiry + revert-forward, user-edits-win arbitration (see
  TrustLadder).
- **Proactive outreach**: trust-gated notification categories (session windows, readiness
  peaks, block transitions, project milestones, goal ETAs), frequency-capped, per-category
  opt-out.
- **Coach voice pass**: every reason names the goal/block/project it serves — and links its
  lesson.
- **Academy**: trust & autonomy track (what each tier means, how to take decisions back,
  how to read the watcher's verdicts) — the "tool, not strangle" lessons, shipped exactly
  when autonomy ships.

### Phase F — v3.5 "Future slots go live"
- `stress_hrv`: HC HRV read → readiness + deload drivers.
- `protein_nutrition`: once nutrition logging exists — targets vs bodyweight+phase,
  under-fueling × stall correlation ("3 lifts stalled + protein averaging 0.6 g/lb — this
  is a kitchen problem"), and a protein goal type joins the portfolio catalogue.
- `hydration_supplements`: consistency tracking → small readiness/insight contributions.
- Slots flip COMING_SOON → ACTIVE with zero rearchitecting (registry contract from Phase A).
- **Academy**: one lesson per signal as it activates (why protein gates growth, what HRV
  tells us, etc.) — same additive rule as the signals themselves.

---

## Critical files

- **Brain**: `domain/coach/` (`AutoCoachPlanner`, `TrustLedger`, `OutcomeWatcher`,
  `WeeklyReview`, `SessionOpinion`, `CoachGenBias`, `SuggestionCalibrator`) + new
  `GoalPortfolio` / `TodayDirective` / `BlockPlanner` / `ProjectScanner` / `SessionAdaptor`
  / `PersonalProfile` / `CoachSignal`.
- **Academy**: `domain/academy/` (`AcademyRegistry`, lesson content assets,
  `Lesson`/`LessonEvent` entities), lesson-card UI component.
- **Engine**: `domain/adapt/` (`AdaptationSnapshot`, `SnapshotAssembler`,
  `ReadinessAdvisor`, `DeloadAdvisor`, `ProgressionAdvisor`, `InsightEngine`,
  `AdaptThresholds`, `RecommendationArbiter`).
- **Orchestration**: `data/repo/CoachRepository.kt`, `data/repo/AdaptationRepository.kt`.
- **Data**: `data/db/` (Migrations, ForgeDatabase, CoachDao + new
  `TrainingBlock`/`CheckinEntry`/`CoachGoal`/`CoachProject`/`Lesson`/`LessonEvent`
  entities), `data/health/HealthConnectManager.kt` (HRV, weight already present).
- **Generation link**: `program/VolumeModel.kt`, `program/ProgramGenerator.kt`,
  `program/GoalProfiles.kt`.
- **UI (later phases)**: `ui/coach/*`, day screen brief, Settings coach page, Academy
  section.

---

## Verification

- Every pure module gets the existing test treatment
  (`app/src/test/.../domain/coach/`, `domain/adapt/` corpus is the template) —
  snapshot-built fakes, threshold-edge cases, determinism. AcademyRegistry unlock
  derivation is pure → same treatment.
- Room migrations: schema JSON + `MigrationTest.kt` per version bump.
- End-to-end per phase: seed a fake history (existing test builders), run weekly pass →
  assert goal-aware/block-aware decisions; manual run of the app for UI phases (`/run`).
- Watcher regression: every new decision type must be judgeable (ok/failed) inside a
  window — **no unwatched writes, ever.**
- Academy audit (per phase): grep-able 1:1 check — every shipped coach concept has a
  lesson; every lesson is reachable from a live coach moment; cold-start directive never
  renders blank on a fresh install.
- *(Rev 2:)* Every new entity added to the JSON export —
  `BackupRepository.exportFullDataJson` has a **hardcoded entity list** that already
  excludes coach tables; the ZIP backup covers new tables automatically, the JSON export
  does not. Check-ins, goals, blocks, and projects are exactly the data users will want
  out.
- *(Rev 2:)* Life-events regression: a seeded 3-week gap produces a ramp week and zero
  stall/watcher verdicts across the gap; a sick-flagged week never demotes trust.

---

## Status

- **Rev 1** (2026-07-15): plan authored; committed with the Academy curriculum.
- **Rev 2** (2026-07-24): re-verified against 0.8.8.3 / schema v31 and amended per
  `COACH_ENGINE_PLAN_REVIEW.md` — Wear W0–W6 shipped (HRV/steps/sleep-stage plumbing
  landed, lowering Phase A's cost), Life events concept added, Phase B scope grew
  (sick flag, layoff ramp, injury restriction, check-in extras), trust ladder hardened,
  watcher verdicts three-valued.
- **No v3 phase started.** Phase A is next.
