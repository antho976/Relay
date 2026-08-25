# Coach / adaptation / program-generation — pre-release bug scan

Scope: `domain/adapt/`, `domain/coach/`, `domain/engine/`, `domain/warmup/`, `domain/rank/`,
`data/repo/CoachRepository.kt`, `data/repo/ProgramRepository.kt`, `data/repo/AdaptationRepository.kt`,
`program/` (generator, VolumeModel, ExerciseLibrary).

Everything below was read in source. Pure advisors (`TrustLedger`, `TrustLadder`, `OutcomeWatcher`,
`AutoCoachPlanner`, `CoachGenBias`, `DeloadAdvisor`, `WarmupEngine`, `RankLadder`, `GoalPortfolio`)
have good unit coverage and are largely sound. **Every finding below sits in an untested seam**:
`CoachRepository` (the whole apply/skip/undo lifecycle) has *no* test file at all, and the
snapshot→advisor→program-write round trip is only tested one advisor at a time.

Good news up front, so the ranking reads honestly:
- The weekly-pass **pass-level** idempotency guard is real and correct. `CoachRepository.kt:170-240`
  serialises on `weeklyPassMutex`, keys on ISO week (`weekId()`, :792), commits pass+decisions in
  one transaction with `OnConflictStrategy.IGNORE` on the week PK (`CoachDao.kt:14-32`), and only
  auto-applies when `won == true`. A second open on the same Monday, a timezone flight, or a
  backwards clock landing inside an already-recorded week all return the existing row.
- **Decision-level** double-apply is also guarded: `applyDecisionLocked` re-reads status inside
  `lifecycleMutex` and bails unless `STATUS_PROPOSED` (`CoachRepository.kt:466-468`).
- `CoachGenBias.from()` is a genuine fixed point — recomputed from the same rows every generate,
  clamped ±2, so folding never compounds.

---

## [CRITICAL] Undoing a coach volume change silently steals the coach's rep-range override and permanently user-locks the slot

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:592-596`
(with `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/ProgramCustomizationRepository.kt:169-177`)

**What:** `program_customization` is ONE row per `(day_key, exercise_id)` carrying `rep_range_override`,
`sets_override` AND a single `source` column (`ProgramCustomization.kt:24-35`). Two different coach
decision types write the same row. The volume undo path is:

```kotlin
"volume_up", "volume_down" -> {
    if (programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.source == OverlaySource.COACH)
        programCustomizationRepo.setSetsOverride(
            d.dayKey, d.targetKey, d.undoData?.toIntOrNull() ?: 0, source = OverlaySource.USER
        )
}
```

and `setSetsOverride` does `(existing ?: new).copy(setsOverride = sets, source = source)` — it copies
the **whole existing row**, so an unrelated `repRangeOverride` written earlier by a *different* coach
decision survives, but its `source` is flipped from `COACH` to `USER`.

Three things break at once:
1. The rep-range override is now attributed to the user, so `undoDecisionLocked`'s compare-and-restore
   for the rep_shift (`:586`) sees `source != COACH` and refuses to restore — the decision is marked
   `reverted`/`failed` while the coach's 12-15 range stays live on the program forever.
2. `userOwnsSlot` (`:446-448`) and the planning-time lock scan (`:376-388`) now both treat the slot as
   a user customization, so **the coach can never touch that lift again** — no rotations, no rep
   shifts, no volume, silently, for the life of the account.
3. The trust ledger records a revert (bad outcome) for a change that was never actually undone.

The LIFO guard does not help: `newerCoachDecisionOwnsSlot` (`:453-461`) only matches *same-kind*
decisions (`rep_shift` vs `rep_shift`, `volume*` vs `volume*`), so a newer rep_shift never blocks an
older volume undo.

**Scenario:** Week 3 the coach proposes "Shift Lat Pulldown from 8-10 to 12-15" — user applies (row:
`repRangeOverride="12-15"`, `source=COACH`). Week 5 the coach proposes "Add a set to Lat Pulldown
(3 → 4)" — user applies (same row: `setsOverride=4`, still `source=COACH`). Week 6 the user decides
the extra set is too much and taps Undo on it. Result: sets go back to 3 (correct), but the row is
now `repRangeOverride="12-15", source=USER`. Lat Pulldown is stuck at 12-15 reps permanently, the
coach has silently written that slot off as user-owned, and undoing the rep shift is a no-op that
still counts as a failure against `rep_shift` trust.

**Fix:** Make the undo write field-scoped rather than row-scoped. Add
`ProgramCustomizationRepository.clearSetsOverride(dayKey, exerciseId)` mirroring `clearRepRange`
(`:222-225`, which correctly leaves `source` alone), and in the volume undo branch write only the
`setsOverride` field, leaving `source` untouched. Longer term, `source` should be per-field
(`sets_source` / `reps_source`) since two independent overlays share one row.

---

## [CRITICAL] A still-proposed volume decision applies a stale absolute set count after any regenerate

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:500-505`
(payload minted at `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/AutoCoachPlanner.kt:257-263` and `:285-293`)

