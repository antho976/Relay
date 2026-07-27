# Coach v3 + Engine — Plan Audit (rev-2 verification pass)

> Audit of `COACH_V3_PLAN.md` (rev 2), `ENGINE_PLAN.md` (rev 2), `ACADEMY_LESSONS.md`, and
> `ROADMAP.md`, verified line-by-line against the source at **0.8.8.3 / Room schema v31**
> (`app/build.gradle.kts:39-40`, `ForgeDatabase.kt:131`), on branch `relay/term-1` —
> i.e. *after* the "Plan updates" commit that produced rev 2. Every claim below was
> checked against code; file:line references are the evidence.

> **Status: applied (2026-07-24).** All 38 findings are folded into
> `COACH_V3_PLAN.md` rev 3, `ENGINE_PLAN.md` rev 3, `ACADEMY_LESSONS.md` rev 3 and
> `ROADMAP.md` rev 3. `COACH_ENGINE_PLAN_REVIEW.md` was deleted (its verified-claims table
> lives on as the appendix of the Coach plan) and the stale `WorkoutRepository.kt.tmp…`
> file was removed from the tree. Two findings were resolved differently from the wording
> below — see "Resolutions worth noting" at the end. This document is now the audit trail,
> not a to-do list.

**Verdict.** The rev-2 diagnosis of v2 still holds — re-verified: moods, `toFailure`,
`setType`, `difficultyTag`, `health.hrv` and `health.dailySteps` are all still
advisor-unread (zero hits across `domain/adapt` + `domain/coach`); bodyweight is still
absent from `AdaptationSnapshot`; `VolumeModel.weeklyCap` is still a hardcoded map
(`VolumeModel.kt:27-39`); `ExerciseBout` still carries no `sessionType`
(`AdaptationSnapshot.kt:111-118`); there is still no age/DOB anywhere (only `USER_SEX`,
`PreferencesDataStore.kt:201`). What follows is what is wrong with the plans *now*:
five factual errors, six places where the plans duplicate or ignore shipped machinery,
ten underspecified mechanics, six cross-document inconsistencies, six structural fixes,
and five deletions.

Highest-value items: **#6** (goal migration breaks trophies), **#10** (freestyle mode has
no directive story), **#13/#14** (the verdict and ledger schema changes the plans promise
but never specify), **#17** (no lesson renderer exists anywhere in the app).

---

## 1. Factual errors — fix before building

**1. `SessionBreak` is not a layoff signal.**
It is a timestamped water/rest/snack break *inside* a session, FK'd CASCADE to `Session`
(`data/db/entities/SessionBreak.kt`, #139). Both `COACH_V3_PLAN.md` (Life events concept +
Phase B) and the old review §4a list it as a layoff input. Layoff detection has exactly two
real inputs: `VacationPeriod` (user-declared date ranges) and raw session-gap detection.

**2. "No sick flag anywhere" is false — and the existing one is already consumed.**
`CardioRestReason.SICK` is user-writable today (`domain/cardio/CardioEffort.kt`) and read by
`ReadinessAdvisor.kt:79` (−4%) and `DeloadAdvisor.kt:184`. Same story for `SORE`
(`ReadinessAdvisor.kt:80`, `DeloadAdvisor.kt:185`). The Phase-B sick flag must subsume or
explicitly displace this path, or the product ships two sick concepts that double-penalize
readiness. Same reconciliation needed between the existing generic soreness signal and
ReadinessV2's per-muscle soreness gates.

**3. "The coach ignores vacation" is false.**
`CoachRepository.kt:195` already holds the *entire weekly pass* with an explained hold while
on vacation ("On vacation — the coach is paused until you're back."). Life events must
extend that gate, and the plan must state how "suppress stall/watcher verdicts across the
gap" differs from the existing pass hold.

**4. Widget deep links are not stubbed.**
`EXTRA_START_DAY_KEY` is read at `MainActivity.kt:125` and `:302`, and threaded into
`ForgeNavHost(initialDayKey = …)` at `:420`. Only `EXTRA_RESUME_SESSION`
(`widget/ForgeWidget.kt:47`) is set-but-unread. Phase B's "wire the widget deep links" is a
one-line job, not a task.

