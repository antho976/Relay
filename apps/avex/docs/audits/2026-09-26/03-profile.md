# Profile UI (profile, gallery, camera, body data)

Scope: `ui/profile/**`. 41 files, 10,233 lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/`.

**Counts:** P1 1 · P2 11 · P3 15 · P4 16. Also still open from RELEASE_AUDIT_2026-09-12: R1, R3, R4, R5, R6. About 1,100 lines have no caller.

## P1

### [P1] Gallery viewer and compare windows stay on top of the app lock — still open from RELEASE_AUDIT_2026-09-12 R1
- Category: bug (security/privacy)
- Location: `ui/profile/MirrorTestViewer.kt:205`, `ui/profile/MirrorTestCompare.kt:142` (also `MainActivity.kt:523-553`, `ui/nav/ForgeNavHost.kt:439-456`, `security/AppLockManager.kt:59`)
- Problem: both open a Compose `Dialog` (a separate Android window). The app lock is a sibling overlay inside the activity window, over a nav host that stays composed. The MIRROR_TEST route only checks `galleryLocked = galleryLockEnabled && !sessionValid`. With app lock on and gallery lock off: open a photo, background until it locks, return. The lock screen shows, but the photo (or slider compare) is still drawn above it.
- Fix: hoist `appLock.appLocked` into `MirrorTestScreen` and null `viewer`/`comparePair` while locked (or only compose the dialogs when unlocked); or gate MIRROR_TEST and PROGRESS_CAMERA on `appLocked || galleryLocked`. Add a UI test.
- Confidence: high

## P2

### [P2] Backdating beyond the 90-entry window erases that day's note
- Category: bug (data loss)
- Location: `ui/profile/BodyweightLogSheet.kt:94-96, 113, 235`; `ui/profile/ProfileViewModel.kt:67-70`; `data/db/dao/BodyweightDao.kt:16`; `data/repo/BodyweightRepository.kt:54-84`
- Problem: the sheet's `entries` is `observeRecent(90)`; the date picker allows any past day. Pick an older day that already has a note: the lookup misses, the field shows the latest weight and an empty note; Save calls `record(keepExistingNote=false)` → note = null and the day's weight is overwritten.
- Fix: seed from `dao.byDateKey(date)` via a VM lookup in `LaunchedEffect(date)`; pass note = null when the note field was never touched.
- Confidence: high

### [P2] BODY row rounds weight to whole stones
- Category: bug (units)
- Location: `ui/profile/ProfileSurfaceSections.kt:484-489`
- Problem: `display.lastOrNull()?.roundToInt()` on the stones value, so 180 lb (12 st 12 lb) shows "13 ST" (error up to ±7 lb). The delta prints decimal stones with no unit. *Re-checked by the coordinator.*
- Fix: for stones use `formatWeight(lb, ST)` with `unit = null`; format the delta with `formatWeightDelta`.
- Confidence: high

### [P2] Photo delete is one tap, no confirm, no Undo
- Category: bug (data loss)
- Location: `ui/profile/MirrorTestViewer.kt:241-243`; `MirrorTestScreen.kt:352`; `data/repo/ProgressPhotoRepository.kt:330-343`
- Problem: the trash button sits beside the tappable date and deletes the bytes immediately. For in-app camera shots this is the only copy (the temp file is deleted after publish). The rest of the app follows §13 "undo over confirm".
- Fix: soft-delete + Undo snackbar after the viewer closes, or a confirmation dialog.
- Confidence: high

### [P2] Multi-photo import is cancelled on leaving the gallery; failures are silent
- Category: bug
- Location: `ui/profile/MirrorTestViewModel.kt:122-129` (also `:112`)
- Problem: the sequential import loop runs in `viewModelScope`; each photo is a bounded copy of up to 64 MB plus an `ImageDecoder` pass, so 15 photos take seconds. Back after a few cancels the rest with no message; null results (too big, undecodable) are discarded. `setUserName` already uses `NonCancellable` for this reason.
- Fix: app scope / WorkManager / `NonCancellable`; count failures and surface "N photos couldn't be imported".
- Confidence: high

### [P2] Gallery state lost on rotation / process death
- Category: bug (Compose state)
- Location: `ui/profile/MirrorTestScreen.kt:88-100, 124-133`; `MirrorTestViewer.kt:131-137`
- Problem: no `configChanges`, so rotation recreates the activity; navigation, filter, compare, viewer state and the viewer's drafts are plain `remember`. Rotating mid-note closes the viewer and loses the note. Importing into an open album while the picker is in front, then recreation (rotation or low-memory death): the callback reads reset state (`album = ""`, empty pose/muscle lens) and the photos land in Unsorted untagged.
- Fix: `rememberSaveable` for `showAlbums`, `openAlbum`, `filtersOpen`, `compareMode`; a `Saver` for `GalleryFilter`; save the viewer target as file names + index; per-file saveable drafts; save the pending import context before `picker.launch`.
- Confidence: high (rotation), medium (process death)

### [P2] Rotation loses profile logging drafts — still open from R4 (cover rename is new)
- Category: bug
- Location: `ui/profile/ProfileScreen.kt:202-205`; `BodyweightLogSheet.kt:89-113`; `BodyFatLogSheet.kt:71-87`; `BodyMeasurementLogSheet.kt:53-60`; `BodyMeasurementsScreen.kt:86`; `ProfileHeader.kt:117-118`
- Fix: saveable sheet flags and per-sheet drafts, with a dirty flag so flow updates don't overwrite typed input.
- Confidence: high

### [P2] An unchanged measurement can't be recorded as today's reading — still open from R3
- Category: bug
- Location: `ui/profile/BodyMeasurementLogSheet.kt:53-78`
- Problem: only fields differing from the seed are saved; re-measuring the same 85 cm leaves Save disabled, and an unchanged field is silently dropped when another changes.
- Fix: track "touched" separately from "changed", or add a "same as last" control.
- Confidence: high

### [P2] Measurements can never be deleted or corrected after their day
- Category: bug / dead-code
- Location: `ui/profile/BodyMeasurementsViewModel.kt:62` (`delete` has no callers); `BodyMeasurementsScreen.kt:170-185`; `data/repo/BodyMeasurementRepository.kt:35-46`
- Problem: logging only writes today; a typo like 8.5 cm (passes the 5–300 cm range) is permanent after midnight, skews the sparkline and becomes the "SINCE first" baseline (`BodyMeasurementsScreen.kt:271, 307`; `ProfileSurfaceSections.kt:543-547`).
- Fix: per-type history with delete + Undo wired to the existing `delete(id)`, or allow backdating.
- Confidence: high

### [P2] Before/after share card persists after photo deletion and factory reset
- Category: bug (privacy)
- Location: `ui/profile/BeforeAfterCardRenderer.kt:111-113`; `core/io/ExportFiles.kt:20-24`; `data/repo/ResetRepository.kt:100-110`
- Problem: `exports/avex_before_after_card.png` (both physique photos) is never deleted; `factoryReset` doesn't clear `exports/`; it shows under Storage → Exports. Excluded from backup, which limits exposure. The (parked) rank card has the same issue.
- Fix: write share cards to `cacheDir/share/` or delete after the chooser returns; clear `exportsDir` in `factoryReset`/`deleteAll`.
- Confidence: high

### [P2] Camera shutter has no accessible name; toggles don't expose state — still open from R5
- Category: release (a11y)
- Location: `ui/profile/ProgressCameraScreen.kt:224-229, 217-219, 237-248`
- Fix: shutter `semantics { contentDescription = "Take photo"; role = Button }`, disabled while capturing; `toggleable(role = Switch)` for Ghost/Grid/Timer.
- Confidence: high

### [P2] Permanently denied camera permission has no recovery — still open from R6
- Category: bug
- Location: `ui/profile/ProgressCameraScreen.kt:99-106, 179, 264-279`
- Problem: "Allow camera" re-requests and is denied immediately; no settings path, no re-check on resume.
- Fix: switch to "Open settings" when the denial is permanent; re-check in `LifecycleResumeEffect`.
- Confidence: high

## P3

### [P3] Closing the camera mid-capture drops the shot and orphans it in cacheDir
- Category: bug (lifecycle / privacy)
- Location: `ui/profile/ProgressCameraScreen.kt:121-138, 188`; `ProgressCameraViewModel.kt:65-81`
- Problem: Close stays enabled while capturing; shutter → X cancels `viewModelScope`, so `addCaptured` launches into a dead scope or is cancelled mid-publish. The photo never appears and `cap_*.jpg` stays in cache (factory reset doesn't clear it). `onError` also leaves a partial temp file.
- Fix: disable/confirm Close while capturing; save under `NonCancellable`/app scope; delete temps in `onError`/`onCleared`; sweep `cap_*`.
- Confidence: medium

### [P3] Profile writes are cancellable, unlike the protected `setUserName`
- Location: `ui/profile/ProfileViewModel.kt:92-95, 129-132, 339-356`; `BodyMeasurementsViewModel.kt:58-60`; `MirrorTestViewModel.kt:131-139`
- Problem: sheets close on Save and Back usually follows; the measurement save loops one upsert per type, so Save → Back can persist 2 of 5; the bodyweight read-then-upsert can be cut between steps.
- Fix: a shared `NonCancellable` launch helper; a DAO `@Transaction upsertAll` for measurements.
- Confidence: medium

### [P3] Re-saving an untouched kg/lb or body-fat seed quantises the stored value
- Location: `ui/profile/BodyweightLogSheet.kt:101-103, 116-129`; `BodyFatLogSheet.kt:82-89`
- Problem: the stones branch guards against this (its comment calls it a real bug); kg/lb don't. A Health Connect 180.43 lb seeds "81.8" kg and saves 180.34 lb, then mirrors back to HC. Body fat 18.46 → 18.5.
- Fix: if the text equals the formatted seed, save the stored seed.

### [P3] Viewer weight field drops the comma decimal
- Location: `ui/profile/MirrorTestViewer.kt:384`
- Problem: filter keeps digits and '.', so "99,5" lb becomes "995" (in the sane range) and is saved on the photo.
- Fix: `filterDecimalInput(it).take(6)`.

### [P3] Photo re-date picker allows future dates
- Location: `ui/profile/MirrorTestViewer.kt:421-452`
- Problem: no `selectableDates`; a photo dated 2027 by mistake becomes the permanent "NOW" and gets the wrong bodyweight.
- Fix: reuse `BodyLogDatePickerDialog`'s `SelectableDates`.

### [P3] Profile goes stale after a delete through the day-sheet drill-down
- Location: `ui/profile/ProfileViewModel.kt:152-153, 173-202`; `ProfileScreen.kt:458-464`
- Problem: `load()` runs only in `init`. Day → cardio detail → Delete → Back: the day stays lit, counts unchanged, tapping it opens an empty sheet.
- Fix: refresh in `LifecycleResumeEffect`, or observe a revision flow like photos do.

### [P3] WEIGHT delta uses raw endpoints, so one noisy weigh-in flips the arrow
- Location: `ui/profile/ProfileSurfaceSections.kt:484-495, 562-569` (compare dead `ProfileBody.kt:145-168, 388-400`)
- Problem: the old row smoothed with a 7-day average; the live row shows "↑ 1.0" after one heavy morning during a steady cut.
- Fix: reuse `sevenDayMovingAverage` for series and delta.
- Confidence: medium

### [P3] Silent failures
- Location: `ui/profile/LeanMassViewModel.kt:49`; `ProfileViewModel.kt:339-346`
- Problem: LEAN MASS "Sync" with nothing newer shows nothing (unlike bodyweight/body fat "No newer…"); a failed avatar adopt leaves the cover unchanged silently.
- Fix: return a result; show a message.

### [P3] 13 `Locale.getDefault()` sites despite the `currentLocale()` rule; 4 frozen at class init
- Category: bad-code (locale)
- Location: `MirrorTestControls.kt:139-140, 187-188, 254`; `MirrorTestComponents.kt:161`; `ProfileActivityMonth.kt:397-398`; `BodyweightLogSheet.kt:141, 144`; `BodyFatLogSheet.kt:95, 98`; `BodyMeasurementsScreen.kt:359`; `BeforeAfterCardRenderer.kt:176`; `TrophyCaseSection.kt:210`
- Problem: after a per-app language switch, gallery headers, month search ("june" vs visible "juin"), TalkBack day readings and sheet dates stay in the old language until process death (`ui/common/CurrentLocale.kt` documents this).
- Fix: pass `currentLocale()` into these helpers; drop top-level formatter vals.

### [P3] Mixed decimal separators on one screen
- Location: `ui/profile/ProfileSurfaceSections.kt:450, 502, 521, 574`
- Problem: on a German device BODY FAT reads "18,5" and deltas "↓ 1,2" while SIZES reads "81.5" and volume "1.2k" (`WeightFormatter` pins Locale.US).
- Fix: one formatter.

### [P3] "Delete album" is one tap with no confirm or Undo
- Location: `ui/profile/MirrorTestComponents.kt:282-293`; `MirrorTestScreen.kt:286-289`
- Fix: confirm with photo count, or Undo by restoring the saved `setAlbum` assignments.

### [P3] Gallery strip ghost cells unlabelled; photo cells have no action label
- Category: a11y
- Location: `ui/profile/ProfileExtras.kt:251-261, 336-342`
- Fix: one merged strip node "Open gallery", or label each cell and clear ghost-cell semantics.

### [P3] Compare slider can't be used with TalkBack
- Category: a11y
- Location: `ui/profile/MirrorTestCompare.kt:219-255`
- Fix: `progressBarRangeInfo` + `setProgress` semantics, or overlay a real Slider.

### [P3] ACTIVITY calendar reads future days as "rest day"
- Category: a11y
- Location: `ui/profile/ProfileActivityMonth.kt:270-283, 388-395`
- Fix: pass `isFuture` and read those days as "upcoming".

### [P3] About 1,100 unreachable lines
- Category: dead-code
- Location: `ui/profile/ProfileBody.kt` (all); `ProfileYearHeatmap.kt` (all); `ProfileActivityYear.kt` (all, parked); `ProfileLedger.kt:82-157, 211`; `ProfileExtras.kt:69-176` (GoalLinesSection); `ProfileCharts.kt:116-154` (ProgressRing); `ProfileTiles.kt:49-52` (ChartCaption)
- Problem: no callers in app/wear/shared main or test. These files hold allowlisted doctrine debt; `BodyMeasurementsScreen.kt:362-364` points readers at the dead code. `ProfileLedger.formatVolume` is an internal overload with the same name as `domain.units.formatVolume` but different behaviour (no unit suffix).
- Fix: delete (git keeps them) and shrink the allowlist.

## P4

- **dead-code:** `ProfileViewModel.kt:74-81` + `ProfileScreen.kt:196, 514` (`bodyweightGoalLb` collected only to be suppressed, keeping a goal-table subscription alive); `ProfileViewModel.kt:311-334` (`setPhotoNote`, `addPhoto`, `deletePhoto`); `:94, 131` ("Saved." set after the sheet closes, never seen); `MirrorTestViewModel.kt:112`; `DefaultAvatars.kt:73`; `BodyMeasurementsViewModel.kt:29`; `MirrorTestControls.kt:102, 275`; `GalleryOverview.kt:272` (unused `accent`); `ProfileCharts.kt:44-49` (unused params); `ProfileScreen.kt:181-182, 245` (nullable `onBack` always non-null).
- **dead-code / perf:** parked gamification still computes on every Profile open (`ProfileViewModel.kt:204-219`); the rank-up baseline advances and the celebration flag is set and never cleared, so earlier tier-ups are skipped once the flag turns on. `RankCardRenderer` does file I/O on `Dispatchers.Default`.
- **simplify:** duplicated helpers: `compactCount` vs `formatCount`; the 30-day delta written 4 times with 2 `DELTA_WINDOW_MS` constants; Month vs Year ramp/legend; sheet date labels; `ProgressPhotoImage` vs `GalleryFullImage`; two `share()` copies.
- **bad-code:** state writes during composition (`MirrorTestViewer.kt:116`; `MirrorTestCompare.kt:122`; `MirrorTestScreen.kt:357-362`), latent today.
- **bad-code — stale comments:** `state/ProfileUiState.kt:61-63`; `ProfileSkeleton.kt:24-28` (skeleton doesn't match the layout, so the page jumps on load; 20dp vs 24dp gutter); `GalleryFilterBar.kt:201-209`; `MirrorTestViewModel.kt:63`; `BodyMeasurementsScreen.kt:362-364`; `ProfileScreen.kt:181`.
- **bug (minor):** missing `zone` remember key; "today" computed per photo (`MirrorTestScreen.kt:137-139`; `MirrorTestControls.kt:123`).
- **perf / a11y:** chart paths rebuilt every reveal frame; `BodyweightSparkline` uses raw px strokes; `ProfileSparkline` has no description (`GalleryOverview.kt:323-356`; `ProfileCharts.kt:58-107`).
- **a11y:** "Progress photo" description hard-coded and reused for the avatar; grid cells announce it twice (`ProgressPhotoImage.kt:39`; `ProfileHeader.kt:144`; `MirrorTestComponents.kt:179-181`).
- **bug (minor UX):** compare mode with exactly 1 photo is never exited (`GalleryFilterBar.kt:96`; `MirrorTestScreen.kt:180`).
- **bug (minor):** stones pound field accepts ≥ 14 (`BodyweightLogSheet.kt:194, 126-127`).
- **perf:** main-thread `canonicalFile` per photo cell composition (`ProfileExtras.kt:270`; `MirrorTestComponents.kt:181`; `GalleryOverview.kt:159`).
- **bad-code:** `BeforeAfterCardRenderer.kt:61-65` decodes outside its try.
- **doctrine (allowlisted):** `ProfileHeader.kt:208` `maxLines = 1` on the user's name, `:157` `10.sp`; error-coloured text at `MirrorTestComponents.kt:289`, `ProgressCameraScreen.kt:210`, `MirrorTestViewer.kt:403`, `BodyweightLogSheet.kt:296`.
- **a11y:** cover name/editor unlabelled (`ProfileHeader.kt:189-211`).
- **bug (minor):** lean-mass grant re-checked only in init though KDoc says on resume (`LeanMassViewModel.kt:36-41`).
- **dead-code:** unused imports in `ProfileBody.kt` (10), `GalleryLibrary.kt`, `MirrorTestViewer.kt`, `ProfileHeader.kt`, `AvatarPickerSheet.kt`, `RankCardRenderer.kt`.

## Files reviewed (41)
`ui/profile/`: AvatarPickerSheet, BeforeAfterCardRenderer, BodyFatLogSheet, BodyMeasurementLogSheet, BodyMeasurementsScreen, BodyMeasurementsViewModel, BodyweightLogSheet, DefaultAvatars, GalleryFilterBar, GalleryLibrary, GalleryOverview, GallerySameWeight, LeanMassViewModel, MirrorTestCompare, MirrorTestComponents, MirrorTestControls, MirrorTestScreen, MirrorTestViewer, MirrorTestViewModel, ProfileActivityMonth, ProfileActivityYear, ProfileBody, ProfileCharts, ProfileExtras, ProfileHeader, ProfileLedger, ProfileScreen, ProfileSkeleton, ProfileSurfaceSections, ProfileTiles, ProfileViewModel, ProfileYearHeatmap, ProgressCameraScreen, ProgressCameraViewModel, ProgressPhotoImage, RankCardRenderer, RankEmblem, RankSection, TrophyCaseSection, TrophyIconMotion, `state/ProfileUiState`.
