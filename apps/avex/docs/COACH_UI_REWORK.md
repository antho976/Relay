# Coach UI — rework plan

> **Status: steps 1–7 built 2026-07-26** (see §6). Step 8 (lens rename) waits on a call; step 9
> (the cross-app fixes in §5) is untouched and is the larger piece. Everything below is the plan as
> written; the build-order table records what shipped.
>
> Written 2026-07-26 against `relay/term-2` @ `d6ccdd9`
> (0.8.8.3). Scope is the **Coach page and its neighbours' coach surfaces** — the UI only. The
> engine work is `COACH_V3_PLAN.md` / `ENGINE_PLAN.md`; this plan assumes the engine as it stands
> today and never asks it for data it does not already produce.
>
> Binding: `.claude/DESIGN.md` (Coach = **Overview archetype**, §3). Start from
> `ui/recipes/OverviewRecipe.kt`, not from this document's prose.

---

## 0. Method, and what this is based on

- Read every file in `ui/coach/` (15 files, ~2.1k lines), `CoachViewModel`, `CoachRepository`'s
  data model, and every coach surface on Home.
- Built and installed this branch's debug APK on the S21 Ultra and walked all three lenses.
  Screenshots are in the session scratchpad.
- **Limitation, stated up front:** the device account has zero sessions, so everything observed is
  the **first-run state**. Populated states below are read from the code, not seen. The in-app
  "try demo data" hook exists but is wired to nothing — `OverviewViewModel.loadSampleData:322` has
  no call site anywhere in `ui/`, so there is no way to populate the app from the UI. (Small
  finding in its own right; see §7.)
- First-run is not a corner case here. `AUDIT.md` already flags it: *"first-run is the one state
  every new user sees and the one nobody re-checks."* It is also the state the Coach page is worst in.

---

## 1. What the coach actually does

Everything the UI is allowed to draw, and where it comes from.

| Output | Source | Shape |
|---|---|---|
| Weekly pass + hold reason | `CoachRepository.ensureWeeklyPass` → `AutoCoachPlanner.evaluate` | one status + one reason string per week |
| Decisions | `CoachDecision` rows | summary, reason, type, target, status, outcome, undo payload |
| Weekly review | `WeeklyReview` | sessions vs target, volume + delta%, PRs, fatigue score, cardio minutes, focus line |
| Baseline countdown | `sessionsLogged` / `minSessions` | n of m |
| Fatigue score + checks | `DeloadAdvisor` | score vs threshold, plus 6 named checks each with its own reading |
| Tracked lifts | `CoachWatch.trackedLifts` + `e1rmBySlot` | name, bouts, stalling flag, e1RM series |
| Recovery inputs | `CoachWatch.recoverySignals` + `HealthSeries` | label, count, active; sleep nights, resting HR, HRV |
| Learned biases | `CoachGenBias` | label + detail |
| Trust per change type | `TrustLedger` | streak of m, earned |
| Milestones | `CoachTimeline.milestones` | label, detail, reached |
| Week-by-week record | `CoachTimeline.weeks` | pass + its decisions |
| Goal portfolio | `GoalPortfolio` | reading, on-track, ETA weeks, conflicts |
| Training block | `BlockPlanner` | phase of 4, intent, test-week flag |
| Project | `ProjectScanner` | name, why, plan, finish line |
| Personal profile | `PersonalProfile` | best session spacing, per-muscle volume caps |
| Signal registry | `SignalRegistry` | 11 slots × active / awaiting / not-built |

**Produced but not on the Coach page:** `TodayDirective` and `PreSessionBrief` (rendered on Home,
`OverviewScreen.kt:422`), `SessionOpinion` (live session), and the whole
`ProgressionAdvisor + DeloadAdvisor` feed (Home, `OverviewScreen.kt:590`).

**Produced and rendered nowhere:** `SessionAdaptor` — defined, tested, zero consumers.