**5. "Session type has no writer" is half wrong.**
`DayUiEvent.SetSessionType` (`state/DayUiEvent.kt:85`) → `DaySessionHandlers.kt:34` →
`WorkoutRepository.setSessionType:415` → `SessionDao.setSessionType:126` all exist. The only
missing piece is a UI control that emits the event. `ExerciseBout.sessionType` is genuinely
absent. Phase A's prerequisite is smaller than the plan assumes.

*(Rev 2's own correction — no birthday/age in the app — re-verified true.)*

---

## 2. Shipped machinery the plans duplicate or don't know about

**6. The goal system is far richer than "ExerciseGoal/ExtendedGoal".**
`ExtendedGoal` already supports `1rm | weekly_volume | frequency | monthly_prs`, with
`stretch_value` **and `completed_at`** (so "the spec covers creation but not completion" is
wrong — lifecycle state exists). `domain/goal/CustomGoal.kt` adds `GoalMetric`
(cardio_distance, cardio_minutes, sessions, volume, bodyweight) × `GoalPeriod`
(week/month/all), encoded into `extended_goal.goal_type`. Two consequences:
- the GoalPortfolio catalogue overlaps ~70% with shipped code, so Phase A is partly a
  *promotion*, not a greenfield build;
- goals are read by `domain/trophy/TrophyEvaluator.kt`, `program/Trophies.kt`, `ui/goals/`
  and `ui/cardio/` — **migrating goals into `CoachGoal` breaks trophies** unless planned.

Add a "goal migration + downstream consumers" section to Phase A.

**7. Engine's Health Floor duplicates shipped surfaces.**
`WHO_WEEKLY_ACTIVITY_MIN = 150` (`domain/cardio/CardioGuidelines.kt:9`), the WHO meter
(`ui/cardio/CardioComponents.kt:146`), and the user's own `CARDIO_WEEKLY_TARGET_MIN` pref
(`PreferencesDataStore.kt:252`) already exist. Health Floor must adopt/migrate these, never
add a third competing weekly-minutes target.

**8. Interference will double-count.**
`ReadinessAdvisor.kt:84-86` already shaves a point for non-rest cardio in the last 24 h.
Engine's `ConditioningLoad` term must *replace* that block. Say so where the plans declare
the "one interference formula in the product".

**9. "Signals" is already a Coach Lab concept.**
`ui/coach/CoachSignalsSection.kt` renders a Signals lens over `RecoverySignal`. The new
`CoachSignal` / SignalRegistry needs a different name, or an explicit statement that it
renders inside that existing lens.

**10. Freestyle mode is invisible to both plans (zero mentions in any doc).**
`FREESTYLE_MODE` exists (`PreferencesDataStore.kt:204`) and the coach **bails before running
any pass** (`CoachRepository.kt:745`, "Nothing to coach without a plan"). Decision Zero and
the Today Directive are therefore undefined for the exact cohort with no program — today
they get nothing. Every new surface (directive, brief, Academy) needs a declared freestyle
behavior. This is the largest unaddressed product hole after life events.

**11. Coach-off mode is a precedent worth citing.**
`CoachRepository.kt:185-191` writes inert `STATUS_SHADOW` rows when the coach is off, ignored
by TrustLedger / CoachGenBias / the watcher / LIFO-undo guards. New decision types should
follow that pattern instead of inventing a second "disabled" path.

---

## 3. Underspecified mechanics that will bite mid-build

**12. `reason.lessonId` describes a type that doesn't exist.**
`reason` is a plain `String` on `Recommendation` (`Recommendation.kt:25`, 8 implementors) and
a Room column on `CoachDecision` (`CoachPass.kt:45`). Pick one now: a nullable `lesson_id`
sibling column + field (cheap, one migration), or a `Reason` value-type refactor (touches
every advisor and every UI consumer).

**13. Three-valued verdicts have no storage plan.**
`coach_decision.outcome` is `"pending" | "ok" | "failed"` (`CoachPass.kt:53`);
`OutcomeWatcher` only ever emits ok/failed (`:64`, `:100`, `:104`); `CoachGenBias.from`
filters on `outcome != "failed"` (`CoachGenBias.kt:59`); `TrustLedger.assess` counts the same
values. "Not followed" means a new outcome value **plus** coordinated edits in all four
places **plus** a migration. Neither plan mentions any of it.

**14. The ledger is week-keyed; the new cadences are not.**
`CoachPass` is PK'd by `week_id` and every pass writes a row (`CoachPass.kt:19-25`).
PostSessionDebrief, TodayDirective and SessionAdaptor acts are session/daily writes with no
home in that schema — yet "no unwatched writes, ever" applies to them. Decide: a
`scope`/cadence column plus non-week keys on `CoachDecision`, or a sibling ledger.

**15. Structural undo has no data model.**
`undo_data` is a single before-state blob (`CoachPass.kt:56`). Undo-window expiry and
"revert forward" need at least an expiry column and a regeneration path. Currently one
sentence of prose.

**16. `NextSessionAdjustments` is described as persisted but missing from the entity list**
in Critical files (which names TrainingBlock / CheckinEntry / CoachGoal / CoachProject /
Lesson / LessonEvent).

**17. Academy content has no renderer.**
There is no markdown library and no markdown code anywhere in the app (zero hits across
`app/` and `gradle/libs.versions.toml`). "Structured markdown/asset files" is undefined work
sitting on Phase B's critical path: either add an offline renderer dependency or define a
small block DSL (heading / paragraph / bullet / callout) plus a Compose renderer. Name the
choice in the plan — 13 lessons ship against it.

**18. Per-muscle soreness needs a named taxonomy.**
State that the check-in muscle picker uses `MuscleGroup`, and derive the "last 48 h trained
muscles" candidate set from `exerciseHistory` + the library muscle map. Otherwise it is a UI
promise with no source.

**19. The directive never says what it replaces.**
Overview already leads with next-workout / coach content (`ui/overview/OverviewScreen.kt`).
Without an explicit displacement rule, Phase B ships a third card saying a similar thing.

**20. Engine E-A defers "persist enrichment?" to build time.**
Decide now. Recommendation: don't persist — `ConditioningLoad` is pure and cheap to
recompute, which keeps E-A migration-free exactly as the phase hopes.

**21. Sequence-mode directive degradation is declared but never written.**
Define the actual output for the common case: spacing says train, next-up is a leg day one
day after legs, weekday layout unknown.

---

## 4. Cross-document inconsistencies

**22.** `ACADEMY_LESSONS.md` C5 still teaches **"any failure demotes"** — contradicts rev 2's
rate-based demotion with hysteresis.

**23.** `COACH_V3_PLAN.md` lists **4 Academy tracks**; `ACADEMY_LESSONS.md` has **5** (adds
"The Engine").

**24.** Coach Phase B says **"≈10 lessons"**; the Academy Phase B batch is **13**
(F1–F10 + C1–C3).

**25.** **C3 unlocks in Phase A but is authored in Phase B.**
`coach.strength_on_a_cut` fires on WeightPhase=CUT stall suppression, which Phase A ships —
violating the plan's own "every shipped coach concept has a lesson" audit rule. Either move
the behavior to B or author C3 in A.

**26.** **F6's unlock trigger is still wrong**, now for a new reason: per finding #2 a
sick-flag moment can already fire today via the rest-reason path, before Phase B exists.

**27.** **Engine E-B's "minimal standalone goal row" hedge is dead by construction** —
`ROADMAP.md` orders Coach A/B before Engine E-A/E-B. Delete it, or mark it contingency-only.

---

## 5. Structure and process

**28. Phase B is three phases in a trench coat.**
CheckinEntry + sheet, life events (three mechanisms), a full ReadinessV2 rebuild,
TodayDirective, PreSessionBrief, surface wiring, 13 lessons *and* a lesson renderer. Split:
- **B1** — check-in + ReadinessV2 + life events (data + brain)
- **B2** — Today Directive + PreSessionBrief + surfaces
- **B3** — Academy foundation + renderer

Each is independently shippable, which is the plan's own stated bar.

**29. Phase A likewise.** Split **A1** (snapshot/data: bodyweight series, moods consumed,
failure tags, session-type picker + `ExerciseBout.sessionType`) from **A2** (GoalPortfolio +
SignalRegistry + AcademyRegistry skeletons).

**30. No sizing and no per-phase "done when"** beyond the test list. Add one acceptance line
per phase, and a kill criterion — what result makes you cut a concept rather than iterate it.

**31. Name the schema version per phase.** Six-plus new entities across A–D with no declared
version targets invites mid-build churn (v31 today).

**32. Add a coach-off / freestyle audit per phase**, mirroring Engine's rung-1 sensor-ladder
audit — same discipline, different axis (see #10, #11).

**33. Verification should include the ZIP restore path**, not just `exportFullDataJson`
(`BackupRepository.kt:149` is hand-rolled JSON and already omits coach tables — confirmed).

---

## 6. Delete

**34. `docs/COACH_ENGINE_PLAN_REVIEW.md`** — fully folded into rev 2, and now actively
misleading on three points (#1, #2, #4). Keep only its §1 verified-claims table as an
appendix in `COACH_V3_PLAN.md`; delete the rest.

**35. The `(Rev 2:)` / `(rev-2 correction:)` running commentary in both plans.**
Rev 2 *is* the baseline now. Flatten to clean prose and keep the changelog in the Status
block only — roughly 40 inline annotations across the two docs, and the main reason they're
hard to read.

**36. Engine's retelling of rev 1's `SessionHrSample` error.** Keep the corrected design (a
sibling `cardio_hr_sample` table; the shipped PK is `(session_id, at_ms)` with a NOT-NULL
CASCADE FK — verified in `data/db/entities/SessionHrSample.kt`), drop the history.

**37. Engine's `outdoor_watch_cardio` future slot** duplicates the same slot in
`WEAR_OS_PLAN.md`. One owner.

**38. Repo hygiene (not a doc issue, but it is committed):**
`forge-android/app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt.tmp.18109.6a0b4651f58a`
is a tracked stale duplicate of `WorkoutRepository.kt` (700 lines vs 709, missing the wear-HR
DI). It pollutes greps — it surfaced in this audit's `SessionBreak` search. Delete it.

---

## Resolutions worth noting

Where rev 3 resolved a finding differently from, or beyond, the wording above:

- **#6 (goal migration)** resolved as **additive, not a migration** — plan Mechanics M1.
  `CoachGoal` ships alongside `ExerciseGoal`/`ExtendedGoal`; the portfolio reads them and
  offers a one-tap "manage as a coach goal" promotion. Nothing is rewritten or dropped, so
  `goal_crusher` / `goals_5` cannot regress. Retiring the old editors becomes a separate,
  explicitly scoped decision instead of a side effect.
- **#17 (lesson renderer)** resolved as **no new dependency** — plan M5. Lessons are
  structured blocks (`Heading | Paragraph | Bullets | Callout | Example`) rendered by one
  Compose component, chosen over a markdown parser because 33 short lessons don't justify
  one and `Example` blocks need to interpolate the user's own numbers.
- **#30 (sizing)** partially applied: every phase gained a "done when" acceptance line, but
  **no time estimates** — solo-dev velocity here is unmeasured and invented numbers would
  be worse than none. Explicit abandonment criteria were also left out; "done when" defines
  success, not the trigger to cut a concept. Both remain open if you want them.
- **#25 (C3 phase mismatch)** resolved by moving the lesson to A2 **and** by defining when
  the 1:1 audit is enforced — at the end of each release series (v3.0 = A1+A2,
  v3.1 = B1+B2+B3), so an intermediate phase may ship a concept a phase ahead of its lesson
  without the rule becoming unenforceable.
- **#34 (delete the review doc)** done; its §1 verified-claims table survives as the
  appendix of `COACH_V3_PLAN.md`, re-verified at rev 3 and extended with the claims this
  audit checked.
