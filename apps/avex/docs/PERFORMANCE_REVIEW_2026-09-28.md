# Performance review — 2026-09-28

Goal: every page as snappy as possible — fast cold start, instant push/pop and hub swipes, no jank
while typing, scrolling or resting between sets.

Scope: the whole `forge-android` app module plus `:shared` and the baseline-profile module. The review
was read-only and done by reading code (plus `EXPLAIN QUERY PLAN` against the v38 schema). **Nothing
was profiled on a device, so every timing is an estimate** — confirm the big ones with a Perfetto
trace / Layout Inspector recomposition counts on a *release* build before and after fixing.

Build context that shapes the findings: Kotlin 2.2.10 → Compose **strong skipping is on**. Lambdas are
auto-memoized; unstable params (any `List`/`Set`/`Map`, any `:shared` class) are compared by
**identity**, so a collection allocated during composition makes its child recompose every time.

Paths below are relative to `forge-android/app/src/main/java/com/forge/app/` unless noted.

---

## The five things that matter most

1. **The whole-history adaptation snapshot is rebuilt many times per page, uncached.**
   It is the root cause behind Coach, Home, Stats, Academy, Settings→Coach and the day screen.
2. **No baseline profile ships**, so every screen runs JIT on its first visits.
3. **The launch intro covers every cold start for about 1.3 s**, and longer when themed.
4. **The active-workout screen recomposes whole-screen once a second during rests**, and the finished
   rest bubble recomposes every frame.
5. **Progress photos have no bitmap cache**, and the Profile cover decodes at 2560 px on each open.

---

## 1. Data / engine layer

### P1 — `snapshotCached()` does no caching; one Coach visit builds 4 whole-history snapshots (HIGH)
- **Where:**
  - `data/repo/AdaptationRepository.kt:245` — `snapshotCached() = snapshot()`. The cache was
    removed in b134493 for freshness.
  - `assembleSnapshot()` (`:124-216`) does all of the following:
    - loads every finished session, logged exercise and logged set;
    - reads ~8 prefs;
    - calls `lifeEvents()`;
    - ranks swap candidates for every slot;
    - reads 90 days of Health Connect through sequential IPCs.
- **Callers per visit:**

  | Where | Snapshot builds |
  |---|---|
  | Coach (`ui/coach/CoachViewModel.kt:139-151`): `brief()`, `coachLab()`, `snapshotCached()`, `academyRepo.syncCoachMoments()` | **4, run one after another**, while the page shows an empty Box (`CoachScreen.kt:137`) |
  | Home, every resume/swipe-back: `DirectiveRepository.today()` (`OverviewScreen.kt:359-371`) | 1 |
  | `NotificationFeed.refresh()` → `syncCoachMoments()` + `pendingBanner()` (`ForgeNavHost.kt:178-187`) | 1–2 more; fires twice on cold start (VM `init` + the replayed ON_RESUME) |
  | Stats, Academy, Settings→Coach page, `WearStatePublisher.publishGlanceNow` | 1 each |

- **Stale comments:** several comments still assume the cache exists and are now wrong:
  `CoachRepository.kt:957,1075`, `AcademyRepository.kt:65`, the CoachViewModel doc.
- **Fix:** cache the snapshot on a *generation* counter bumped by Room's `InvalidationTracker` for
  the input tables, plus the engine-input prefs, `programRepo.revision` and day rollover. A `Mutex`
  makes concurrent callers share one build. This keeps the freshness guarantee b134493 wanted: it
  never serves a snapshot across a mutation.
  ```kotlin
  db.invalidationTracker.addObserver(object : InvalidationTracker.Observer(arrayOf(
      "session","logged_exercise","logged_set","mood_entry","cardio_entry","bodyweight_entry",
      "checkin_entry","injury_restriction","vacation_period","program_customization",
      "exercise_customization")) { override fun onInvalidated(t: Set<String>) { gen.incrementAndGet() } })
  suspend fun snapshotCached() = cached?.takeIf { it.first == gen.get() }?.second
      ?: mutex.withLock { cached?.takeIf { it.first == gen.get() }?.second
          ?: assembleSnapshot().also { cached = gen.get() to it } }
  ```
  This needs an app-scoped `CoroutineScope` in DI; none exists yet.
- **Also:**
  - Build the snapshot once in `CoachViewModel.load()` and pass it to `brief(snap)`,
    `coachLab(snap)` and `syncCoachMoments(snap)`.
  - Publish the brief as soon as it is ready instead of after everything.
  - Launch `syncCoachMoments` after state is published.
  - Inside `assembleSnapshot`, run the independent reads with `async`.

