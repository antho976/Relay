# Whole-app audit: Wear, shared protocol/timer, background work and widgets

Revision: `9637809`. Read-only audit. No production or test sources were edited. The root reviewer ran the complete Gradle gate; this report does not claim device execution.

## Retained findings

### W01 [P1] Primary watch set commands lose their identity on recreation/process death

**Files:** `forge-android/wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:72-80,248-260`; `forge-android/wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:312-327,447-450`; `forge-android/wear/src/main/java/com/forge/wear/data/WearEditRecovery.kt:18-26`.

**Trigger:** Tap Log set while the link is dropping, then recreate/kill the watch app before acknowledgement. If the phone accepted the first command but its updated session/ack never reached the watch, reopen against cached state and tap the same visible set again. Keep the exercise on an incomplete multi-set slot so the phone's exercise guard still accepts it.

**Evidence and impact:** `pendingId`, `timedOutId` and the exact retry payload exist only in Compose `remember`; the transport retry is process-scope RAM. `sendLogSet` does not call the durable store. That store's enum handles only RPE and UNDO. Reopening therefore sends a fresh UUID, which the otherwise-correct Room ledger treats as a second set. If the original never arrived, the attempted log disappears without a durable retry instead. The old ledger claim that the watch side is durable is broader than the implemented guarantee.

**Fix:** Persist log UUID plus complete payload before sending, keep it in the repository independently of any screen, and clear only on matching authoritative result/reconciliation. Retry the same identity after recreation. Add an integration regression for accepted-command/lost-ack/watch-recreation and for unsent-command/process-death.

**Confidence:** Source-confirmed boundary. Paired hardware crash/reconnect reproduction unperformed. The fixed phone-side transactional ledger is not being re-reported.

### W02 [P2] A delayed watch command is converted using the phone's new unit

**Files:** `forge-android/shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt:127-146`; `forge-android/wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:312-327`; `forge-android/app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:137-140`.

**Trigger:** Watch displays 100 KG; delay an outgoing log during a connection flap; change the phone's unit to LB before delivery. The current session/slot remains the same.

**Evidence and impact:** The command carries a display-number string and no unit, despite `SessionLiveDto` carrying the unit used to display it. The phone reads current settings at execution and stores 100 lb instead of about 220.46 lb. The reverse direction can inflate the load and may trigger a confirmation rather than preserve the original intent.

**Fix:** Include the unit from the displayed snapshot in the additive command contract, or send a canonical weight value with explicit plate/bodyweight semantics. Convert using that snapshot and retain a documented legacy fallback.

**Confidence:** Source-confirmed semantic mismatch; no device run.

### W03 [P2] The watch timer haptic restarts the full duration after reopening

**Files:** `forge-android/wear/src/main/java/com/forge/wear/ui/WearRoot.kt:87-110`; compare `forge-android/wear/src/main/java/com/forge/wear/ui/TimerView.kt:55-69` and `forge-android/wear/src/main/java/com/forge/wear/data/TimerReceiptAnchor.kt:8-23`.

**Trigger:** A 150-second timer arrives, the watch activity is recreated/reopened after 60 seconds, and the phone link drops before expiry.

**Evidence and impact:** TimerView correctly reads the persisted first-receipt anchor and shows 90 seconds. WearRoot's independent haptic effect instead delays `endAtMs - publishedAtMs`, another 150 seconds. The visible countdown reaches zero a minute before the wrist buzz. With a working link the phone's zero payload masks the bug, which is why disconnected behavior matters. A cached expired running payload can also schedule a late buzz on reopen.

**Fix:** Use the same receipt/deadline calculation for the alert and the display, and retain consumed timer identity outside a composition. Test haptic scheduling across remount with no subsequent phone payload.

**Confidence:** Source/arithmetic confirmed. Existing TimerReceiptAnchorTest tests display math only, so the last audit's display fix is real but does not cover this alert path.

