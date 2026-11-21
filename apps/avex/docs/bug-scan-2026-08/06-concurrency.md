# Avex pre-release audit — concurrency, coroutines, lifecycle, state management

Scope: ViewModels, `data/repo/`, `data/prefs/`, `service/`, `service/wear/`, and the session-logging
path (`ui/gym/session/`, `ui/gym/train/`, `ui/gym/freestyle/`).

Recurring theme: **the day screen treats `_state.value.exercises` as the source of truth for what was
logged**, and it is refreshed by non-atomic read-modify-write coroutines that can be interleaved,
outrun by the watch, or cancelled. Every finding below that is ranked CRITICAL flows from that.

Notably, the codebase *already* solved this exact class of bug in two places —
`DaySwapHandlers.applySessionSwap` (`swapsInFlight` guard, DaySwapHandlers.kt:45-59) and
`ProgressPhotoRepository` (`writeMutex`, ProgressPhotoRepository.kt:90) — with comments describing
the precise failure mode. The primary set-logging path has no equivalent guard.

---

## [CRITICAL] Two concurrent `logSet` coroutines each insert their own `logged_exercise` row, silently hiding one set

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DayExerciseHandlers.kt:220-307` (the `?:`
insert at :290-298), `DayViewModelRefresh.kt:135-152` (`ensureLoggedExercise`, same shape),
`app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:180-197, 518`,
`app/src/main/java/com/forge/app/ui/gym/train/DayViewModelRefresh.kt:28-34, 108-111`

**What:** `logSet` launches into `viewModelScope` (Main.immediate) and reaches its first suspension
point at `settingsRepo.plateWeightLb.first()` (DayExerciseHandlers.kt:238) — a DataStore read —
*before* it decides whether a `logged_exercise` row exists:

```kotlin
val leId = currentUi.loggedExerciseId
    ?: workoutRepo.addExerciseToSession(sessionId = sessionId, exerciseId = effectiveExerciseId, ...)
```

`currentUi.loggedExerciseId` is read out of **UI state**, which is only refreshed after the write
round-trip completes (`refreshExercise`, several DB reads later). Two coroutines that start before
that refresh both see `null` and both INSERT. `logged_exercise` has no unique index on
(session_id, slot) — `refreshExercises` explicitly copes with the collision by keeping
"the entry with the MOST logged sets" (DayViewModelRefresh.kt:28-34, and again at :108-111), which
means **the loser's set is never rendered again and is excluded from the session totals**.

The submit button does nothing to prevent the second tap: `submitSet()` (SetInputRow.kt:180-197)
neither clears the fields nor sets a busy flag — the fields re-seed only when `nextSetNumber` changes,
which requires the completed round-trip — and `canSubmit` (SetInputRow.kt:214) therefore stays `true`
across the whole window. `onClick = { if (canSubmit) submitSet() }` (SetInputRow.kt:518) fires twice.

The same unguarded `?: addExerciseToSession` lives in `ensureLoggedExercise`
(DayViewModelRefresh.kt:141-151), reached from `RateExercise`, `UpdateNote`, `ToggleSkipped`
(DayExerciseHandlers.kt:48-75) — so it does not even need a double-tap on one control.

Compare `DaySwapHandlers.kt:45-59`, whose comment states the failure verbatim:
> "Two 'just today' swaps landing on the same slot before that refresh both read null and both
> INSERT, leaving two logged_exercise rows for one slot ... logged_exercise has no unique index on
> (session, slot) to catch it, so the guard is the only thing standing there."

The guard exists for swaps. It does not exist for logging sets.

**Scenario:** First set of Incline DB Press. User taps LOG SET, the button doesn't visibly change, so
they tap again ~150 ms later (or the double-tap is accidental — a very common gym gesture with sweaty
hands). Coroutine A inserts `logged_exercise` id 41 and a set under it; coroutine B, still holding the
pre-refresh snapshot, inserts `logged_exercise` id 42 and its own set. `refreshExercise` picks
whichever of the two has the most sets (both have one → `maxByOrNull` returns the first). One logged
set is now invisible on the day screen forever, is not counted by `finishWorkout`'s totals (see the
finding below), and shows up in Session Detail as a phantom duplicate exercise.

A second variant with no duplicate rows: once `loggedExerciseId` is non-null, both coroutines compute
`setIndex = currentUi.loggedSets.size` from the same stale snapshot and write two sets with the
**same `setIndex`** (DayExerciseHandlers.kt:300-307 → WorkoutRepository.kt:602-621), so
`sortedBy { it.setIndex }` ordering becomes arbitrary.

**Fix:** Add a per-slot in-flight guard mirroring `swapsInFlight` (an in-flight `Set<String>` or a
`Mutex` keyed by exercise id) around `logSet`/`ensureLoggedExercise`, and — belt and braces — make
`ensureLoggedExercise` atomic in the repository: `database.withTransaction { loggedExerciseDao
.forSessionAndSlot(...) ?: insert(...) }`, plus a `UNIQUE(session_id, COALESCE(slot_id, exercise_id))`
index so the DB refuses the second row. Derive `setIndex` from `loggedSetDao.countForLoggedExercise`
inside the same transaction rather than from UI state. Also disable the LOG SET button while a
submission is in flight.

---

## [CRITICAL] `refreshExercise` writes back a pre-suspension snapshot of the exercise list — a concurrently logged set is dropped from state (and therefore from the session's persisted totals)

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DayViewModelRefresh.kt:97-124`

**What:**

```kotlin
internal suspend fun DayViewModel.refreshExercise(exerciseId: String) {
    val current = _state.value.exercises              // :99  snapshot taken here
    ...
    val matching = workoutRepo.loggedExercisesForSession(sessionId)   // :108 SUSPENDS
    val logged = ... workoutRepo.setsFor(it.id) ...                   // :111 SUSPENDS
    val rebuilt = buildExerciseUi(..., settingsRepo.plateWeightLb.first(), settingsRepo.maxDbWeightLb.first(), ...) // :112-121 SUSPENDS (~7 DB round-trips)
    val newList = current.toMutableList().also { it[idx] = rebuilt }  // :122 stale `current`
    _state.update { it.copy(isLoading = false, exercises = annotateNextExerciseDeltas(newList)) } // :123
}
```

