# Gym UI: freestyle, stats, session detail, history, notes

Scope: `ui/gym/{freestyle,stats,session,history,notes}/**`. 42 files, 8,548 lines (incl. the 245-line generated `BodyAnatomy.kt`), all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/ui/gym/` unless noted.

**Counts:** P1 0 · P2 5 · P3 15 · P4 13.

## P2

### [P2] Repeating a past workout reclassifies imported and seed-program moves as custom Chest moves, then writes CHEST into the custom registry on save
- Category: bug (wrong training data saved)
- Location: `freestyle/FreestyleTemplateViewModel.kt:135-150`; `freestyle/FreestyleLogModel.kt:281-294`; `freestyle/FreestyleLogScreen.kt:288`; `freestyle/FreestyleLogViewModel.kt:163-167`
- Problem: `loadTemplate` treats every id unknown to `ExerciseLibrary.byId` as custom and always supplies a `customName`. `toItems` builds `FsExercise(custom = true, muscle = te.muscleCode ?: MuscleGroup.entries.first())`; `muscleCode` is only set for `custom-` ids, so every other id gets CHEST. This covers importer `ext-…` ids (`data/repo/WorkoutImportRepository.kt:660-667`) and seed ids (`ua1…lb6`). On save, `customMuscleCode = "chest"` goes to `registerCustomExercise(CustomExerciseDef("ext-lat-pulldown-cable", …, "chest"))`.
- Scenario: import a Strong/Hevy history with an unmatched "Lat Pulldown (Cable)", repeat that workout in freestyle and save. The card shows CHEST while logging; afterwards `Program.exercise(id)` resolves through `CustomExerciseRegistry.plan` (`program/Program.kt:359-365`) to CHEST, so all past and future sets of that id feed chest in muscle volume, anatomy maps, Records and e1RM lists. Seed ids after a regenerate behave the same (a pull-up comes back with a weight field and a chest label).
- Fix: mark a template move custom only when `isCustomExerciseId(libId)`; for other non-library ids take muscle/unit/timed from `Program.exercise(libId)` with no registration; in `save`, register only `custom-` ids.
- Confidence: high

### [P2] Records "Tap one for its trend" often does nothing
- Category: bug
- Location: `stats/StatsContent.kt:142, 200-221`; `stats/StatsOverview.kt:126-131`; `stats/StatsStrength.kt:82-93, 115-128`; `data/repo/StatsStrengthAggregations.kt:105-137`
- Problem: a Records tap only sets `focusLift`/`focusNonce`. `e1rmLifts` is the top 6 by *current* e1RM; Records are the top 6 by all-time heaviest weight; a row expands only if its lift is in `e1rmLifts` with ≥2 sessions. So a record for the 7th-ranked lift, or a one-session lift, does nothing. When it does expand, "str-lifts" is a separate LazyColumn item above "str-records" and nothing scrolls, so it opens off screen.
- Fix: hoist a `LazyListState` and scroll to "str-lifts" in `onOpenLift`; build drill entries for every Records lift or make only drillable records tappable.
- Confidence: high

### [P2] History search field is fed by async state, so fast typing can drop characters
- Category: bug
- Location: `history/SessionHistoryScreen.kt:120-124, 221-236`; `history/SessionHistoryViewModel.kt:89-105`
- Problem: `OutlinedTextField(value = state.query)` but `state.query` only updates after the whole combine (filter, group, label) runs with `flowOn(Dispatchers.Default)` and `stateIn`. The field recomposes with the stale value and resets the IME buffer; "bench" typed fast on a large history can land as "bnch". `NotesSearchViewModel.kt:35-36` names the symptom but only partly works around it.
- Fix: hold the query as synchronous UI state (`rememberSaveable` or VM `mutableStateOf`/`SavedStateHandle`) and push it to `filters`.
- Confidence: medium

### [P2] "This week" muscle sets and the hero map use a rolling 7 days, not the ISO week the hero figures use — still open from 2026-09-12 stats-ui.md S1
- Category: bug
- Location: `stats/StatsContent.kt:255-258`; `stats/StatsOverview.kt:61, 99-100`; `data/repo/StatsRepository.kt:481-488`
- Problem: train legs Sunday, open Stats Monday: the hero reads 0 sessions / 0 sets this week while the muscle map beside it and "Sets per muscle this week" count Sunday's quads.
- Fix: bucket from the start of the ISO week, or relabel both "last 7 days".
- Confidence: high

### [P2] Heatmap day cells are unlabelled buttons with ~22dp targets — still open from stats-ui.md S3
- Category: release (a11y)
- Location: `stats/components/CalendarHeatmap.kt:115-135`; `bounceClick` (`ui/common/BounceClick.kt:45-53`) adds no label or role
- Problem: TalkBack reads up to 91 identical "double-tap to activate" nodes per page; ~22dp cells on a 360dp phone cause mis-taps.
- Fix: per-cell localized "date · N sets" description, `Role.Button`, "Open day log" label via `clickableLabeled`; a grid-level summary.
- Confidence: high

## P3

### [P3] Repeating a workout drops legacy timed-hold sets
- Location: `freestyle/FreestyleLogModel.kt:304-311` (compare legacy handling at `:105-110`, `:135-137`)
- Problem: timed moves use `hold = durationSeconds?.let(::holdText) ?: ""` then keep only `isLogged` sets; legacy holds (seconds in `reps`, `durationSeconds` null) vanish. An old 60 s plank comes back with zero sets.
- Fix: `durationSeconds ?: reps` for timed moves; one shared `LoggedSet`→`FsSet` mapper.

### [P3] Balance verdict is "·" in the "good" accent colour when one side is zero
- Location: `stats/StatsVolume.kt:132-138`; `stats/state/StatsEngineUi.kt:47-49`
- Problem: `ratio` is null when a side is 0, so `balanced` is null; 12 push / 0 pull shows a neutral dot, never "leans push".
- Fix: a one-sided pair with total ≥ `MIN_BALANCE_SETS` is not balanced.

### [P3] RPE card defaults to an unrated exercise and prints "No RPE" twice
- Location: `session/SessionDetailMetricCards.kt:227-269`; `session/SessionDetailCharts.kt:98-107`
- Problem: the filter is `sets.isNotEmpty()` (the doc says only exercises with RPE). With RPE only on exercise 3 the card opens on exercise 1 and shows the empty text twice.
- Fix: filter on `sets.any { it.rpe != null }`; suppress the chart's duplicate empty text.

### [P3] Heart-rate rows show raw ids for rotated-out exercises
- Location: `session/SessionDetailViewModel.kt:53-57`
- Problem: `swappedName ?: Program.exercise(id)?.name ?: exerciseId` — the pattern `Program.exerciseDisplayName` documents replacing (`program/Program.kt:367-373`). After a regenerate, HR rows read "ua1", "ua3".
- Fix: `Program.exerciseDisplayName(le.exerciseId, le.swappedName)`.

### [P3] Race between two LaunchedEffects loses "last time" data
- Location: `freestyle/FreestyleLogScreen.kt:145-156`
- Problem: `lastTime = lastTime + (id to viewModel.lastSets(id))` reads the map before the suspending call; two effects interleave and one write overwrites the other. The lost id keeps no seeded slab, LAST row or △ LAST marks for the session.
- Fix: `val s = lastSets(id); lastTime = lastTime + (id to s)`, or a VM `MutableStateFlow` with `update {}`.
- Confidence: medium

### [P3] Freestyle screen recomposes wholesale every second
- Category: perf
- Location: `freestyle/FreestyleLogScreen.kt:119-121, 262-263, 307-317, 483-503`
- Problem: `nowMs` is read in the root body, so every tick recomputes volume and the recent-moves rail (incl. SimpleDateFormat) and recreates item lambdas; the open browser gets a new `exclude` Set per tick and recomposes at 1 Hz; the screen is `keepScreenOn`.
- Fix: read the ticker only in header/stopwatch leaves; `remember` `recentMoves` and `exclude`; day label from a day-granular key.

### [P3] Exercise browser state lost on rotation
- Location: `freestyle/ExerciseBrowserScreen.kt:166-169` (also `:333`; `FreestyleTemplatePicker.kt:72`)
- Problem: `query`, `muscle`, `favoritesOnly`, `picked` are plain `remember`; pick 4 moves and rotate → empty.
- Fix: `rememberSaveable` with savers, or VM/`SavedStateHandle`.

### [P3] Custom moves can't be found again, so history fragments
- Location: `freestyle/ExerciseBrowserScreen.kt:76-90, 293-310`; `freestyle/ExerciseBrowserViewModel.kt:41-44`
- Problem: search and the recent rail cover library moves only; searching "sled" offers Create "sled" (`custom-sled`) while the real "Sled Push" history stays under `custom-sled-push`.
- Fix: include `CustomExerciseRegistry.all` in search and the rail.

### [P3] NotesSearch route is unreachable
- Category: dead-code
- Location: `notes/NotesSearchScreen.kt`, `notes/NotesSearchViewModel.kt`, `ui/nav/ForgeNavHost.kt:291-293`
- Problem: `HubScreen.kt:137/161` passes `onOpenNotes` but neither receiver (`OverviewScreen.kt:330`, `DayListScreen.kt:65/106`) calls it; no deep links. Screen, VM and `LoggedExerciseDao.searchNotes` ship unreachable.
- Fix: add an entry point, or delete.

### [P3] Shared global SimpleDateFormats freeze zone/locale and are used across threads — still open (gym-ui.md #4)
- Location: `history/HistoryRows.kt:210-227`; `notes/NotesSearchScreen.kt:125-126`
- Problem: after London → New York with the process alive, a 02:00 London Saturday session reads "FRI · AUG 22"; month names stay in the old language; formatters are called from `Dispatchers.Default` (`SessionHistoryViewModel.kt:94`) and main, and SimpleDateFormat isn't thread-safe.
- Fix: format the computed `LocalDate` with a `DateTimeFormatter` from the current locale.

### [P3] Three unread stats fields recomputed on every set write
- Category: dead-code / perf
- Location: `stats/StatsViewModel.kt:116-120`; `stats/state/StatsUiState.kt:55-64`; `stats/StatsInterest.kt:65-67`; `data/repo/StatsRepository.kt:490, 505-511`
- Problem: no UI reads `trainingTimes`, `prsByDayOfWeek`, `lifetime`; each emission runs two full-history builders plus an extra `prSessionStartTimes()` query. The KDoc's "four tabs Strength/Volume/Body/Trends" is wrong (lenses are Strength, Volume, Effort, Days).
- Fix: remove fields, builders and query; fix the KDoc.

### [P3] Charts expose no values to accessibility — partly still open (stats-ui.md S5)
- Category: a11y
- Location: `stats/components/Sparkline.kt:63`, `ScatterChart.kt:35`, `Bars.kt:29`; `stats/StatsInterest.kt:40-58, 133`; `session/SessionDetailCharts.kt:138, 269`
- Problem: e1RM trend, tonnage, strength curve, Banister, tier bar, RPE histogram, `PerSetLine`, HR trace have no semantics; `SetBars` does (`:215-217`), so switching to Line silently removes it.
- Fix: `contentDescription` params with value summaries.

### [P3] Freestyle reorder is drag-only
- Category: a11y
- Location: `freestyle/FreestyleLogScreen.kt:297-301, 389-390`; `ui/common/DragReorder.kt:145-156`
- Fix: "Move up/Move down" `CustomAccessibilityAction`s, as `WeekBarRail.kt:114-115` does.

### [P3] Nested and sub-48dp tap targets
- Category: a11y / doctrine §2③, §14
- Location: `freestyle/ExerciseBrowserScreen.kt:499-522` (star ~32dp nested in the tile click), `:575-580` (20dp clear icon); `freestyle/FreestyleTemplatePicker.kt:202-207` (20dp clear icon); `freestyle/FreestyleLogParts.kt:276-282` (× inside the fold-clickable title row)
- Fix: move the star out of the tile click; `IconButton` for clear icons; make × a sibling of the fold click.

### [P3] Filter and radio chips don't announce selected state
- Category: a11y
- Location: `freestyle/ExerciseBrowserScreen.kt:371-376` (Role.RadioButton, no `selected`), `:406-413`, `:441-446`, `:466-471`, `:499-500`
- Fix: `Modifier.selectable` for chips, `toggleable` for tiles.

## P4

- **dead-code:** `stats/state/StatsUiState.kt:252-256` `InsightFlag` (0 refs); types used only by builders with 0 main call sites: `WeekActivityRow`, `WeeklyDuration`, `OverloadSummary`, `PrRecency`, `PatternAxis`, `RepRangeDist`, `RepMaxEntry`/`RepMaxSet`, `MuscleVolume`, `HistoryPoint`, `ExerciseFrequency`, `TimeToPrEntry`, `WeeklyEffortCounts`, `DayTypeVolumeStats` and their builders (`buildWeekActivity`, `buildOverloadSummary`, `buildRepMaxes`, `buildTimeToPr`, `computeConsistencyStreak`, …); `MuscleSetCount.low/high`; `StatsEngineUi.kt` `PlateauFlagUi`/`plateauFlagOf` (tests only), `PulseBand.label`, `ReadinessPulse.drivers`, `BalanceRatioUi.total` (its `ratio`/`balanced` duplicate `RatioCounts`); `StatsCommon.kt:38` `STATS_HERO_CHART_H`; `StatsCard` `modifier`/`onClick` never passed; `FreestyleTemplateViewModel.kt:81-82` `hasTemplates`.
- **simplify:** Sparkline's "inline form" is unreachable (only caller always passes grid fractions, `showStartEndpoint = false`, range floor); the end marker at `x = width` is half clipped (`stats/components/Sparkline.kt:41-58, 106-108`; `LineChart.kt:24-35`). Inline into LineChart, keep `olsTrend`, inset the marker.
- **bad-code:** `holdText` duplicates `formatHold` without its Locale.US pin or negative clamp; a backwards wall-clock step formats "0:-5", `parseHold` returns null and Log set silently does nothing; the stopwatch uses `currentTimeMillis` (`freestyle/FreestyleLogModel.kt:185-186`). Use `formatHold` + `SystemClock.elapsedRealtime()`.
- **bad-code — duplication:** identical search fields `ExerciseBrowserScreen.kt:544-583` / `FreestyleTemplatePicker.kt:171-210`; `StatsInterest.kt:32` `rpeLabel` duplicates `ui/common/Format.kt:7`; `FsThumb` repeats `FreestyleLogParts.kt:182-188`; NotesSearch date format duplicates `formatHistoryDate`; `SegmentRow` lives in `session/` but is used by stats.
- **simplify:** two anatomy geometry caches parse the same SVG strings (`BodyHeatmap.kt:34-68` vs `MuscleFigure.kt:44-64`); one lazy `AnatomyGeometry`.
- **bad-code — misleading comments/labels:** `FreestyleDraft.kt:26-30, 71` say name/muscle are written only for custom moves but `FreestyleLogModel.kt:225-228` writes them for all (and they override the library at `:267-268`); `FreestyleLogModel.kt:275-278` describes an unreachable path; "Recently performed"/"Recent" label a frequency-ranked list; `NotesSearchViewModel.kt:35-36` claims the field reads `_query` directly; several top bars cite "§4.6: bell + back" with no bell.
- **doctrine:** inline font sizes (`SessionDetailMetricCards.kt:133, 251` 11sp; `SessionDetailCharts.kt:105` 10sp; `SessionDetailComponents.kt:356` 10sp); error-coloured body text (`StatsContent.kt:94-99`).
- **bug (data fidelity):** freestyle sets are all stamped with the save time (`FreestyleLogViewModel.kt:168-186` → `WorkoutRepository.kt:879-907`), so HR-per-exercise spans and time-of-day stats see one instant. Keep per-set `loggedAtMs`. (medium)
- **bad-code:** freestyle reads the weight unit from its own flow with an LB default (`FreestyleLogViewModel.kt:80-83`); kg users briefly see "LB"; the autosave effect captures the unit unkeyed. Use `LocalForgeSettings.current.weightUnit`. (medium)
- **bug:** "today" frozen across midnight — still open (stats-ui.md S4): `CalendarHeatmap.kt:60`; `StatsInterest.kt:78-80` (Banister `remember(dailyActivity)` with `LocalDate.now()`); `StatsAdherence.kt:34-37`; the History "TODAY" label.
- **bug:** History tag chips are case-sensitive while the filter isn't; "Heavy"/"heavy" show as two chips (`history/HistoryFiltering.kt:97-102` vs `:111`).
- **bad-code:** per-lift PRs matched by display name per recomposition (`StatsStrength.kt:87`); add `exerciseId` to `PrEntry`.
- **dead-code:** unused imports (`FreestyleLogScreen.kt:46`, `BodyHeatmap.kt:22`, `StatsContent.kt:12`, `StatsCommon.kt:9`, `SessionDetailComponents.kt:28`, `SessionDetailCharts.kt:4, 6, 8, 13`, `SessionDetailScreen.kt:7`); avoidable `!!` (`ExerciseBrowserScreen.kt:182`, `StatsOverview.kt:55`).

## Files reviewed (42)
- `freestyle/`: ExerciseBrowserScreen, ExerciseBrowserViewModel, FreestyleDraft, FreestyleLogModel, FreestyleLogParts, FreestyleLogScreen, FreestyleLogViewModel, FreestyleStartParts, FreestyleTemplatePicker, FreestyleTemplateViewModel
- `stats/`: StatsAdherence, StatsBody, StatsCommon, StatsContent, StatsInterest, StatsOverview, StatsReadiness, StatsStrength, StatsViewModel, StatsVolume
- `stats/components/`: Bars, BodyAnatomy, BodyHeatmap, CalendarHeatmap, LineChart, MuscleFigure, ScatterChart, Sparkline
- `stats/state/`: StatsChartData, StatsEngineUi, StatsUiState
- `session/`: SessionDetailCharts, SessionDetailComponents, SessionDetailMetricCards, SessionDetailScreen, SessionDetailViewModel, `state/SessionDetailUiState`
- `history/`: HistoryFiltering, HistoryRows, SessionHistoryScreen, SessionHistoryViewModel
- `notes/`: NotesSearchScreen, NotesSearchViewModel
