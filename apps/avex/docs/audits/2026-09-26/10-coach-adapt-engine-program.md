# Coach, adaptation engine, conditioning engine, program generation

Scope: `domain/coach/**`, `domain/adapt/**`, `domain/engine/**`, `program/**`. 51 files, ~10.6k lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/` unless noted.

**Counts:** P1 2 · P2 7 · P3 18 · P4 7. Four are still open from `docs/audits/2026-09-12/domain.md` (D1, D2, D4, D5).

**Reproductions** (run in the session scratchpad, not committed): `kg_progress.py` (kg progression math); the pure `program` package compiled with kotlin-compiler-embeddable 2.0.21 and three harnesses (`PinHarness.kt`, `EdgeHarness.kt`, `CapHarness.kt`); `WeekIdLocale.java` (week-id formatting per locale); `grid_fp2.py` (floating-point error in grid rounding).

## P1

### [P1] Every progression target for kg users moves on a 2.5 lb grid, so the "next weight" is +0.1–0.9 kg and never a loadable weight
- Category: bug (wrong prescription)
- Location: `domain/adapt/ProgressionAdvisor.kt:242-243, 293, 464, 484`; `domain/adapt/AdaptThresholds.kt:14`; displayed via `ui/gym/train/DayViewModelBuilders.kt:190-192`
- Problem: `progressSuggestion` and `backOffSuggestion` compute `floorToGrid((prevMax ± dumbbellStepLb) * scale, 2.5)` in stored pounds whatever unit the user chose; the chip then converts to kg. A kg user logs 60 kg × 10 at the top of an 8-10 range (stored 132.3 lb): target = floor(134.8 / 2.5) × 2.5 = 132.5 lb → chip "60.1 kg", reason "hit top of range"; back-off shows 57.8 kg. 20 kg → 20.4 / 18.1; 40 → 40.8 / 38.6; 100 → 100.9 / 98.7; 140 → 140.6 / 138.3. The plateau ladder's micro-load and reset (`:464`, `:484`) use the same grid. Double progression never adds real load for kg users. `PreSessionBrief.kt:118-123` already takes its step from the user's unit, so the two surfaces disagree. *Re-checked by the coordinator.*
- Fix: pass the user's `WeightUnit` (or a step already converted to lb) into `suggestNextLoad`/`evaluate`, and use `WeightSteps.weightStep(unit, false) × lbPerUnit` as both grid and increment (2.5 kg for kg users, 5 lb or 2.5 lb dumbbells for lb, 0.5 st for stones). Round in the display unit and convert back, as `PreSessionBrief.target` does. Test: 60 kg at top of range → 62.5 kg.
- Confidence: high

### [P1] Weight chip keeps full weight on low-readiness days and in the deload week whenever the rep range isn't filled — still open from domain.md D1
- Category: bug (wrong prescription)
- Location: `domain/adapt/ProgressionAdvisor.kt:161-166` (consolidate), `175-180` (keep-weight)
- Problem: `scale` (readiness, intensity pick, block phase) is computed at `:119-143`, but both same-weight branches return `prevMax` unscaled and drop `scaleNote`. 100 lb × 8 in an 8-12 range with readiness −5 → "keep this weight 100"; with `phase = DELOAD` (scale 0.85) still 100 lb. This is the most common branch: every set below the top of its range lands here. *Re-checked by the coordinator.*
- Fix: work out the base weight per branch, then apply `scale` once with `floorToGrid(prevMax * scale, step)` (unit-aware step from the finding above); keep the reason and append `scaleNote`. For PLATES, round down to a whole plate or stay silent when scale < 1.
- Confidence: high

## P2

### [P2] A pinned isolation exercise can be placed twice in the same day
- Category: bug
- Location: `program/ProgramGenerator.kt:216` (`placePins`), `222`, `240-252`
- Problem: `placePins` reserves an isolation pin for the muscle's last PUMP/HYP slot, but earlier slots draw from `candidates.filterNot { it.id in usedInDay }`, which doesn't exclude reserved pins; `weightedPick` floors weights at 0.0001, so an earlier slot can pick it and the pin places it again. Measured (2000 seeds/config): 5-day dumbbells + bench with pinned `db-hammer-curl` → 741 of 10,000 days (~37% of weeks), e.g. seed 0 pull day `[db-row, db-rear-delt-fly, db-pullover, db-hammer-curl, db-hammer-curl]`; full gym with pinned `db-lateral-raise` → 96 of 10,000. Both slots share one `plan.id`, so `SetLogUseCase.loggedFor` (`domain/session/SetLogUseCase.kt:85-86`) maps both to one logged row and (day, exercise) overrides collide; the generator's own comment (`:219-221`) says this "breaks per-exercise logging".
- Fix: `val reserved = pinnedFor.values.map { it.id }.toSet()` before the slot loop; non-pinned slots use `filterNot { it.id in usedInDay || it.id in reserved }`. Add a quality test: no duplicate `libId` in any day across seeds with pins.
- Confidence: high

### [P2] A rep-range shift is computed from one day's slot and applied to another when the lift is on two days
- Category: bug
- Location: `domain/coach/AutoCoachPlanner.kt:179-180, 196-204`; `domain/adapt/ProgressionAdvisor.kt:338-340, 440-450`
- Problem: `evaluate` dedupes by exercise id keeping the *first* slot (so `fromReps`/`toReps` come from it); `AutoCoachPlanner.slotByExercise` uses `associateBy`, keeping the *last* day, which is where the override is written. Seed program: Goblet Squat is lower-a 4 × 10-12 and lower-b 3 × 12-15. On a stall the Brief says "Shift Goblet Squat from 10-12 to 8-10" and the change lands on lower-b, so the high-rep day becomes 8-10 while lower-a stays 10-12 — heavy and light days swap, and the summary misstates the change.
- Fix: emit one `RepRangeShift` per (dayKey, slot), or carry `dayKey` on the recommendation and resolve from the same slot; minimum, `firstOrNull` in both places.
- Confidence: high

### [P2] Rep-shift verdict compares against the lift's lifetime best, so a shift that restarts progress can be judged "failed"
- Category: bug
- Location: `domain/coach/OutcomeWatcher.kt:118-129`
- Problem: `priorBest` reads the whole history (the snapshot carries all of it, `data/repo/AdaptationRepository.kt:129-131`) and isn't split by `performedExerciseId`. e1RM peaked at 250 nine months ago and has stalled at 230 (why the shift was proposed); after the shift the best is 232 < 250 × 0.97 → "failed", a revert proposal, and a broken TrustLedger streak though strength went up.
- Fix: baseline = best of the last `plateauMinBouts` non-skipped bouts before `appliedAt`, same `performedExerciseId` as the post-change bouts.
- Confidence: high

### [P2] Deload "strength regression" check compares two different lifts across a swap
- Category: bug
- Location: `domain/adapt/DeloadAdvisor.kt:184-191`
- Problem: history is keyed by slot (`SnapshotAssembler.kt:61`); a swap files the new lift under the old slot. Barbell row (prior best 160) → DB row (62), bench → DB bench counts as two regressions (+2 fatigue); with overdue (+1) and plateau (+1) a deload call is close. The H-06 rule was already applied in `VolumeResponse` and `sinceLastSwap`.
- Fix: group each slot's bouts by `performedExerciseId ?: slotId` and compare within the same performed lift.
- Confidence: high

### [P2] Training blocks never advance for locales that format digits outside ASCII (ar-EG, fa-IR, bn-BD, mr-IN, hi-IN-u-nu-deva)
- Category: bug
- Location: `domain/coach/BlockPlanner.kt:118-133` (parsing); ids from `data/repo/CoachRepository.kt:1020-1021`
- Problem: week ids use `"%d-W%02d".format(...)` in the default locale, producing Arabic-Indic digits under ar-EG; `BlockPlanner.weekStart` matches `(\d{4})-W(\d{2})` (ASCII only), returns null, `weeksBetween` returns 0, and `advance` returns the block unchanged every week — stuck at "Week 1 of 5", deload never arrives. `RestAdvisor.mmss` (`:225`) formats display text the same way.
- Fix: format week ids with `Locale.ROOT` and migrate/re-parse stored ids; better, have `advance` take `LocalDate`s.
- Confidence: medium (verified on the JVM, not a device)

### [P2] Imported weighted timed-only history crashes the plateau ladder — still open from domain.md D5
- Category: bug (crash)
- Location: `domain/adapt/ProgressionAdvisor.kt:348, 358, 399-401, 507`
- Problem: the bout filter admits any weighted non-assisted set; `bestWorkingE1rm` excludes `durationSeconds` sets and `bestE1rm` then `!!`s the null → NPE in `evaluate`. `cutSuppressedStalls` has the same crash, also skips `sinceLastSwap`, and is called unguarded from `data/repo/AcademyRepository.kt:98`.
- Fix: filter bouts with `b.sets.any { it.isWorkingStrengthSet() }` (or `mapNotNull` per-bout e1RM); remove the `!!`.
- Confidence: high

### [P2] A swap is still judged "ok" with no logged set and without checking the requested replacement — still open from domain.md D2
- Category: bug
- Location: `domain/coach/OutcomeWatcher.kt:85-103`
- Problem: M-08 only handles a window with no non-skipped bout. A note-only bout with zero sets, or a bout on a different movement (`performedExerciseId != d.payload`), still reaches `windowClosed -> ok` and earns auto-apply trust.
- Fix: "ok" requires `boutsSince.any { !it.skipped && it.performedExerciseId == d.payload && it.sets.any { s -> s.isRepSet() } }`, else NOT_FOLLOWED.
- Confidence: high

## P3

### [P3] Beginners, and an "Observing"-tier coach, get two changes a week; the one-change beginner cap never applies
- Location: `domain/coach/AutoCoachPlanner.kt:244`; `domain/coach/TrustLadder.kt:85-91`; `data/repo/CoachRepository.kt:476-481`
- Problem: CoachRepository always passes `TrustLadder.assess(...).changesPerWeek` (null only if `assess` throws); OBSERVE and PROPOSE return 2, so `?: if (experience == "beginner") 1 else 2` never runs. Breaks "Beginners get at most ONE change a week (hardening 4)" (`:96-97`). OBSERVE says "It won't suggest anything yet" but proposes two.
- Fix: `cap = minOf(inputs.changesPerWeek ?: Int.MAX_VALUE, if (beginner) 1 else 2)` for tiers ≤ AUTO_APPLY; decide whether OBSERVE allows 0 or 1.

### [P3] Rating a set "easy" lengthens a short rest
- Location: `domain/adapt/RestAdvisor.kt:173-175`
- Problem: `(seconds - 30).coerceAtLeast(60)` with isolation bases allowed 30–600 s (`SettingsRepository.kt:722`): a 45 s base + EASY → 60 s while the reason says "−30s after an easy set". Tuned bases below 90 s show the same "−30s" with no change.
- Fix: `maxOf(seconds - 30, minOf(seconds, 60))`; only add the reason when it changed.

### [P3] Rest tuning learns against a per-plan base while the timer prescribes a per-set base
- Location: `domain/adapt/RestAdvisor.kt:104` vs `:144-147`
- Problem: `samples()` calls `SessionEstimate.restSeconds(plan, …)` without performed reps, so a 4-6 plan always gets the +60 s heavy base (180 s), while `restSeconds()` drops it for >6-rep sets (timer 120 s); `RestEvent` stores no reps. A get_stronger user doing 8 reps on a 4-6 slot and resting ~120 s reads 0.67; after 8 samples every compound rest is cut by a third (5-rep heavy set → 120 s; 8-12 compound → 75 s).
- Fix: store the prescribed base (or reps) on `RestEvent`, or compute the sample base from `plannedSeconds` before tuning.
- Confidence: medium

### [P3] Deload rep drop-off check treats a warm-up set as the "first set"
- Location: `domain/adapt/DeloadAdvisor.kt:162-171`; same pattern `E1rm.kt:60` (`isRepSet`), `VolumeResponse.kt:84`, `domain/coach/GoalPortfolio.kt:174`, `ProjectScanner.kt:182`
- Problem: `isRepSet()` keeps `setType == "warmup"` rows (written by freestyle, `ui/gym/freestyle/FreestyleLogParts.kt:693`). A 12-rep warm-up then 5, 5, 5 reads as a 58% fade; six such bouts fire "+1 reps dropping ~58% within sessions". EffortModel says warm-ups carry no working-effort information.
- Fix: `fun LoggedSet.isWorkingRepSet() = isRepSet() && setType != EffortModel.SET_TYPE_WARMUP`; use it for drop-off and per-muscle set counts.

### [P3] InsightEngine strength reads skip the shared working-set / test-day / swap rules — still open (domain.md cleanup)
- Location: `domain/adapt/InsightEngine.kt:58-64, 123-137, 317-324`
- Problem: `boutE1rms`/`sweetSpotRepRange` filter only `weightLb != null && !isAssisted`, keeping holds, warm-ups, TEST/TECHNIQUE/FIRST_BACK bouts and swapped-in lifts. 2 warm-ups × 10 @ 95 + 3 × 5 @ 185 over 5 sessions → "strongest at 1-5 reps" (the 6-10 bucket is only warm-ups). A DB row slot at 60 lb session-swapped to barbell row at 135 reports "DB Row (1-arm) is up ~125% in 3 months".
- Fix: `bestWorkingE1rm`, exclude warm-ups, `countsForProgression`, key by `performedExerciseId ?: slotId`.

### [P3] "Estimate vs reality" uses a different estimate than the day card
- Location: `domain/adapt/InsightEngine.kt:270` vs `ui/gym/train/DayListViewModel.kt:139-142`
- Problem: the card uses `RestAdvisor.estimateMinutes(plan, tuning, …)`; the insight uses `SessionEstimate.estimateMinutes` with default rests and no tuning. With 180/120 s rests on a 6 × 3 day the card says ~50 min and the insight "runs ~55 min (estimated ~40)".
- Fix: carry rest settings/tuning into the snapshot; use `RestAdvisor.estimateMinutes`.

### [P3] Personal volume caps learned on one split cut a new split's slots to 1 or 0 sets
- Location: `domain/coach/PersonalProfile.kt:113-122`; `program/VolumeModel.kt:133-144`
- Problem: PersonalProfile floors the cap at *current* `slotCount × MIN_SETS` and claims VolumeModel "refuses to trim past that floor", but VolumeModel uses floor 0 for personal caps (asserted in `CoachingPolicyTest.kt:82`). A biceps cap of 5 learned on 3 days → on 6 days `CapHarness.kt` gives `pull-a/BICEPS/PUMP=0`, `pull-b/BICEPS/PUMP=0` (pull days 6 → 5 exercises); 5 days → 1-set slots; the Coach screen still says "Biceps: up to 5 sets a week".
- Fix: express personal caps per slot (a factor on the default), or recompute the floor against the template being generated; never go below 1 set unless dropping deliberately; fix the comment.
- Confidence: medium

### [P3] Bodyweight compounds with numeric default reps get strength reps ("Bodyweight Squat 4×4-6")
- Location: `program/ProgramGenerator.kt:369-375`
- Problem: `repsFor` replaces any numeric default with the scheme range; bodyweight-only + get_stronger gives `bw-squat x4 4-6`, `bw-good-morning x4 4-6` (defaults 15-20).
- Fix: keep `defaultReps` for `ExerciseUnit.BODYWEIGHT`, or never go below the movement's own minimum.

### [P3] A rep-range shift loses per-side notation
- Location: `domain/adapt/ProgressionAdvisor.kt:441-442`
- Problem: `RepRange.parse("8-10/leg")` → 8..10, shifted to "12-15" with no "/leg"; per-leg reps become ambiguous totals.
- Fix: carry the suffix onto `toReps`.

### [P3] `cutSuppressedStalls` duplicates the stall loop and has drifted from `evaluate`
- Category: bad-code
- Location: `domain/adapt/ProgressionAdvisor.kt:389-416` vs `:338-368`
- Problem: omits `sinceLastSwap`, so after a swap it reports a stall `evaluate` doesn't count, and the Academy unlocks `coach.strength_on_a_cut`/protein lessons for a lift the coach doesn't consider stalled; also carries the D5 crash.
- Fix: extract `stallRead(slot, s, t)` and call it from both.

### [P3] Readiness "weight swinging" input is never wired up
- Category: dead-code
- Location: `domain/adapt/ReadinessAdvisor.kt:59, 229-233, 254-276`
- Problem: `bodyweightFlux` exists only on `assess`; the only production caller (`AdaptationRepository.kt:260`) calls `evaluate`, which never forwards it; the class doc lists it as live. `soreMuscles` and `lifeEvents` outputs of `assess` are discarded by every caller.
- Fix: compute and pass flux, or delete the parameter, rule and doc claim.

### [P3] "Cardio load yesterday" is a rolling 24-hour window
- Location: `domain/engine/ConditioningLoad.kt:78-85`; `domain/adapt/ReadinessAdvisor.kt:215-219`
- Problem: `entries.filter { it.date >= nowMs - DAY_MS }` on start times: a 90-min zone-3 run at 07:00 yesterday is missed at 18:00 today; a 07:00 run today is deducted as "yesterday". ReadinessAdvisor already fixed this for gym sessions (`:110-120`).
- Fix: take a `zoneId` and sum entries whose local date is `today - 1`, as `yesterdaySteps` does.

### [P3] Most Coach v3 output is computed but never shown
- Category: dead-code / release
- Location: `domain/coach/SessionAdaptor.kt` (no production reference); `CoachSignal.kt` `SignalRegistry` (tests only; doc says it "renders inside Coach Lab"); `LifeEvents.kt:234-243` `explain` (tests only); `PreSessionBrief.kt` (built on every directive refresh, `data/repo/DirectiveRepository.kt:150`, stored in `OverviewUiState.brief`, read by no composable); `GoalPortfolio.kt` `evaluate`/`conflicts` (VM fields never read; `harvestCompleted` has no caller); `ProjectScanner.kt` (proposals never rendered, `ui/coach/CoachViewModel.kt:158-169`); `WeeklyReview.kt` (`focusLine`, `fatigueBand`, `stalledLifts`, `trackedLifts`, `restingHrBpm`, `cardioMinutesLastWeek`, `usedHealthSignals` never shown); `TrustLadder.kt` (`shouldDemote`, `describe`, `volumeStep`, `mayInitiate`, `mayActFirst`, `userCap`, `autonomyConsented` unused); `PersonalProfile.kt` `sweetSpotReps`, `strongestHour` (computed every build, never shown; they make `hasPersonalData` true and can render an empty "YOUR NUMBERS" header, `ui/coach/CoachLearned.kt:99-105`); the `domain/engine` planner stack behind a never-injected `ConditioningRepository` (a documented hold).
- Problem: unreachable features that cost work on hot paths (a PreSessionBrief per Overview refresh; `WeeklyReview.assemble` runs `ProgressionAdvisor.evaluate` + `DeloadAdvisor.fatigue` for an unseen focus line). The three latent bugs below reach users the moment any of this is wired.
- Fix: wire (after fixing the latent bugs) or gate/delete for release; remove unused TrustLadder API, PersonalProfile fields and unrendered `WeeklyReviewData` fields plus their computation.

### [P3] Latent (unrendered): ProjectScanner says "Bring up your back: 0 working sets a week" after one week off
- Location: `domain/coach/ProjectScanner.kt:96-114`
- Problem: `laggingMuscle` reads a rolling 7 days (copy says "week after week") with no layoff/vacation guard; after a week away every muscle scores 40 + cap. On a followed 3-day plan it flags muscles the generator deliberately keeps low (calves 4 vs cap 14), treating a ceiling as a target.
- Fix: require the shortfall in ≥3 of the last 4 ISO weeks; skip on `LifeEvents.layoff` or short weeks; compare against planned sets (`VolumeTargets`).

### [P3] Latent (unrendered): PreSessionBrief seeds a first-time isolation from compounds
- Location: `domain/coach/PreSessionBrief.kt:170-181`
- Problem: `seedFromSimilar` averages every same-muscle, same-unit slot × 0.7: a first Cable Fly with 150 lb Machine Chest Press history is seeded at 105 lb, "seeded from your similar lifts".
- Fix: seed only from the same role (ideally same `MovementPattern`), or per-role fractions; else null.

### [P3] Latent (unrendered): ConditioningPlanner's ramp guard can never fire
- Location: `domain/engine/ConditioningPlanner.kt:82-83`; `ConditioningLoad.kt:62-72`; caller `data/repo/ConditioningRepository.kt:73-77`
- Problem: `rampRate` compares this week with the three before, but the caller passes `cardioDao.since(weekStart)`, so prior weeks are 0 and `rampCapped` is always false. `liftingDaysAhead` is calendar days left, not lifting days (`:103-106`). `INTERVAL_CLEARANCE_HOURS` unused.
- Fix: pass ≥4 weeks and filter "this week" inside `planWeek`; compute program days remaining.

### [P3] A final "not followed" verdict displays "still watching" — still open from domain.md D4
- Location: `domain/coach/CoachOutcome.kt:19-35` (shown at `ui/coach/CoachUi.kt:91`)
- Fix: `OUTCOME_NOT_FOLLOWED -> "not judged"`; show the countdown only for `OUTCOME_PENDING`.

### [P3] Test coverage gaps on critical math
- Category: release
- Problem: none of the findings above is caught by the suites. Untested: `suggestNextLoad` for kg users; keep-weight/consolidate under readiness or DELOAD scaling; `cutSuppressedStalls` (0 tests); `ProgramGenerator.effectiveVolumeBias` and `plannedSetsPerDay` (0); a pin placed twice in one day (`ProgramGeneratorQualityTest` checks only the heavy slot); `PersonalProfile.recoveryDays`/`sweetSpotReps`/`strongestHour`; personal caps across a split change; `DeloadAdvisor.checks` with warm-up or swapped bouts; OutcomeWatcher against an old lifetime best; `RestAdvisor.restSeconds` EASY with a base < 90 s; the beginner cap when `changesPerWeek` is non-null; `BlockPlanner.advance` with non-ASCII ids; the D5 timed-hold crash.
- Fix: add the regression test named in each finding.

## P4

- **bad-code:** the fatigue "building" band is hard-coded in three places (`AutoCoachPlanner.kt:311`, `InsightEngine.kt:252`, `WeeklyReview.kt:105`) separately from `t.consolidateBandPoints` (`AutoCoachPlanner.kt:221`); add `AdaptThresholds.fatigueBuilding(score)`.
- **bad-code:** the "tracked lift" check is duplicated (`AutoCoachPlanner.kt:359-362`, `WeeklyReview.kt:97-99`) and counts every `exerciseHistory` key (old programs, freestyle, TEST/TECHNIQUE, swaps): "All 9 tracked lifts are progressing" for a 6-lift program. Derive from `stallRead` over program slots.
- **dead-code:** `AdaptThresholds.kt:127-130` `readinessCardioLoadMinutes`/`readinessCardioLoadPenalty` (superseded, still documented as live); `ConditioningPlanner.kt:22` `INTERVAL_CLEARANCE_HOURS`; `program/ExerciseLibrary.kt:901` `forMuscle` (tests only); `program/Trophies.kt:46-47` `UnlockRule.MaxBenchAtLeast`/`MaxSquatAtLeast` used by no trophy while `TrophyRepository.kt:105-106` still runs two queries for them.
- **bug (minor):** `ReadinessAdvisor.kt:89-106` "fresh after 2 rest days" when only one day was rest (`daysSince` counts today, so the +1 bonus fires after one rest day); `:289` `hrvDrop` gates on the resting-HR sample threshold.
- **simplify:** several rep-range parsers and two floor-to-grid implementations (`ProgressionAdvisor.kt:52-53, 581`; `PreSessionBrief.kt:192-194`; `SessionEstimate.kt:80-84` compiles its Regex every call; `ProgramGenerator.kt:362`). `floorToGrid` has no epsilon: at 310 lb in DELOAD with readiness −4, `312.5 * 0.816 = 254.99999999999997` floors to 252.5 instead of 255 (`grid_fp2.py`). One `WeightGrid.floor(value, step)` with epsilon; `RepRange.parse` everywhere.
- **bad-code:** gap/spacing rules use elapsed ms where the coach moved to calendar days (`AutoCoachPlanner.kt:136-140` vs `LifeEvents.kt:175-176`; `PersonalProfile.kt:135`; `InsightEngine.kt:478`): LifeEvents reports a 14-day layoff while the planner sees 13.5 days and restructures anyway.
- **nits:** `program/Trophies.kt:9-10` says 11 icon types (15 exist); `:39` `TotalSessionsAtLeast` counts logged exercises (rename `TotalLoggedExercisesAtLeast`); `ExerciseLibrary.kt:838-839` `seated-leg-curl` under the Glutes header; `DeloadAdvisor.kt:155, 158` "rated sets" counts bouts; `InsightEngine.kt:139` `<=` + `toInt()` suppresses a real 5.9% "most improved"; `TodayDirective.kt:239` default-locale weekday in English copy; `ProjectScanner.kt:185-186` no-op `mapValues`; quad/ham balance defined three ways (`InsightEngine.kt:90`, `GoalPortfolio.kt:389-393`, `ProjectScanner.kt:80`) — one `BalancePair` in `adapt`.

## Files reviewed (51)
- `domain/coach/`: AutoCoachPlanner, BlockPlanner, CoachGenBias, CoachGoalKind, CoachOutcome, CoachSignal, GoalPortfolio, LifeEvents, OutcomeWatcher, PersonalProfile, PreSessionBrief, ProjectScanner, SessionAdaptor, SessionOpinion, SuggestionCalibrator, TodayDirective, TrustLadder, TrustLedger, WeeklyReview, WeightPhase
- `domain/adapt/`: AdaptThresholds, AdaptationSnapshot, DeloadAdvisor, E1rm, EffortModel, InsightEngine, OrderingAdvisor, ProgressionAdvisor, ReadinessAdvisor, Recommendation, RecommendationArbiter, RestAdvisor, RestingHrTrend, SnapshotAssembler, VolumeResponse
- `domain/engine/`: AerobicBase, ConditioningLoad, ConditioningPlanner, ConditioningProfile, ZoneCoach
- `program/`: CustomExerciseRegistry, ExerciseLibrary, GoalProfiles, Program, ProgramGenerator, SessionEstimate, SplitTemplates, Trophies, Types, VolumeModel, VolumeTargets