---

## 2. What the UI is today

`ui/coach/` — one `Scaffold` + `LazyColumn`, a hero, three lens pills, thirteen sections.

```
CoachScreen.kt          the shell: top bar, hero, pills, lens dispatch
CoachHero.kt            eyebrow → serif verdict → aside → 3–4 figures  (or LearningHero pre-baseline)
CoachUi.kt              CoachSection / CoachAction / CoachFlagDot / LiftTrendRow, COACH_GUTTER, COACH_ROW_PAD
CoachCharts.kt          sparkline, fatigue meter, ghost spark, watch bar, progress row, sleep bars, HR line
CoachWeekSection.kt     ← the Now lens (misnamed file)
CoachGoalsSection.kt    Goals            (Now)
CoachBlockSection.kt    Block            (Now)
CoachProjectSection.kt  Project          (Now)
CoachSignalsSection.kt  Signals lens
CoachSlotRail.kt        "What it can read" rail
CoachJourneySection.kt  Journey lens
TrustProgressBar.kt     segmented meter
GoalPickerDialog.kt     add-a-goal dialog
```

**Now** — Goals · Block · Project · The call · Under watch · Coming up
**Signals** — Lifts on watch · Recovery load · What it can read · What it reads · Learned so far
**Journey** — The record · Earned autopilot

Entry points: the Coach hub tab (`HubScreen.kt:158`), plus three legacy routes that deep-link a
lens (`COACH_BRIEF` → Now, `COACH_LAB` → Signals, `COACH_TIMELINE` → Journey).

---

## 3. Diagnosis

### 3.1 The page is organised by subsystem, not by question

Thirteen section names, each named after an internal concept: *Goals, Block, Project, The call,
Under watch, Coming up, Lifts on watch, Recovery load, What it can read, What it reads, Learned so
far, The record, Earned autopilot.* A user arrives with three questions — **what do I do, why, is
it working** — and has to reverse-engineer which of thirteen headings answers which.

This is the root cause. Most of what follows is a symptom of it.

### 3.2 First-run: the page opens with three prompts and no reading

Observed on device. The Now lens leads with **Goals** (an italic hint + two accent links), then
**Block** (a rail + a two-line explainer + a link), then **Project** (four stacked prose lines + a
link). The first actual reading is *Coming up*, below the fold.

Specific breaches:

- **Goals section has no mark at all** (`CoachGoalsSection.kt:45`) — a hint and two links. §12: every
  data section leads with a MARK. Goal rows *are* data (reading + ETA + on-track), and §2② gives the
  shape: value vs target → **meter bar**. It draws none, at zero or with data.
- **Project section has no mark at all** (`CoachProjectSection.kt:44-59`) — name, why, plan, finish
  line, four text lines. §4.3's prose budget is ONE ~12-word caption per section.
- **Three `InlineEmptyHint`s can render on one lens** (Goals, Block, Project). §12: ≤1 per lens.
- **Block's explainer is mechanics narration** ("A block gives the next few weeks one intent, and
  schedules the deload instead of waiting for one") — §4.3 says that is cut, not trimmed.
- **§4.8 inverted.** Placement is rank; live data outranks setup prompts. Today the three
  configuration prompts sit above the week's actual call.
- **The pre-baseline hero has no figures.** The figure row is inside `brief.review?.let`
  (`CoachHero.kt:99`), and `review` is null until the baseline completes, so first-run shows an
  eyebrow and one thin bar. §3 requires ≥1 mark that works at zero and the figures to *be* the hero
  when there is no verdict. Sessions / volume / PRs all exist from session one and would show
  honest zeros (§12).

### 3.3 "What it can read" is a failure mode this codebase has already named twice

`SignalSlotRail` renders **11 uniform dot-text rows** (observed). `FAILURES.md` → *Checklist
section — "the AI look"*: more than ~4 uniform dot-text rows, fix is ONE mark plus a single focused
detail. `SETTLED.md` records the 9-row milestone ladder and Coach's pre-baseline dot-checklist being
removed for exactly this. The rail reintroduced the shape.