**What:** The planner computes the payload as an **absolute** target from the set count it saw at
pass time — `payload = (pick.slot.targetSets + 1).toString()` — and the apply path writes it
absolutely: `programCustomizationRepo.setSetsOverride(d.dayKey, d.targetKey, newSets, ...)`.

Between the pass and the tap, the program can be replaced wholesale. `ProgramRepository.generate`
(`:147-152`) replaces every slot, calls `reconcileCustomizations` (which **deletes every non-`custom_`
override row**, `:336-341`), and folds only rows whose status is already `applied`
(`CoachDao.foldAllAppliedDeltas`, `:72-77` — `WHERE status = 'applied'`). A **proposed** decision is
untouched by all of that, so its stale absolute payload survives into a program it no longer
describes. `applyDecisionLocked` re-validates only `userOwnsSlot` (`:472`) — which is false, because
the override row was just deleted — and never checks that the slot still exists or still has the set
count the summary promises.

The same hole exists for `rerollDay` (`ProgramRepository.kt:212-245`) and for the deload regenerate
(`AdaptationRepository.applyDeloadWeek`, `:336-372`), which regenerates at `DELOAD_FACTOR = 0.55`.

**Scenario:** Monday's brief says "Add a set to Incline DB Press (3 → 4)". Wednesday the user hits
"Generate deload week" from the Overview card, which regenerates everything at 55% volume — Incline
DB Press is now 2 sets, or is off the plan entirely. Thursday the user opens the Week Brief (the
proposal is still there, still worded "3 → 4") and taps Apply. Either they get **4 sets in the middle
of a deload week** — a 100% volume increase on a recovery week the coach itself called — or, if the
exercise no longer exists on that day, an orphan `program_customization` row is written that nothing
reads while the brief reports "1 change applied this week". A pure reroll can equally turn "add a
set" into a silent set *removal* when the fresh baseline landed above the stale payload.

**Fix:** Store the delta, not the absolute (`payload = "+1"` / `"-1"`), and resolve it against the
live slot at apply time. At minimum, add a pre-apply guard to `applyDecisionLocked` for
`volume_up`/`volume_down`/`rep_shift`/`swap` that re-reads the current program and skips (status
`STATUS_SKIPPED`) when the target slot is missing or its current set count is not what the decision
was computed against — and expire proposals on regenerate the way applied rows are folded (a
`WHERE status IN ('applied','proposed')` variant, marking proposals `skipped` rather than `folded`).

---

## [HIGH] "Run a deload week" is permanent — nothing ever restores full volume

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/AdaptationRepository.kt:336-372`
(with `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/ProgramRepository.kt:155`,
`/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:405-409`,
`/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/prefs/SettingsRepository.kt:676-677`)

**What:** Applying a deload decision calls `applyDeloadWeek()`, which regenerates the entire program
with `deload = true` (`ProgramGenerator.DELOAD_FACTOR = 0.55`, and `minSets` dropped to 1 —
`ProgramGenerator.kt:118-125`). That regenerated program becomes the **baseline** in `program_slot`.
The only thing that ever undoes it is a *non-deload* `generate()` (`ProgramRepository.kt:155`), which
happens on a manual "Refresh trainings" or on auto-rotation. Auto-rotation is skipped during the
deload week (`WorkoutRepository.kt:405-409`) and — critically — `rotationCadence` **defaults to
`"never"`** (`SettingsRepository.kt:677`). The deload apply is also explicitly documented as
non-undoable (`CoachRepository.kt:474-479`, `markApplied(id, now, null)`).

Nothing in the UI restores it either: the only writer of `applyDeloadWeek` is the coach decision and
the Overview `deload.suggest` card (`OverviewViewModel.kt:397`). There is no "your deload week is
over" path.

**Scenario:** Default-settings user, 10 weeks in, fatigue score crosses 5. Monday's brief proposes
"Run a deload week — lighter volume across the board". They tap Apply. Their 4-day program drops from
~16 working sets/day to ~8. Two weeks later `DeloadAdvisor` un-mutes (`deloadRecentDeloadSuppressDays
= 10`) and the coach happily reports "All 6 tracked lifts are progressing" — because they are, at
half volume, forever. The user is now permanently detraining and the coach's own weekly review says
the plan is working.

**Fix:** Either (a) auto-exit: when `clock.nowMs() - deloadWeekStartMs >= DELOAD_WEEK_MS`, regenerate
once with `deload = false` (a natural place is the weekly pass, right after `advanceForWeek`), or
(b) don't regenerate at all for a deload — apply it as a temporary per-slot overlay that the
customization reconcile drops after seven days. (a) is smaller; either way the Week Brief should say
when the deload ends.

---

## [HIGH] The plateau ladder reads a persistently-swapped slot with the ORIGINAL exercise's unit, name and rep text

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/AdaptationRepository.kt:155-160`
(consumed at `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt:306`, `:385-387`, `:417-448`)