### P2 — Health Connect `readRecovery` has no cache and does ~6+ sequential IPCs (HIGH for HC users)
- **Where:**
  - `data/health/HealthConnectManager.kt:485-531`: sleep, then resting HR, then HRV, each paged,
    then steps.
  - `grantedPermissions()` (`:217`) is an uncached binder call, repeated for `canReadSteps()` and in
    `NotificationFeed.refresh`.
  - It is repeated **11×** on the Wearable settings page (`ui/settings/HealthConnectViewModel.kt:170-181`).
- **Fix:**
  - In-memory `HealthSnap` cache with a 5–10 min TTL. Answer the day screen's 18-day read by
    filtering the cached 90 days.
  - Run the four reads in parallel.
  - Fetch `grantedPermissions()` once per call site and derive every flag from that one set.
  - Cache it for about 30 s, invalidated on the permission-result callback.

### P3 — `EngineInputSignals` loads three whole tables just to detect "something changed" (HIGH)
- **Where:** `data/repo/EngineInputSignals.kt:20-38`.
  - It observes all finished sessions, all logged exercises and all logged sets (with a temp-B-tree
    sort), compares them element by element, then discards them.
  - Consumers are Stats and Coach; every visit and every write re-runs all three.
- **Fix:** replace with one-row SQL fingerprints over `invalidationTracker.createFlow(...)`
  (COUNT/TOTAL of ids, reps, weight, rpe…), `distinctUntilChanged()`. Or expose P1's generation as a
  `StateFlow` and delete this class.

### P4 — Stats aggregation recomputed from scratch on every visit and every write (MED-HIGH)
- **Where:**
  - `data/repo/StatsRepository.kt:507-572`: the full set-history join feeds ~12 `build*` passes and
    has no `distinctUntilChanged`.
  - `StatsViewModel.kt:120`: `WhileSubscribed(5_000)` means swiping away from Stats and back after
    5 s recomputes from zero.
  - `:560-566`: one `effectivePlanForDay` query per program day.
- **Fix:**
  - Add `distinctUntilChanged()` after the DAO flow.
  - Share it from a singleton with `shareIn(appScope, WhileSubscribed(60_000), replay = 1)`.
  - Derive planned sets from the list already in hand.

### P5 — Finish / save-and-exit wait on non-UI work before the summary appears (MED)
- **Where:**
  - `data/repo/WorkoutRepository.kt:276-295` awaits, in sequence, `maybeRotateProgram`, the Health
    Connect mirror writes (each with its own permission IPC) and the Glance `updateAll`.
  - Then `ui/gym/train/DaySessionHandlers.kt:170` awaits `trophyRepo.evaluateAndUnlockNew()`: 14
    aggregate queries including full `logged_set` scans. Its own comment says the summary no longer
    shows trophies.
  - Three summary reads then follow one after another.
- **Fix:**
  - Publish the summary first.
  - Run mirrors, the widget refresh and trophies on an app scope.
  - `async` the three summary reads.
- **Related:**
  - The widget refresh is also awaited inside `startMutex` on session **start** (`WorkoutRepository.kt:188`).
  - `ProgramRepository.ensureLoaded()` (`:86-107`) always reloads (N+1 slot queries) and bumps
    `revision`. It is called from every widget update and every `OverviewViewModel` creation, and it
    cascades into Stats/Coach/DayList/Settings/WatchMirror recomputes. Add an
    `if (Program.isLoaded) return` fast path and do a single slots query.

### P6 — Logging a set takes ~10 sequential DB round-trips before the row appears (MED)
- **Where:** `ui/gym/train/DayExerciseHandlers.kt:233-385`, in this order:
  1. ensure the logged exercise;
  2. insert;
  3. timer;
  4. `closeOpenRestEvent`;
  5. `recordSuggestionOutcome`;
  6. `refreshExercise` (≈7 queries + 3 DataStore reads).
- **Also:**
  - Typing a note triggers the same rebuild every 500 ms (`NoteField.kt:85-91`).
  - The watch mirror re-runs `buildDto` plus a Play Services `putDataItem` on **every set, even with
    no watch paired** (`service/wear/WearStatePublisher.kt:64-113`).
- **Fix:**
  - Optimistically append the new `LoggedSet` right after the insert.
  - Run the bookkeeping writes in a separate `launch` and keep `refreshExercise` as background
    reconciliation.
  - For notes, patch the note in state.
  - Gate the watch collectors on `pairedWearApp()`.

### P7 — Smaller data-layer items (LOW–MED)