It also breaks two other rules:

- **Mark echo.** Sleep and Resting heart rate appear in this rail *and again* in "What it reads",
  eight rows below, on the same lens. §4.3: one home.
- It advertises **unbuilt features** (Protein, Hydration and supplements, Cycle) inside the coach's
  evidence lens, and shows *awaiting-data* rings for signals the engine does not read even when
  present — `COACH_ENGINE_PLAN_AUDIT.md` confirms moods, HRV and daily steps are advisor-unread.
  The rail promises reads that do not happen.

### 3.4 Two coaches

| | Coach page | Home |
|---|---|---|
| Engine | `AutoCoachPlanner.evaluate` (`CoachRepository.kt:205`) | `ProgressionAdvisor + DeloadAdvisor` (`AdaptationRepository.kt:258`) |
| Cadence | weekly pass | live off the snapshot |
| Surface | "The call", Apply / Skip / Undo | "COACH" section, Apply / Dismiss (`OverviewScreen.kt:590`) |
| Lifecycle | proposed → applied → watched → ok/failed → folded | muted for 14 days |

Both are labelled **COACH**. They have different vocabularies, different apply semantics, and no
awareness of each other. Home additionally carries a coach banner (`:317`), the Today directive
(`:422`) and a fatigue nudge (`:631`) that restates the Coach page's Recovery load score.

### 3.5 Two goal systems

- `CoachGoalRepository` / `GoalPortfolio` — the Coach page's Goals section and `GoalPickerDialog`.
- `GoalRepository` / `ExtendedGoalRepository` / `CustomGoal` — the Goals screen, `GoalEditorScreen`,
  Home's GOALS section, Cardio's goal rows.

They share no storage. A bench target set on the Goals screen is invisible to the coach; one added
in `GoalPickerDialog` never appears on the Goals screen. Two dialogs, two editors, one user
intention.

### 3.6 "Autopilot" means two different numbers on two lenses

Now → *Coming up* shows the closest-to-earning type: **0 OF 3**, sub "Rep-range shifts"
(`CoachWeekSection.kt:230`). Journey → *Earned autopilot* shows types earned: **0 of 4 earned**
(`CoachJourneySection.kt:92`). Same word, same page, two readings. Mark echo plus a numeric
contradiction.

### 3.7 Mechanical debt

`AUDIT.md` froze sixteen violations at the Coach v3 merge and says explicitly: *"Worth a pass when
Coach v3 is next touched."* This is that pass.

| Rule | Where |
|---|---|
| `font-size` at call site | `CoachBlockSection.kt` (8sp) · `CoachProjectSection.kt` (9sp) · `GoalPickerDialog.kt` (12sp) |
| `screen-name-title` | `GoalPickerDialog.kt` |
| `max-lines` on user content | `CoachGoalsSection.kt` (goal title) · `CoachSignalsSection.kt` (lift name, forming row) |
| `alpha` 0.4 | `GoalPickerDialog.kt` |
| `em-dash` in rendered strings | `domain/coach/GoalPortfolio.kt` |

Found in this pass, not in the audit:

- **`statsEntrance` indices collide and run out of order.** Now: Goals=2, Block=3, Project=4, then
  The call=2, Under watch=3, Coming up=4 — every slot used twice, so six sections share three
  stagger steps. Signals: lifts=2, recovery=3, **slots=5, inputs=4**, learned=5 — the cascade runs
  2,3,5,4,5 against a visual order of 1,2,3,4,5.
- **The row rhythm is not one value.** §7 requires ONE vertical padding for all of a lens's rows and
  names `COACH_ROW_PAD` = 6 as it. In practice: 6 (`CoachUi.kt:41`), 8 (learned biases), 12
  (`WatchedRow`, `TrustRow`), 14 (`CoachProgressRow`), 16 (`DecisionRow`).