`current` is captured at :99 and written back wholesale at :122-123 after ~7 suspending DB reads.
`_state.update` is atomic in the CAS sense but the *value* being written is built from a snapshot
that predates every interleaved mutation, so this is a textbook lost update on the entire exercise
list. Everything except the one rebuilt entry is reverted to its state at :99.

Every mutating handler ends in `refreshExercise`/`refreshExerciseForSet` — `LogSet`, `DeleteSet`,
`EditSet`, `RateExercise`, `UpdateNote`, `ToggleSkipped`, `SetRpe`, `ToggleAmrap`,
`ToggleSetDifficultyTag`, `SetSupersetGroup`, `SetGoal`, `SetRestTimerOverride` — so concurrent pairs
are easy to produce.

**Scenario (superset, the most likely real trigger):** User is alternating Bench and Row. They log a
Bench set; coroutine A enters `refreshExercise("bench")` and snapshots the list (Bench still shows 2
sets). Within the ~150 ms of A's DB reads, they log a Row set; coroutine B snapshots the same list.
A completes and publishes `[Bench(3 sets), Row(2 sets)]`. B then publishes its own snapshot with only
Row replaced: `[Bench(2 sets), Row(3 sets)]`. **The just-logged Bench set has vanished from the UI.**
The row is still in Room, but the screen no longer shows it — and when the user then taps FINISH
WORKOUT, `finishWorkout` computes the session's persisted `totalVolumeLb`, `setCount` and `prCount`
from that state (see next finding), so the loss becomes permanent in history and in lifetime stats.

The second, quieter trigger is the note field, whose 500 ms debounce fires `UpdateNote` → `setNote` →
`refreshExercise` from a different coroutine than whatever the user is tapping.

**Fix:** Do not capture the list before suspending. Perform the DB reads first and only then mutate
inside `_state.update { }`:

```kotlin
val rebuilt = buildExerciseUi(...)            // all suspending work first
_state.update { s ->
    val i = s.exercises.indexOfFirst { it.plan.id == exerciseId }
    if (i < 0) s else s.copy(exercises = annotateNextExerciseDeltas(
        s.exercises.toMutableList().also { it[i] = rebuilt }))
}
```

`refreshExercises` (DayViewModelRefresh.kt:17-89) has the same shape at :35-38/:85 and needs the same
treatment.

---

