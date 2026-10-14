---
name: perf-android
description: Android performance with Kotlin, Jetpack Compose and Room - cold/warm/hot startup, TTID and TTFD (ReportDrawn), Application.onCreate and App Startup, Baseline and Startup Profiles, Macrobenchmark (StartupTimingMetric, FrameTimingMetric, TraceSectionMetric, CompilationMode) and Microbenchmark, locked clocks and noise, jank budgets, JankStats, Perfetto, Play vitals thresholds (ANR, slow and frozen frames, wake locks), Compose stability and strong skipping, compiler reports, recomposition counts, deferred state reads, lazy list keys and contentType, R8, Room main-thread rules, Flow invalidation storms, indexes and transactions, Paging, WorkManager, wake locks, network and StrictMode. Use for any Android performance, startup, jank, battery or Compose recomposition work.
---

# Android performance

Measure **release builds with R8 on**, on a **real, preferably slower device**. Debug builds are
misleadingly slow (lazy lists especially) and point profiles at the wrong code. Versions below drift;
check the current release notes before pinning one.

## 1. Startup

| Start | What survives | Play "excessive" TTID threshold |
|---|---|---|
| Cold | nothing — process created | ≥ 5 s |
| Warm | the process (activity recreated) | ≥ 2 s |
| Hot | process and activity | ≥ 1.5 s on developer.android.com, 1 s in Play Console help — aim for 1 s |

Those are failure lines, not targets: aim for cold start well under a second on a mid-range phone.

- **TTID** (time to initial display, the first frame) is reported by the system:
  `ActivityManager: Displayed pkg/.Activity: +534ms`. From the shell:
  `adb shell am start -S -W pkg/.MainActivity` (cold, waits, prints `TotalTime`).
- **TTFD** (time to full display, usable content) exists only if the app reports it. In Compose use
  `ReportDrawn()`, `ReportDrawnWhen { items.isNotEmpty() }` or `ReportDrawnAfter { … }`; in Views,
  `fullyDrawnReporter.addReporter()` / `removeReporter()`. Calling it before the first frame makes
  TTFD = TTID.
- **`Application.onCreate` is the usual culprit** (look for `callApplicationOnCreate` in a trace).
  Initialize on first use; replace one ContentProvider per library with the App Startup library
  (`Initializer<T>`, one shared `InitializationProvider`, `tools:node="remove"` + 
  `AppInitializer.initializeComponent()` for lazy ones). Every ContentProvider costs at startup.
- **First activity**: small tree, no I/O or bitmap decode on the main thread, defer non-critical
  composition, show cached state from disk first. Use the `SplashScreen` API, not a splash activity.
- **Field**: `ApplicationStartInfo` (API 35) gives start type and phase timings from real devices;
  `ApplicationExitInfo` (API 30) says why the previous process died.

## 2. Baseline and Startup Profiles

A **Baseline Profile** tells ART which methods to compile ahead of time; Google cites ~30 % faster
code from first launch (case studies: −25 % to −40 % on startup). A **Startup Profile** is a
build-time subset R8 uses to lay startup classes into the primary dex: a further ~15–30 %. R8 is
required for the dex layout gain.

- Producer module `:baselineprofile` (`com.android.test` + `androidx.baselineprofile` plugin,
  `targetProjectPath = ":app"`), and the same plugin plus `baselineProfile(project(":baselineprofile"))`
  in the app.
- Generator: `BaselineProfileRule().collect(packageName, includeInStartupProfile = true) { startActivityAndWait(); /* the critical journeys */ }`.
  Cover only critical journeys — startup, the main screen, one scroll, the most common navigation;
  `includeInStartupProfile` only for startup.
- `./gradlew :app:generateReleaseBaselineProfile`; `saveInSrc` keeps the output committed under
  `src/<variant>/generated/baselineProfiles` so builds do not regenerate it.
- Needs a rooted device, an emulator, a Gradle Managed Device with an AOSP image, or API 33+.
  ART ignores a compiled `baseline.prof` ≥ 1.5 MB. Outside Play, the profile is applied by
  ProfileInstaller on first run (API 24–27) or by background dexopt later; include
  `androidx.profileinstaller`.
- **Prove it helps**: a Macrobenchmark comparing `CompilationMode.None()` with
  `CompilationMode.Partial()` on cold start.

## 3. Macrobenchmark and Microbenchmark