- **Accent-as-text count.** Every `CoachAction` is accent body text at 2.35:1 (Navy). §14 forbids
  *adding* accent-coloured body text until `SETTLED.md`'s open decision resolves. The Now lens shows
  four on a first run (`Add a goal`, `Academy`, `Start a block`, `Start this`) and eight once two
  decisions and a running project are live.
- `CoachWeekSection.kt` contains `coachNowLens` — the file name predates the rename.
- `CoachViewModel` is 279 lines against the ~150 guide, and reaches for seven repositories.
- Stray file in the tree: `ui/coach/CoachGoalsSection.kt.tmp.58609.9efa00c14e7f`.
- `domain/coach/SessionAdaptor.kt` — dead: defined and tested, never called.

### 3.8 What is already good — keep it

Not everything needs replacing:

- **Under watch** is the best section in the feature: a change, its two-week window as a bar, its
  live verdict. It shows the reading and the conclusion together, which is exactly §4.9.
- **The call**'s evidence-under-the-decision pattern (sparkline for a lift, fatigue meter for a
  deload) is right and should become the page's spine.
- **Recovery load**'s check panel — six named checks each with its own reading, live ones first — is
  the model §4.9 asks for, and reads from session one.
- **The record**'s week rows, with the flag dot reserved for exceptions only.
- `CoachCharts.kt` as a mark kit. It is reusable and doctrine-clean; the rework needs almost no new
  drawing code.

---

## 4. The target

### 4.1 One sentence

**The Coach page is the decisions and the evidence behind them.** "What do I do today" stays on
Home; the Coach page never restates it. Every section either shows a decision, the reading that
drove it, or whether it worked.

### 4.2 Lenses — rename to the questions

| Today | Target | Answers |
|---|---|---|
| Now | **Now** | what the coach decided this week, and what I can do about it |
| Signals | **Why** | the readings behind the decision |
| Journey | **Record** | did the past decisions work |

One short word each, per §4.4. *(Rename is a call for Antho — §8.)*

### 4.3 Hero

```
COACH · WEEK OF JUL 20                     mono eyebrow, identity + human week
Two proposals                              serif, ONLY when it carries a decision/result (§3)
Volume held while sleep dipped.            italic aside — the coach's own read, or nothing
4 of 5    12.4k    2      0 of 4           EditorialFigure row — ALWAYS, honest zeros
SESSIONS  VOLUME   PRS    BASELINE
```

Changes from today: the figure row **renders from session one** rather than only after the baseline
completes, and the baseline countdown becomes its fourth figure instead of a separate bar that
replaces the whole hero. Pre-baseline there is no serif line and the figures are the hero — which is
what §3 already prescribes and what `SETTLED.md` recorded ("status states = eyebrow + figures").

### 4.4 Sections

Ordered live-first (§4.8). Each row states the mark, so no section can end up as prose.

**Now**

| # | Section | Mark (§2②) | At zero | Notes |
|---|---|---|---|---|
| 1 | **The call** | per decision: e1RM sparkline, or the fatigue meter for a deload | *the section still renders* — "No change this week" over a weeks-since-last-change bar | add the deciding reading as row meta ("flat 4 weeks · 0%"), keep reason as the italic aside. Apply / Skip stay |
| 2 | **Under watch** | two-week window bar per change, coloured by verdict | omitted only when nothing was ever applied | unchanged — this section is already right |
| 3 | **Working toward** | one meter per goal (value vs target), ETA as right meta | empty tracks + one `Add a goal →` | today's Goals section, given the mark §2② already specifies. Conflict line stays as the one caption |
| 4 | **Block** | 4-phase rail, live phase filled | unlit rail + `Start a block →` | **delete** the two-line explainer; the rail is the explanation |
| 5 | **Project** | finish-line meter (n of m weeks held) | omitted when there is neither a project nor a proposal | collapse `why`/`plan`/`finishLine` to ONE line; lead with the reading that made it the lever ("0 min cardio in 30 days") |
| 6 | **Coming up** | countdown bars | honest empty tracks | keep Next brief · Next verdict · Milestones. **Autopilot moves out** — Record owns trust |