## [CRITICAL] `finishWorkout` stamps the session's volume / set count / PR count from UI state, not from the database

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DaySessionHandlers.kt:80-95` (and
`saveAndExit` at :157-177), `data/repo/WorkoutRepository.kt:222-243`

**What:**

```kotlin
val exercises = _state.value.exercises
val allSets = exercises.flatMap { it.loggedSets }
val totalVolumeLb = VolumeCalculator.sessionVolumeLb(allSets)
val prCount = exercises.count { it.wasPr }
val activeSeconds = workoutRepo.finishSession(sessionId, totalVolumeLb, prCount, allSets.size)
```

`finishSession` writes these straight into the `session` row as denormalised columns
(WorkoutRepository.kt:231-239). Those columns are what every downstream surface reads: the history
list (`ui/gym/history/HistoryRows.kt:93,104`, `HistoryFiltering.kt:71,118`), Stats
(`StatsRepository.kt:145,336-340,471`) and — most damagingly — the Profile's **lifetime** totals
(`ProfileRepository.kt:142-144,165,181,203,312`: `totalVolumeLb`, `totalSets`, `totalPrs`,
`setsThisWeek`, `prsThisWeek`). Nothing recomputes them from `logged_sets` afterwards.

So any set that is present in Room but missing from `_state.value.exercises` at the moment FINISH is
tapped is permanently erased from the user's statistics.

**Scenario:** Any of these produce it —
1. the stale-snapshot race above dropped a set from state;
2. the user logged sets **from the watch** (see next finding) — the day screen never sees them at all;
3. a `refreshExercise` for the last-logged exercise is still in flight when FINISH is tapped (the
   button is live the whole time), so the final set isn't in state yet.

Concretely for (3): user logs their last set of the last exercise and immediately taps
"Done, finish workout" (`DaySessionContent.kt:263-266` fires `FinishExerciseEarly` **and**
`FinishWorkout` in the same click). The session is stamped with the volume as of one set earlier.
Their history row says "23 sets · 14,200 lb" when they did 24 sets.

**Fix:** Compute the totals in the repository from the database inside `finishSession`, in one
transaction: `val sets = loggedSetDao.allForSession(sessionId)` →
`VolumeCalculator.sessionVolumeLb(sets)`, `sets.size`, and
`loggedExerciseDao.forSession(sessionId).count { it.wasPr }` (exactly what
`resolveOrphanSession` already does correctly at WorkoutRepository.kt:475-487). Change
`finishSession`'s signature to stop accepting caller-supplied totals.

---

## [CRITICAL] Sets logged from the watch are invisible to the live day screen and are excluded from the finished session's totals

**File:** `app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:63-167`,
`app/src/main/java/com/forge/app/ui/gym/train/DayViewModel.kt:110-204`,
`app/src/main/java/com/forge/app/service/wear/WatchSessionMirror.kt:50-64`

**What:** `WatchSessionMirror` correctly observes Room (`loggedExerciseDao.observeForSession`,
`loggedSetDao.observeAllForSession`) so the wrist stays live. **`DayViewModel` observes nothing of the
kind.** Its only collectors are the rest timer, three DataStore settings flows and the custom-warmup
flow (DayViewModel.kt:112-203). Its `exercises` list is refreshed *only* by its own write handlers.

Consequences, all in the same session:
- A watch-logged set never appears on the phone screen.
- The next phone-logged set computes `setIndex = currentUi.loggedSets.size` from a count that is now
  short by however many sets the watch added → duplicate `setIndex` values inside one
  `logged_exercise`.
- Both sides independently do `row?.id ?: addExerciseToSession(...)`
  (SetLogUseCase.kt:148-155 vs DayExerciseHandlers.kt:290-298) with no shared lock or transaction, so
  a near-simultaneous phone + watch log on the same slot produces the duplicate-row corruption of
  finding 1.
- `finishWorkout` then stamps `totalVolumeLb` / `setCount` / `prCount` from the phone's state, which
  never contained the watch sets.
- `openRestEvent` (DayTimerHandlers.kt:35-63) is phone-only, so watch logs neither close the open rest
  interval nor open a new one — the realized-rest samples that feed `RestAdvisor` are corrupted for
  the whole session.

**Scenario:** User starts Push day on the phone, pockets it, and logs sets 2–4 of Overhead Press from
the watch. They pull the phone out at the end, see "1 set" on the OHP card (the phone's stale view),
and tap FINISH. The session is written with `setCount` and `totalVolumeLb` missing three sets and
missing any PR the watch flagged. Their lifetime tonnage on Profile is permanently short.

**Fix:** Make the day screen a DB observer for the live session, not a write-and-refresh cache —
collect `loggedExerciseDao.observeForSession(sessionId)` + `loggedSetDao.observeAllForSession(sessionId)`
in `DayViewModel` and derive `exercises` from those (Room already invalidates on every insert from
either side). At minimum, until then, (a) recompute the finish totals from the DB (previous finding),
and (b) route both write paths through one repository function that resolves-or-creates the
`logged_exercise` row and derives `setIndex` inside a single `withTransaction`.

---

## [CRITICAL] The exercise note's 500 ms debounce is cancelled when the card leaves composition, silently discarding the note

**File:** `app/src/main/java/com/forge/app/ui/gym/train/components/NoteField.kt:66-71`,
`components/ExerciseCardComponents.kt:424-443`, `components/ExerciseCard.kt:141, 399`,
`ui/gym/train/DayExerciseHandlers.kt:324-331`, `ui/gym/train/DaySessionContent.kt:259-266`

**What:**

```kotlin
LaunchedEffect(field.text) {
    if (field.text != baseline) {
        delay(500)
        onCommit(field.text)
    }
}
```

This is the only commit path for the note. `LaunchedEffect` is cancelled when the composable leaves
composition, so any text typed within 500 ms of the field disappearing is never handed to
`UpdateNote` and never reaches `logged_exercise.note`. There is no `DisposableEffect { onDispose { flush() } }`
and no VM-scoped fallback.

The field disappears in three routine ways:
- `ExerciseCardFooter` (which hosts `NoteField`, ExerciseCardComponents.kt:435-443) renders **only
  inside the `state.isExpanded` branch** (ExerciseCard.kt:141 vs :399), and `logSet` auto-collapses
  the card the moment the target set count is reached (DayExerciseHandlers.kt:324-331).
- In the focused single-exercise view, `onAdvance` / `onFinishEarly` set `shownExerciseId = exNextId`
  (DaySessionContent.kt:259, :263-266), removing the whole card.
- The NOTE chip is a toggle (`showNote = !showNote`, ExerciseCardComponents.kt:427) that unmounts the
  field.

`baseline` is also `remember { initialNote.orEmpty() }` with **no key** (NoteField.kt:55), so after a
successful commit + refresh the baseline is stale; a later edit back to the original persisted text
compares unequal and re-commits — harmless here, but it confirms the effect is doing double duty as
both dirty-tracking and persistence.

**Scenario:** User finishes their last set of Romanian Deadlift, types "left hamstring tight — drop to
95 next week" into Notes, and immediately taps "Done with this exercise" to move on (or logs the final
set, which auto-collapses the card). Under 500 ms have passed. The card unmounts, the effect is
cancelled, `onCommit` never runs, and the note is gone with no error and no visual cue that anything
was lost.

**Fix:** Keep the debounce for the happy path but add a guaranteed flush:
`DisposableEffect(Unit) { onDispose { if (field.text != lastCommitted) onCommit(field.text) } }`,
with `lastCommitted` tracked in a `remember`. Since `onCommit` ultimately runs in `viewModelScope`,
also wrap the repository write in `withContext(NonCancellable)` the way `GoalsViewModel` already does
(GoalsViewModel.kt:117-133) so a simultaneous screen pop can't cancel it.

---

## [HIGH] Double-tapping FINISH WORKOUT runs the whole finish path twice — and loses the rotation counter update

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DaySessionHandlers.kt:80-155`,
`ui/gym/train/DaySessionContent.kt:151, 259, 265`,
`data/repo/WorkoutRepository.kt:222-243, 396-417`

**What:** `finishWorkout()` has no in-flight guard and the FINISH control stays enabled until
`isFinished = true` is written at the very end of a coroutine that does ~8 DB round-trips
(`finishSession`, `setFirstWorkoutDone`, `evaluateAndUnlockNew`, `lifetimePrCount`,
`previousSessionForDay`, `bestPreviousVolumeForDay`). That window is easily 200-400 ms.
Contrast `SessionDetailViewModel`, which *does* guard its equivalents (`exportJob` at :82-89 and
`reLogJob` at :95-110).

Two concurrent `finishSession` calls both reach `maybeRotateProgram()` (WorkoutRepository.kt:277 →
:396-417), which is a read-modify-write on a DataStore key performed **outside** `edit { }`:

```kotlin
val n    = settingsRepo.rotationEveryN.first().coerceAtLeast(1)
val next = settingsRepo.rotationCounter.first() + 1     // :408
if (next < n) { settingsRepo.setRotationCounter(next); return }   // :409-412
settingsRepo.setRotationCounter(0)                                 // :413
programRepository.rerollAll()                                      // :417
```