- **Macrobenchmark** measures journeys on a real install from a separate `com.android.test` module.
  The target must be non-debuggable and `<profileable>`, ideally minified: add a `benchmark` build
  type `initWith(release)` signed with the debug key.
- Metrics: `StartupTimingMetric()` (`timeToInitialDisplayMs`, `timeToFullDisplayMs`; read the
  median), `FrameTimingMetric()` (`frameDurationCpuMs`, `frameOverrunMs` at p50–p99; a positive
  overrun is a missed deadline; API 31+), `TraceSectionMetric("Section", Mode.Sum)` for your own
  `trace("…") {}` sections, `PowerMetric` (Pixel 6+, system-wide).
- `StartupMode.COLD` kills the process between iterations; ~10 iterations; every iteration leaves
  a Perfetto trace in the output directory — open the slowest one.
- Compose scroll tests need `Modifier.semantics { testTagsAsResourceId = true }` to find nodes.
- Before Android 14, compilation state does not persist between runs (the APK is reinstalled).
- **Microbenchmark** (`androidx.benchmark:benchmark-junit4`) for hot functions: AOT-compiled,
  warmed, with clock locking (`./gradlew lockClocks` on a rooted device, sustained-performance
  mode otherwise). The JSON records whether clocks were locked and thermal sleeps — read them when
  results are noisy.
- In CI: run on real devices (Firebase Test Lab or a device farm), `dryRunMode.enable=true` on
  pull requests to check benchmarks still run, full runs nightly; treat results as a time series
  and alert on sustained shifts. Emulators and low-battery devices refuse to benchmark.

## 4. Jank

| Refresh | Budget |
|---|---|
| 60 Hz | 16.7 ms |
| 90 Hz | 11.1 ms |
| 120 Hz | 8.3 ms |

A **slow frame** is over 16 ms; a **frozen frame** over 700 ms. An **ANR** is 5 s without
handling input — but people perceive slowness from 100–200 ms.