**Why**

| # | Section | Mark | At zero | Notes |
|---|---|---|---|---|
| 1 | **Lifts on watch** | e1RM sparkline per lift + delta | forming lifts collapse to ONE row (already correct) | drop `maxLines = 1` from the lift name (§14) |
| 2 | **Recovery load** | segmented fatigue meter + the six-check panel | progress toward the real gate | unchanged |
| 3 | **What it reads** | each input's own chart under its row (sleep bars, HR line) | `ConnectPill` on the row | absorb the slot rail's one useful fact as a closing line: "Reading 6 of 11 signals · 3 waiting on your data" |
| 4 | **Learned so far** | — | the one allowed hint | **move to Record** if it stays mark-less; what it learned is part of the record |

**Cut: "What it can read"** (`CoachSlotRail.kt`). See §3.3. If the roadmap must be visible to
users, it belongs in the Academy or Settings, not on the coach's evidence lens.

**Record**

| # | Section | Mark | At zero | Notes |
|---|---|---|---|---|
| 1 | **The record** | week rows, exception-only flag dot; a held/total rail as the section mark | the one allowed hint | unchanged apart from the section mark |
| 2 | **Earned autopilot** | trust bar per change type | ONE line naming the per-type unlock | sole home for the word "autopilot" |
| 3 | **Learned so far** | — | — | moved here |

### 4.5 Actions

- `Apply` on an open decision is the page's do-it-now action. Per §8 it should be the **one filled
  capsule**, and `Apply all N` grouped at the END of the lens, not mid-scroll.
- `Skip` / `Undo` stay level ② weight.
- Navigation (`Add a goal →`, `Start a block →`, `Academy →`) is level ③. **Cap it at three per
  lens** and pull `Academy →` out of the Goals section — it is a peer feature, not a goal action.
  It belongs as the top bar's ≤1 action, or grouped at the end.
- Do not add accent-coloured body text beyond what exists until `SETTLED.md`'s contrast decision
  lands (§14).

---

## 5. Cross-app fixes this depends on

These are not optional polish; §4.3's "one home" is unenforceable without them.

1. **Name the two coaches differently, or merge them.** Home's `ProgressionAdvisor` feed and the
   Coach page's weekly decisions cannot both be "COACH". Cheapest honest split: Home's section
   becomes **TODAY** (live nudges), the Coach page keeps **the week's call**. The real fix is one
   decision model with two cadences, which is `COACH_V3_PLAN.md`'s job.
2. **Drop Home's fatigue nudge** (`OverviewScreen.kt:631`) or reduce it to a pointer that carries no
   score. Today it restates the Recovery load meter one swipe away.
3. **Unify the goal systems** (§3.5). One `Goal`, one editor. `GoalPortfolio` reads it;
   `GoalPickerDialog` is deleted in favour of `GoalEditorScreen`. Until this lands, the Coach page's
   Goals section is a second truth.

---

## 6. Build order

Each step is independently shippable and leaves the page working.