### W04 [P2] Background session arrival does not satisfy the health service's while-in-use permission requirement

**Files:** `forge-android/wear/src/main/java/com/forge/wear/WearApp.kt:27-29,49-61`; `forge-android/wear/src/main/java/com/forge/wear/service/WearHrService.kt:57-75,89-90,168-174`; `forge-android/wear/src/main/AndroidManifest.xml:28-37`; `forge-android/wear/src/main/java/com/forge/wear/MainActivity.kt:27-34,40-63`.

**Trigger:** Grant foreground heart-rate access, leave the watch app, then begin a workout on the phone. The Data Layer causes WearApp to attempt the health FGS from a background application. A specific crash-prone configuration is foreground HR granted and activity recognition denied, so the while-in-use permission is its only eligible health permission.

**Evidence and impact:** The permission check tests the stored grant, not current while-in-use eligibility. The manifest explicitly omits background sensor permissions based on the false assumption that starting a health FGS itself makes access while-in-use. Android documents that this grant check can return granted in the background while service creation is still forbidden. `runCatching` around `startForegroundService` cannot catch a later exception in service `onStartCommand`/`startForeground`. Other platform combinations can simply fail to stream. Returning to the activity with grants already present does not reapply the session, since the session collector deduplicates IDs and MainActivity reapplies only after a new grant.

**Fix:** Reconcile HR startup from a visible activity and stop gracefully if startForeground refuses. If automatic background startup is a requirement, implement the appropriate explicit background permission path and eligibility checks instead. Validate HR-granted/calories-denied and normal-grants cases on supported Wear versions.