| Where | Problem | Fix |
|---|---|---|
| `TrophyRepository.snapshot()` (`:63-127`) | CPU work on **Main**, called from `TrophiesViewModel`, with no `flowOn` | `withContext(Dispatchers.Default)` |
| `TrophyRepository` counts | 5 separate COUNT queries | Fold into one query |
| `CoachRepository.coachLab()` (`:956`) | Advisors run on the caller's dispatcher, which is **Main** from Settings→Coach and CoachViewModel | `withContext(Dispatchers.Default)` |
| `RecapViewModel.load()` | Runs on Main | `withContext(Default)` |
| `RecapViewModel` | Compiles `Regex("\\s+")` per row | Hoist the regex |
| `BlockRepository.active()` (`:50-55`) | A read wrapped in `withTransaction`, so it queues behind writers on every Day open | Plain read; transact only in the repair case |
| `SettingsRepository.pref{}` (`:171`) | JSON prefs (`customExercises`, `quietHoursSchedule`, `scheduleHistory`, …) re-parsed on **every** DataStore write by every collector | Dedupe on the raw string before parsing |
| `SettingsRepository` fast-changing keys | Rest-timer keys (written every set) and drafts (every 600 ms) share the main prefs file | Move them to a separate small DataStore |
| `RestEventDao.recent(200)`, `SuggestionOutcomeDao`, `AdviceEventDao` | Full scan and sort | `ORDER BY id DESC` |
| `LoggedSetDao.allForFinishedSessions` | Needless `ORDER BY` (the caller re-sorts) | Drop it |
| Indexes (next migration) | Missing | `cardio_entry(date)`, `bodyweight_entry(recorded_at)`, composite `logged_set(logged_exercise_id, set_index)` |
| `TrophyRepository.unlockMany`, `BodyweightRepository.importHistoryFromHealthConnect` | Row-at-a-time writes, each commit invalidating observers | One transaction or `upsertAll` |
| `OverviewViewModel.kt:219` | Subscribes to the whole weekly-stats chain for one boolean | `sessionDao.trackedFinishedCount() > 0` |
| `observeWeeklyStats()` | Built separately by Overview, `WearStatePublisher` and Overview again | `shareIn` one instance |
| `SessionHistoryViewModel`, `FreestyleTemplateViewModel` | Full-list flows re-emit on any `logged_exercise` write | `distinctUntilChanged()` |
| `StartupGate.kt:44-47` | `runBlocking` on every DB open-helper access | `if (!ready)` fast path |
| `ForgeDatabaseFactory` | Low-RAM devices fall back to TRUNCATE journaling | Force WAL explicitly |

---

## 2. Startup and app shell

### S1 — Launch intro blocks every cold start (HIGH)
- **Where:**
  - `MainActivity.kt:517`: `showIntro = true` on every process start.
  - `ui/common/AvexIntro.kt:107-151`: 320 ms reveal + 700 ms hold + 240 ms fade ≈ **1.26 s**. The
    themed variant is 2.3–3.3 s.
- **Knock-on effects:**
  - With app lock on, the biometric prompt waits for the intro.
  - Home's entrance animations and the NavHost's 320 ms full-screen alpha fade play invisibly
    underneath the plate.
- **Fix:**
  - Play the intro only on first launch after install/update, or once per day.
  - Otherwise shorten it: reveal ~180 ms, hold ≤250 ms, fade 150 ms.
  - Never play it while the lock is active.
  - Drop `rootAlpha` when the intro is covering.

### S2 — No baseline profile ships, and the generator would profile onboarding (HIGH)
- **Current state:**
  - There is no `baseline-prof.txt`.
  - `baselineprofile/.../BaselineProfileGenerator.kt` only launches the app and waits. On a fresh
    install that lands on **onboarding**, not Home, and it never swipes the hub or opens a screen.
- **Fix:**
  - Complete onboarding, via UiAutomator or a debug seed intent.
  - Swipe all hub pages both ways, open and back out of Settings, a gym day, History (with a fling),
    Profile and a lesson.
  - Set `baselineProfile { saveInSrc = true; dexLayoutOptimization = true }` and commit the output.
  - Run the generator on a Gradle Managed Device in CI.
  - Add `ReportDrawnWhen { homeLoaded }`.
- **Expected gain:** typically ~20–30% on cold start plus first-navigation jank.

### S3 — Shell-level work on startup and resume (MED)
- **`Application.onCreate` field-injects eagerly** (`ForgeApp.kt:27-32`, `MainActivity.kt:91-94`),
  building on Main before the Activity exists:
  - the Room database and ~15 DAOs;
  - `ProgramRepository`, `StatsRepository`, `ResetRepository` (18 deps);
  - `WearStatePublisher`, which pulls in `DirectiveRepository` and `AdaptationRepository`.

  Inject `dagger.Lazy<…>` and resolve inside the IO launch. Do the same for
  `MainActivity.importRepo`, which is only used on share-import.