| Step | Work | Status |
|---|---|---|
| **1** | Hero figures from session one; baseline as the 4th figure | **done** — plus: the SESSIONS figure drops out pre-baseline, where it only echoed BASELINE |
| **2** | Reorder the Now lens live-first; fix the `statsEntrance` indices | **done** — one emitter, `CoachNowLens.kt`, indices 2…8 |
| **3** | Give Goals its meters; collapse Project to reading + finish-line meter; strip Block's explainer | **done** — new shared `CoachMeter`; the phase rail now fills up to the live phase |
| **4** | Cut `CoachSlotRail`; fold its one fact into "What it reads" | **done** — file deleted, coverage line added |
| **5** | Move Autopilot to Record; move Learned so far to Record | **done** — plus `Your numbers`, which was hanging off the Project section |
| **6** | `The call` zero state; Apply as the filled capsule, `Apply all` at the end | **done** — the zero state renders only while learning; a settled quiet week stays with the hero (§4.3) |
| **7** | Mechanical debt sweep (§3.7) + rename `CoachWeekSection.kt` → `CoachNowLens.kt`, delete the stray `.tmp` file and `SessionAdaptor.kt` | **done** — 12 buckets paid down, 936 → 924 |
| **8** | Lens rename, if Antho approves (§8) | open |
| **9** | Cross-app fixes (§5) | open — the larger piece |

**Not done, and worth naming:** archiving a goal is still unreachable. `CoachViewModel.archiveGoal`
exists and nothing calls it; the old section passed it a parameter it never used. Adding a
destructive per-row action needs an undo story (§12), and the goal model is due to merge with the
Goals screen anyway (§5.3), so the editor there is the right home for it rather than a new pill on
the coach's row.

Steps 1–2 are the two-shot rule (§4.7): show Antho the reworked overview before touching the
sub-lenses.

---

## 7. Verification

```
gradle -p forge-android :app:testDebugUnitTest
```

Runs `DesignDoctrineTest`, `DoctrineParityTest`, `DoctrineSelfCheckTest`, `RecipeScreenshotTest`.
A rework *removes* violations, so the gate will fail with "debt was PAID DOWN here" — bank it with:

```
gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' \
  -Dforge.paydown=true --rerun-tasks
```

Never `-Dforge.regen=true` for this work; that accepts new violations too.

Then, per §15:

- [ ] Every section drawn at all seven states (§12) — check the **all-zero state of the section**,
      not of each row (`FAILURES.md`, *Empty by omission*).
- [ ] 100% **and** 200% font scale on every touched screen.
- [ ] Every Canvas mark carries a value-reading `contentDescription` (§14).
- [ ] Empty marks measured, not eyeballed — `FAILURES.md`, *Invisible ghost*: the bar-track rung
      measures ~1.08:1 alone; a track that IS the mark takes `muted @0.55` / `@0.30`.
- [ ] `design/MAP.md` updated; removals recorded in `SETTLED.md` the same turn (§16).

**Also worth doing:** add a Coach **first-run golden**. `AUDIT.md` defers real-screen goldens and
names the exact gap this page fell into. The Coach first-run is the highest-value candidate, since
it is both the worst state today and the one nobody re-opens.

**And wire the demo-data hook.** `OverviewViewModel.loadSampleData` has no call site, so there is no
way to see a populated Coach on a device without logging eight weeks by hand. One `action →` on the
zero-session welcome card makes every populated state reviewable.

---

## 8. Decisions for Antho

1. **Lens names** — keep `Now / Signals / Journey`, or move to `Now / Why / Record`? The second pair
   is the user's vocabulary; the first is shipped and familiar.
2. **Does the Today directive stay on Home only?** This plan says yes and makes the Coach page the
   decisions-and-evidence page. The alternative — Coach owns Today and Home points at it — is a
   bigger change to Home's hero.
3. **Cut "What it can read" entirely, or relocate it?** It is the product's visible roadmap. Cutting
   it from the coach lens is right by doctrine; whether the roadmap deserves a home elsewhere is a
   product call.
4. **Goal unification (§5.3)** — worth doing now, or after `COACH_V3_PLAN.md` Phase A2 settles the
   goal model? Doing it twice would be wasteful.
5. **Contrast** — up to eight accent `action →` links on one lens is the most concentrated instance
   of `SETTLED.md`'s open decision anywhere in the app. Good place to trial option (b): onBg text
   with the `→` glyph alone in accent.