**Confidence:** Current source plus platform contract. Exact OEM exception/recovery behavior is a required device check, not an observed crash. Sources: [foreground service types](https://developer.android.com/develop/background-work/services/fgs/service-types), [while-in-use startup restrictions](https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start).

### W05 [P2] A direct session-ID transition reuses the previous session's HR exercise and calories

**Files:** `forge-android/wear/src/main/java/com/forge/wear/service/WearHrService.kt:50-75,146-155`; `forge-android/wear/src/main/java/com/forge/wear/WearApp.kt:27-29,49-61`.

**Trigger:** The watch remains alive with session A while disconnected. Finish A and start B on the phone before reconnecting. Latest-wins DataItems legitimately deliver B directly without an intermediate null.

**Evidence and impact:** `onStartCommand` overwrites `sessionId` but `exerciseStarted` remains true. It does not end/restart the Health Services exercise, reset cumulative `totalKcal`, clear pending samples, or bind chunks to a captured session ID. The old exercise's calorie total is then attributed to B; recent old samples can also pass the phone's one-minute start tolerance. A session change during an awaited chunk can switch later chunks to B.

**Fix:** Handle session-ID replacement explicitly: end and restart the exercise, reset session-owned state, and capture the session identity with each batch. Test A-to-B without a null emission and a switch during a suspended send.

**Confidence:** Source-confirmed state transition. Physical disconnect test unperformed.

### W06 [P2] Wear tile requests block the app's main thread across Data Layer IPC

**Files:** `forge-android/wear/src/main/java/com/forge/wear/glance/WearTiles.kt:34-42`; `forge-android/wear/src/main/java/com/forge/wear/glance/WearGlanceStore.kt:28-42`.

**Trigger:** Render Today/Week tile while Google Play services/DataClient startup or reads are delayed.

**Evidence and impact:** `onTileRequest` calls `runBlocking` for two sequential unbounded DataClient awaits before returning its ListenableFuture. The source comment says binder thread, but the exact cached `androidx.wear.tiles:tiles:1.6.2` binary marks this method `@MainThread`, verified with `javap -v`. This blocks watch UI/lifecycle handling for the whole IPC delay and risks timeout/ANR on a stalled connection. Each fetch also requests every DataItem instead of the one path it needs.

**Fix:** Return an asynchronously completed ListenableFuture immediately (or use a coroutine-compatible tile service), perform bounded reads off the main thread, and query the target paths or one snapshot for both fields.

**Confidence:** Verified against installed dependency bytecode. No runtime latency measurement. [AndroidX TileService source](https://android.googlesource.com/platform/frameworks/support/+/6f3a3e4ec6d310a0b030ea711cfd1b7029118c8d/wear/tiles/tiles/src/main/java/androidx/wear/tiles/TileService.java).

### W07 [P2] Freestyle users still get a planned-workout widget and launch action

**Files:** `forge-android/app/src/main/java/com/forge/app/widget/ForgeWidget.kt:96-117,130-136,211-241`; `forge-android/app/src/main/java/com/forge/app/data/prefs/SettingsRepository.kt:1149-1153`.

**Trigger:** Choose Go with the flow with an existing/seed plan and add or refresh the widget.

**Evidence and impact:** The freestyle setting deliberately preserves the seed program. The widget resolves `nextDayKey` and `nextDayPlan` before reading that setting and uses them unconditionally for both rendering and tap parameters. Freestyle is considered only in the fallback that a retained plan normally bypasses. The user sees/opens a fixed planned workout after explicitly choosing freestyle.

**Fix:** Gate next-plan resolution and next-day launch parameters on `!freestyleMode`, preserving active-session resume behavior. Add a widget state regression with freestyle=true and nonempty Program.days.

**Confidence:** Source-confirmed; no launcher/device screenshot.

### W08 [P2] Weekly recap initial delay and UPDATE semantics do not anchor the promised Monday

**Files:** `forge-android/app/src/main/java/com/forge/app/service/WeeklyRecapWorker.kt:199-209`; called every startup by `forge-android/app/src/main/java/com/forge/app/ForgeApp.kt:59`.

**Trigger:** Install/open Saturday, then reopen Sunday before the first recap.

**Evidence:** Executed the real cached WorkManager 2.10.1 `WorkSpec.Companion.calculateNextRunTime` helper with the production interval, flex, and initial-delay values. Saturday 2026-09-12 12:00 Toronto computes initial-delay target Monday Sep 14 12:00, but the first eligible execution is Monday Sep 21 06:00 because the first flexible period also adds `interval-flex`. UPDATE preserves original enqueue time, so reopening Sunday and replacing initialDelay with the newly computed one-day delay shifts that to Sunday Sep 20 06:00. After the first run, initialDelay is no longer the anchor, so UPDATE does not repair an old arbitrary phase either.

**Impact:** First recap misses the intended upcoming Monday, and repeated opens can phase it onto a different weekday. Delayed brief notifications and outdated week-selection timing follow.

**Fix:** Use WorkManager's explicit next-schedule override carefully, or a one-shot calendar schedule that re-arms after completion, with idempotent week delivery. Test against WorkSpec's actual calculation rather than testing just the date helper.

**Confidence:** Executable library-backed arithmetic probe, not a device scheduler observation. Probe source [AvexWorkScheduleProbe.java](AvexWorkScheduleProbe.java); runner [run_probes.py](run_probes.py); output [validation.json](validation.json). [UPDATE preserves enqueue time](https://developer.android.com/reference/androidx/work/ExistingPeriodicWorkPolicy?authuser=0).

### W09 [P2] Cold-start scheduling can cancel the training reminder that launched the app

**Files:** `forge-android/app/src/main/java/com/forge/app/ForgeApp.kt:62-71`; `forge-android/app/src/main/java/com/forge/app/service/ReminderScheduler.kt:41-43`; `forge-android/app/src/main/java/com/forge/app/service/TrainingReminderWorker.kt:40-44,122-127,137-143`.

**Trigger:** The opted-in reminder becomes due while Avex's process is dead, or a delayed due reminder exists when another background component wakes the app.

**Evidence and impact:** Application startup always calls ensureScheduled, which REPLACEs the unique periodic request. That cancels the existing pending/running work, including the request causing this cold launch, and schedules the next occurrence of the hour. At/after the selected hour this is tomorrow. Whether the current worker posts first is a race, not a guaranteed reminder. The comment claiming next-occurrence rearming can never skip a fire is false for an already-due request.

**Fix:** Make routine boot scheduling preserve existing due/running work; reanchor only on an actual clock/zone/preference change, or use a calendar one-shot chain with separate reschedule identity and delivery idempotency. Add a WorkManager cold-start test with a due request.

**Confidence:** Source and WorkManager cancellation contract; race frequency unmeasured. [Periodic replacement contract](https://developer.android.com/reference/androidx/work/ExistingPeriodicWorkPolicy?authuser=0).

## Additional source-level concerns requiring a targeted concurrency/recovery regression

- `RestTimerController.kt:193-201`: tick reads `current`, calls synchronized remainingNow, then writes a copied snapshot outside the monitor. Binder-thread start/stop/pause can run between the read/calculation/write; canceling a coroutine does not stop its already-running non-suspending body. A stale tick can restore a stopped timer or overwrite a new timer/pause. Synchronize the whole tick transition or use generation-aware atomic state updates. All existing timer tests are single-threaded.
- `WearStatePublisher.kt:100` clears `lastPublishedTimerEndAtMs` at paused-zero, while `WorkoutSessionService.kt:201` only reads it after DayViewModel emits the separate Unit completion event. Publisher and VM/service collectors can race, causing the fallback to capture 0 and double-buzz despite a valid wrist ack. Carry the completed timer identity in the event instead of reading mutable current state.
- Pending undo/RPE negative acknowledgements clear the durable edit and retry line in `WearDataRepository.kt:234-237`; TimerView/RpeScreen do not surface that refusal. Example: offline undo retried after the phone's 15-second window. Transport success is not edit success. Distinguish terminal refusal visibly from a successful edit.
- An unreceived/cold-seeded timer payload's first local receipt is treated as if publication happened now (`TimerReceiptAnchor`/`RestCountdown`), so reconnect delay can extend the displayed rest by that delay. Persisted receipt fixes reopening an already-seen payload, not first receipt of a queued stale one. A clock-sync/deadline protocol should distinguish transport age from clock skew.

These are not included in the retained finding count because this pass did not execute the required interleavings/platform recovery scenarios.

## Cleanup / outdated-code opportunities

- Remove dead `WearProtocol.KEY_PAYLOAD`: transport writes raw `PutDataRequest.data`, never a DataMap; its comment advertises an obsolete contract.
- `WearColors.accentDim`, `accentWash` and `WearGlanceStore.session()` have no scoped/repository call sites. Safe candidates for deliberate removal or documenting planned use, not a claimed release performance gain.
- Consolidate target/current-slot assembly shared by `WatchSessionMirror` and `SetLogUseCase`; CurrentSlotResolver is shared, but most row/plan/swap resolution remains copied. Keep changes paired with contract tests.
- Source contains long historical bug narratives that now contradict behavior or obscure current guarantees, including WearTiles' binder-thread claim, WearHrService's START_STICKY explanation while null intents immediately stop, and ForgeWidget's claim day-navigation remains future work. Move history to audit docs and leave concise invariants.
- WearCommandLedger reparses the complete legacy JSON ledger on each new command while holding the Room transaction, and never migrates/deletes it after upgrade. Import it once transactionally rather than retain per-command migration work forever.
- The new Room wear_command ledger has no retention policy. This is a storage-growth tradeoff, not grounds to delete dedupe records arbitrarily; define a safe command expiry/replay horizon before pruning.

## Verified existing fixes and bounds

Inspected existing phone Room-ledger tests, HR chunking tests, slot resolution, read-only protocol tests, rest math, receipt persistence, edit store, permission split, and haptic ledger tests. The phone transaction/ack replay, bounded HR persistence, signed load display, explicit protocol wire names, existing timer display remount fix, and RPE-vs-log ack classification are present and are not re-reported as absent.

No production edits, no Gradle invocation from this subagent, no release/upload/signing action, no device or emulator run. Root independently reports 1,694 app + 50 Wear + 30 shared tests passed, both lint and release bundles passed, Roborazzi passed, and Android-test compilation passed. This report's additional executable check was the cached WorkManager helper probe; TileService threading was verified with exact installed artifact bytecode. Broad health/data/coach algorithms and phone UI rendering remain with the other reviewers.

`WidgetMidnightWorker` self-cancel finding was independently found and handed to the domain reviewer for a single report entry.

## File coverage

All production Kotlin/config/resource files in `wear` and `shared`, every phone `service` and `widget` source, and the related files listed below were read. Test files listed were inspected to establish regression coverage. Binary assets were not visually rendered. Build outputs are excluded.

Complete-file inspection list (85 files):

- `forge-android/app/src/main/java/com/forge/app/ForgeApp.kt`
- `forge-android/app/src/main/java/com/forge/app/data/db/dao/WearCommandDao.kt`
- `forge-android/app/src/main/java/com/forge/app/data/db/entities/WearCommand.kt`
- `forge-android/app/src/main/java/com/forge/app/data/repo/CustomizationRepository.kt`
- `forge-android/app/src/main/java/com/forge/app/data/repo/NotificationFeed.kt`
- `forge-android/app/src/main/java/com/forge/app/domain/notify/Milestones.kt`
- `forge-android/app/src/main/java/com/forge/app/domain/notify/PrMilestone.kt`
- `forge-android/app/src/main/java/com/forge/app/domain/notify/QuietHoursSchedule.kt`
- `forge-android/app/src/main/java/com/forge/app/domain/notify/TrainingReminder.kt`
- `forge-android/app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt`
- `forge-android/app/src/main/java/com/forge/app/service/AutoBackupWorker.kt`
- `forge-android/app/src/main/java/com/forge/app/service/ForgeNotifications.kt`
- `forge-android/app/src/main/java/com/forge/app/service/ReminderScheduler.kt`
- `forge-android/app/src/main/java/com/forge/app/service/TimeChangeReceiver.kt`
- `forge-android/app/src/main/java/com/forge/app/service/TrainingReminderWorker.kt`
- `forge-android/app/src/main/java/com/forge/app/service/WeeklyRecapWorker.kt`
- `forge-android/app/src/main/java/com/forge/app/service/WidgetMidnightWorker.kt`
- `forge-android/app/src/main/java/com/forge/app/service/WorkoutSessionBridge.kt`
- `forge-android/app/src/main/java/com/forge/app/service/WorkoutSessionService.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/CurrentSlotResolver.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/SessionTimerHolder.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WatchSessionMirror.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearCommandHandler.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearCommandLedger.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearConnection.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearFocusHolder.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearHrIngest.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearStatePublisher.kt`
- `forge-android/app/src/main/java/com/forge/app/service/wear/WearSyncService.kt`
- `forge-android/app/src/main/java/com/forge/app/widget/ForgeWidget.kt`
- `forge-android/app/src/main/java/com/forge/app/widget/WidgetOpenRequest.kt`
- `forge-android/app/src/test/java/com/forge/app/service/wear/CurrentSlotResolverTest.kt`
- `forge-android/app/src/test/java/com/forge/app/service/wear/WearCommandLedgerTest.kt`
- `forge-android/app/src/test/java/com/forge/app/service/wear/WearConnectionTest.kt`
- `forge-android/app/src/test/java/com/forge/app/service/wear/WearHrBatchTest.kt`
- `forge-android/app/src/test/java/com/forge/app/widget/WidgetOpenRequestTest.kt`
- `forge-android/shared/build.gradle.kts`
- `forge-android/shared/src/main/kotlin/com/forge/app/core/time/Clock.kt`
- `forge-android/shared/src/main/kotlin/com/forge/app/domain/timer/RestTimerController.kt`
- `forge-android/shared/src/main/kotlin/com/forge/shared/protocol/WearCodec.kt`
- `forge-android/shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt`
- `forge-android/shared/src/main/kotlin/com/forge/shared/protocol/WearProtocol.kt`
- `forge-android/shared/src/main/kotlin/com/forge/shared/weight/WeightSteps.kt`
- `forge-android/shared/src/test/kotlin/com/forge/app/domain/timer/RestTimerControllerTest.kt`
- `forge-android/shared/src/test/kotlin/com/forge/shared/protocol/LoadAdjustmentTest.kt`
- `forge-android/shared/src/test/kotlin/com/forge/shared/protocol/WearCodecTest.kt`
- `forge-android/shared/src/test/kotlin/com/forge/shared/weight/WeightStepsTest.kt`
- `forge-android/wear/build.gradle.kts`
- `forge-android/wear/proguard-rules.pro`
- `forge-android/wear/src/main/AndroidManifest.xml`
- `forge-android/wear/src/main/java/com/forge/wear/MainActivity.kt`
- `forge-android/wear/src/main/java/com/forge/wear/WearApp.kt`
- `forge-android/wear/src/main/java/com/forge/wear/WearHealthPermissions.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/SessionOngoing.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/TimerReceiptAnchor.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/WearClockSkew.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/WearDataRepository.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/WearEditRecovery.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/WristEditStore.kt`
- `forge-android/wear/src/main/java/com/forge/wear/data/WristHaptics.kt`
- `forge-android/wear/src/main/java/com/forge/wear/glance/WearComplications.kt`
- `forge-android/wear/src/main/java/com/forge/wear/glance/WearGlanceStore.kt`
- `forge-android/wear/src/main/java/com/forge/wear/glance/WearTiles.kt`
- `forge-android/wear/src/main/java/com/forge/wear/service/WearDataListenerService.kt`
- `forge-android/wear/src/main/java/com/forge/wear/service/WearHrService.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/AckResolution.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/RestCountdown.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/SessionScreen.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/TimerView.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/WearComponents.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/WearRoot.kt`
- `forge-android/wear/src/main/java/com/forge/wear/ui/WearTheme.kt`
- `forge-android/wear/src/main/res/drawable/ic_launcher_foreground.xml`
- `forge-android/wear/src/main/res/mipmap-anydpi/ic_launcher.xml`
- `forge-android/wear/src/main/res/values/colors.xml`
- `forge-android/wear/src/main/res/values/strings.xml`
- `forge-android/wear/src/main/res/values/themes.xml`
- `forge-android/wear/src/main/res/values/wear.xml`
- `forge-android/wear/src/test/java/com/forge/wear/WearHealthPermissionsTest.kt`
- `forge-android/wear/src/test/java/com/forge/wear/data/TimerReceiptAnchorTest.kt`
- `forge-android/wear/src/test/java/com/forge/wear/data/WearClockSkewTest.kt`
- `forge-android/wear/src/test/java/com/forge/wear/data/WearEditRecoveryTest.kt`
- `forge-android/wear/src/test/java/com/forge/wear/data/WristEditStoreTest.kt`
- `forge-android/wear/src/test/java/com/forge/wear/ui/AckResolutionTest.kt`
- `forge-android/wear/src/test/java/com/forge/wear/ui/RestCountdownTest.kt`

Boundary excerpts additionally read: `SettingsRepository.kt` (freestyle/unit settings), `SessionDao.kt` (widget/history queries), `DayViewModel.kt` (timer event emission), `gradle/libs.versions.toml` (exact Wear/WorkManager versions). Old reports read: `docs/audit-fixes-2026-09-07.md`, `docs/AUDIT_DEFERRED.md`, relevant original Wear findings in `docs/AVEX_PRODUCTION_SOURCE_AUDIT_2026-09-01.md`.