**What:** The snapshot's program comes from `programCustomizationRepo.effectivePlanForDay(...)`, which
merges the `program_customization` overlay (sets/reps/removed) but **not** the `exercise_customization`
swap overlay — swaps live in a separate globally-keyed table (`ExerciseCustomization.kt:15-35`) and
`effectivePlanForDay` never touches it (`ProgramCustomizationRepository.kt:97-140`). So
`ProgramSlotSnap.exerciseId/name/muscle/unit/repsText` are all the **base** exercise's, while
`exerciseHistory[slotId]` correctly contains bouts performed on the **swapped** exercise
(`SnapshotAssembler.kt:54-69`, keyed by `effectiveSlotId`).

The in-session chip does resolve the swap (`DayViewModelBuilders.kt:59-61` → `effectiveUnit`, passed
to `suggestNextLoad` at `:114`). The snapshot path does not. Every consumer of
`ProgressionAdvisor.evaluate` — Overview coach feed (`AdaptationRepository.kt:263`), Stats
(`:306`), the Week Brief's stall count (`WeeklyReview.kt:81`), the deload plateau driver
(`DeloadAdvisor.kt:327`), Coach Lab (`CoachRepository.kt:672`) and the weekly pass itself
(`AutoCoachPlanner.kt:149`) — therefore prices weights on the wrong unit and names the wrong lift.

**Scenario:** The coach rotates the "Barbell Bench Press" slot (unit `PLATES`, `plateLb = 15`) to
"DB Bench Press". The user logs 60 lb dumbbells for eight weeks and stalls. The ladder fires the
high-effort reset branch: `slot.unit == PLATES` → `target = (60 - 15).coerceAtLeast(15) = 45` →
`inputTextFor(45.0, PLATES, 15.0)` = **"3 plates"**. The Overview card reads *"Barbell Bench Press
stalled 8 sessions at high effort — drop ~10% and build back up: 3 plates"* for a lift that is
neither called Barbell Bench Press nor loaded with plates. The rep-shift branch (`:398-408`) is
equally wrong: it parses `slot.repsText` from the base plan and shows "Shift Barbell Bench Press
from 6-8 to 12-15".

**Fix:** Resolve the persistent swap when building the snapshot — in
`AdaptationRepository.assembleSnapshot` overlay `exercise_customization` onto each `ExercisePlan`
(`swappedName` → name, `swappedUnit` → unit, `swappedExerciseId` → library lookup for
muscle/tags/defaultReps) while keeping `plan.id` as the **slot** key so history and coach targeting
stay stable. That is exactly what `DayViewModelBuilders` already does for the chip; one shared
resolver would keep the two paths from disagreeing.

---

