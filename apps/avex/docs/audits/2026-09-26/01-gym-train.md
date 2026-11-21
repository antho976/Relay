# Gym Train UI (live session screen)

Scope: `ui/gym/train/**` (incl. `components/`, `state/`). 38 files, 9,205 lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/ui/gym/train/` unless noted.

**Counts:** P1 0 · P2 7 · P3 20 · P4 6. Three P2s are still open from `docs/audits/2026-09-12/gym-ui.md` (G1, G2, G4).

Checked and already fixed, not re-reported: BUG_SCAN C-1, C-2, C-3; Room never observed; U-1; U-2 (comma decimals); `formatPlateCount` locale; stopwatch unattended-run ceiling after process death; the "N for PR" hint.

## P2

### [P2] False "New personal record" celebration on every reopen of an in-progress session
- Category: bug
- Location: `DayScreen.kt:74-93`
- Problem: a new VM always composes first with `exercises` empty (the session read suspends), so the first `LaunchedEffect` records baselines of 0 sets / 0 PR sets. When resumed sets land, `totalPrSets` goes 0 → N and the whole PR celebration fires (haptic, confetti, TalkBack "New personal record!"). *Re-checked by the coordinator.*
- Scenario: log a PR, Leave → Resume later, reopen the day (or after process death). A resumed session without a PR fires the SET_LOGGED haptic instead. Rotation is unaffected (VM retained).
- Fix: only set baselines once `!state.isLoading`; better, emit a one-shot `PrLogged` effect from `logSet`.
- Confidence: high

### [P2] UP NEXT delta pill: unit-less pounds, raw Double text, plates shown in pounds, wrong exercise
- Category: bug
- Location: `DayViewModelBuilders.kt:268-276`; `DaySessionContent.kt:319-323`; `components/ExerciseCardComponents.kt:506-515`
- Problem: `"$sign$abs"` of `suggestedDeltaLb` is a pound value with no unit. A kg user at 50 kg (110.2 lb stored) whose target is `floorToGrid(112.7, 2.5) = 112.5` sees "+2.299999999999997 ↑". On PLATES exercises it shows "+15" (pounds) not "+1 pl". The delta is taken from list index `idx+1` while the name comes from `nextEx` (next *incomplete*), so they diverge once the next list item is done. Violates DESIGN §11. *Re-checked by the coordinator.*
- Fix: carry the raw Double and format at render with `formatWeightDelta(delta, weightUnit)` (plate count for PLATES), read from `nextEx?.suggestedDeltaLb`. Delete `annotateNextExerciseDeltas` and `nextSuggestedWeightDelta`.
- Confidence: high

### [P2] Last-slot advance finishes the workout with earlier exercises incomplete — still open from gym-ui.md G1
- Category: bug
- Location: `DaySessionContent.kt:242-245, 258-266`
- Problem: `exNextId` only looks at later, non-skipped exercises; it ignores `isComplete` and never wraps. Jump to the last exercise from Up Next, log it, and FINISH WORKOUT dispatches `FinishWorkout` with earlier exercises incomplete. "MOVE TO NEXT" can land on an already-complete exercise.
- Fix: use `upcoming.firstOrNull()` (lines 214-218, which already wrap) for both the target and the MOVE/FINISH label.
- Confidence: high

### [P2] Raw stored-lb "last" text and hardcoded "WEIGHT · LB" — still open from gym-ui.md G2, wider than reported
- Category: bug
- Location: `components/ExerciseCard.kt:215-216, 302`; `components/ExerciseCardComponents.kt:119-122`
- Problem: a kg user sees "last 110.2 × 8" above a field labelled KG; the table header is hardcoded LB. `CollapsedRow` is live in the expanded Up Next list (`:523`) and DONE/SKIPPED list (`DaySessionContent.kt:353`). Timed holds render "last BW × 0".
- Fix: shared `lastSetLabel()` (formatWeight / plate count / `formatHoldLabel`), modelled on `SetRow.kt:384-394`; header from `unitLabel(weightUnit)`.
- Confidence: high

### [P2] Set-entry fields have no accessible label — still open from gym-ui.md G4
- Category: release (a11y)
- Location: `components/SetInputRow.kt:769-807` (BigNumberField, used at `:290`, `:369`)
- Fix: label parameter → `semantics { contentDescription = "Weight in kg, set N" }`.
- Confidence: high

### [P2] Double tap on a leave/finish action sends two PopBacks and pops the hub → blank screen
- Category: bug
- Location: `DaySessionHandlers.kt:180-193, 195-207, 209-219, 225-235, 247-249`; `DayScreen.kt:129-133`; `ui/nav/ForgeNavHost.kt:305`
- Problem: COMPLETE, Discard, Resume later and cross-day Resume have no re-entrancy guard (unlike `finishJob`, `swapsInFlight`, `logSetsInFlight`). The trigger stays tappable until DB writes finish; the CrossDay dialog stays up through the exit transition (`crossDaySession` never cleared). Two taps → two PopBacks → the second `popBackStack()` empties [OVERVIEW, gymDay] and the NavHost renders nothing. COMPLETE also writes the journal twice.
- Fix: one `leaving`/`exitJob` guard in the VM; clear dialog state synchronously; pop only if the current entry is still this one (`dropUnlessResumed`).
- Confidence: medium

### [P2] Back while "Loading session…" races the session start
- Category: bug
- Location: `DayViewModel.kt:153-166`; `DaySessionHandlers.kt:195-207, 252-292`; `DayViewModelRefresh.kt:215-218`
- Problem: BackHandler is active while loading. `requestBack` skips the discard when `sessionId` is null; the untracked begin coroutine keeps running until `onCleared` and can still create/resume the session and call `startSessionService()`. Nothing calls `endSession()`, and `WorkoutSessionService.kt:105-107` only stops when bridge state goes null. Result: an orphan "in progress" notification, an empty active session that later triggers the cross-day prompt, and the next open skips warmup because it counts as a resume.
- Fix: hold the start as `beginJob` and `cancelAndJoin()` it in leave/discard/back paths; guard `startSessionService` with `!leaving`, or disable back until loaded.
- Confidence: medium

## P3

### [P3] `refreshExercises` publishes a pre-suspension snapshot, reverting concurrent edits
- Location: `DayViewModelRefresh.kt:36-39, 74`
- Problem: in-memory `bonusSets`, `finishedEarly`, expanded state and order are captured before ~7×N DB reads, then the whole list is replaced (the same pattern `refreshExercise` fixed at `:112-119`). Triggered by the wrist observer, readiness landing, swaps, adding an exercise, clearing a swap. A "Done with this exercise" or "+ SET" tapped during a rebuild reverts; a phone-logged set vanishes for ~1.5 s.
- Fix: merge in-memory fields from `s.exercises` inside `_state.update`; serialise full rebuilds.
- Confidence: medium

### [P3] Readiness/deload scaling can be missed on the first build
- Location: `DayViewModel.kt:244-251`; `DayViewModelBuilders.kt:133-137`
- Problem: readiness (incl. a Health Connect IPC) and block phase load in parallel; on landing, the code refreshes only if `exercises` is non-empty. If they land after the builds read `readiness` but before the list is published, every card keeps an unscaled suggestion (full load in a deload week) and `recordSuggestionOutcome` records it.
- Fix: await readiness as a Deferred (with timeout) before building, or refresh again when it lands mid-build.
- Confidence: medium

### [P3] "Apply →" on the ordering suggestion keeps the old first exercise focused
- Location: `DaySessionContent.kt:123-130`; `DaySessionHandlers.kt:72-78`; `DayViewModelRefresh.kt:190-213`
- Problem: `shownExerciseId` is re-pointed only when null or when its exercise disappears, so after Apply the card still shows the old first exercise, now "EXERCISE 03 / 06". The silent auto-apply usually hits the same.
- Fix: re-point while no sets are logged, or hold the shown exercise in `DayUiState` and set it in `applyOrderedExercises`.

### [P3] Ghost duel never credits a timed hold
- Location: `state/DayUiState.kt:181-208`
- Problem: holds store reps = 0 + `durationSeconds`; `beatsPriorSet` compares weight and reps only (`0 > 0`). A plank session reads "0 / 3 sets" and feeds `SessionOpinion`.
- Fix: compare `durationSeconds` first when either set is a timed hold.

### [P3] Chart sheet "PERSONAL BESTS" covers only the last 8 sessions
- Location: `components/ExerciseChartSheet.kt:92-108`; `DayViewModelBuilders.kt:89, 165-175`
- Problem: best/heaviest come from `sessionAggregatesForExercise(limit = 8)`; the real all-time best (`pbDeferred`) is fetched and never shown. `durationMin` is wall-clock finish − start, so a session resumed next day reads "1440 min".
- Fix: pass all-time values into the sheet (or rename the heading); use `active_seconds`.

### [P3] Silent discard on back trusts UI state and can delete watch-logged sets
- Location: `DaySessionHandlers.kt:195-206`; `state/DayUiState.kt:142-149`; `DayViewModel.kt:185-195`
- Problem: wrist sets reach the UI only after a 1.5 s debounce plus a rebuild; a back press inside that window CASCADE-deletes them without a prompt. OpenSwapPicker already checks the DB for this reason (SM-5).
- Fix: check the DB set count before discarding silently.
- Confidence: medium

### [P3] Set-logged Undo hard-capped at 5 s (same root as R2 in RELEASE_AUDIT_2026-09-12)
- Category: a11y
- Location: `DayExerciseHandlers.kt:368-377`; `DayScreen.kt:121-125`
- Problem: clearing `undoableSetId` after 5 s re-keys the LaunchedEffect and cancels `showSnackbar`, overriding accessibility-extended timeouts. The screen runs a private SnackbarHost against DESIGN §8.
- Fix: clear the id from the snackbar result, not a timer.

### [P3] Whole-card `combinedClickable(onClick = {})`
- Category: a11y / bad-code
- Location: `components/ExerciseCard.kt:105-121`
- Problem: blank-area taps ripple the whole card for nothing; TalkBack merges all card text into one "button"; nested tap targets (§14); the long-press label still says "set the rest timer" though that menu row was removed (`DayScreen.kt:252-256`).
- Fix: long-press on the name, or a custom accessibility action; fix the label.
- Confidence: medium

### [P3] Touch targets under 48 dp on the live screen
- Category: a11y
- Location: `DaySessionContent.kt:190-198` (Apply/dismiss), `:444` (FINISH ~30 dp); `ExerciseCardComponents.kt:330` (NOTE/VIDEO/SWAP/SKIP chips ~23 dp), `:398-405`, `:151`, `:159` (44 dp); `RestTimerBubble.kt:313-354` (chips ~28 dp); `SetRow.kt:290-295, 511-523`; `SetInputRow.kt:691, 749-757` (40 dp)
- Problem: `clickableLabeled` adds no minimum size.
- Fix: `heightIn(min = 48.dp)` or `minimumInteractiveComponentSize()`.

### [P3] Canvas marks and clickables missing descriptions
- Category: a11y
- Location: `ExerciseChartSheet.kt:205`, `ExerciseCardComponents.kt:300` (Canvas, no description); `:254` (strip that opens the chart), `:492` (Up Next toggle, no expanded state), `ExerciseCard.kt:185` (name opens swap), `RestTimerBubble.kt:203-212` (long-press "+30 s") have no label; `:332` ActionChip reads "NOTE, NOTE"; `:373` "Open the rest timer" is mislabelled (next finding).
- Fix: value-reading descriptions; `clickableLabeled`.

### [P3] Tapping the inline rest timer permanently changes that exercise's default rest
- Category: bad-code / UX
- Location: `ExerciseCard.kt:344`; `DaySessionContent.kt:101-110, 304`
- Problem: the tap opens RestTimerSetterDialog, whose save writes a persistent customization override; the running rest is untouched.
- Fix: send `RestTimerOpen` on tap; move "default rest for this exercise" into the controls dialog.
- Confidence: medium

### [P3] Finished rest bubble recomposes every frame until the next set
- Category: perf
- Location: `RestTimerBubble.kt:125-137`
- Problem: the infinite pulse is read in composition while the screen is kept on.
- Fix: read `appear` and the pulse inside `graphicsLayer {}`.

### [P3] Rest-timer controls dialog doesn't wrap or scroll at 200% font — still open (gym-ui.md follow-up)
- Category: a11y
- Location: `RestTimerBubble.kt:264-356` (Row at `:311`)
- Fix: FlowRow for chips; `verticalScroll`.
- Confidence: medium

### [P3] Any non-blank weight text is accepted; a typo logs a weightless set
- Location: `SetInputRow.kt:229-230, 250-253, 373`
- Problem: `KeyboardType.Text` + `canSubmit = isNotBlank`. "6O" stores `weightLb = null`: 0 volume, never a PR, silently.
- Fix: require the text to parse (`parseToLb`, or WeightParser for PLATES) or be "bw".
- Confidence: medium

### [P3] Warmup ramp is generated but never shown
- Category: dead-code / bug
- Location: `WarmupFlow.kt:63`; `DayWarmupBuilder.kt:16-73`; `domain/warmup/WarmupEngine.kt:281`
- Problem: WarmupFlow renders only `filterIsInstance<WarmupDrill>()`; the `WarmupRampSet` steps and their load math are discarded.
- Fix: render the ramp rows, or drop the ramp work.

### [P3] Step-mode note and suggestion reasons never render
- Category: dead-code
- Location: `DayViewModelBuilders.kt:193-204`; `ExerciseCard.kt:190-196, 357-358`; `SetInputRow.kt:577-589`
- Problem: `suggestionReason` reaches only SetInputRow's unreachable legacy branch; the comment claiming it "shows on card + input row" is false.
- Fix: render one short aside, or delete the plumbing.

### [P3] Rotation loses the focused exercise, typed input and open sheets
- Location: `DaySessionContent.kt:77-78, 123`
- Problem: `shownExerciseId`, `chartForExerciseId`, `restTimerSetterForId` use plain `remember`; after a jump-ahead + rotation the screen returns to the first incomplete exercise, and the saveable input (keyed under the other exercise's AnimatedContent) is lost.
- Fix: `rememberSaveable`, or VM state.

### [P3] FINISH at zero sets commits an empty workout without confirmation
- Location: `DaySessionContent.kt:444`; `DaySessionHandlers.kt:80-104`; `data/repo/WorkoutRepository.kt:596-625`
- Problem: a finished 0-set session is written to history, counts toward rotation, evaluates trophies, mirrors to Health Connect and runs `setFirstWorkoutDone()`.
- Fix: route to discard when there's no unsaved work, or confirm.
- Confidence: medium (path certain, intent not)

### [P3] Unreachable Train tab still runs work — still open (gym-ui.md cleanup #1)
- Category: dead-code
- Location: `DayListScreen.kt:105-359`; `components/DayCard.kt`; `components/DayCardComponents.kt`; `DayListViewModel.kt`; `state/DayListUiState.kt`; `ui/nav/HubScreen.kt:133-149`
- Problem: the only caller passes `initialTab = 1`, yet the default `hiltViewModel()` builds DayListViewModel on the Stats page, whose init runs a `recentRestEvents()` collection loop for nothing.
- Fix: thin StatsScreen wrapper; delete the rest.

### [P3] ~20 DayUiEvents are never dispatched — still open (domain.md, gym-ui.md cleanup #2)
- Category: dead-code
- Location: `state/DayUiEvent.kt`; `DayExerciseHandlers.kt:20-24, 90-111, 135-155, 171-193, 216-231`; `DaySessionHandlers.kt:34-52, 163-178`; `DayWarmupHandlers.kt:22`; `DayScreen.kt:279-318`; `DayDialogs.kt:93-124`; `components/TrainingHelpers.kt`
- Never dispatched (grepped app/wear/shared/tests): ToggleExpanded, SkipWarmup, SetSessionType, SetUntracked, SetIntensity, ConfirmPreSessionPicker, WarmupReaction, MoveExercise, ToggleAmrap, ToggleAssisted, ToggleFailure, SetSetType, SetDropAnnotation, SetUseKg, SetSupersetGroup, ShowWarmupSuggester, ShowPlateCalculator, LogBreak, SaveAndExit. Wired only to never-invoked callbacks: OpenGoalSetter, SetExerciseUnit, ToggleSetDifficultyTag. Unreachable as a result: GoalSetter, WarmupSuggester, PlateCalculator dialogs, `isExpanded`, intensity pick.
- Latent bugs inside the dead code: MoveExercise `removeAt(-1)` / `coerceIn(0, -1)` throw on an empty list (`:147-155`); SetExerciseUnit(null) clears then recreates the swap (`:104-106`); PlateCalculator treats display-unit `prefillWeight` as pounds (`DayScreen.kt:315`).
- Fix: delete, or wire and fix.

### [P3] Unread fields cost DB reads per card per set log
- Category: dead-code / perf
- Location: `DayViewModelBuilders.kt:52, 103-104, 149-163, 225, 246, 249-252`; `state/DayUiState.kt:13, 151-178, 315-321`
- Never read: lastSessionPreviewText, allTimePbText, vsLastStatus (+ VsLastStatus enum), suggestedRepsReason, goalProgressFraction, goalWeightLb (a `goalRepo.get` per card, read only by the dead GoalSetter), canSkipWarmup, nextUpExerciseName, sessionProgressText, remainingSetsCount.
- Fix: delete.

## P4

- **dead-code — unused parameters/branches:** ExerciseCard `onToggle` + collapsed branch, `onMoveUp`, `onMoveDown`, `onOpenGoalSetter`, `onSetExerciseUnit`; SetRow `onToggleDifficultyTag`, `onToggleAmrap`, `onToggleAssisted`, `onToggleFailure`, always-true branch at `:237`; SetInputRow legacy branch `:574-626` + `suggestedWeight`/`suggestionReason`; DayScreen `dayKey` (hidden by `@Suppress`); DismissSummary mood/tags always empty, so MoodPrompt, MoodChip, JournalField, TagPicker, SESSION_TAGS are dead (still open, gym-ui.md cleanup #3).
- **release:** `DUMMY_TRAINING_DATA` TEMP fixtures in main (`DayViewModelBuilders.kt:24-29, 177-218`). Delete.
- **perf:** the rest tick recomposes all of DayContent every second: new lists and a deep `ex.copy` per tick, `swapCandidates` recomputed while the sheet is open (`DaySessionContent.kt:214-219, 249, 252, 334-336`; `DayScreen.kt:206-212`). `remember(state.exercises)`; pass the timer as its own State.
- **bad-code — doctrine debt:** em dashes in rendered strings (`DayScreen.kt:238`, `DayDialogs.kt:74, 106`, `DaySwapHandlers.kt:17, 193`, `ExerciseChartSheet.kt:143`); call-site `fontSize` throughout components (frozen in the DesignDoctrineTest baseline).
- **bad-code — minor:** `DayWarmupHandlers.kt:25, 31` use `LocalDate.now()` instead of the injected clock; `DayViewModel.kt:214-233` samples `restTuning` once while rest bases are live (DayListViewModel fixed the same mismatch); `AddExerciseSheet.kt:29-45` documented as "full-catalog" but lists only the program's exercises; `SessionSummarySheet.kt:81-86` uses the raw haptic API instead of `forgeHaptic`; LeaveSessionDialog copy (`DayDialogs.kt:31`) says sets are saved even with zero sets; `var prevTotalSets = remember {…}` (`DayScreen.kt:76-77`) should be `val`.

## Files reviewed (38)
`ui/gym/train/`: DayDialogs, DayExerciseHandlers, DayListScreen, DayListViewModel, DayScreen, DaySessionContent, DaySessionHandlers, DaySwapHandlers, DayTimerHandlers, DayViewModel, DayViewModelBuilders, DayViewModelRefresh, DayWarmupBuilder, DayWarmupHandlers, RestTimerHapticCues;
`components/`: AddExerciseSheet, DayCard, DayCardComponents, DifficultyRater, DislikeSwapPromptDialog, ExerciseCard, ExerciseCardComponents, ExerciseChartSheet, NoteField, RestTimerBubble, SessionSummaryComponents, SessionSummarySheet, SetInputRow, SetRow, SetTable, SwapPickerSheet, TrainIcons, TrainingHelpers, WarmupFlow;
`state/`: DayListUiState, DayUiEvent, DayUiState, SessionSummary.
