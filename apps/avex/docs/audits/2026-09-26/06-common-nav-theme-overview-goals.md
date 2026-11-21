# Shared components, navigation, theme, Home/overview, check-in, goals

Scope: `ui/common/**`, `ui/nav/**`, `ui/theme/**`, `ui/overview/**`, `ui/checkin/**`, `ui/goals/**`. 64 files, ~10,262 lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/`.

**Counts:** P1 1 · P2 8 · P3 12 · P4 16.

## P1

### [P1] Root dialogs, sheets and the Undo snackbar stay usable above the app lock — still open from RELEASE_AUDIT_2026-09-12 R1 (additional windows)
- Category: bug (security, data loss)
- Location: `ui/nav/ForgeNavHost.kt:492`; `ui/common/ProgramChangeGuardHost.kt:23-43`; `ui/common/SnackbarControllerHost.kt:78-87`; `ui/common/DayLog.kt:93`; `ui/overview/SummarySheet.kt:56`; `ui/checkin/CheckinSheet.kt:60`; `ui/common/ExerciseLibraryPicker.kt:93`; ordering at `MainActivity.kt:536-552`
- Problem: MainActivity keeps the nav host composed under an opaque lock overlay; `AlertDialog`, `ModalBottomSheet` and `Dialog` each open their own window above it. `SnackbarControllerHost` is drawn *after* the lock overlay, so Undo is operable too.
- Scenario: with a workout in progress, trigger a deload or regenerate so the program-change guard dialog appears. Background and return while locked: "Discard & continue" is still tappable and discards the session and its sets without authentication. Open sheets show private training data over the lock.
- Fix: don't compose window-owning hosts/sheets while locked (keep their state in VMs); draw the snackbar host before the lock overlay or skip it while locked. See the consolidated R1 fix in `README.md`.
- Confidence: high (code trace; not device-verified)

## P2

### [P2] Goal editor drops the comma decimal separator
- Category: bug (data correctness)
- Location: `ui/goals/GoalEditorScreen.kt:389, 542`
- Problem: both fields filter with `isDigit() || '.'`; on a comma-decimal keyboard "82,5" becomes "825" → an 825 kg target or 55 km distance goal. `DecimalInput.kt:36-46` documents this exact bug and its fix. Same stale filter at `ui/gym/train/DayDialogs.kt:110` and `ui/profile/MirrorTestViewer.kt:384`.
- Fix: `filterDecimalInput`.
- Confidence: high

### [P2] Re-saving an unchanged lift goal drifts the target
- Category: bug
- Location: `ui/goals/GoalEditorScreen.kt:382-405`
- Problem: seeded rounded to 0.1 in the display unit and always written back: 225 lb shows "102.1" kg and saves 225.09 lb. `achieved` requires best ≥ target (`data/repo/GoalRepository.kt:56`), so a reached goal becomes unreached. `CustomEditStep` (`:501-511`) already guards; `LiftWeightStep` doesn't.
- Fix: only save when the text differs from the seed.
- Confidence: high

### [P2] Double-tap on goal editor buttons duplicates goals and pops twice
- Category: bug
- Location: `ui/goals/GoalEditorScreen.kt:197-212`; `ui/nav/ForgeNavHost.kt:436`
- Problem: each button writes then `onDone()` with no guard; the screen stays tappable through its exit animation. `ExtendedGoalRepository.create` inserts unconditionally (`:71`), so a double-tap on "Add goal" creates two goals and also pops the Goals list back to Home. There is no `dropUnlessResumed`-style guard anywhere in the app.
- Fix: `dropUnlessResumed { }` or a one-shot `committed` flag.
- Confidence: high

### [P2] Three settings are never read, and Home ignores week start
- Category: bug / dead-code
- Location: `ui/theme/ForgeUiSettings.kt:17-24`; `MainActivity.kt:462-482`; `ui/overview/OverviewScreen.kt:139, 386`
- Problem: `compactSetLogging`, `dateFormat`, `timeFormat24h` have no consumer outside the Settings preview strings (see also `04-settings…`). Home's week strip is hardcoded Monday-first and ignores `firstDayMonday`. `hiddenOverviewTiles`/`overviewTileOrder` are dead.
- Fix: wire into consumers, or remove the rows and fields before release.
- Confidence: high

### [P2] Check-in sheet doesn't scroll
- Category: bug / a11y
- Location: `ui/checkin/CheckinSheet.kt:65-71`
- Problem: a plain Column in a ModalBottomSheet holds four scales, an 11-muscle picker, the weight row and Save; at 150–200% font or with the keyboard open, Save is clipped. `DayLog.kt:98-100` documents and fixed the same failure.
- Fix: `verticalScroll` (+ `imePadding`), or LazyColumn.
- Confidence: medium (height estimated)

### [P2] Exercise picker rows expose no checked/selected state; 44dp rows — still open (U04, broader than single-select)
- Category: a11y
- Location: `ui/common/ExerciseLibraryPicker.kt:122-140`
- Problem: `clickableLabeled` with a checkbox/radio role but no state; the drawn Checkbox has `onCheckedChange = null`, which adds no state for TalkBack.
- Fix: `toggleable`/`selectable`; `heightIn(min = 48.dp)`.
- Confidence: high

### [P2] Undo window is a fixed 4 seconds — still open (R2)
- Category: a11y
- Location: `ui/common/SnackbarController.kt:63-68, 94`; `ui/common/SnackbarControllerHost.kt:61-68`
- Problem: the system "Time to take action" setting is never consulted.
- Fix: `AccessibilityManager.calculateRecommendedTimeoutMillis`.
- Confidence: high

### [P2] Untracked sessions get BEST and ±% comparisons on Home — still open (S2)
- Category: bug
- Location: `ui/overview/OverviewUiStateMapper.kt:51-57`
- Problem: the recent list includes untracked sessions but day averages count tracked only (`SessionDao.kt:377`); the mapper subtracts an untracked session from a sum it was never in, and an untracked session can be badged BEST.
- Fix: for untracked sessions, comparison = null, BEST = false.
- Confidence: high

## P3

### [P3] Double navigation stacks duplicate screens; nothing uses `launchSingleTop`
- Location: `ui/nav/ForgeNavHost.kt:138, 355`; `ui/nav/HubScreen.kt:134-174`
- Problem: a widget tap mid-workout pushes a second DayScreen for the same session; double-tapping Start/Settings/Profile pushes two copies. Only `openNotifications` (`ForgeNavHost.kt:167`) guards. (Related to the Train double-PopBack P2 in `01-gym-train.md`.)
- Fix: `launchSingleTop = true`; for gym days, pop to Overview first or skip when the same day is on top.

### [P3] Notification actions from a feed opened over a deep screen go nowhere
- Location: `ui/nav/ForgeNavHost.kt:350-358`
- Problem: the arrival banner can open Notifications over Settings; "Open coach brief" or a cardio resume then pops back to Settings, not the hub; the requested tab stays pending and jumps later.
- Fix: `popBackStack(Routes.OVERVIEW, false)` first, as `goHome` and the widget path do.

### [P3] Home hero drops the user's custom day name
- Location: `ui/overview/OverviewScreen.kt:493`; `domain/…/TodayDirective.kt:226`; `data/repo/DirectiveRepository.kt:133`
- Problem: the directive headline uses the program's default name; a renamed day flashes its custom name then reverts.
- Fix: resolve the custom name for the directive's day.

### [P3] `coachEnabled` is collected but never used on Home
- Location: `ui/overview/OverviewScreen.kt:345`; `ui/overview/OverviewViewModel.kt:126`
- Problem: users who turned the coach off still get coach directives (rest days with "Train anyway", "Open Academy", readiness wording); `DirectiveRepository` never checks the setting.
- Fix: show the plain next-up hero when the coach is off, or delete the flow.
- Confidence: medium (depends on product intent)

### [P3] SegmentPill is ~22dp tall and is used for form input
- Category: a11y / doctrine
- Location: `ui/common/SegmentPill.kt:36-44`; `ui/checkin/CheckinSheet.kt:106-124, 196-205`; `ui/goals/GoalEditorScreen.kt:450-462`
- Problem: check-in 1–5 scales are ~30×22dp with no role; DESIGN §8 reserves SegmentPill for switching views.
- Fix: `minimumInteractiveComponentSize()`; move check-in scales, muscle picker, "Feeling unwell" and goal timeframe to `ForgeChoiceChip`/`ForgeSegmentedChoice`.

### [P3] "Show on Home" switch state isn't announced
- Category: a11y
- Location: `ui/goals/GoalEditorScreen.kt:338-367`
- Fix: `toggleableLabeled`.

### [P3] Errors swallowed in root hosts
- Location: `ui/common/SnackbarController.kt:104`; `ui/common/ProgramChangeGuard.kt:89`; `ui/common/ArrivalBannerHost.kt:169`
- Problem: a failed Undo leaves the item deleted silently; a failed "Discard & continue" closes the dialog and does nothing; a failed "mark announced" removes the banner from the queue, then it is re-added and replays.
- Fix: snackbar on failure; only dequeue the arrival after marking succeeds.

### [P3] Home RECENT labels old sessions with a bare weekday
- Location: `ui/overview/OverviewUiStateMapper.kt:147-156`
- Problem: a session 9–20 days old shows "TUE", reading as this week.
- Fix: weekday within 6 days, otherwise a date.

### [P3] Check-in scales have no end labels
- Location: `ui/checkin/CheckinSheet.kt:176-207`
- Problem: Sleep 5 is good, Soreness 5 is bad, with no labels; inverted answers feed today's targets.
- Fix: end captions and per-pill value descriptions.
- Confidence: medium

### [P3] Dead Home component files (~400 lines)
- Category: dead-code
- Location: `ui/overview/components/NavTile.kt` (whole file); `ui/overview/components/OverviewComponents.kt:37-203` (`WeekDayBox`, `OverviewStat`, `RecentRow`, `MiniChip`); `ui/overview/components/HomeSurfaceCards.kt:46-116` (`HomeHero`)
- Problem: no callers incl. tests; the dead public `RecentRow` shares its name with the live private one at `OverviewScreen.kt:166`.
- Fix: delete.

### [P3] Dead Overview state pipeline
- Category: dead-code
- Location: `ui/overview/OverviewViewModel.kt:108-114, 214-219, 303-305, 309-377, 386-391` and deps at `49-53`; `ui/overview/state/OverviewUiState.kt:45-66, 83-86, 95-100, 107, 118-122`; `ui/overview/SummarySheet.kt:82-134`
- Problem: coach feed, `loadSampleData`, `sessionExerciseLines` and three constructor deps have no callers; ~12 OverviewUiState fields are never read; SummarySheet only opens for cardio, so its gym branches are unreachable.
- Fix: delete.

### [P3] Unused Home callbacks leave routes unreachable
- Category: dead-code
- Location: `ui/overview/OverviewScreen.kt:329-333`; `ui/nav/HubScreen.kt:160-167`; `ui/nav/ForgeNavHost.kt:258-260, 384-389, 404-417, 495-548`; `ui/nav/Routes.kt:5, 6, 8`
- Problem: `onGoToTrophies`, `onOpenNotes`, `onGoToNutrition`, `onOpenCoachBrief`, `onOpenCoachLab` are never called, so the Nutrition route + placeholder screen, Coach Lab, Coach Timeline and the Academy route are unreachable; `GYM_TRAIN`, `GYM_STATS`, `CARDIO` route constants are unused.
- Fix: delete, or wire real entry points.

## P4

- **dead-code — shared components:** `EmptyState`, `EntranceItem`, `monthsAgoPhrase`, `ForgeUiSettings.useKg` unused (deleting `EntranceItem` also needs DESIGN §8 + DoctrineParityTest updates); `ArrivalController.current`/`clear()` test-only and no wipe path calls `clear()`; LaunchScenes (~600 lines) intentionally parked but ships in the APK; `OnThisDayMemory` lives in the UI state package but is only used by the data layer.
- **dead-code — theme tokens:** all `Indigo*` colours, `indigoColorScheme`, `AccentIndigoDefault` unused; `AccentEmber` unreferenced; accent presets duplicated as hex strings in `ui/settings/AccentColorPicker.kt:81-83`.
- **bad-code:** `ui/nav/NavIcons.kt:152-185` copies the builder helpers from `VectorBuilders.kt`.
- **bug (latent):** `ui/common/DragReorder.kt:154` bare `animateItem()` ignores reduced motion; `onMove` captured once (`:60-62`).
- **doctrine:** `DayLog.kt:96` and `SummarySheet.kt:59` use the background colour where modals use surface; the guard dialog uses `collectAsState` and stock button colours, and its destructive confirm isn't error-tinted.
- **locale:** `ui/common/DayLog.kt:84` uses `Locale.getDefault()` rather than `currentLocale()`.
- **a11y:** `NotificationBell.kt:72` defaults to a 44dp target; the bell and `TopBarIconButton` (`OverviewScreen.kt:308`) read their label twice.
- **bad-code — stale docs/names:** `ForgeBottomBar.kt:30` (tab ordinal ≠ pager page when Coach is hidden); `HubScreen.kt:36` (Profile as a tab); `ForgeUiSettings.kt:27` (default accent is Red, not Navy); `rememberDrawProgress`'s key never replays despite its doc; `HistoryComponents.kt` only holds `SummaryStat`; `OverviewViewModel.kt:59-73` comments about removed code.
- **perf:** `ui/theme/ForgeTheme.kt:32` rebuilds the colour scheme every recomposition, invalidating the whole tree; shimmer allocates a brush per placeholder per frame; the launch wordmark creates a RenderEffect every frame; unread count via a static composition local recomposes the whole nav host; `bounceClick` uses `Modifier.composed` at ~100 call sites.
- **simplify:** every SegmentPill call site passes the theme's own four colours; no `modifier` parameter.
- **goals nits:** `GoalsScreen.kt:200-213` spacer doesn't animate with its row; the lift picker query (`GoalEditorScreen.kt:284`) is lost on rotation and duplicates ExerciseLibraryPicker; hint sentences in the mono font (`:273`).
- **Home nits:** `OverviewScreen.kt:735` alpha 0.3 off the doctrine ladder; week-strip TalkBack says "of 7" while header shows "/ N" (`:616`); `selectedItem!!` at `:400`; a logged rest day takes a RECENT slot titled "Cardio · Rest Day"; `OverviewViewModel.kt:262` runs a full weekly-stats query to check whether any session exists; shipped Home depends on the `ui.experiment` package.
- **check-in nits:** "Close" is a ~36dp text link; a picked scale value can't be cleared; row labels are 9sp.
- **hygiene:** `AvexWordmark.kt:118` shadows a name; `ConfettiOverlay.kt:11` unused import; ForgeTheme duplicates `parseAccentHex`; `modalRoutes` rebuilt every recomposition; `ForgeBottomBar.kt:108` overrides font size to its existing value.

## Files reviewed (64)
- `ui/common/` (36): Capsules, SnackbarController, SegmentPill, ArrivalController, ForgeMotionKit, SparklineSeries, ProgramChangeGuard, DayLog, WeekBarRail, AvexIntro, RelativeTime, ListMotion, AvexWordmark, EmptyState, ProgramChangeGuardHost, Editorial, ConfettiOverlay, ForgeModifiers, ForgeShimmer, VectorBuilders, AccentHex, Format, HapticExt, SnackbarControllerHost, ArrivalBannerHost, BounceClick, LaunchScenes, DurableWrite, DragReorder, ExerciseIcons, NotificationBell, TouchExploration, CurrentLocale, Selectables, ForgeSwitch, ExerciseLibraryPicker
- `ui/nav/` (6): NavIcons, ForgeNavHost, Routes, HubScreen, ForgeBottomBar, HubViewModel
- `ui/theme/` (7): AccentContrast, Color, ForgeTheme, Type, ForgeUiSettings, Motion, Shape
- `ui/overview/` (9): components/NavTile, components/OverviewComponents, components/HomeSurfaceCards, OverviewScreen, SummarySheet, OverviewUiStateMapper, state/OverviewUiState, OverviewViewModel, HistoryComponents
- `ui/checkin/` (2): CheckinSheet, CheckinViewModel
- `ui/goals/` (4): GoalEditorScreen, GoalsComponents, GoalsScreen, GoalsViewModel
