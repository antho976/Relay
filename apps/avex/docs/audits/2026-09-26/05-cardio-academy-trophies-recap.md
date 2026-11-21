# Cardio, Academy, Trophies, Recap UI

Scope: `ui/cardio/**`, `ui/academy/**`, `ui/trophies/**`, `ui/recap/**`. 38 files, 9,881 lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/`.

**Counts:** P1 0 · P2 9 (4 new, 5 still open from 2026-09-12 U02–U06) · P3 17 · P4 20.
2026-09-12 **U01** (Academy rotation hides a recommendation) is fixed by the 2026-09-26 rebuild.

## P2

### [P2] Weeks page averages and "cleared" count include weeks before the user's first session
- Category: bug
- Location: `ui/cardio/CardioWeeksScreen.kt:132-134`; `ui/cardio/CardioWeeksViewModel.kt:126-134`
- Problem: `weeksToCover` returns at least 8, so `cardioWeekSeries` pads the chart with zero weeks before the oldest entry, and `judged` counts that padding. A user whose first log was two weeks ago (180 min, 160 min, then this week) sees "48 min a week" (340/7) and "2 of 7 weeks cleared" instead of 170 min, 2 of 2. Every user sees this for their first 8 weeks. *Re-checked by the coordinator.*
- Fix: add `firstWeekStartMs` (Monday of the oldest non-rest entry) to `CardioWeeksState`, and filter `judged` to `weekStartMs >= firstWeekStartMs`. Keep the padding for drawing only.
- Confidence: high

### [P2] Week-detail "avg pace" divides all minutes by the distance of distance-bearing sessions only
- Category: bug
- Location: `ui/cardio/components/CardioWeekDetail.kt:133,143` (`domain/cardio/CardioWeekAggregate.kt:40-49`)
- Problem: `pacePerUnit(agg.minutes, agg.distanceKm)` includes HIIT and other no-distance minutes. A 45-min HIIT plus a 30-min 5 km run shows "15:00 /km avg" instead of 6:00. Rides and runs are blended into one pace.
- Fix: add `pacedMinutes` to the aggregate (only where `distanceKm > 0`), or hide the figure when more than one activity type logged distance.
- Confidence: high

### [P2] Session HR MAX is the max of bucket averages, so it under-reports the peak
- Category: bug
- Location: `ui/cardio/CardioViewModel.kt:342-344`; `ui/cardio/components/CardioSessionDetailSheet.kt:415-416` (`domain/health/WatchWorkout.kt:35-47`)
- Problem: only `downsampleHr(...)` output (≤120 averaged buckets) is kept. A 45-min run at 1 Hz averages ~22 s per bucket, so a 186 bpm sprint shows "MAX 172".
- Fix: compute avg/max from the raw series before downsampling; pass the downsampled series to the chart only.
- Confidence: high

### [P2] Editing any field silently re-rounds the stored distance
- Category: bug
- Location: `ui/cardio/components/CardioLogSheet.kt:125` (seed), `:161` (parse on save), `:137` (elevation seeded as integer); persisted at `CardioViewModel.kt:463`, `CardioSessionDetailViewModel.kt:194`
- Problem: the field is seeded with a one-decimal `distanceInputValue` and save always re-parses it. An adopted watch distance of 10.047 km becomes 10.0 after a note-only edit; a "5" km entry edited in miles mode becomes 4.989 km (changing records, pace and goals); a 5.37 km watch import is inserted as 5.4. *Re-checked by the coordinator.*
- Fix: remember the seeded text; if unchanged on save, keep `editing.distanceKm`. Same for elevation.
- Confidence: high

### [P2] Routed detail (History/Weeks) lacks HR and watch stats — still open from 2026-09-12 U02
- Category: bug
- Location: `ui/cardio/CardioSessionDetailScreen.kt:68-80`; `ui/cardio/CardioSessionDetailViewModel.kt:123-143`
- Problem: no `hr`, `watchStats` or `onAdoptWatchStats` is passed; the loader reads steps and route only.
- Fix: share one detail loader (steps, route, HR, watch) between both ViewModels.
- Confidence: high

### [P2] Watch stats hidden unless there are ≥2 HR points — still open (U03)
- Category: bug
- Location: `ui/cardio/components/CardioSessionDetailSheet.kt:228, 440-474`
- Fix: render the watch-stats line as its own item gated on `watchStats != null`.
- Confidence: high

### [P2] Chips and the glyph picker expose no selected state — still open (U04)
- Category: release (a11y)
- Location: `ui/cardio/components/CardioLogComponents.kt:341-366`; `CustomActivityDialog.kt:115-133`
- Fix: replace `PillChip` with the existing `ForgeChoiceChip` (`ui/common/Selectables.kt:78`), which already sets `selected` semantics. Make the glyph grid `selectable(role = RadioButton)` inside a `selectableGroup`.
- Confidence: high

### [P2] Filled numeric fields have no accessible name — still open (U05)
- Category: release (a11y)
- Location: `ui/cardio/components/CardioLogComponents.kt:236-272, 302-339`
- Fix: `contentDescription = "$caption, $unit"` on the BasicTextField, or use a labelled field.
- Confidence: high

### [P2] Weeks chart resets to page 0 after opening a week — still open (U06)
- Category: bug
- Location: `ui/cardio/CardioWeeksScreen.kt:106` (declared after the early return at `:101`)
- Fix: move `rememberSaveable { mutableIntStateOf(0) }` above the detail branch.
- Confidence: high

## P3

### [P3] "hide" dismisses only the 3 visible watch imports
- Category: bug
- Location: `ui/cardio/CardioViewModel.kt:157-160` (pool limit 6), `187-195` (`take(3)`), `396-399`
- Problem: with 5 unlogged watch workouts, tapping hide immediately re-renders the section with the other 2.
- Fix: dismiss the whole filtered pool, or store a "hidden until a newer workout arrives" marker.

### [P3] A watch import shows "Save changes" on a new entry
- Category: bug
- Location: `ui/cardio/CardioViewModel.kt:382-393`; `CardioLogSheet.kt:348-349`; `CardioLogSheetSections.kt:219-223`
- Problem: the import prefill is passed as `editing` with id 0, so `editing != null` is true.
- Fix: `editing != null && editing.id != 0L`, or a separate `prefill` parameter.

### [P3] Trophies not re-evaluated after routed edits, watch-stat adoption or Undo
- Category: bug
- Location: `ui/cardio/CardioSessionDetailViewModel.kt:162-210`; `CardioViewModel.kt:368-379, 427`
- Problem: at 97 km total, editing a run from 5 km to 8 km through History leaves the "100 km" trophy locked until the next gym finish or new cardio log.
- Fix: call `evaluateAndUnlockNew()` after every cardio write, ideally once in `CardioRepository.add/update`.

### [P3] Cardio save logic duplicated across two ViewModels — still open from 2026-09-12 recommendation 2
- Category: bad-code
- Location: `CardioViewModel.kt:435-488` vs `CardioSessionDetailViewModel.kt:171-210`; day-bounds math repeated at `CardioViewModel.kt:332-335, 411-414`, `CardioSessionDetailViewModel.kt:130-133`
- Problem: the copies have drifted: the routed save has no double-tap guard and no trophy evaluation.
- Fix: a `CardioDraft.toEntry(id)` helper plus one shared saver; a `dayBounds(ms, zone)` helper in `core/time`.

### [P3] Times hard-coded 12-hour while the log sheet honours the 24-hour setting
- Category: bug
- Location: `CardioSessionDetailSheet.kt:107`; `CardioWeekDetailComponents.kt:165, 174-181`; `WatchImportsSection.kt:70` (contrast `CardioLogSheet.kt:187-189`)
- Problem: on a 24-hour device the sheet shows "19:24" while detail, timeline and imports show "7:24 PM" and "busiest 6pm–7pm".
- Fix: `DateFormat.getTimeFormat(context)` or `ofLocalizedTime(SHORT)`.

### [P3] New-entry start time defaults to the moment of saving; time picker allows future times today
- Category: bug
- Location: `CardioLogSheet.kt:142`; `CardioLogSheetSections.kt:303-335`; mirrored at `data/repo/CardioRepository.kt:81-82`
- Problem: a 45-min run logged at 09:00 is stored as 09:00–09:45; Health Connect receives an `activelyRecorded` session ending in the future.
- Fix: default a new entry's start to `now - duration` when the time was never picked; clamp picked times to `<= now` today.
- Confidence: medium

### [P3] No plausibility bound on duration or distance
- Category: bug
- Location: `CardioLogComponents.kt:372, 375-397`; `CardioLogSheet.kt:167`
- Problem: "6000" minutes or "999999" km can be saved; one typo inflates weekly totals and streaks and permanently unlocks trophies.
- Fix: cap duration at 24 h and distance at ~500 km in the sheet and the ViewModel.

### [P3] Tap targets under 48dp
- Category: release (a11y)
- Location: `CardioLogComponents.kt:361` (PillChip ~30dp), `:82` (date capsule ~34dp); `CardioComponents.kt:96-103` (`weeks →` ~32dp); `WatchImportsSection.kt:56-63` (`hide`); `CardioSessionDetailSheet.kt:464-471` (`use watch stats →` ~22dp); `AcademyFigures.kt:249` (slider 40dp while its KDoc at `:226` says 48dp)
- Fix: `minimumInteractiveComponentSize()`; `height(48.dp)` for the slider.

### [P3] Date/time capsules hide their value from TalkBack
- Category: release (a11y)
- Location: `CardioLogComponents.kt:86-92`
- Problem: `contentDescription` "Pick date" replaces the date text, so the current date/time is never announced.
- Fix: move the label onto the click action and clear the "▾" glyph's semantics.

### [P3] Dead `CardioUiState.weekAggregate`, recomputed on every DB emission
- Category: dead-code
- Location: `CardioViewModel.kt:235, 275, 542`; `CardioUiState.kt:53`
- Problem: a whole-history aggregate runs on every cardio write and day tick; no reader anywhere in app/wear/shared/tests.
- Fix: delete the field and its computation.

### [P3] Dead inline history-expand path and dead `onBack`
- Category: dead-code
- Location: `CardioScreen.kt:66, 181-182, 235-237, 407-430, 552-570`; `CardioViewModel.kt:418-419, 528`; `CardioUiState.kt:77-78`
- Problem: the only caller (`HubScreen.kt:126-132`) always passes `onOpenHistory` and never `onBack`.
- Fix: make `onOpenHistory` non-null; delete `historyExpanded`/`toggleHistoryExpanded`, the expand block, `SeeAllRow`'s expand parameters and `onBack`.

### [P3] Trophies ViewModel computes state no screen renders
- Category: dead-code
- Location: `TrophiesViewModel.kt:41, 48-55, 114-123, 140`; `TrophiesUiState.kt:37-57`
- Problem: `closestTrophyNudge`, `nearMisses` (backed by a live Room flow) and `maxScore` are read only by a test. The hero shows the first locked trophy in catalog order instead.
- Fix: render `closestTrophyNudge` in the hero, or delete the three fields and the flow.

### [P3] Health Connect refresh runs twice on first open and on every tab visit, uncancelled
- Category: bad-code (perf)
- Location: `CardioScreen.kt:83-96`; `CardioViewModel.kt:110-111, 147-161`
- Problem: `init` refreshes, then the lifecycle observer replays ON_RESUME and refreshes again; every swipe back re-adds the observer. Each refresh costs three permission IPCs plus up to 5 pages of watch workouts; overlapping refreshes race so a stale result can land last.
- Fix: a cancellable `refreshJob`; drop the call from `init`.

### [P3] Latent init-order NPE in CardioViewModel
- Category: bug
- Location: `CardioViewModel.kt:110-122 → 157`; `watchCandidates` declared at `:178`
- Problem: `init` launches on `Main.immediate` and writes `watchCandidates` before the property initialiser runs. Safe today only because `grantedPermissions()` always suspends.
- Fix: declare the properties above `init`.
- Confidence: medium

### [P3] Academy sketch draw path allocates heavily per frame
- Category: bad-code (perf)
- Location: `AcademySketches.kt:272, 376, 404-411, 506-508, 538-559`
- Problem: `rememberTextMeasurer()` caches 8 entries but sketches draw more labels (gettingStronger has 13), so every frame of the 3.5 s draw-in re-lays-out every label; new PathMeasure/Path/Brush per mark per frame.
- Fix: `cacheSize = 64`, cache contour lengths per mark, reuse one scratch Path and cached Brushes.
- Confidence: medium

### [P3] Interactive figures rebuild the whole Sketch in composition on every drag frame
- Category: bad-code (perf)
- Location: `AcademyFigures.kt:298-302, 321-326, 355-371, 383-399`
- Fix: quantise the value and `remember` the sketch, or draw only the moving marker in the draw phase.
- Confidence: medium

### [P3] RouteThumbnail distorts aspect ratio and allocates in draw — still open (2026-09-12 follow-up)
- Category: bug
- Location: `RouteThumbnail.kt:31-50`
- Problem: lat/lon scaled independently with no cos(lat) correction; a narrow east–west route renders as a blob.
- Fix: project once in `remember`, one uniform scale, build the path in `drawWithCache`.

## P4

- **dead-code:** unused `weekNum` (`CardioScreen.kt:115`, `WeekFields` import at `:57`).
- **dead-code:** `TrophyIconBadge.animateEntrance` never passed true (`TrophyIconBadge.kt:53, 62-63`).
- **dead-code:** `AcademyViewModel.UiState.newCount` unused; `runCatching{collect}` swallows errors including cancellation (`AcademyViewModel.kt:54, 77-82`). Use `.catch{}`.
- **bad-code:** stale/misleading comments: `RouteThumbnail.kt:21-22`, `CardioSessionDetailScreen.kt:16-19`, `CardioUiState.kt:37, 77`, `CardioViewModel.kt:54-58`, `CardioPaceTrendSection.kt:36-42`, `CardioComponents.kt:106-107` and `CardioSessionDetailSheet.kt:152-153` ("wrap" but plain Rows), `TrophyIconBadge.kt:43-46`, `RecapScreen.kt:59` (a bell that isn't there), `AcademyScreen.kt:242-243`.
- **bad-code:** literal "150" beside `WHO_WEEKLY_ACTIVITY_MIN` (`CardioComponents.kt:158-159`; `CardioWeekDetail.kt:171-172`).
- **bad-code:** mixed "d MMM"/"MMM d" date orders in one flow (`CardioWeeksScreen.kt:266, 278`, `CardioWeekBars.kt:72`, `CardioScreen.kt:119`, `CardioWeekDetail.kt:94`). One shared short-date formatter.
- **bug:** watch-import rows show only weekday + time across a 14-day window; two Saturdays look identical (`WatchImportsSection.kt:70`).
- **simplify:** guarded `!!` at `CardioSessionDetailSheet.kt:443`, `CardioWeekDetailComponents.kt:108`, `TrophiesComponents.kt:172`; string check `endsWith("kcal")` at `CardioSessionDetailSheet.kt:462`.
- **doctrine §2③:** "use watch stats →" uses the navigation arrow for an in-place mutation (`CardioSessionDetailSheet.kt:464-471`).
- **doctrine:** Trophies screen puts "TROPHIES" in the top bar (§4.6, `TrophiesScreen.kt:71-79`), uses hairline dividers (§1, `TrophiesScreen.kt:144-147`, `TrophiesComponents.kt:106`) and em dashes in copy (§11/§12, `TrophiesComponents.kt:90, 99`).
- **bug:** Trophies sort is a plain `remember` and resets on rotation while the filter survives (`TrophiesScreen.kt:52`).
- **bug:** retired-lesson error state has no Scaffold or back arrow (`LessonScreen.kt:32-40`).
- **a11y:** thumbnails set `contentDescription = ""` (`AcademyThumbs.kt:50 → AcademySketches.kt:304`); each bullet "·" is its own node (`LessonBlocks.kt:78`).
- **bad-code:** drawing numbers use the default locale ("1,6" on a French device, `AcademySketches.kt:504, 711`); magic `0.45359237` duplicates `KG_PER_LB` (`AcademyFigures.kt:385`).
- **bad-code:** injected Clock bypassed (`CardioComponents.kt:207`; `CardioLogSheet.kt:142`; `CardioLogSheetSections.kt:242`; `RecapViewModel.kt:99, 147-148`).
- **bad-code:** Recap uses English-only `Month.name` (`RecapScreen.kt:90`; `RecapViewModel.kt:151`); `load()` has no error path so a DAO exception crashes (`RecapViewModel.kt:94`).
- **perf:** four sequential Health Connect reads when a session detail opens (`CardioViewModel.kt:336-344`); run with `async`.
- **perf:** `openWeek` sits inside the full-series combine, so each bar tap recomputes the whole history (`CardioWeeksViewModel.kt:88-111`).
- **bad-code:** duplicated primitives: ISO-Monday math (`CardioScreen.kt:116`, `CardioEntryRow.kt:140`); two Mon–Sun bar rows (`CardioComponents.kt:298-362`, `CardioWeekDetail.kt:214-252`); the same track Box twice (`CardioBars.kt:127-135, 180-190`).
- **bug:** a lesson that fits on one screen is marked complete on open with no scroll; the effect captures a stale `onReachedEnd` (`Reader.kt:93-98`). Require a real scroll; `rememberUpdatedState`.

## Files reviewed (38)
- ui/cardio: CardioComponents, CardioScreen, CardioSessionDetailScreen, CardioSessionDetailViewModel, CardioViewModel, CardioWeeksScreen, CardioWeeksViewModel, LocalCardioTypes
- ui/cardio/state: CardioUiState
- ui/cardio/components: CardioBars, CardioEntryRow, CardioLogComponents, CardioLogSheet, CardioLogSheetSections, CardioPaceTrendSection, CardioSessionDetailSheet, CardioWeekBars, CardioWeekDetail, CardioWeekDetailComponents, CustomActivityDialog, RouteThumbnail, WatchImportsSection
- ui/academy: AcademyFigures, AcademyScreen, AcademySketches, AcademyThumbs, AcademyViewModel, LessonBlocks, LessonScreen, LessonViewModel, Reader
- ui/trophies: TrophiesComponents, TrophiesScreen, TrophiesViewModel, components/TrophyIconBadge, state/TrophiesUiState
- ui/recap: RecapScreen, RecapViewModel