- **WorkManager's first `getInstance()` runs on Main before `setContent`** (`MainActivity.kt:390-393`).
  Move `AutoBackupWorker.schedule` and `WidgetMidnightWorker.schedule` into
  `ForgeApp.startAppServices()`, which already runs on IO; both use KEEP policy.
- **`LocalUnreadNotifications` is a `staticCompositionLocalOf`** provided at the nav root
  (`ui/common/NotificationBell.kt:37`, `ForgeNavHost.kt:201`), so every unread-count change
  recomposes the **whole app tree**. Use `compositionLocalOf`; only the bell reads it.
- **`NotificationsViewModel`** (`init { refresh() }` plus the ON_RESUME observer) double-refreshes
  on cold start. Drop the init call, throttle resume refreshes to ≥60 s, and defer the first one until
  Home has drawn.
- **`ForgeNavHost.kt:83`**: `modalRoutes = setOf(...)` is rebuilt on every root recomposition and
  captured by all four transition lambdas, so NavHost can't skip. Make it a top-level `val`.
- **`MainActivity.kt:466-503`**: `uiSettings` is 15 DataStore collectors through 10 `combine`s on
  Main, starting from `ForgeUiSettings()` defaults. The first frame may compose with the default
  accent and units, then recompose the whole theme. Derive it from one `allPreferences.map{}` and
  seed it from `startupPreferences()`.
- **Wear Play-Services calls run at every process start** even with no watch (`ForgeApp.kt:91`).
  Defer them or gate them on a paired watch.
- **Double full-screen gradient fill** (the window background plus the `ForgeTheme` root
  background). Call `window.setBackgroundDrawable(null)` after the first frame.
- **Widget `updatePeriodMillis = 3600000`** wakes the whole process hourly even though explicit
  refreshes plus a midnight worker already exist. Set it to 0.
- **`proguard-rules.pro`**: `-keepclassmembers enum *` disables R8 enum unboxing app-wide, including
  libraries. Scope it to `com.forge.app.**`.

### S4 — Hub pager composes pages on demand (MED)
- **Where:** `ui/nav/HubScreen.kt:120` leaves `beyondViewportPageCount` at 0.
- **Effect:** the first swipe to Cardio/Stats/Coach/Academy composes the whole screen *and* starts its
  ViewModel's loads mid-gesture. Home is torn down and rebuilt when you go two pages away.
- **Fix:** after Home has drawn, pre-create the adjacent hubs' ViewModels, and/or raise
  `beyondViewportPageCount` to 1 after about 1.5 s. Measure with `FrameTimingMetric`.
- **Smaller issues in `HubScreen`:**
  - `badges = mapOf(...)` is new on every recomposition.
  - `BackHandler(enabled = currentPage != home)` recomposes on every page change.
  - The `HubViewModel` initial values flip the Coach tab count after the first emission.

### S5 — App-wide modifiers and motion (MED)
- **`bounceClick` / `bounceCombinedClick`** (`ui/common/BounceClick.kt`, **107 call sites**) use
  `composed {}`. Every row and card therefore allocates an interaction source, a pressed-state
  collector, an `animateFloatAsState` and a layer. Reimplement as an `IndicationNodeFactory` /
  `Modifier.Node`: zero composition cost, same visuals.
- **Group-kit members** (`ForgeGroups.kt:169-225`) carry about 9–11 animation states each
  (4× `animateDpAsState` for corners, several `animateColorAsState`). `ForgeGearTile` allocates them
  for targets that never change. Drive each member from one `updateTransition` float and derive the
  rest.
- **`ForgeSwitch`** has 5 animation states per switch.
- **The `ForgeGroups` segmented thumb** reads its offset in composition.
- **Entrance motion** (`ForgeMotionKit.kt:41-56`, `ForgeNavHost.kt:209-232`): a 320 ms push, then a
  per-section stagger of 45 ms × index into a bouncy spring. The page "arrives" twice and the last
  section settles ~0.8–1 s after the tap. Use a 20 ms step capped at 3 and `snappy()`, or skip the
  stagger after a nav transition. Consider a 240–260 ms push/pop and a shorter exit fade.
- **`rememberDrawProgress` / `rememberCoachDraw`** (`ForgeMotionKit.kt:97-108`,
  `ui/coach/CoachCharts.kt:41-51`) return `anim.value`. That value is read in composition, so every
  chart recomposes every frame for ~900 ms on entry, which is exactly during the transition. There
  are ~15 callers: Stats, SessionDetail, ExerciseChartSheet, CardioPaceTrend, Coach, Profile and
  SurfaceKit. Return `State<Float>`, read it only in `drawWithCache { onDrawBehind { … } }`, and build
  paths once per size.