## [HIGH] A swap never resets the stall counter, so the coach re-proposes the same rotation forever

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt:316-325`
(with `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/AutoCoachPlanner.kt:163-177`
and `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/AdaptationRepository.kt:166-170`)

**What:** The stall length is computed over the slot's **entire** e1RM series:

```kotlin
var best = e1rms.first(); var lastImprovedIdx = 0
for (i in 1 until e1rms.size) { if (e1rms[i] > best * (1 + t.stallTolerance)) { best = e1rms[i]; lastImprovedIdx = i } }
val stall = e1rms.lastIndex - lastImprovedIdx
```

Nothing partitions that series at a swap boundary — `ExerciseBout.swappedName` is carried on the
snapshot but is only used for display names and the "make the swap permanent?" insight, never to
segment the strength series. A swap into a variation with a **lower absolute** e1RM (barbell →
dumbbell, machine → free weight, bilateral → unilateral) therefore leaves `best` pinned to the
pre-swap lift's number, and the stall counter keeps climbing from there — permanently.

Two things compound it:
- `swapCandidateIds` (`AdaptationRepository.kt:166-170`) filters only `it != plan.id`, i.e. the
  *base* slot id, so the exercise the slot has already been swapped **to** is still a candidate, and
  `candidateIds.firstOrNull()` (`AutoCoachPlanner.kt:167`) is deterministic — the same replacement
  every week.
- `declinedStructural` (`CoachRepository.kt:407-412`) only suppresses re-proposal when the most recent
  decision was `skipped` or `reverted`. An **applied** swap is never suppressed.

**Scenario:** "Barbell Row" (best e1RM ≈ 160 lb) stalls for 8 sessions. Week 1 the coach proposes
"Rotate Barbell Row → DB Row"; user applies. They log 50 lb dumbbells → e1RM ≈ 62, and every
subsequent bout is far below `best * 1.005`. `lastImprovedIdx` stays frozen on the last barbell bout,
so `stall` grows by one every session. Week 2 the pass fires again at `stall = 9 ≥
swapAfterStalledBouts` and proposes **"Rotate Barbell Row → DB Row"** again — a change already in
effect, worded with a name that is no longer in the program. Applying it rewrites the identical
overlay. This repeats every week, permanently occupying the 1-2 change/week cap so no real
adjustment ever gets proposed, and the `OutcomeWatcher` closes each one `ok` after 14 days
(`OutcomeWatcher.kt:84` — swaps are judged on *attendance only*, no strength check), which walks
`swap` straight to earned autopilot.

**Fix:** Two parts. (1) Reset the stall baseline at a swap boundary — either drop pre-swap bouts from
the ladder's series when the slot's live swap changed, or normalise each bout's e1RM to the exercise
it was actually performed on. (2) Exclude the slot's current *effective* exercise id from
`swapCandidateIds`, and treat a recently-applied swap on a slot as a cool-off (the same
`declinedStructural` key, extended to `STATUS_APPLIED` within N weeks).

---

## [HIGH] Autopilot is earned from applied-but-unvalidated decisions, so two weeks of taps unlock it

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/TrustLedger.kt:83-92`
(consumed at `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:249-256`)

**What:** The streak counts *decisions*, not weeks, and an `applied` row with `outcome == "pending"`
counts as accepted:

```kotlin
val accepted = when (d.status) {
    "applied" -> d.outcome != "failed"   // pending counts
    "folded"  -> d.outcome == "ok"
    else -> false
}
```