Also duplicated on the second run: `writeFinishMirrors` → `maybeWriteActiveCalories`
(WorkoutRepository.kt:251-276), i.e. the session's calories are pushed to Health Connect twice.

**Scenario A (lost preference write):** rotation is set to "every 4 sessions", counter is at 2. The
user double-taps FINISH. Both coroutines read `2`, both compute `3`, both write `3`. The counter is
one short — the program re-roll the user is expecting after four workouts silently arrives after five.

**Scenario B (double regeneration):** counter is at 3 with `n = 4`. Both coroutines read `3`, both see
`next == n`, both write `0` and both call `rerollAll()` — two full program generations with different
random seeds racing each other; whichever transaction commits second is the program the user gets, and
the other generation's writes are thrown away. The user sees their program change, then change again.

**Scenario C:** Health Connect / Samsung Health shows the workout's active calories counted twice.

**Fix:** Guard `finishWorkout` and `saveAndExit` with an in-flight `Job?` (the pattern already used in
`SessionDetailViewModel`) *and* set `isFinished` optimistically before the suspending work. Make
`maybeRotateProgram`'s counter update atomic — move the read+increment+reset into a single
`forgePreferences.edit { }` (add e.g. `suspend fun bumpRotationCounter(limit: Int): Boolean` to
`SettingsRepository` that does the whole compare-and-set inside `edit`), and make `finishSession`
itself a no-op when `session.finishedAt != null`.

---

## [HIGH] Freestyle "Save workout" is neither transactional nor guarded: a cancelled or double-tapped save leaves a half-written or duplicated workout

**File:** `app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogViewModel.kt:98-128`,
`ui/gym/freestyle/FreestyleLogScreen.kt:319-357, 375-382`

**What:** `save()` writes an entire workout — `createFreestyleSession`, then per exercise
`addExerciseToSession` + N × `logSet` + up to 4 tag setters per set + `flagPrForLoggedExercise`, then
`finishSession`, then `clearFreestyleDraft` — as a bare loop in `viewModelScope` with **no
`database.withTransaction`** and no in-flight guard. (`reLogSession`, which does the very same kind of
bulk copy, *is* wrapped in `withTransaction` at WorkoutRepository.kt:142-181 — so the pattern is
established and simply missing here.)

Two failure modes:

1. **Cancellation mid-write.** The user taps "Save workout" and then the back arrow. `leave()`
   (FreestyleLogScreen.kt:304-310) calls `onBack()`, the destination pops, the ViewModel is cleared,
   `viewModelScope` is cancelled somewhere inside the loop. What is left in Room is a session with
   `finishedAt = null` and *some* of the exercises — invisible in history (which lists finished
   sessions), still holding the `active session` slot, and the draft was never cleared.
2. **Double tap.** `ForgePrimaryCapsule("Save workout", onClick = { save() }, enabled = canSave)`
   (FreestyleLogScreen.kt:375-382) has no busy state, and `save()` itself does not check `leaving`
   (which it sets at :355). `canSave` stays true because `items` never changes. A second tap creates a
   **second complete duplicate session** — double volume, double sets, double PR credit in lifetime
   stats.

**Scenario:** User logs a 40-minute freestyle workout, taps "Save workout", sees no immediate change
(the save is doing dozens of DB writes plus a PR pass per exercise), and taps it again. Two identical
"Open workout" sessions appear in history; Profile's lifetime tonnage counts the whole workout twice,
and it cannot be un-double-counted without deleting a session by hand.

**Fix:** Wrap the whole of `save()`'s body in `database.withTransaction { }` (move it into
`WorkoutRepository` as `saveFreestyleSession(items, startedAtMs)`, alongside `reLogSession`), add a
`private var saveJob: Job?` guard plus `enabled = canSave && !saving` on the capsule, and run the
persist under `withContext(NonCancellable)` so a back-press cannot leave a torn session.

---

## [HIGH] `leave()`'s last-chance draft flush runs in `viewModelScope` and is cancelled by the navigation it triggers

**File:** `app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogScreen.kt:290-310`,
`ui/gym/freestyle/FreestyleLogViewModel.kt:83-91`,
`data/prefs/SettingsRepository.kt:967-969`

**What:**

```kotlin
fun leave() {
    leaving = true
    if (draftChecked && pendingDraft == null && items.isNotEmpty()) {
        viewModel.saveDraft(draftFrom(items, openedAtMs))   // viewModelScope.launch { ... }
    }
    onBack()                                                // pops → VM cleared → scope cancelled
}
```

`saveDraft` is `viewModelScope.launch { settingsRepo.saveFreestyleDraft(json) }`
(FreestyleLogViewModel.kt:84-86), which reaches `context.forgePreferences.edit { }`
(SettingsRepository.kt:968) — a suspending disk write. `onBack()` runs on the very next line and pops
the destination, clearing the ViewModel and cancelling the scope. The flush and the teardown race.

This flush exists precisely because the debounced autosave (`LaunchedEffect(items, ...) { delay(600); saveDraft(...) }`,
FreestyleLogScreen.kt:290-297) will not have fired for edits made in the last 600 ms — so when it loses
the race, the user loses exactly the edits it was written to protect.

**Scenario:** User adds a final set (185 × 8) to the last exercise and immediately hits back to check
something. The 600 ms debounce hasn't elapsed. `leave()` fires the flush, `onBack()` pops the screen,
the DataStore write is cancelled. On returning to the logger, the resume prompt offers the draft as of
the previous debounce tick — the last set is gone.

**Fix:** Perform the flush on a scope that outlives the screen. Either `withContext(NonCancellable)`
inside `saveDraft` (`viewModelScope.launch { withContext(NonCancellable) { settingsRepo.saveFreestyleDraft(json) } }`),
or move the draft store behind an `@Singleton` with an application-scoped `CoroutineScope`
(`ForgeApp.appScope` already exists, ForgeApp.kt:34). The same `withContext(NonCancellable)` treatment
that `GoalsViewModel.kt:117-176` documents and applies should be the house rule for any write whose
caller immediately navigates.