- **Perfetto system traces** (Android Studio's System Trace, or `record_android_trace`) are the
  ground truth: the FrameTimeline track shows which frames missed and why (main thread, RenderThread,
  GPU, binder). Method tracing adds enough overhead to cause false jank — confirm in a system trace.
- **`trace("name") { … }`** (androidx.tracing) marks your code in traces; each section costs
  ~10 µs, so do not wrap tiny functions.
- **JankStats** (`androidx.metrics:metrics-performance`) reports per-frame jank in the field with
  UI state tags (`PerformanceMetricsState`) so you know *which screen* janked. One instance per
  window; enable in `onResume`, disable in `onPause`; copy `FrameData` out (it is reused).
- `adb shell dumpsys gfxinfo <pkg>` for a quick frame histogram; `am trace-ipc` to find binder
  calls on the main thread.
- **Play vitals**: user-perceived ANR rate bad-behaviour threshold 0.47 % overall (8 % per device
  model), crash rate 1.09 %; excessive slow frames = > 50 % of a session's frames over 16 ms;
  excessive frozen frames = > 0.1 % over 700 ms. Excessive partial wake locks (≥ 2 h in 24 h in
  > 5 % of sessions) is a core vital.

## 5. Compose

**Phases**: composition → layout → draw. Work done in a later phase skips the earlier ones, so read
fast-changing state as late as possible.

- **Strong skipping** is on by default with the Kotlin 2.0.20+ Compose compiler: every restartable
  composable is skippable; unstable parameters compare by identity (`===`), stable ones by
  `equals()`; lambdas are remembered by their captures. So a new instance of an equal-valued
  unstable object (a Room row re-emitted after any table write, a fresh `List`) still recomposes.
  Fix with `@Stable`/`@Immutable` on UI models, immutable collections
  (`kotlinx.collections.immutable`), or a stability configuration file listing types from modules
  without the Compose compiler (`composeCompiler { stabilityConfigurationFile = … }`).
- **Compiler reports** (`composeCompiler { reportsDestination = …; metricsDestination = … }`, on a
  release build): look for composables that are `restartable` but not `skippable`, and `unstable`
  parameters on hot screens. Do not chase skippability everywhere — fix what the profile shows.
- **Recomposition counts**: Layout Inspector → "Show Recomposition Counts" (compositions and
  skips per node). Type or scroll once, then read which nodes recomposed that should not have.
- **Defer reads**: pass `() -> T` instead of `T` down the tree; use lambda modifiers that run in
  layout or draw — `Modifier.offset { IntOffset(…) }`, `graphicsLayer { … }`, `drawBehind { … }` —
  for scroll- or animation-driven values.
- **`derivedStateOf`** when a fast-changing input produces a rarely-changing output
  (`listState.firstVisibleItemIndex > 0`); **`remember(keys)`** for expensive computation — better,
  move it to the ViewModel.
- **Never write state you read in the same composition** (backwards write): infinite recomposition.
- **Lazy lists**: `items(list, key = { it.id }, contentType = { it.kind })`; no zero-sized items
  (give images a size or placeholder); no nested same-direction scrollables; Compose 1.9+
  `LazyLayoutCacheWindow` to prefetch ahead and keep behind.
- **Composition tracing** (`androidx.compose.runtime:runtime-tracing`, API 30+) puts composable
  names into system traces; do not ship the tracing binary in release.

## 6. Room and SQLite

- **Never on the main thread**: one-shot reads and writes are `suspend`; observable reads are
  `Flow`. StrictMode in debug catches stragglers.
- **Flow invalidation storms**: a Room `Flow` re-runs whenever *any* row of *any* table it reads
  changes. Narrow queries (columns, `WHERE`), split hot tables from cold ones, `.distinctUntilChanged()`
  after mapping, and avoid many overlapping Flows on one hot table. Combine with stable UI models so
  re-emitted equal rows do not recompose.
- **Batch writes in one transaction** (`withTransaction` / `useWriterConnection { immediateTransaction {} }`);
  one writer at a time. Per-row queries in a loop are ~1000× slower than one set query.
- **Index** columns in `WHERE`, `ORDER BY`, `GROUP BY` (`@Entity(indices = [Index("a", "b")])`);
  check with `EXPLAIN QUERY PLAN` (`SEARCH … USING INDEX` good, `SCAN` bad). Join in SQL into a
  result class; never lazy-load relations from UI code.
- WAL is Room's default; `synchronous = NORMAL` with WAL trades the last commits on power loss for
  far cheaper writes, without corruption.
- **Paging 3** for long or unbounded lists: DAO `PagingSource` → `Pager(…).flow.cachedIn(viewModelScope)`
  → `collectAsLazyPagingItems()` with `itemKey { it.id }`.
- Room 3 (`androidx.room3`, stable since mid-2026) is KSP-only, driver-based
  (`BundledSQLiteDriver` recommended) and coroutine-first; Room 2.x is in maintenance. Migrating is a
  project decision, not a performance fix.
- `adb shell setprop log.tag.SQLiteTime VERBOSE` logs query times; Perfetto's `database` category
  shows them in traces.

## 7. Battery and network

- **WorkManager** for deferrable work, with constraints (`UNMETERED`, charging, battery not low,
  idle for low-priority work); periodic work is ≥ 15 min; merge tasks that share constraints so the
  device wakes once. Expedited work only for urgent, user-visible work. Frequent
  `STOP_REASON_TIMEOUT` is a red flag (Android 14+ may restrict the app).
- **Wake locks**: avoid; if needed, shortest possible, inside a foreground service, released in
  `finally`, a fixed tag, `acquire(timeout)` as a backstop.
- **Network**: one shared `OkHttpClient` (connection pooling, HTTP/2 multiplexing); close response
  bodies; batch requests to avoid repeated radio wake-ups; register network callbacks instead of
  polling; defer large transfers until unmetered.
- **Persistent connections** (WebSocket, long poll) cost radio time: heartbeat no more often than
  the server needs, back off on failure, and drop the connection when nothing on screen uses it.
- **StrictMode** in debug builds, set in `Application.onCreate`: thread policy `detectDiskReads`,
  `detectDiskWrites`, `detectNetwork`, `detectCustomSlowCalls`; VM policy
  `detectLeakedSqlLiteObjects`, `detectLeakedClosableObjects`, `detectActivityLeaks`.

## 8. Google's order of operations

R8 on → a Baseline Profile for startup and the main journeys → a Macrobenchmark proving it
(`None` vs `Partial`) → frame timing on the main lists → Play vitals for ANRs, wake locks and crashes.

## 9. No SDK in the session?

Without an Android SDK, nothing in `:app` builds and no device benchmark runs. JVM-only modules
(pure Kotlin logic) can still be measured with JMH or plain timing harnesses. Write the
benchmark and the profile setup, say plainly that it was not run, and leave the command to run it.