- **`SurfacePalette`** (`ui/experiment/SurfaceKit.kt:88-134`) holds a `List<Color>` and is rebuilt on
  every call, so Home/Profile children taking `palette` never skip. Mark it `@Immutable` and
  `remember` it.
- **Add a Compose stability config** (`kotlin.collections.*`, `java.time.*`, the `:shared` timer
  state) and turn on compiler reports.

---

## 3. Active workout (gym/train, freestyle, cardio)

### W1 — Rest timer copies all of `DayUiState` every second (HIGH)
- **Where:** `ui/gym/train/DayViewModel.kt:168-173`:
  `restTimer.state.collect { _state.update { it.copy(restTimer = timer) } }`.
- **Per-tick cost:**
  - The whole `DayScreen`, the LazyColumn DSL and every visible item recompose.
  - Several lists are re-allocated during composition (`DaySessionContent.kt:136, 232, 259, 266,
    271-272, 354`): `sessionDone`, `otherSetTimes`, `upcomingExercises`. The new lists stop
    `ExerciseCard` from skipping.
  - `ExerciseCard` then re-compiles `Regex("""\d+""")` (`ExerciseCard.kt:55,371`) and re-formats
    strings.
  - `SessionHero` walks every logged set twice.
  - With the swap sheet open, `ExerciseLibrary.swapCandidates` re-runs every second
    (`DayScreen.kt:209-215`).
- **Fix:**
  - Expose the timer as its own `StateFlow` instead of copying it into `DayUiState`, and read it only
    in the FAB, the inline timer slot and the haptics effect.
  - `remember` the derived lists keyed on `state.exercises`.
  - Pass primitives to `SessionHero` and `TimeFitRow`.
  - Hoist the regex.

### W2 — Finished rest bubble recomposes every frame (HIGH, battery too)
- **Where:** `train/components/RestTimerBubble.kt:135`. `finishedPulseScale()` (an infinite
  transition) is read in composition.
- **Effect:** the bubble rebuilds its `Brush`, `drawBehind`, semantics and click lambdas at 60–120 Hz
  for as long as "ready" is showing. That is often minutes, with `keepScreenOn` holding the screen
  awake.
- **Fix:**
  - Read the scale in `graphicsLayer {}` and the ring colour in `drawBehind {}`.
  - `remember` the brush.
  - Consider a finite `repeatable(8)` pulse.

### W3 — Freestyle logger recomposes whole screen once a second (HIGH)
- **Where:** `ui/gym/freestyle/FreestyleLogScreen.kt:113-115, 319-329`. A `produceState` 1 Hz clock
  is read in the screen body.
- **Effect:** every card re-parses its sets, and the ledger rows get new tag lists each tick.
- **Fix:** read the clock only in a small `ElapsedText`, and `remember` `recentMoves` and `exclude`.
- **Related:** `LastSessionStrip` runs its own second, out-of-phase 1 Hz clock
  (`ExerciseCardComponents.kt:258-267`).

### W4 — History search TextField goes through an async pipeline (HIGH)
- **Where:** `ui/gym/history/SessionHistoryScreen.kt:109-113`. The field's text comes back from
  `state.query` only after `combine` + `flowOn(Default)` + a full history filter.
- **Effect:** input lag, dropped or reordered characters, and cursor jumps when typing fast.
- **Fix:** hold the text locally with `rememberSaveable` and push it to the VM, as
  `NotesSearchViewModel` already does. Precompute lower-cased search keys.

### W5 — Medium items
- **Day open (M4):** shows "Loading session…" until ~10 sequential awaits finish. Render the plan
  skeleton immediately, since `dayPlan` is synchronous. `async` `getDayName` and the settings reads.
  `readinessScale()` is called without a snapshot, so cards get built twice (fixed by P1/P2).
- **`CardioScreen.kt:138-182`:** a plain `when` tears down the overview on every sheet open, losing
  the scroll position and replaying the entrance animations. Use `SaveableStateProvider` or draw the
  sheet as an overlay.
- **`CardioLogSheet.kt:95-163`:** every field is read at function scope, so each keystroke recomposes
  the whole form. Use a `@Stable` form holder with `derivedStateOf` for `canSubmit`.
- **Exercise browser and template picker:** they lower-case ~800 names on every keystroke on Main.
  Precompute the keys.