The comment acknowledges the pending case as intentional ("it stays under the watcher and demotes
later if it fails"), but the *decision*-granularity is what makes it fire early: one weekly pass can
emit up to `cap` decisions of the same type (`AutoCoachPlanner.kt:226-227`, `cap = 2` for a
non-beginner, and `structural` can hold two `rep_shift`s for two different stalled lifts —
`:178-190`). `CONSERVATIVE_STREAK` is 3.

**Scenario:** Intermediate user, week 1: two lifts hit `repShiftAfterStalledBouts`, so the brief has
two rep-shift proposals. The user taps "Apply all". Week 2: two more stalled lifts, "Apply all"
again. Four `applied` rep_shift rows, **zero** watcher verdicts (the earliest 14-day window has not
closed). `TrustLedger.earnedTypes` returns `{rep_shift}` (streak 4 ≥ 3), and week 3's pass
auto-applies every rep-shift it proposes with no tap — `autoApplyEarnedTypes` reads history strictly
before the current week (`CoachRepository.kt:251`), so the four unvalidated rows are all it sees. The
coach starts acting autonomously on a track record that contains no evidence it was ever right.
`TrustLadder`'s stricter global gates (8 *judged* calls, 3 weeks) do not apply here — per-type
autopilot in `autoApplyEarnedTypes` consults `TrustLedger` only.

**Fix:** Require validated acceptances for promotion: count only rows with `outcome == "ok"` toward
the streak (pending rows stay neutral, exactly as `folded` + pending already does at `:74`), or
require the streak to span at least `required` distinct `weekId`s so one Apply-All can't earn a tier.

---

## [HIGH] `TrustLadder`'s revert cap is lifetime, not windowed — three undos ever permanently freeze the coach at PROPOSE

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/TrustLadder.kt:41-42, 120, 125-132`

**What:** The constant is documented as a window and implemented as a lifetime total:

```kotlin
/** Reverts in the recent window that cap the coach regardless of its win rate. */
const val REVERT_CAP = 3
...
val reverts = decisions.count { it.status == "reverted" }   // whole ledger, no window
...
reverts >= REVERT_CAP -> Tier.PROPOSE
```

`status == "reverted"` is written by every user undo (`CoachDao.markReverted`, `:59-60`) *and* by
every coach self-revert applied through a `revert` decision (`CoachRepository.kt:506-510` →
`undoDecisionLocked` → `:600`). So the coach's own correction pipeline — the thing that is supposed
to *demonstrate* good judgment — counts against it identically to a user rejection.

**Scenario:** A user 18 months in has undone three coach changes across those 18 months (or the
watcher caught three failures and the coach reverted them itself). `assess` clamps `earned` to
`Tier.PROPOSE` forever, regardless of a 95% win rate over 200 judged calls. `changesPerWeek` is
stuck at 2, `mayInitiate` is permanently false, and the "Autopilot" / initiative milestones can never
be reached. `shouldDemote` (`:156`) hard-codes the same rule, so hysteresis never lets it recover.
The Coach Lab readout will keep saying "Proposing" while showing a 95% win rate — which reads as a
broken coach, precisely what the tier design set out to avoid.

**Fix:** Window it, matching the doc. Count reverts inside a recent span (e.g. the last 12 weeks by
`weekId`, or the last N decided calls), and exclude coach-initiated reverts (`revert`-type parents)
from the count — a change the coach itself walked back is evidence the watcher works, not evidence
of bad advice.

---

## [MEDIUM] A single sick check-in on pass day voids every open outcome verdict, including windows that were fully lived

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/LifeEvents.kt:179-186`
(consumed at `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/OutcomeWatcher.kt:66-73`)

**What:** `suppressesVerdict(appliedAtMs, windowEndMs, state)` opens with an unconditional
`if (state.sick) return true`. It never compares the illness to the window it is judging. `state.sick`
is *today's* flag — a check-in or a `restReason == "sick"` cardio row inside the last
`SICK_WINDOW_DAYS = 3` (`:98-104`). The layoff branch below it does the overlap arithmetic correctly;
the sick branch does not.

`OutcomeWatcher` applies it only when the window has closed, and the resulting verdict is written
straight to the durable `outcome` column (`CoachRepository.kt:365`) as `not_followed` — which is
**terminal**: `pendingOutcome()` (`CoachDao.kt:94`) no longer returns the row, so the change is never
re-judged. `not_followed` is invisible to both `TrustLedger` (`:70`) and `TrustLadder` (`:117-119`)
and drops out of `CoachGenBias` (`:60-61`).

**Scenario:** A user applies four coach changes over a fortnight and trains every session. On the
Monday the last window closes they log "sick" in the morning check-in. Every one of those four
changes is closed out as *"you were away or unwell for this window, so it isn't judged"* — including
three whose entire 14 days predate the illness. The coach loses a fortnight of evidence, the trust
streak neither advances nor breaks, and `CoachGenBias` discards the volume/rep learning those changes
carried, so the next regenerate quietly drops them from the baseline.

**Fix:** Make the sick branch overlap-aware like the layoff branch — track when sick flags were
recorded (`CheckinEntry.recordedAt` is already loaded) and suppress only when a sick day falls inside
`[appliedAtMs, windowEndMs]`. `LifeEvents.State` needs to carry the sick *dates*, not just a boolean,
for this.

---

## [MEDIUM] A refused revert still marks itself applied, producing a zombie revert proposal every week forever

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:506-510`
(with `:557-563` and `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/OutcomeWatcher.kt:147-163`)

**What:**

```kotlin
"revert" -> {
    val originalId = d.payload?.toLongOrNull() ?: return
    undoDecisionLocked(originalId)          // return value ignored
    coachDao.markApplied(id, clock.nowMs(), null)
}
```

`undoDecisionLocked` returns silently — without touching the original row — when the original is no
longer `STATUS_APPLIED` (`:559`) or when the per-slot LIFO guard fires (`:563`). The revert decision
marks itself `applied` regardless. The original stays `status='applied', outcome='failed'`, which is
exactly the query `appliedFailed()` (`CoachDao.kt:102-103`) feeding
`OutcomeWatcher.revertProposalsFor` — so the same revert is re-derived at the top of every subsequent
pass, forever, ahead of every real adjustment (`AutoCoachPlanner.kt:196`, reverts are first in the
candidate list and the cap is 1-2).

The refused revert also gets `outcome = "ok"` from the watcher's catch-all after 14 days
(`OutcomeWatcher.kt:124`), so a revert that did nothing counts as a win for the `revert` type in the
milestone ladder and the global win rate.

**Scenario:** Two coach volume decisions land on the same (day, exercise) — id 10 (`volume_up`, later
judged failed) and id 20 (`volume_down`, applied and healthy). The revert proposal for id 10 fires;
`newerCoachDecisionOwnsSlot(id 10)` sees id 20 still active and refuses. Id 10 remains
applied+failed. Every Monday from then on the brief opens with *"Revert: Add a set to Cable Fly"*,
the user taps it, nothing happens, and it is back next week — while consuming the week's entire
change budget.

**Fix:** Have `undoDecisionLocked` return a `Boolean` (or throw) and only `markApplied` the revert
when the undo actually ran; otherwise mark the revert `skipped` **and** retire the original (set its
status to `folded`/`reverted`) so `appliedFailed()` stops re-deriving it. A refused revert is a
permanent refusal, not a retry.

---

## [MEDIUM] Same deload, two entry points, no cross-suppression — the user can regenerate their whole program twice

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:474-479`
and `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/overview/OverviewViewModel.kt:397`

**What:** `DeloadAdvisor.evaluate` surfaces on Overview as the `deload.suggest` card (applied via
`adaptationRepo.applyDeloadWeek()` directly, bypassing the whole coach lifecycle) *and* as a
`deload` `CoachDecision` in the Week Brief. The Overview path calls `logAdviceApplied("deload.suggest")`
which mutes only the Overview card for 14 days via `mutedAdviceIds` (`AdaptationRepository.kt:322-327`,
applied in `coachFeed` at `:264`). The coach decision path never consults `mutedAdviceIds`, and once a
`deload` decision row exists it stays `proposed` until tapped — `DeloadAdvisor` suppression via
`lastDeloadAppliedMs` only prevents *future* proposals, not the one already written.

**Scenario:** Monday the pass proposes a deload (row written). Tuesday the user taps "Generate deload
week" on Overview — program regenerated at 55%, in-progress workout discarded, `deloadWeekStartMs =
Tuesday`. Wednesday they open the Week Brief, still see "Run a deload week — lighter volume across
the board", and tap Apply. The program is regenerated **again** (fresh `System.nanoTime()` seed, so
different exercise picks), the in-progress workout is discarded again, and `deloadWeekStartMs` is
pushed to Wednesday — extending the deload and the advisor's mute window by a day each time.

**Fix:** Have `applyDecisionLocked("deload")` no-op (mark `skipped`) when a deload is already active
(`settings.deloadWeekStartMs` inside `DELOAD_WEEK_MS`), and have the weekly pass drop a still-proposed
`deload` decision once `lastDeloadAppliedMs` is set. Better: make the Overview card apply the coach
decision when one exists, so there is one lifecycle rather than two.

---

## [MEDIUM] The personal volume cap flips on a coin-toss comparison with no dead band — a muscle's weekly ceiling can swing 12 ↔ 24 sets

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/PersonalProfile.kt:98-113`
(consumed at `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/ProgramRepository.kt:191-193`
and `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/AdaptationRepository.kt:354-356`,
applied in `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/program/VolumeModel.kt:110-111`)

**What:**

```kotlin
val responsive = high.average() > low.average()
val target = if (responsive) default * (1 + CAP_BAND) else default * (1 - CAP_BAND)
```

It is a strict `>` with **no minimum gap**. A difference of 0.001 lb between the high-volume and
low-volume tiers' average lagged e1RM change decides whether the muscle's weekly ceiling is
`default × 1.35` or `default × 0.65` — for CHEST that is 24 vs 12 sets, a 2× swing, recomputed on
every regenerate.

The comparison is over noisy inputs: per-week best e1RM diffs, tiers split at the plain mean
(`:101`), only three weeks required per tier (`:107`), and the "weeks" are epoch/604800000 buckets
(`:88`) — Thursday-aligned, not ISO weeks, so they don't line up with the rest of the engine.

Notably, `InsightEngine.volumeResponse` runs the *same* computation for display and correctly refuses
to speak unless `abs(highAvg - lowAvg) >= insightVolumeDeltaGapLb` (`InsightEngine.kt:472`). The
generation path — which actually changes the user's program — has no such gate.

**Scenario:** A user with 9 training weeks on chest has `high.average() = 2.4 lb` and
`low.average() = 2.3 lb` — statistically indistinguishable. Chest's cap is set to 24. They log one
more week; a single heavy week lands in the low tier and flips the sign. Next "Refresh trainings"
prices chest at 12 sets, and `VolumeModel.allocate`'s cap loop shaves their chest work roughly in
half (`VolumeModel.kt:112-118`). Nothing in the UI explains it, and the coach's own drift cap still
believes it donated volume that the cap just removed (see next finding).

**Fix:** Reuse the insight's dead band — require `abs(high.average() - low.average()) >=
insightVolumeDeltaGapLb` before personalising at all, and return no entry (population default)
otherwise. Consider interpolating toward the band rather than jumping to its edge, and switch the
week bucket to the ISO-week key `InsightEngine.weekKey` already provides (`InsightEngine.kt:423-426`).

---

## [MEDIUM] The coach's volume drift cap counts sets the weekly cap silently shaved back off

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:402`
(with `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/program/VolumeModel.kt:90-119`)

**What:** The planner's ±2 drift cap reads `CoachGenBias.from(allDecisions).volumeBias` — the coach's
*intent*. `VolumeModel.allocate` applies that bias (`:91-104`) and then runs the per-muscle weekly-cap
trim (`:106-119`), which can remove every set the bias just added: `while (total > cap) { …
result[biggest] -= 1 }`. The two never reconcile, so the coach keeps believing it holds a +2 credit
on a muscle that received nothing.

This becomes reachable exactly when `PersonalProfile` decides a muscle is *not* responsive and drops
its cap 35% (previous finding): the baseline already exceeds the cap, so any coach bias is trimmed
first.

**Scenario:** Chest's personal cap is 12. The coach adds a set to chest in week 4 and again in week 7
(net bias +2, both applied and folded). The user refreshes trainings; `allocate` adds the two sets,
then the cap loop takes them straight back off. The program has the same chest volume as before. But
`volumeNetByMuscle[CHEST] == 2 >= VOLUME_DRIFT_CAP`, so `AutoCoachPlanner.volumeDecisions` (`:273`)
refuses to ever propose chest volume again — the coach has permanently spent a budget it never
received, and the Coach Lab reports "+2 set(s) carried forward" for chest (`CoachRepository.kt:716-718`)
against a program where they don't exist.

**Fix:** Have `VolumeModel.allocate` report what it actually applied (return the effective bias
alongside the allocation) and reconcile the ledger — fold trimmed decisions to `outcome =
not_followed` or drop them from the bias. Cheaper interim: derive `volumeNetByMuscle` from the live
program (effective sets vs the un-biased generator baseline) instead of from the decision ledger.

---

## [MEDIUM] A decision whose payload no longer resolves stays `proposed` forever and Apply is a silent no-op

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:481, 495, 501, 507, 511`

**What:** Every branch of `applyDecisionLocked` bails with a bare `return` on a bad payload without
changing status:

```kotlin
"swap" -> { val def = d.payload?.let { ExerciseLibrary.byId(it) } ?: return
"rep_shift" -> { val to = d.payload ?: return
"volume_up", "volume_down" -> { val newSets = d.payload?.toIntOrNull() ?: return
"revert" -> { val originalId = d.payload?.toLongOrNull() ?: return
else -> return
```

The row remains `STATUS_PROPOSED`, so it reappears in the brief and in `applyAll` on every open, and
`summaryFor` keeps reporting "1 proposal for this week" (`:750-754`). The UI's `runAndRefresh`
(`CoachViewModel.kt:161-171`) re-renders the unchanged row with no error — the tap simply does
nothing, repeatedly.

The reachable case is `ExerciseLibrary.byId(payload) == null`: a swap proposed before an app update
that renamed or removed a library id, or a decision restored from a backup taken on a different
build (`BackupRepository` carries `coach_decision`).

**Scenario:** A user's brief carries "Rotate DB Row → MWM Straight-Arm Pulldown" from before an
update in which that id was retired. Every Monday they tap Apply, nothing happens, the proposal is
still there, and "Apply all" leaves it behind too. There is no way to clear it from the UI.

**Fix:** Mark unresolvable decisions `STATUS_SKIPPED` (or a new `stale` status) rather than returning
silently, and have `ensureWeeklyPass` sweep prior weeks' still-`proposed` rows to `skipped` when a
new week's pass is created — a proposal from three weeks ago is not a live offer.

---

## [MEDIUM] A shadow-recorded HOLD never re-evaluates when the coach is switched back on

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:180-192, 200-222`

**What:** The mid-week re-enable path only regenerates a pass whose status is exactly
`STATUS_SHADOW`:

```kotlin
coachDao.pass(weekId)?.let { existing ->
    if (existing.status != STATUS_SHADOW || coachOff) return@withLock existing
    coachDao.clearPass(weekId)
}
```

But `proposeStatus` (`:192`) is only used on the `CoachPassStatus.SHADOW` branch (`:218`). When the
planner returns HOLD while the coach is off, the row is written with `STATUS_HOLD`, not
`STATUS_SHADOW` (`:220`). A vacation pass is `STATUS_HOLD` too (`:202`), and a crashed pass is
`STATUS_ERROR` (`:227`).

**Scenario:** A user has the coach switched off. Monday the app opens; the planner holds ("All 5
tracked lifts are progressing"), and the pass is recorded `hold`. Wednesday they switch the coach on,
open the Coach tab, and get last Monday's silent hold — with no proposals — until the following
Monday, even though their history has changed. The same is true after a vacation ends mid-week and
after an errored pass: the error is sticky for the rest of the ISO week with no retry.

**Fix:** Extend the regenerate condition to any pass recorded while the coach was off, and to
`STATUS_ERROR` (an error pass should be retried, not cached for a week). Persisting a
`recorded_while_off` flag on `coach_pass` is cleaner than inferring it from the status.

---

## [LOW] The plateau reset can prescribe a 0 lb target

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt:414-426`
(with `floorToGrid` at `:515`)

**What:** The high-effort reset branch computes
`floorToGrid(prevMax * (1 - t.resetFraction), t.dumbbellStepLb)` with no lower bound.
`floorToGrid(w, g) = (w / g).toInt() * g` floors to the grid, so any `prevMax <= dumbbellStepLb /
(1 - resetFraction)` ≈ 2.78 lb produces `0.0`. Both chip paths guard this explicitly
(`progressSuggestion` `:214` and `backOffSuggestion` `:259` both `return null` on `target <= 0.0`);
the snapshot path does not, and emits `weightChange(..., target = 0.0, inputText = "0")`.

The PLATES branch is protected (`.coerceAtLeast(plateLb)`), so this is DUMBBELL/WEIGHT-only. It also
means the "~10%" in the reason is often wrong on light loads — a stalled 20 lb dumbbell curl resets to
`floorToGrid(18.0, 2.5) = 17.5`, which is fine, but a 5 lb lateral raise resets to 2.5, a 50% cut
described as "drop ~10%".

**Scenario:** A user rehabbing a shoulder logs 2.5 lb front raises, stalls for five sessions at high
effort. Stats and the Overview coach feed show *"Front Raise stalled 5 sessions at high effort — drop
~10% and build back up"* with a target of **0**.

**Fix:** Add the same `target <= 0.0` guard the chip paths have (fall through to the micro-load
branch, or coerce to one grid step), and consider `roundTo` rather than `floorToGrid` so the stated
percentage matches the prescribed one.

---

## [LOW] `mostImproved` and `WeeklyReview.prs` count assisted-set weights

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/adapt/InsightEngine.kt:125`

**What:** `bouts.…mapNotNull { b -> b.sets.mapNotNull { it.weightLb }.maxOrNull() }` — no
`!it.isAssisted` filter, unlike every other strength read in the engine (`bestWorkingE1rm`,
`E1rm.kt:33-35`, filters it; `WeeklyReview.kt:72` does too). A band-assisted set logged with a
weight will therefore anchor the "most improved" percentage.

**Scenario:** A user doing band-assisted pull-ups logs the band as a weight. The insight reports
"Pull-up is up ~40% in 3 months" off assistance changes rather than strength.

**Fix:** Add `.filterNot { it.isAssisted }` before `mapNotNull { it.weightLb }`, matching
`bestWorkingE1rm`.

---

## [LOW] `timeOfDayPerformance` can divide by zero and emit a NaN percentage

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/adapt/InsightEngine.kt:409-417`

**What:** `gap = amMean / pmMean - 1` (or its inverse) with no guard on the denominator. The mean
guard at `:405` protects the per-lift normaliser, not the aggregate. `Double.NaN < someInt` is
`false`, so the `if (gap * 100 < t.insightTimePerfPct) return null` gate does **not** filter NaN, and
`NaN.roundToInt()` is `0`.

**Scenario:** A user who logs every weight as `0` (bodyweight movements entered as "0" rather than
"BW") over 12+ bouts on both sides of 14:00 gets *"Your estimated 1RMs run ~0% higher when you train
later in the day"*.

**Fix:** `if (!gap.isFinite() || gap * 100 < t.insightTimePerfPct) return null`, plus an
`amMean > 0 && pmMean > 0` precondition.

---

## [LOW] Dead lifecycle columns: `undo_expires_at`, `scope_key`, `lesson_id` are declared and never written

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/db/entities/CoachPass.kt:66, 73-81`

**What:** `CoachDecision` declares `lessonId`, `scope`, `scopeKey` and `undoExpiresAt`. Grepping the
whole `main/` source tree, the only `CoachDecision(...)` construction is
`CoachRepository.kt:211-217`, which sets none of them. `undoExpiresAt` in particular is documented as
the gate for "past this stamp the coach offers revert forward instead of one-tap undo" — that
behaviour does not exist, so `undoDecision` will happily unwind a structural change the user has been
training under for months (subject only to the per-slot LIFO guard). `scopeKey` is never populated,
so the day/session cadences the column was added for have no key to sort or group on.

**Scenario:** A user undoes a rep-range shift the coach applied five months ago. The overlay is
rewritten to a rep range from before the current training block, and the change is recorded as a
`revert` — which, per the `TrustLadder` finding above, counts permanently against the coach's tier.

**Fix:** Either write and enforce `undoExpiresAt` at `markApplied` time (refuse `undoDecision` past
it, offering the documented "revert forward" regenerate instead), or delete the columns so the schema
doesn't imply a guarantee the code doesn't make.
