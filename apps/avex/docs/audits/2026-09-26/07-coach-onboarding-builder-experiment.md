# Coach UI, onboarding, program builder, experiment

Scope: `ui/coach/**`, `ui/onboarding/**`, `ui/programbuilder/**`, `ui/experiment/**`. 33 files, 8,677 lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/ui/` unless noted.

**Counts:** P1 0 · P2 2 · P3 12 · P4 17.

Corrections the agent made after checking callers: the plate-weight default dropped on skip is P4, not P2 (it only affects the curated plate-count machine exercises); builder reordering doesn't mis-attribute history (history is keyed by library id).

## P2

### [P2] Advanced-tracking pop-up reappears as soon as the user turns advanced tracking off
- Category: bug
- Location: `coach/CoachScreen.kt:188-195, 192, 294-296`; `coach/CoachViewModel.kt:177-179`; `data/prefs/SettingsRepository.kt:1003-1004` (default 0L)
- Problem: "Turn on" writes only `coachAdvanced = true`; `advancedPromptAfter` stays 0 and visibility is `!advanced && now >= promptAfter`. Tap Turn on, then "Hide advanced tracking →" at the page foot: the offer slides straight back in. Same on the next visit after turning it off in Settings → Coach. Contradicts `CoachAdvancedPrompt.kt:52-53` and the pref's KDoc.
- Fix: in `setAdvanced(v)`: `if (v) settingsRepo.setCoachAdvancedPromptAfter(Long.MAX_VALUE)`; same in Settings' setter.
- Confidence: high

### [P2] Program builder: Undo tapped after Save restores the removed day only in the viewer; Back then discards it silently
- Category: bug
- Location: `programbuilder/ProgramBuilderViewModel.kt:326-352` (`save` never clears `undoRemoval`), `:303-306`; `programbuilder/ProgramBuilderScreen.kt:100-106, 110, 115-117`
- Problem: View program → pencil → Remove day → Save within the ~4 s snackbar. The viewer shows with the snackbar still up; tapping Undo re-inserts the day in memory (`dirty = true`) and the viewer shows it as part of the program; Back → `attemptClose` → `onClose()` with no prompt. The day isn't in the saved program. Same for exercise removal.
- Fix: on successful save, `undoRemoval = null` and dismiss the snackbar; make `undoRemove` a no-op when not editing.
- Confidence: high

## P3

### [P3] Coach "Connect" pill appears for connected Health Connect inputs, with stale charts under it
- Location: `coach/CoachStand.kt:332-333, 365, 199-206`; `data/repo/CoachRepository.kt:930-940` (`active` = readings in last 14 days); `data/health/HealthConnectManager.kt:485-525` (42-day snapshot)
- Problem: HC granted, watch not worn for 3 weeks: Sleep shows "Connect" (Settings then says already connected) and draws "LAST 14 NIGHTS" from nights 3–6 weeks old. Resting HR same.
- Fix: a real `connected` flag on `RecoverySignal`; "none recently" when connected but empty; gate charts on in-window samples.

### [P3] Resting-HR baseline drawn on a different y-scale from the sparkline
- Location: `coach/CoachCharts.kt:327-342` vs `:70-74`
- Problem: baseline mapped over min/max of values + baseline; sparkline over values alone. Recent 60–66 with baseline 55 puts the line on the 60 bpm low, so elevated HR reads "at baseline" — the fatigue case the chart exists for.
- Fix: `domainMin/domainMax` params on `CoachSparkline`, same range for both.

### [P3] `runAndRefresh` writes back a stale snapshot
- Location: `coach/CoachViewModel.kt:223-228` (the file warns against this at `:240-241`, `:320-322`)
- Problem: `s = _state.value`, three slow reads, `_state.value = s.copy(...)`. Apply fires `EngineInputSignals` so `load()` runs concurrently; toggling advanced or Start/End block during the refresh is reverted, and a stale `blockBusy = true` leaves the block action dead.
- Fix: read first, then `_state.update { it.copy(brief = b ?: it.brief, watch = w ?: it.watch, timeline = t ?: it.timeline) }`.
- Confidence: high on mechanism, medium on frequency

### [P3] Undo can silently do nothing; failures are swallowed
- Location: `coach/CoachAccount.kt:333-341`; `coach/CoachViewModel.kt:204, 216-220`; `data/repo/CoachRepository.kt:763-775`
- Problem: the repo returns false when a newer decision owns the slot (LIFO) or the window expired; the UI still shows Undo and the VM ignores both result and exceptions. `now` is frozen at load (`CoachScreen.kt:117`), so the pill can outlive `undoExpiresAt`.
- Fix: a one-shot "couldn't undo: <reason>" event; compute `undoable` in the repository incl. LIFO.

### [P3] Skip setup overwrites goal and experience the user already answered
- Location: `onboarding/OnboardingScreen.kt:416-424`
- Problem: Custom → "Get stronger" → "New to lifting" → skip stores build_muscle/intermediate, steering rep and volume suggestions; the dialog doesn't say so.
- Fix: `goal.ifEmpty { "build_muscle" }`, `experience.ifEmpty { "intermediate" }`, pass `plateWeightLb`.

### [P3] Builder saves empty programs and empty days
- Location: `programbuilder/ProgramBuilderScreen.kt:213-220`; `programbuilder/ProgramBuilderViewModel.kt:326-347`; `data/repo/ProgramRepository.kt:114-128`
- Problem: Add day → Save persists a zero-exercise live training day; removing every day → Save wipes the program, customizations and coach swaps and folds coach deltas with no confirm.
- Fix: disable Save with a hint for no days / empty days, or confirm.
- Confidence: medium

### [P3] Accent-coloured text actions fail contrast and contradict CoachAction's rule
- Category: bad-code / a11y
- Location: `coach/CoachBlock.kt:79`; `coach/CoachAccount.kt:107-111`; rule at `coach/CoachUi.kt:257-260`, `coach/CoachScreen.kt:292-293`
- Problem: "Start a block →" and "Apply all N →" render at 2.3–3.4:1 on Olive, Gold and Navy accents, and use the navigation-link style for do-it-now actions.
- Fix: `c.onBg`.

### [P3] Touch targets under 48dp
- Category: a11y
- Location: `coach/CoachUi.kt:262-277` (`CoachAction` ≈ 36dp); `coach/CoachAccount.kt:337-340` (Undo ≈ 28dp); `onboarding/OnboardingPrimitives.kt:190-199` (SkipLink ≈ 40dp)
- Problem: Skip on call tiles, Undo, Ignore, Apply all, Start/End block, Hide advanced tracking and onboarding's skip.
- Fix: `minimumInteractiveComponentSize()`, or make the row the target.

### [P3] Watch countdown uses elapsed ms; the watcher uses calendar days
- Location: `coach/CoachAccount.kt:39-40, 309-326`; `domain/coach/OutcomeWatcher.kt:43, 63-72`; `domain/coach/CoachOutcome.kt:23-31`
- Problem: applied Monday 20:00 → Tuesday 08:00 shows "14 days left" while the watcher counts 13; later "1 day left" on the morning the watcher closes. CoachOutcome fixed this once already.
- Fix: a shared calendar-day `daysLeft()`; use `OutcomeWatcher.WINDOW_DAYS`.

### [P3] `ui/experiment` ships in release; header and design-test exemptions are stale
- Category: release
- Location: `experiment/SurfaceKit.kt:60-85`; `app/src/test/java/com/forge/app/ui/DesignDoctrine.kt:304-350`
- Problem: no flag, always on; imported by OverviewScreen, HomeSurfaceCards, ProfileScreen, ProfileSurfaceSections, ProfileExtras, GoalsComponents, FreestyleStartParts. The header claims only Home and Profile use it and "deleting restores the app"; the branch-scoped doctrine exemptions (raw colour, alpha ladder, maxLines) persist after the merge.
- Fix: move used primitives into `ui/common` with DESIGN.md entries and tokens, delete the exclusion block; at minimum rename the package and fix the header.

### [P3] Dead goal/project surface (~330 lines)
- Category: dead-code
- Location: `coach/CoachViewModel.kt:40, 43, 85-99, 231-251, 293-335`; `coach/GoalPickerDialog.kt` (whole file); `coach/CoachScreen.kt:202-205`
- Problem: no references outside CoachViewModel; every VM instance still injects `CoachGoalRepository` and `ProjectRepository`.
- Fix: delete functions, fields, constructor params and the dialog; trim the screenshot fixture.

### [P3] `onFinished` is a no-op at its only call site; the builder's `blank` path is unreachable
- Category: dead-code / bug
- Location: `onboarding/OnboardingScreen.kt:144-151, 253, 281, 425`; `MainActivity.kt:527`; `programbuilder/ProgramBuilderScreen.kt:72, 76, 83`; `programbuilder/ProgramBuilderViewModel.kt:150-153`; `nav/Routes.kt:60` (no caller passes `blank = true`)
- Problem: "Build my plan" lands on Home, not in the builder.
- Fix: route PLAN_CUSTOM to `programBuilder(blank = true)` after `ONBOARDING_DONE`, or remove the param/branch and relabel the CTA.

## P4

- **bug:** skip drops the locale plate-weight default (kg-locale skip → 15 lb plate instead of 10 kg); a lb→kg flip shows no chip selected (`onboarding/OnboardingScreen.kt:194-196, 416-424`; `OnboardingViewModel.kt:85`; `OnboardingExtras.kt:245-265`). Only curated plate-count exercises affected.
- **simplify:** onboarding completion's custom and freestyle branches are identical; problem areas are only ever added (stale union after process death + un-flag) (`OnboardingViewModel.kt:117-146, 113`). Merge branches; add `setProblemAreas(set)`.
- **bug:** no in-flight guard on the onboarding CTA; a double tap regenerates twice (mutex + same seed, so wasted work, no corruption) (`OnboardingScreen.kt:241-254, 285, 318, 321`; `OnboardingViewModel.kt:92-153`).
- **bug:** uncaught exceptions in `viewModelScope` crash the app and lose the builder draft (`ProgramBuilderViewModel.kt:326-351` try/finally only, `:154-161, 172-175`; `OnboardingViewModel.kt:93-153`; `OnboardingDraftKeeper.kt:59-64`). (medium)
- **bad-code:** builder runs its own Undo snackbar instead of `SnackbarController` (DESIGN.md; counted as debt in `DesignDoctrine.kt:365`), which is what enables the Undo-after-Save P2 (`ProgramBuilderScreen.kt:91-106`; `ProgramBuilderDayDetail.kt:85, 121`; `ProgramBuilderViewModel.kt:134-145, 260`).
- **simplify:** `accountItemCount` hand-mirrors the account's item emission (`coach/CoachAccount.kt:381-409` vs `:54-271`; filter chain duplicated `:199-205`, `:397-401`). Build `accountRows(state)` once.
- **dead-code:** `CoachStand.kt:377-384` `CoachStandUnavailable`, `:57` unused `onConnectHealth`; `CoachBlock.kt:157-168` `GroupHeaderPlain`; `CoachUi.kt:289-305` `CoachFlagDot`; `CoachCharts.kt:242, 245-246, 269-272` unused params; unused imports `CoachLearned.kt:17-18`, `CoachScreen.kt:56, 58`.
- **dead-code:** nine unused SurfaceKit primitives (~330 lines): `MUTED_ON_CARD_ALPHA` (public), `PEEK_*` + `peekCardWidth`, `CardEyebrow`, `DeltaBadge`, `SurfaceMeter`, `WeekRail`, `PeekCardRow`, `GhostCard`, `GhostTrack` (`experiment/SurfaceKit.kt:114-796`); unused imports.
- **dead-code:** `onboarding/OnboardingPrimitives.kt:122-125` `UnitSegment` (still open, 2026-09-12 academy-cardio-ui.md:163); unused imports in `OnboardingPrimitives.kt`, `OnboardingScreen.kt`, `PlanModeMedia.kt`. Enable the unused-imports lint check.
- **a11y:** `ToggleRow` announces a Button, not a Switch (`OnboardingPrimitives.kt:226-236`); use `toggleableLabeled`.
- **bug:** `CoachWatchBar` "play once" animation replays on every refresh (key includes the live fraction) (`CoachCharts.kt:214`).
- **bug:** double status-bar inset on routed Coach entries (~24dp gap) (`CoachScreen.kt:231-237` with TopAppBar at `:134-148`).
- **bug:** deep-link scroll re-fires on rotation (`CoachScreen.kt:122-126`); one-shot `rememberSaveable` guard.
- **bad-code:** two chart lines and the paused meter use the too-faint `outline@0.25` that CoachColors forbids (`CoachStand.kt:112`; `CoachCharts.kt:305, 337`).
- **bad-code:** recovery signals matched on display strings in four places (`CoachStand.kt:200, 205, 333`; `CoachIcons.kt:87-93`); add a `Kind` enum.
- **a11y:** charts without value descriptions (`CoachCall.kt:200-203`; `CoachCharts.kt:136-154, 278-311, 323-343`).
- **perf:** plan-mode fallback animation recomposes every frame and re-measures 7–11 text layouts per card per frame on API 26–27 (`onboarding/PlanModeVignettes.kt:76-88, 117-136, 156-178`). (medium)
- **bug (minor):** `endBlock` clears the block even when `end()` failed (`CoachViewModel.kt:271-276`); `startBlock` no-ops silently when `brief` is null (`:257`); the error branch can render an empty line (`CoachAccount.kt:423-424`); "Day N" names collide after a deletion (`ProgramBuilderViewModel.kt:237-242`); clearing custom reps leaves the last non-blank prefix saved (`ProgramBuilderDayDetail.kt:407-412`); gate hints only on gym/gear pages (`OnboardingScreen.kt:272-278`).
- **perf:** every onboarding answer triggers two full-screen recompositions (`OnboardingScreen.kt:160, 220`); collect `draftLoad` only until Ready.
- **bad-code — stale KDoc:** `OnboardingSoreSpots.kt:108-113`; `OnboardingIcons.kt:51-52`; `ProgramBuilderViewModel.kt:134-135`; `ProgramBuilderScreen.kt:72`; `OnboardingExtras.kt:204` (placeholder "e.g. 170" shown to kg users).

**Checked and OK:** builder reorder doesn't corrupt history; onboarding draft survives process death and rotation (`ONBOARDING_DONE` written last, draft cleared in the same edit); onboarding preview matches the saved week; LazyColumn keys unique; both draft JSON formats are schema-gated.

**Not verified (out of scope, not counted):** opening the Coach page can trigger an unguarded auto-applied deload that regenerates the program — see the data-repo P1 in `08-data-repositories.md`, which confirms this path deletes an active session.

## Files reviewed (33)
- `coach/`: CoachAccount, CoachAdvancedPrompt, CoachBlock, CoachCall, CoachCharts, CoachIcons, CoachLearned, CoachScreen, CoachStand, CoachUi, CoachViewModel, GoalPickerDialog, TrustProgressBar
- `onboarding/`: OnboardingDraft, OnboardingDraftKeeper, OnboardingExtras, OnboardingGymSteps, OnboardingIcons, OnboardingPrimitives, OnboardingScaffold, OnboardingScreen, OnboardingSoreSpots, OnboardingSteps, OnboardingViewModel, OnboardingWeekMeter, PlanModeMedia, PlanModeVignettes
- `programbuilder/`: ProgramBuilderDayDetail, ProgramBuilderDraft, ProgramBuilderModels, ProgramBuilderScreen, ProgramBuilderViewModel
- `experiment/`: SurfaceKit