- **Empty-state flash:** DayList shows "No plan yet" and History shows "No sessions yet" before the
  first emission. Add `isLoading`.
- **Exercise switch nests `AnimatedContent`'s SizeTransform inside `animateContentSize()`**, so the
  list relayouts for 300 ms. Use `SizeTransform { _, _ -> snap() }`.
- **Freestyle IME:** `isImeVisible` is read at screen scope. Read it inside `bottomBar`.
- **`SessionDetailViewModel.kt:49-72`:** HR analysis runs on Main. Use `withContext(Default)`.
- **Missing `contentType`** on mixed lists: Cardio, Freestyle, AddExerciseSheet, TemplatePicker.

---

## 4. Home, Coach, Profile, Trophies, Recap

### H1 — Home headline and CTA change after first paint (HIGH)
- **Where:** `OverviewScreen.kt:359-371`. The resume observer lives in a pager page that is rebuilt
  on every swipe, and `addObserver` replays ON_RESUME. So `refreshDirective()` (a full snapshot) plus
  `refreshMovement()` (Health Connect) run on cold start, on every swipe back to Home and on every pop.
- **Effect:** `_directive` starts `null`, so the hero first shows the fallback. A few hundred ms later
  the headline, reason and **main button** change (e.g. "Start session" → "Train anyway"), under the
  user's thumb.
- **Fix:**
  - Memoize `TodayAnswer` on (local day, engine-input generation) and seed the VM synchronously from
    `peek()`.
  - Move the resume refresh into the VM with a TTL.
  - Take steps from `snapshot.health.dailySteps`.

### H2 — Home first frame shows empty-state copy until ~16 flows report (MED-HIGH)
- **Where:** `OverviewViewModel.kt:127-212`.
- **Effect:** a returning user briefly sees "Your first session lands here." and "Pin a goal".
  `observeWeeklyStats` computes a streak over every finished timestamp that Home never displays.
- **Fix:**
  - Add a `loaded` flag, or split per-section flows.
  - Give Home a lean weekly query.
  - Use `SharingStarted.Eagerly` for the landing tab.

### H3 — Progress photos: no memory cache; 2560 px cover decode on each Profile open (HIGH)
- **Where:**
  - `ui/profile/ProgressPhotoImage.kt:27-49`: the bitmap lives only in `produceState`.
  - `ProfileHeader.kt:142`: `reqPx = 2560`, ≈12 MB, for a ~1080×960 banner. Its comment says "the
    decoder caches by path", which is not true.
  - `core/io/OrientedBitmaps.kt:90-104`: opens the file 3×, then scales, then rotates, all as
    software bitmaps, so the GPU upload stalls the render thread mid-transition.
  - Decodes run on unbounded `Dispatchers.IO`, so a fling launches dozens.
  - The viewer pager has no neighbour prefetch.
- **Fix:**
  - A process-wide `LruCache<String, ImageBitmap>`, keyed on path + lastModified + reqPx, sized to
    about 1/6 of the heap.
  - Decode on `Dispatchers.IO.limitedParallelism(3)`.
  - `copy(Bitmap.Config.HARDWARE)` (minSdk 26 allows it).
  - Decode the cover at banner size, or store a pre-cropped banner.
  - Generate ~512 px thumbnails at import.
  - Set `beyondViewportPageCount = 1` on the viewer.
  - Keep the parsed photo index in memory. `ProgressPhotoRepository.photos()` re-parses JSON and
    stats every file on each call.

### H4 — Medium and low items
- **Profile:**
  - `load()` waits on ~6 sequential reads before applying `profileRepo.cached()`, including blocking
    `exists()` and `lastModified()` on Main (`ProfileViewModel.kt:189-218`, `AvatarRepository.kt`).
    Apply the cache synchronously first.
  - `avatarFile = viewModel.avatarFile()` is a new `File` on every recomposition
    (`ProfileScreen.kt:236`), so the header never skips.
  - The `bodyweightGoalLb` collection is unused.
- **Trophies:** CPU work on Main (see P7).
- **Recap:** runs on Main with a per-row regex (see P7).
- **Gallery search** (`MirrorTestControls.kt:120-134`): builds per-photo search text and a regex on
  every keystroke on Main. Precompute them in the VM.
- **Compare slider** (`MirrorTestCompare.kt:200-221`): `.offset(x = …)` reads the drag in
  composition. Use the `offset { }` lambda and `remember(after) { fileFor(after) }`.
- **Formatters:**
  - A `SimpleDateFormat` is built per cell in `MirrorTestComponents.kt:197,271`, `GalleryOverview`,
    `ProfileExtras`, `TrophiesComponents` and `MirrorTestViewer.kt:225`.
  - `BodyMeasurementsScreen.kt:354-358` calls `DateTimeFormatter.ofPattern` per row.