---

## [HIGH] `CheckinViewModel.save()` swallows the write's failure (and its cancellation) and then tells the user it saved

**File:** `app/src/main/java/com/forge/app/ui/checkin/CheckinViewModel.kt:86-111`

**What:**

```kotlin
viewModelScope.launch {
    runCatching {
        checkinRepo.save(...)
        s.weightText.toDoubleOrNull()?.let { bodyweightRepo.log(it) }
    }                                       // result discarded entirely
    _state.value = _state.value.copy(visible = false, answeredToday = true)
}
```

The `Result` is never inspected. On any failure the sheet closes and the state flips to
`answeredToday = true` — the app's own record that the user has answered — while nothing was written.
`runCatching` also catches `CancellationException`, so if the VM is cleared mid-write the same thing
happens (the app elsewhere is careful about exactly this: `OverviewViewModel.kt:189-193` and
`AdaptationRepository.kt:105-112` both explicitly re-throw `CancellationException`).

`skip()` at :106-111 has the identical shape.

**Scenario:** User does the morning check-in, enters this morning's bodyweight (181.4), and taps save.
The `bodyweightRepo.log` write throws (disk pressure, a constraint, or the coroutine is cancelled
because they backed out of the sheet in the same gesture). The sheet closes, the coach's prompt
back-off treats them as "answered today" so they are not asked again, and the weigh-in is gone —
breaking the bodyweight trend line and the readiness signal for that day.

**Fix:** Only advance the UI on success:
`runCatching { ... }.onSuccess { _state.value = ... }.onFailure { if (it is CancellationException) throw it; surface an error }`,
and use the shared `SnackbarController` to tell the user it didn't save. Follow the
`AdaptationRepository.snapshotOrEmpty` precedent of re-throwing `CancellationException` explicitly.

---

## [HIGH] `MainActivity.onCreate` blocks the main thread on five DataStore reads; `ForgeApp.onCreate` copies the database file on the main thread

**File:** `app/src/main/java/com/forge/app/MainActivity.kt:278-289`,
`app/src/main/java/com/forge/app/ForgeApp.kt:39-41, 67-151`

**What:**

```kotlin
val (introIconKey, themedIntro) = runBlocking {
    val privacy = settingsRepo.privacyMode.first()
    val lockEnabled = settingsRepo.appLockEnabled.first()
    ...
    applyAdaptiveWindowBackground(settingsRepo.amoledMode.first())
    settingsRepo.appIcon.first() to settingsRepo.themedLaunchIntro.first()
}
```

Five `.first()` collections of `forgePreferences.data` inside `runBlocking` on the UI thread. On a cold
start this is a real file read (plus DataStore file creation on first ever launch) with the main thread
parked. If the preferences file is unreadable, `DataStore.data` throws and this crashes in `onCreate`
with no recovery. `StrictMode`'s `detectDiskReads` (ForgeApp.kt:185-193) is `penaltyLog()` and
debug-only, so this never surfaces in a release build until it's an ANR report.

Worse, `ForgeApp.onCreate` calls `applyPendingRestore()` (ForgeApp.kt:41) **synchronously**, and that
does `pending.copyTo(staged, overwrite = true)` (ForgeApp.kt:145) for the database, the preferences
file, the avatar, plus `deleteRecursively()` / `renameTo` over the whole progress-photos folder
(:97-116) — all on the main thread before the first frame.

**Scenario:** A user restores a backup. On the next launch, `Application.onCreate` blocks the main
thread copying a multi-megabyte `forge.db` byte-for-byte (plus the photo folder move) before
`MainActivity` even starts, then `MainActivity.onCreate` blocks again on DataStore. On a mid-range
device with a large history this is seconds of frozen UI and a plausible "Avex isn't responding" ANR
right after a restore — the single moment a user is least willing to force-stop.

**Fix:** Hoist the DataStore reads out of `runBlocking`: keep the splash on screen with
`ViewTreeObserver.OnPreDrawListener` / `SplashScreen.setKeepOnScreenCondition` while a `lifecycleScope`
coroutine resolves privacy/lock/amoled/icon, applying them before the first content frame. For
`applyPendingRestore`, prefer `File.renameTo` on the same filesystem (already used for the photo
folder) over `copyTo` so the swap is O(1), and if a copy is genuinely needed, show a "finishing
restore" screen while it runs off the main thread.

---

## [MEDIUM] `RestTimerController`'s mutable fields are written from a Binder thread while its tick job reads them on Main

**File:** `shared/src/main/kotlin/com/forge/app/domain/timer/RestTimerController.kt:47, 50, 53-146`,
`app/src/main/java/com/forge/app/service/wear/SessionTimerHolder.kt:22`,
`app/src/main/java/com/forge/app/service/wear/WearSyncService.kt:35-66`

**What:** `SessionTimerHolder` runs the controller on `Dispatchers.Main.immediate`
(SessionTimerHolder.kt:22). `WearSyncService.onMessageReceived` is delivered on a Data Layer binder
thread and calls `runBlocking { handleTimer(cmd) }` (WearSyncService.kt:44-49), whose body invokes the
controller's plain (non-suspend) `stop()` / `addSeconds(30)` / `start()` **on that binder thread**.
`SetLogUseCase.logFromWatch` likewise calls `timerHolder.controller.start(rest.seconds)`
(SetLogUseCase.kt:146) from the same `runBlocking`.

`endAtMs` (RestTimerController.kt:50) and `tickJob` (:47) are plain `var`s with no `@Volatile`, no
lock, and no dispatcher confinement. `relaunchTickJob()` (:129-146) does `tickJob?.cancel(); tickJob = scope.launch { ... }`
— a non-atomic check-cancel-assign now reachable from two threads. There is no happens-before edge
between the binder-thread write of `endAtMs` and the Main-thread tick loop's read of it in
`remainingNow` (:117).

