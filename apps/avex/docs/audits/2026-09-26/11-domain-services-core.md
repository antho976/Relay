# Domain (non-coach), services, core, widget, security, startup, shared

Scope: `domain/**` except `coach`, `adapt`, `engine` (67 files); `service/**` (19); `core/**` (8); `widget/**` (2); `security/**` (5); `appicon/**` (1); `di/**` (2); root `Features`, `ForgeApp`, `MainActivity`, `RestoreApply`, `RestoreManifest`, `StartupGate` (6); `shared/src/main` (6). 116 files, ~11,670 lines, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/` unless noted.

**Counts:** P1 0 · P2 7 · P3 12 · P4 6 grouped blocks (~40 items). Five P2s are still open from 2026-09-12 (W02, W07, W08, W09, D3).

**Checked and correct at HEAD:** lock re-arm on `onStop`; FLAG_SECURE incl. gallery lock; `ProtectionSentinel`; no Activity context in singletons; unique notification IDs; immutable PendingIntents; `specialUse` FGS declarations; per-command ack paths; HR ingest bounds; codec coercion and NewerVersion refusal acks; `PrDetector` guards; comma normalisation/suffixes in unit parsers; quiet hours; restore state machine; `ImageIntegrity`; `StartupGate`; no Log calls, TODOs or StrictMode in release code.

## P2

### [P2] TimeChangeReceiver is never injected; zone/clock-change handling is dead
- Category: bug
- Location: `service/TimeChangeReceiver.kt:32-60`
- Problem: for an `@AndroidEntryPoint` receiver, Hilt injects in the generated `Hilt_TimeChangeReceiver.onReceive()`; this override never calls `super.onReceive`, so `timeSignals` and `reminderScheduler` stay uninitialised. `runCatching` (`:45`, `:55`) swallows the `UninitializedPropertyAccessException`; no test covers the receiver. Fly Auckland → London with the process alive: day dots, streak and next-up wait up to an hour for the `TimeSignals` poll (`:71`), and the reminder keeps the old zone's hour until the next cold start. `TIME_SET` fails the same way. Only the static widget refresh and midnight re-arm run. *Re-checked by the coordinator.*
- Fix: call `super.onReceive(context, intent)` first; drop the `runCatching` around `onSystemTimeChanged`; add a Robolectric test sending `ACTION_TIMEZONE_CHANGED`.
- Confidence: high

### [P2] Haptic hand-off rarely matches, so the phone buzzes and notifies on top of the watch at every rest
- Category: bug
- Location: `service/wear/WearStatePublisher.kt:88-101`; `service/WorkoutSessionService.kt:129-131, 204-216`; `ForgeApp.kt:36`; `ui/gym/train/DayViewModel.kt:146-150`
- Problem: at expiry the controller emits (0, paused); the publisher treats it as structural and sets `lastPublishedTimerEndAtMs = 0` promptly on `Dispatchers.Default`. The service reads the identity only after the VM's `notifyTimerDone` plus two `Dispatchers.Main` hops, so it almost always reads 0, and `hapticAckedFor(0, …)` is always false (`WearConnection.kt:127`). Result: phone vibration + a HIGH-importance notification after the wrist already buzzed.
- Fix: keep the last running end instant until the next start/stop, or pass `endAtMs` in the timerDone event.
- Confidence: medium (dispatcher analysis; no device trace)

### [P2] Cold start REPLACEs the reminder, including the worker that woke the process — still open (wear.md W09)
- Location: `ForgeApp.kt:62-71`; `service/ReminderScheduler.kt:41-44`; `service/TrainingReminderWorker.kt:122-128`
- Problem: when WorkManager starts a dead process for the 18:00 run, `startAppServices` races `doWork`; if REPLACE lands first the running worker is cancelled and the next run is tomorrow. The `schedule` KDoc ("KEEP on boot") contradicts the code.
- Fix: KEEP at boot, REPLACE only on a preference/zone change; better, a calendar one-shot chain with a "posted for date" guard.

### [P2] Weekly recap UPDATE + initial delay never pins Monday — still open (wear.md W08)
- Location: `service/WeeklyRecapWorker.kt:184-211`
- Problem: unchanged since the audit. Install Saturday → first run the Monday after next; reopen Sunday → moves to a Sunday.
- Fix: self-re-enqueuing one-shot per ISO week with KEEP, or `setNextScheduleTimeOverride`.

### [P2] Midnight widget worker REPLACEs itself before refreshing — still open (domain.md D3)
- Location: `service/WidgetMidnightWorker.kt:34-40, 46-62`; `MainActivity.kt:385` re-arms on every Activity creation
- Problem: `schedule()` with REPLACE under the same unique name cancels the running worker inside `refreshForgeWidgets`, so the midnight redraw can be lost.
- Fix: date-scoped unique name with KEEP, or refresh first then enqueue the successor.

### [P2] Widget renders and launches a planned day in freestyle mode — still open (wear.md W07)
- Location: `widget/ForgeWidget.kt:100-118, 130-137, 208-225`
- Problem: `nextDayKey` is resolved before `freestyleMode` is read; freestyle keeps the seed program, so the widget says "PUSH A · 6 exercises" and the tap opens that day.
- Fix: `nextDayKey = if (freestyleMode) null else resolveNextUp(...)`.

### [P2] Watch set commands converted with the phone's current unit — still open (wear.md W02)
- Category: bug (wrong training data)
- Location: `domain/session/SetLogUseCase.kt:137-141`; `shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt:113-129`
- Problem: a wrist "100" in KG is delayed; the phone switches to LB; the set is stored as 100 lb. Needs a unit change during an in-flight command, hence P2.
- Fix: an additive `unit` field on `LogSetCommand` from the displayed `SessionLiveDto.unit`; fall back to the setting only when null.

## P3

### [P3] Whole-kilo weights render as "100.0 kg" and seed "100.0"
- Location: `domain/units/WeightFormatter.kt:45-48, 63-73, 123, 127-131, 196-201`
- Problem: 100 kg is stored as 220.5 lb, which reads back as 100.017 kg, fails `% 1.0 == 0.0`, and formats "100.0 kg". Checked for 20/40/60/80/100/140 kg. Shows in SetRow (`:125`), charts, deltas ("4.0 kg") and input seeds. *Re-checked by the coordinator.*
- Fix: round to one decimal first, then test for an integer, in one helper shared by `trimDecimal` and `formatWeight`.

### [P3] Exported MainActivity lets any app open a day, which creates an active session
- Category: bug (integrity)
- Location: `MainActivity.kt:174, 350-351`; `ui/nav/ForgeNavHost.kt:129-139`; `ui/gym/train/DayViewModel.kt:165`; `AndroidManifest.xml:106-110`
- Problem: an explicit intent with `forge.widget.START_DAY_KEY="upper-a"` (seed keys are constants) opens that day; its VM calls `startOrResumeSession` and starts the foreground service, even behind the app lock. Side effects: phantom "WORKOUT IN PROGRESS", suppressed reminders, a cross-day prompt.
- Fix: point the widget at a non-exported alias/trampoline and honour the extra only there.
- Confidence: medium

### [P3] Boot scopes have no exception handler, so a transient failure crashes the app
- Location: `ForgeApp.kt:36, 62-72, 76-81`
- Problem: `CoroutineScope(Dispatchers.IO).launch { ensureLoaded()… }` is an unhandled root scope; `ensureLoaded` rethrows after `markLoadFailed` (`data/repo/ProgramRepository.kt:103-105`), so a boot IOException crashes instead of leaving readiness FAILED and skips reminder scheduling. `appScope` has the same exposure.
- Fix: launch in `appScope` with a `CoroutineExceptionHandler` that calls `writeCrashLog`; `runCatching` around `ensureLoaded`.
- Confidence: medium

### [P3] WearSyncService `runBlocking` rethrows, crashing the process from the background
- Location: `service/wear/WearSyncService.kt:50-59`
- Problem: `SQLiteFullException` from `hrDao.insertAll` (`WearHrIngest.kt:59`), a ledger ack decode failure (`WearCommandLedger.kt:52`) or the gate's IOException propagate. With storage full, every 5 s HR batch crashes Avex; the watch retries; the phone crash-loops.
- Fix: `runCatching` per branch with a refusal ack.
- Confidence: medium

### [P3] AutoBackupWorker reports cancellation as failure
- Location: `service/AutoBackupWorker.kt:31-48`
- Problem: `catch (e: Exception)` catches `CancellationException`; a stop on attempt ≥3 writes the failure marker and Settings warns about a failed backup.
- Fix: rethrow `CancellationException` or check `isStopped`.
- Confidence: medium

### [P3] Session notification elapsed time resets on resume
- Location: `ui/gym/train/DayViewModelRefresh.kt:215-218` (caller `DaySessionHandlers.kt:291`); `service/WorkoutSessionService.kt:145-151`
- Problem: `startedAtMs = clock.nowMs()` even when resuming; a workout 40 minutes in shows "Just started".
- Fix: pass `now − priorActiveMs`.

### [P3] Reminder says "Rest day · No workout scheduled today" when recovery defers a session
- Location: `service/TrainingReminderWorker.kt:71-75`; `domain/schedule/WeeklySchedule.kt:100-102`; `TrainingRecovery.kt:41-46`
- Problem: `isRestDay` ignores the mode, contrary to its comment; sequence-mode deferrals (overlapping muscles yesterday, or any freestyle session yesterday) and deferred weekday slots get rest-day copy.
- Fix: gate on weekday mode with a blank slot; distinct copy for recovery deferrals.

### [P3] Per-exercise HR rows repeat for supersets or revisited exercises
- Location: `domain/health/SessionHrAnalysis.kt:44-60` (rendered at `ui/gym/session/SessionDetailCharts.kt:305`)
- Problem: spans are consecutive same-name runs; a 3-round A/B superset gives six rows.
- Fix: aggregate `perExercise` by name, weighted by sample count.

### [P3] Gamification off, but Profile still computes XP, rank, standings and trophies
- Category: release (perf)
- Location: `Features.kt:15`; `data/repo/ProfileRepository.kt:121-132, 160-198`; `data/repo/TrophyRepository.kt:62-83`
- Problem: every Profile open runs ~14 DAO aggregates, an `allFinished()` walk, `bestE1rm`, XP/rank/standing/trophy computation, all discarded. (Same finding from the UI side in `03-profile.md`.)
- Fix: wrap in `if (Features.SHOW_GAMIFICATION)` (a const, so the branch is stripped).

### [P3] Academy unlock keys and `LessonUnlock` are dead; the orphan test is a tautology
- Category: dead-code
- Location: `domain/academy/AcademyRegistry.kt:140-206`; `domain/academy/Lesson.kt:22-23, 46-53`; `app/src/test/.../AcademyRegistryTest.kt:25-29`
- Problem: the 28 `UNLOCK_*` constants have no production refs (real unlocks are keyed by lesson id, `data/repo/AcademyRepository.kt:80-110`); `unlockKeyFor` only feeds `orphanLessons()`, which only the test calls; `LessonUnlock` is never rendered and contradicts the real triggers; `byTrack`, `stateOf`, `LessonEventKind.fromCode` unused; KDoc references a non-existent `[Lesson.unlockedBy]`.
- Fix: delete (or render and align); replace the test with "every shipped lesson has a real trigger".

### [P3] Goal target fields strip the comma — cross-slice duplicate of `06-…`
- Location: `ui/goals/GoalEditorScreen.kt:389, 542`

## P4

- **dead-code:** `MainActivity.kt:126, 192-197` `onVolumeDown` never assigned (`onKeyDown` inert); `ForgeWidget.kt:47, 52, 131-134` `EXTRA_RESUME_SESSION` written, never read; `shared/…/WearProtocol.kt:52-53` `KEY_PAYLOAD` unused (still open, wear.md) + orphan KDoc at `:26`; `shared/…/WeightSteps.kt:29` `REP_STEP`; `domain/cardio/CardioPace.kt:28-30` `pacePerKm`; `CardioWeekSeries.kt:84-106` `cardioWeeksOnTarget`, `cardioLoadDeltaPct` test-only; `domain/mood/Mood.kt:9-14` `emoji`; `WatchSessionMirror.kt:135-145` `lastSetWasPr` never read by the watch but costs a `PrDetector` pass per emission (still open, 07 LOW); `WearSyncService.kt:71` timer `START` handled but never sent; `AndroidManifest.xml:622` `DATE_CHANGED` is never delivered to manifest receivers on API 26+ (`TimeChangeReceiver.kt:29-30` claims otherwise).
- **duplication:** `KG_PER_LB` defined 3× (`WeightFormatter.kt:7`, `WarmupEngine.kt:171`, `MetCalories.kt:12`) + a literal in `AcademyFigures.kt:385`; `ReminderScheduler.apply`/`ensureScheduled` identical; `LengthFormatter.formatLengthDelta` identical to `formatLength`; `FreestyleLogModel.weightStepFor` duplicates `WeightSteps`; `WeeklyRecapWorker.kt:163-167` re-implements `VacationCalendar`; `CardioWeekAggregate`/`CardioWeekSeries` bucket the same totals twice; `WatchSessionMirror.kt:80-116` and `SetLogUseCase.kt:80-115` copy slot resolution (still open, wear.md).
- **stale comments:** `RestTimerController.kt:25-31` (still says owned by DayViewModel); `WearSyncService.kt:16-19`; `SessionType.kt:12-15` (FIRST_BACK has a writer at `WorkoutRepository.kt:182`); `TrophyStatsSnapshot.kt:8-9` ("cheap"); `ForgeWidget.kt:58-64` (deep linking "future work"); `RestoreApply.kt:92-97` (`@return` on the wrong fun); `TimeSignals.kt:44`.
- **copy vs DESIGN §11:** em dashes/emoji in rendered strings (`TrainingReminder.kt:42, 48, 53, 62, 67`; `WeeklyRecapWorker.kt:113, 115`; `ForgeWidget.kt:231, 233`); `ForgeWidget.kt:196` shows the raw uppercased day key (use `Program.dayDisplayName`).
- **misc:** `domain/cardio` enums hold Compose `ImageVector`s (domain depends on Compose); `StandingEngine.kt:99` default-locale formatting; `TrophyEvaluator.kt:52` vs `:108` unlock and progress rules disagree for `SessionDurationAtMost`; `HoldFormatter.kt:22` misplaced `MAX_REPS_DIGITS`; `WearCommandLedger.kt:35, 55-62` re-parses legacy JSON per command, no retention (still open); `RestTimerController.kt:191-205` tick read-then-write outside the monitor, so a watch stop can resurrect the timer (still open); `WidgetMidnightWorker` armed nightly even with no widget placed.

## Files reviewed (116)
All of `domain/**` except coach/adapt/engine (67); `service/**` (19); `core/**` (8); `widget/**` (2); `security/**` (5); `appicon/**` (1); `di/**` (2); root Features, ForgeApp, MainActivity, RestoreApply, RestoreManifest, StartupGate (6); `shared/src/main` WearDtos, WearProtocol, WearCodec, WeightSteps, RestTimerController, Clock (6).