- **Coach:** `callCopy` compiles 1–3 regexes per decision (`CoachCall.kt:41,72-73`) and
  `recordHoldLine` compiles another (`CoachUi.kt:72`). Hoist them and `remember` per decision.
- **Missing `contentType`:** `CoachAccount`, `TrophiesScreen`, `GoalsScreen`.
- **`TrophyIconBadge.kt:62-67`:** allocates an `Animatable` and an effect per badge even when not
  animating.
- **`MirrorTestScreen.kt:356`:** `GalleryGridSpec` is rebuilt every recomposition; `remember` it.
- **`remember` needed:**
  - `ProfileSurfaceSections.kt:262`: body-metric lists.
  - `OverviewScreen.kt:643`: `pinnedGoals()`.
- **Snapshot overwork:** the snapshot ranks swap candidates for every slot, with a regex per
  comparison in `ProgramGenerator.kt:341-346`, even for callers that don't use them. Make it lazy.
  `DirectiveRepository` recomputes `lifeEvents` that the snapshot already holds.

---

## 5. Settings, Academy, Onboarding, Program builder

### T1 — Settings paints defaults on first frame, then corrects (HIGH)
- **Where:** `ui/settings/SettingsViewModel.kt:245-359`: `stateIn(WhileSubscribed, SettingsUiState())`
  over ~53 DataStore subscriptions chained through 48 `.combine`s on Main.
- **Effect:**
  - Subtitles flip, and switch thumbs animate from their defaults to the real values on every open.
  - Every DataStore write anywhere re-runs ~53 read lambdas, including a JSON parse, while Settings
    is open.
- **Fix:** an app-scoped `preferences: StateFlow<Preferences>` (Eagerly), then
  `preferences.map { it.toSettingsUi() }`, seeded synchronously with `preferences.value.toSettingsUi()`.
  Move `computeWeeklyVolume()` onto `programRepo.revision`.
- **Related:** the Coach settings page's `loadCoachData()` runs `coachLab()` on Main just to read
  four counts. Add a cheap `recoverySignals()` query.

### T2 — Academy lesson open and hero draw-in (HIGH)
- **`Reader.kt:148-153`:** the whole lesson body is **one** lazy item. Every paragraph, figure
  (`SketchCanvas` + `Animatable` + `TextMeasurer`) and explorer composes and lays out in the
  navigation frame. The first access builds all 19 figure sketches. Emit one item per block with
  `contentType`, build figures per key, and give `SketchCanvas` its own `graphicsLayer`.
- **`AcademySketches.kt:272`:** `rememberTextMeasurer()` has the default cache size of 8, but most
  hero sketches have 9–13 labels. Every label therefore re-lays out on **every frame** of the 3 s
  draw-in. The rotator repeats this every 10 s. Each frame also allocates a `PathMeasure`, `Path`,
  `Brush`, `PathEffect` and `Stroke`.
  - Quick fix: `rememberTextMeasurer(cacheSize = 32)`.
  - Proper fix: `drawWithCache`, with the progress read only in `onDrawBehind`.
- **Explorers** (`AcademyFigures.kt:294-397`): the slider position is read in composition, so every
  drag frame rebuilds the whole `Sketch` (64-segment curves). Build the static base once and draw
  only the marker.
- **`AcademyViewModel.kt:73-83`:** `observeStates()` only starts after `syncCoachMoments()` (a full
  snapshot) returns. All lessons show as unread, then the featured rotator reshuffles. Run the two
  concurrently.
- **`readMinutes()`:** splits all ~46 KB of lesson text on every recomposition (`Lesson.kt:115-127`).
  Compute it once with `lazy`.

### T3 — Onboarding week preview runs the generator on Main on every tap (HIGH)
- **Where:** `ui/onboarding/OnboardingScreen.kt:236-247`: `remember(...) { viewModel.buildPreview(...) }`
  runs `ProgramGenerator.generate` during composition.
- **Effect:**
  - It re-runs on every gear chip toggle, sore-spot toggle and re-roll, even on pages that don't
    show the preview.
  - Each run can compile **hundreds of regexes** via `SessionEstimate.isHeavy`
    (`program/SessionEstimate.kt:91`, `Regex(...)` built per call). The same function is reached
    from `DayCard.estimateMinutes`.
- **Fix:**
  - Hoist the regex, or `ConcurrentHashMap`-memoize `isHeavy`.
  - Move generation to the VM on Default with `mapLatest`, only on the pages that show it.
  - Cache "reps is numeric" per `ExerciseDef`.