**Scenario:** User is resting; the phone's tick job is running. They press "+30s" on the watch. The
binder thread mutates `endAtMs` and reassigns `tickJob` while the Main-thread loop is mid-iteration.
Depending on the interleaving the user sees the countdown jump backwards, freeze for a second, or —
if the old tick job wins the `tickJob` slot — two tick loops write `_state` concurrently and the
displayed seconds oscillate. The wrist and phone then disagree about the rest interval.

**Fix:** Confine the controller to one dispatcher: make `start/pause/resume/reset/stop/addSeconds`
either suspend and `withContext(scope.coroutineContext)`, or have them post through
`scope.launch(Dispatchers.Main.immediate) { ... }`. Mark `endAtMs` and `tickJob` `@Volatile` as a
minimum. In `WearSyncService`, replace `runBlocking { ... }` with a `goAsync`-style pattern (or an
explicit `runBlocking(Dispatchers.Main.immediate)`) so the whole handler runs on the timer's own thread.

---

## [MEDIUM] Every `SettingsRepository` flow lacks `distinctUntilChanged`, so one DataStore write re-emits ~100 preference flows and re-runs every combine that touches them

**File:** `app/src/main/java/com/forge/app/data/prefs/SettingsRepository.kt` (whole file — zero
occurrences of `distinctUntilChanged`), consumed by `ui/overview/OverviewViewModel.kt:196-292`,
`ui/gym/train/DayViewModel.kt:134-203`, `ui/settings/SettingsViewModel.kt:166-336`

**What:** `DataStore.data` emits the complete `Preferences` object on *every* write, regardless of
which key changed. All ~100 flows in `SettingsRepository` are `context.forgePreferences.data.map { ... }`
with no dedupe, so a write to any one key makes all of them re-emit an identical value. Three flows in
the codebase get it right (`ProgramCustomizationRepository.kt:73`, `WearStatePublisher.kt:51,92`,
`CardioSessionDetailViewModel.kt:115`), none of them in `SettingsRepository`.

`OverviewViewModel.state` is a chain of ~14 `combine`s over ten settings flows plus DB flows
(OverviewViewModel.kt:196-292), each emission running `buildOverviewUiState`, `withTopLifts` (DB
reads) and `goalsFlow` (which itself calls `goalsWithProgress()` twice — more DB reads,
OverviewViewModel.kt:180-195). `DayViewModel`'s equipment/dislikes/frozen `combine` collector
(DayViewModel.kt:186-203) and its warmup collector (:140-144, which calls `rebuildWarmupProtocol()`)
re-run the same way.

**Scenario:** The freestyle logger autosaves its draft to DataStore every 600 ms while the user types
(`FreestyleLogScreen.kt:290-297` → `SettingsRepository.saveFreestyleDraft`). Home is still within its
`WhileSubscribed(5_000)` window, so each of those writes re-runs Overview's entire combine chain —
goal progress queries, top-lift lookups, weekly stats formatting — roughly twice a second, for a
preference the Overview screen does not read. Same for `markMilestoneShown`, `addSystemNotice`,
`setFirstWorkoutDone` fired mid-session.

Not data loss, but it's continuous background DB work during the two moments the app most needs to be
responsive (logging sets and logging a freestyle workout).

**Fix:** Add `.distinctUntilChanged()` to every derived flow in `SettingsRepository` — cleanest as a
private helper, e.g.
`private fun <T> pref(read: (Preferences) -> T) = context.forgePreferences.data.map(read).distinctUntilChanged()`
— and route all ~100 accessors through it.

---

## [MEDIUM] `_state.value = _state.value.copy(x = <suspending call>)` reverts concurrent edits

**File:** `app/src/main/java/com/forge/app/ui/profile/ProfileViewModel.kt:149, 271`
(also `:249, 257, 279, 289` and the same idiom in `CoachViewModel.kt:192, 228`,
`HealthConnectViewModel.kt:131-217`, `academy/*ViewModel.kt`)

**What:**

```kotlin
photoRepo.revision.collect { _state.value = _state.value.copy(photos = photoRepo.photos()) }
```

Kotlin evaluates the receiver `_state.value` first, then the argument `photoRepo.photos()` — which is
`suspend` and does `withContext(Dispatchers.IO) { readIndex()... }`
(ProgressPhotoRepository.kt:92-95). So the snapshot of state is taken *before* the suspension and the
whole object is written back *after* it. Anything another coroutine wrote to `_state` in between is
silently reverted. `deletePhoto` (:269-272) has the same shape.

More generally, `_state.value = _state.value.copy(...)` is a non-atomic read-modify-write on a
StateFlow; the codebase uses the correct `_state.update { }` in the day-screen files but this
older idiom in ~30 places (9 files).

**Scenario:** User renames themselves in the Profile header (`setUserName` → `settingsRepo.setUserName`
then `_state.value = _state.value.copy(name = trimmed)`, :247-251) at the moment a photo import
completes and bumps `photoRepo.revision`. The revision collector had already snapshotted `_state`
(with the old name) before suspending on `photos()`; it writes back, and the header snaps back to the
old name. The name *is* persisted, so it reappears on the next screen open — which reads as a
mysterious flicker/revert rather than an obvious bug.

**Fix:** Compute suspending values first, then mutate atomically:
`val photos = photoRepo.photos(); _state.update { it.copy(photos = photos) }`. Apply `_state.update { }`
everywhere in place of `_state.value = _state.value.copy(...)`.

---

## [MEDIUM] `WorkoutRepository`'s per-field setters are unsynchronised read-modify-writes on `logged_exercise`

**File:** `app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:530-543`

**What:**

```kotlin
suspend fun setRating(loggedExerciseId: Long, rating: EffortRating) {
    val ex = loggedExerciseDao.get(loggedExerciseId) ?: return
    loggedExerciseDao.update(ex.copy(difficulty = rating))
}
// setSkipped :535-538 and setNote :540-543 are identical
```