### T4 — Medium and low items
- **App-icon picker** (`AppIconPicker.kt:190`): 23 WebP previews decode on Main via `painterResource`
  every time the sheet opens. Decode on IO into a process cache.
- **`SettingsSegmented`** (`SettingsKit.kt:857-1046`): the animated position is read in composition,
  and each row adds its own `BoxWithConstraints`, `TextMeasurer` and 3–5 extra text layouts. The
  Format page has six of these rows. Pass the position as a lambda and read it in `layout {}`.
- **Quiet hours:** `DayOfWeek.entries.forEach { setQuietWindow }` does 7 separate DataStore writes.
  Add a bulk setter.
- **Program → Equipment:** ~40 tiles and 15 chips in a non-lazy `Column`, using
  `IntrinsicSize.Max` (a double measure pass).
- **Settings search** (`SettingsMainList.kt:107-158`): all matches go in one lazy item, un-remembered,
  on every keystroke. Use one item per hit, keyed.
- **Exercise likes list:** no `contentType`, and it captures the whole `state`.
- **Onboarding:** every answer recomposes the root twice (the draft save round-trip,
  `OnboardingScreen.kt:161-163,221-227`).
- **Program builder** (`ProgramBuilderViewModel.kt:226-255`): serialises the whole draft to JSON on
  Main on every edit, including every keystroke. Use a `SavedStateProvider` instead. Loading does
  one slots query per day.
- **Accent picker:** every pointer move recomposes the whole picker and runs ~100 HSV conversions.
- **Binder calls in composition:** `getPackageInfo` in `SettingsAboutPage.kt:62` (use `BuildConfig`),
  and the biometric `availability` checks.
- **`PlanModeMedia`:** decodes animated WebPs at 1128 px for a ~282 dp card.

---

## Suggested order of work

| # | Change | Why first | Risk |
|---|---|---|---|
| 1 | P1 + P2 + P3: generation-keyed snapshot cache, HC cache, cheap change signal | Fixes Coach, Home resume, Stats, Academy, Day open and Settings→Coach in one go | Medium: freshness semantics, needs tests (there's `EngineFreshnessTest`) |
| 2 | S2: real baseline profile + generator journeys | ~20–30% cold start, less first-visit jank everywhere | Low (needs a device/GMD) |
| 3 | S1: intro once per day / shortened | Biggest single "time until usable" | Low (product call) |
| 4 | W1 + W2 + W3 + W4: timer out of `DayUiState`, bubble in draw phase, freestyle clock, local search text | In-gym smoothness and battery | Low–Medium |
| 5 | H1 + T1: seed Home directive and Settings synchronously | Removes visible content flips | Low–Medium |
| 6 | H3: photo LruCache, hardware bitmaps, right-sized cover | Profile/gallery | Low |
| 7 | S5: `rememberDrawProgress` → State read in draw; `bounceClick` as a Modifier.Node; group-kit animations | App-wide transition smoothness | Low–Medium |
| 8 | P5 + P6: fire-and-forget finish work, optimistic set append, gate watch mirror | Tap-to-feedback latency | Medium |
| 9 | Everything else | Polish | Low |

**Trivial one-liners** that can land any time:
- Hoist `Regex` in `SessionEstimate.isHeavy`, `ExerciseCard.recommendedRepsOf`, `CoachCall`,
  `CoachUi`, `RecapViewModel` and `photoMatchesQuery`.
- `rememberTextMeasurer(cacheSize = 32)`.
- `withContext(Default)` in `coachLab()`, `TrophyRepository.snapshot()` and `RecapViewModel.load()`.
- `staticCompositionLocalOf` → `compositionLocalOf` for the unread count.
- Top-level `modalRoutes`.
- `remember { viewModel.avatarFile() }`.
- `ORDER BY id DESC` on the `recent()` DAOs.
- Move the WorkManager scheduling to IO.
- `ProgramRepository.ensureLoaded()` fast path.

## Verified as fine (no action)
- No `allowMainThreadQueries`, and every DAO is suspend or Flow.
- The startup DataStore read is on IO with a timeout.
- Backup/export queries are batched, and the importer uses one transaction.
- The Day LazyColumn has keys and `contentType`.
- `SetRow`/`SetInputRow` keep text state locally.
- Shimmer, confetti, drag offsets and the bounce scale are read in the draw/layer phase.
- The gallery grid is lazy, keyed and has `contentType`.
- `ColorScheme`, typography and shapes are stable/remembered.
- The icon objects are `by lazy`.
- The domain advisors are single-pass (no quadratic loops); the cost is upstream in the snapshot.