Each does SELECT-then-UPDATE-whole-row with no transaction, so two of them running concurrently each
write back a full row built from their own pre-read snapshot; the second overwrites the first's column.
`setSessionSwap` right below (:555-578) *is* wrapped in `database.withTransaction` with a comment
explaining why ("Atomic check-then-write (SM-2)... so a concurrent logSet can't insert a set between
them") — the same reasoning applies here and wasn't carried across. `setSupersetGroup` (:657-658)
correctly uses a targeted single-column DAO update, which is the right shape.

**Scenario:** The note debounce fires `UpdateNote` → `setNote` (reads the row: `skipped = false`), and
before it writes, the user taps SKIP → `setSkipped` (reads the row, writes `skipped = true`). `setNote`
then writes its snapshot back with `skipped = false`. The exercise silently un-skips itself; the day
screen shows it as still to do and it counts against the session's honesty percentage
(`DaySessionHandlers.kt:116-119`).

**Fix:** Use targeted single-column `@Query("UPDATE logged_exercise SET difficulty = :v WHERE id = :id")`
DAO updates (as `setSupersetGroup` already does), or wrap each setter in `database.withTransaction { }`.

---

## [MEDIUM] `WorkoutSessionBridge.timerDone` drops events because the service holds a 2.5 s `delay` inside `collect`

**File:** `app/src/main/java/com/forge/app/service/WorkoutSessionBridge.kt:28, 33`,
`app/src/main/java/com/forge/app/service/WorkoutSessionService.kt:121-141`

**What:** `_timerDone` is `MutableSharedFlow<Unit>(extraBufferCapacity = 1)` — replay 0, buffer 1,
default `BufferOverflow.SUSPEND` — and is published with `tryEmit(Unit)` whose boolean result is
discarded (`notifyTimerDone()`, :33). The single collector in the service does the watch-handoff wait
**inside** the collect lambda:

```kotlin
bridge.timerDone.collect {
    if (restTimerAlertEnabled) {
        val watchReachable = wearConnection.reachableWearNodeId() != null
        if (watchReachable) { delay(HAPTIC_ACK_GRACE_MS /* 2_500 */); ... }
        if (settingsRepo.isQuietNow()) vibratePhone() else postTimerDoneNotification()
    }
}
```

While the collector is parked in that 2.5 s delay (plus the `isQuietNow()` DataStore read), the buffer
holds at most one event; a further `tryEmit` returns `false` and the event is dropped on the floor with
no diagnostic.

**Scenario:** User is doing a short-rest circuit with a paired watch. Two rest timers expire inside the
handoff window (a skipped timer immediately restarted, or a phone-timer expiry landing while the
previous one is still in its grace period). The second "rest done" buzz/notification never fires — the
user is left waiting on a cue that will not come, which is precisely the failure the foreground service
exists to prevent.

**Fix:** Take the event out of the critical section — `bridge.timerDone.collect { serviceScope.launch { handleTimerDone() } }` —
or give the flow `extraBufferCapacity` with `BufferOverflow.DROP_OLDEST` and move the grace delay into
its own coroutine. Either way, stop discarding `tryEmit`'s return value.

---

## [MEDIUM] `ProgramChangeGuard.stagedAction` is a single unsynchronised slot; a second staged change silently discards the first

**File:** `app/src/main/java/com/forge/app/ui/common/ProgramChangeGuard.kt:32, 42-53, 56-61`

**What:** `private var stagedAction: (suspend () -> Unit)? = null` is a plain field on a `@Singleton`
shared across every ViewModel that mutates the program (`OverviewViewModel.applyCoach`
OverviewViewModel.kt:398-401, `SettingsViewModel.rerollProgram`, the day re-roll paths). `run()`
overwrites it unconditionally, and `confirm()` reads-then-clears it without any lock. There is one
`_pending` dialog for what is effectively an unbounded queue.

**Scenario:** With a workout in progress, the user taps "Generate deload week" on Home's coach card
(staged, dialog appears), the dialog is dismissed by a recomposition/back gesture without `cancel()`
running, and they then trigger a program re-roll from Settings. `stagedAction` now holds the re-roll.
Confirming "Discard & continue" applies the re-roll — the deload the user actually asked for never
happens, and the in-progress workout is discarded for the wrong action.

**Fix:** Make `stagedAction` a `MutableStateFlow<(suspend () -> Unit)?>` updated atomically alongside
`_pending`, and have `run()` refuse (or replace explicitly and tell the user) when something is already
staged. Guard `confirm()` with `getAndUpdate { null }` so it can't double-run.

---

## [MEDIUM] Everything the day screen holds only in memory is lost on process death, including the rest timer, while the session stays "active"

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DayViewModel.kt:66-99`,
`ui/gym/train/state/DayUiState.kt`, `service/wear/SessionTimerHolder.kt:20-24`,
`ui/gym/train/DayViewModelRefresh.kt:192-198`, `ui/gym/train/DaySessionHandlers.kt:224-234`

**What:** `DayViewModel` takes `SavedStateHandle` but reads only the route arguments from it
(DayViewModel.kt:53-61) and never writes to it. Everything else lives in a plain `MutableStateFlow`
that dies with the process: `bonusSets`, `finishedEarly`, the manual `MoveExercise` ordering,
`warmupChecked` / `warmupReactions`, `isWarmupComplete`, `undoableSetId`, `openRestEvent`
(DayViewModel.kt:84), `dislikePromptSuppressedThisSession` (:88), and the rest timer itself
(`RestTimerController`'s state is in-memory only — RestTimerController.kt:44, held by an app-scoped
singleton, never persisted).

Logged *sets* do survive (they're in Room and `beginSessionForThisDay` resumes the session), so this
isn't set loss. But `stopSessionService()` (DayViewModelRefresh.kt:192-198) is called by
`leaveAndResume` (DaySessionHandlers.kt:224-234) — i.e. leaving the screen while deliberately keeping
the session ACTIVE tears down the foreground service, removing the very thing that was keeping the
process alive, which makes the kill much more likely.

**Scenario:** User logs a set, the 2:30 rest timer starts, they leave the day screen to check Stats
("Resume later"), the foreground service stops, and Android reclaims the process while the phone is in
their pocket. Returning to the workout: the rest timer is gone (no countdown, no buzz), the exercise
they'd marked "done early" is back in the queue, their manual reorder is gone, and the warmup gate may
re-show. The `openRestEvent` loss also means the interval that was being measured is never written, so
the personal rest-tuning model (`RestAdvisor`, DayViewModel.kt:158-168) silently loses a sample.

**Fix:** Persist the rest timer's `endAtMs` + `totalSeconds` + paused flag (DataStore is enough — it's
wall-clock anchored already, so it reconstructs exactly) and restore it in `SessionTimerHolder`. Mirror
the small per-session UI facts (`finishedEarly`, `bonusSets`, order, warmup completion) into
`SavedStateHandle`. Keep the foreground service running whenever an active session row exists, rather
than tying it to the day screen's lifetime.

---

## [MEDIUM] Settings writes run in `viewModelScope` and race the navigation that follows them

**File:** `app/src/main/java/com/forge/app/ui/settings/SettingsViewModel.kt:347-420` (~50 one-liners
of the form `fun setX(v) = viewModelScope.launch { settingsRepo.setX(v) }`)

**What:** Every settings mutation is a bare `viewModelScope.launch { <DataStore edit> }`. The
`SettingsViewModel` is obtained with `hiltViewModel()` from the settings destination
(SettingsScreen.kt:162), so popping that destination clears the ViewModel and cancels any `edit { }`
still in flight. `DataStore.edit` suspends across an actor hop plus a file write + fsync + rename, so
the window is real (and widest on the first write after launch, when the file is created).

The team has already encountered and fixed this exact shape once — `DayScreen.kt:314-320`:
> "Fold the journal into DismissSummary so it's written in the same coroutine that runs before
> PopBack — a separate UpdateJournal event raced the VM clearing and could be lost."

and `GoalsViewModel.kt:117-176` wraps every mutation in `withContext(NonCancellable)` for the same
reason. Settings did not get the treatment.

**Scenario:** User opens Settings → Session, drags "Compound rest" to 180 s, and immediately swipes
back. `setRestCompoundSeconds` is mid-`edit`; the destination pops, the VM is cleared, the write is
cancelled. The user returns later to find rest still at 150 s, with no indication the change didn't
take. Same for the accent colour, unit, and every toggle.

**Fix:** Either wrap the repository call in `withContext(NonCancellable)` (the `GoalsViewModel`
precedent), or move `SettingsRepository` writes onto an application-scoped `CoroutineScope`
(`ForgeApp.appScope` exists at ForgeApp.kt:34) since a preference write is app state, not screen state.

---

## [LOW] `NoteField` instantiates the full `SettingsViewModel` from inside the day screen's exercise card

**File:** `app/src/main/java/com/forge/app/ui/gym/train/components/NoteField.kt:50, 56`

**What:** `settingsViewModel: SettingsViewModel = hiltViewModel()` is a default parameter on
`NoteField`, and it is only used to read `settingsState.noteTemplates` — while `showTemplates = false`
at the one call site (ExerciseCardComponents.kt:441), so the templates aren't even rendered.
`hiltViewModel()` resolves against the day screen's `NavBackStackEntry`, constructing the 793-line
`SettingsViewModel` there and starting its `stateIn(WhileSubscribed)` collection of ~15 DataStore flows
plus DB flows (SettingsViewModel.kt:166-336) — inside a live workout, and (per the
`distinctUntilChanged` finding above) re-running on every unrelated preference write.

**Scenario:** Opening any exercise card's NOTE panel during a workout spins up the entire Settings
state graph on the session screen. Not incorrect, but it is background query work on the one screen
that must stay responsive between sets.

**Fix:** Pass `noteTemplates: List<String> = emptyList()` in as a parameter from a caller that already
has it, and drop the `hiltViewModel()` default.

---

## [LOW] Non-atomic read-modify-writes on shared `StateFlow`s in singletons

**File:** `app/src/main/java/com/forge/app/ui/common/ArrivalController.kt:65-71, 74-76`,
`app/src/main/java/com/forge/app/service/wear/WearFocusHolder.kt:26-31`

**What:** Both mutate a `MutableStateFlow` with `_x.value = _x.value.<derive>` rather than
`_x.update { }`:

```kotlin
// ArrivalController.enqueue
val known = _queue.value.map { it.noticeId }.toSet()
val fresh = arrivals.filter { it.noticeId !in known }
_queue.value = _queue.value + fresh          // read-modify-write

// WearFocusHolder.markEarlyDone
_earlyDone.value = _earlyDone.value?.takeIf { ... }?.let { it.copy(slotIds = it.slotIds + slotId) }
    ?: EarlyDone(sessionId, setOf(slotId))
```

Both are today driven from Main-confined callers, so an interleaving is unlikely — but both are
`@Singleton`s reachable from any scope, and `WearFocusHolder` is read from a Binder thread
(`SetLogUseCase.kt:88` / `WatchSessionMirror.kt:89`, both reached via `WearSyncService`'s
`runBlocking`), so the invariant is not enforced by construction.

**Scenario:** Two arrivals enqueued from different coroutines in the same frame → one banner is
dropped and its notice is never marked announced, so it re-announces on the next launch. For
`WearFocusHolder`, a `FinishExerciseEarly` landing while the wrist reads `earlyDoneFor` could drop a
slot mark, letting the wrist pin to an exercise the phone already filed as done.

**Fix:** Use `_queue.update { }` / `_earlyDone.update { }`. Mark `WearFocusHolder`'s reads as the
cross-thread accesses they are.
