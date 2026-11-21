# Avex production source audit

**Audited revision:** [`bc0076f6c059cdd30cc1ffb5a68eece093c3f2c4`](https://github.com/antho976/Avex/commit/bc0076f6c059cdd30cc1ffb5a68eece093c3f2c4) (remote `main`, 2026-09-01)

**Audit date:** 2026-09-01

**Mode:** read-only. No Avex source or configuration was changed.

## Bottom line

The full production surface was inspected, not sampled. The audit retained **61 reproducible bugs** (1 Critical, 15 High, 37 Medium, 8 Low) and **17 bounded performance opportunities** (12 Medium, 5 Low). Passing tests and release builds do not cover the most serious boundaries here: process death during restore, multi-store commits, Health Connect permissions/history, and phone-to-watch command delivery.

Severity means:

- **Critical:** a realistic path can destroy or irrecoverably corrupt user data.
- **High:** a core feature is unusable, materially mis-prescribes training, or can persist materially wrong data.
- **Medium:** meaningful correctness, privacy, reliability, or release impact, usually with a workaround or narrower trigger.
- **Low:** reproducible but bounded behavior with limited downstream impact.

## Complete coverage

The frozen scope contains **792/792 inspected tracked artifacts**:

| Scope | Files | Physical size |
|---|---:|---:|
| Phone production Kotlin | 568 | 99,993 lines |
| Shared production Kotlin | 6 | 660 lines |
| Wear production Kotlin | 18 | 2,236 lines |
| Baseline-profile production Kotlin | 3 | 87 lines |
| Main manifests/resources | 143 | 50 XML files (1,174 lines), 93 raster files (7,916,995 bytes) |
| Exported Room schemas | 30 | 41,211 lines |
| Build, release, signing example, wrapper, CI configuration/scripts | 24 | 2,501 text lines plus the wrapper JAR |
| **Total** | **792** | **102,976 Kotlin lines plus resources/configuration** |

The 568 phone files were partitioned without gaps or overlap:

| Phone area | Files | Lines |
|---|---:|---:|
| Domain | 112 | 13,001 |
| Database, import, Health Connect, preferences | 91 | 9,067 |
| Repositories, services, program/core/security, app entry points | 86 | 16,887 |
| Compose UI and view models | 279 | 61,038 |

Every text source/configuration file was read through EOF; binary artifacts were decoded or archive-validated in full. Callers, tests, generated manifests, lint reports, and build intermediates were inspected when needed to prove or falsify a candidate. Existing bug-scan reports were deliberately not used as finding sources.

The Remotion release-video project, historical documents, test fixtures, and debug-only recipe gallery are not Android production runtime source. They were excluded from the 792-file denominator. Relevant tests and debug/merged manifests were still inspected as validation evidence.

Resource/configuration checks also covered:

- all 50 XML files parsed successfully;
- all 93 PNG/WebP files decoded successfully, with no exact duplicate hashes; a decoded contact-sheet pass, including animated frames, found no visibly corrupt or obviously stale-brand asset;
- all 30 Room schema JSON files parsed and were structurally diffed;
- the Gradle wrapper JAR is a valid archive (`sha256 55243ef57851f12b070ad14f7f5bb8302daceeebc5bce5ece5fa6edb23e1145c`);
- every GitHub Actions workflow, helper script, Gradle script, version catalog, ProGuard file, wrapper property, signing example, and manifest was read completely.

## Validation performed

Validation ran in a second clean clone so parallel audit work could not contaminate Gradle outputs.

| Gate | Result |
|---|---|
| Uncached phone/shared/Wear JVM suites | **1,357 passed**, 0 failed, 0 skipped (phone 1,303; shared 29; Wear 25) |
| Roborazzi golden verification | Passed |
| Phone `lintRelease` | 0 errors, 124 warnings, 12 hints |
| Wear `lintRelease` | 0 errors, 9 warnings, 4 hints |
| Phone minified release APK + AAB | Built successfully (13.8 MB APK, 20.4 MB AAB) |
| Wear minified release APK + AAB | Built successfully (3.17 MB APK, 3.67 MB AAB) |
| Room migrations | All 59 SQL statements across v12 to v36 independently applied; 24/24 target schemas matched; five JVM chain guards passed |
| CI helper scripts | Python compiled; five summary-script tests passed; shell scripts passed syntax checks |

The fresh-clone release artifacts are unsigned because no upload key was supplied. This audit verified buildability and minification, not release signing.

No physical Android/Wear device or Health Connect provider was attached. Device-only repros below are exact recipes backed by complete source/caller tracing and current platform contracts, but are not described as observed device sessions. No observed-device frame-time or jank claim is made without device traces; arithmetic timing, query, tick and allocation bounds are identified as source/platform-derived.

## Severity-ranked findings

### Critical

#### C-01: process death during restore staging can replace live data with an incomplete backup

**Code:** `BackupRepository.kt:768-773, 868-932`; `RestoreApply.kt:121-197`; `ForgeApp.kt:67-78`.

**Reproduce:** start restoring a valid large backup, then kill the process immediately after `pending_restore.db` is created or while `copyTo` is writing it, before `stagedOk = true`. Relaunch Avex. A normal process kill does not execute the producer's `finally` cleanup.

On boot, `RestoreApply` treats existence of any pending component as a complete requested set. It can rename a truncated database live, or combine a complete pending DB with old preferences/photos when death occurred before those components were staged. The rollback snapshot is discarded before the replacement database is successfully opened by Room.

**Impact:** permanent training-history loss, a corrupt live database, boot failure, or an internally inconsistent restore even though the restore UI never returned success.

**Repair:** stage into a unique directory, atomically publish a manifest/`READY` marker last (component names, sizes and hashes), ignore/quarantine sets without that marker, validate again immediately before commit, and retain the prior snapshot until Room opens the replacement successfully.

### High

#### H-01: same-version SQLite impostors pass restore validation, replace the live DB, then fail Room validation

**Code:** `BackupRepository.kt:847-863, 991-1001`; `RestoreApply.kt:191-197`; `ForgeDatabase.kt:152`; schema 36 identity metadata.

**Reproduce:** create a healthy SQLite file with `PRAGMA user_version=36` and three arbitrary tables named `session`, `logged_exercise`, and `logged_set`. It passes every implemented check (`quick_check`, three names, version), despite having no `room_master_table`, required columns, foreign keys, indices, or Avex schema identity. Room rejects it only after it has replaced the live database and the old snapshot has been discarded.

**Impact:** selecting the wrong or crafted SQLite file can turn a reported successful restore into an app that cannot open, without automatic rollback.

**Repair:** validate a disposable copy through the real Room builder and migrations (forcing an open), or verify the complete supported schema/Room identity. Keep rollback data through the first successful production open.

#### H-02: training-block phases are presentation-only, including the scheduled deload

**Code:** `BlockPlanner.kt:9-15, 157-198`; `AutoCoachPlanner.kt:140-200, 245-315`; `BlockRepository.kt:78-93`.

**Reproduce:** run the default five-week block through Accumulate, Intensify, Peak and Deload. Phase names and explanatory copy change, but the program's loads, sets and rep prescriptions do not. `progressionScale` and `volumeDelta` have no production consumer; `isTestWeek` only drives copy. Entering the scheduled Deload phase does not invoke the existing deload-generation path. With fresh/adherent history, `volumeDecisions` can still propose `volume_up` during the week described as reducing volume.

**Impact:** the core opt-in periodization feature states that it changed training when it did not, and its planned recovery week can recommend the opposite action.

**Repair:** make Deload entry generate the actual deload and suppress every structural/volume increase. Thread phase policy into progression, volume and peak-test prescription. Test each phase at the repository/planner integration boundary, not only the state enum.

#### H-03: Coach plan mutation and its decision ledger are non-atomic, so retry can apply a decision twice

**Code:** `CoachRepository.kt:515-579, 645-648, 287-294`; `CoachViewModel.kt:155-167`.

**Reproduce:** apply a proposed `volume_up` to a three-set slot, then inject failure/cancellation/process death after the override becomes four but before `markAppliedNow`. The decision remains proposed. Retrying reads four and raises it to five; its undo state now records four. Swap and rep-shift retries similarly record the already-mutated value, making undo a no-op.

**Impact:** users can receive a larger change than approved, while Coach history and undo lie about what happened. Auto mode reaches the same partial state without a visible recovery path.

**Repair:** revalidate the current decision, mutate the Room overlay, and mark the ledger in one Room transaction. Rethrow `CancellationException`. Cross-store deload work needs a durable pending/saga state with boot reconciliation.

#### H-04: body-fat Health Connect sync cannot receive either permission

**Code:** `HealthConnectManager.kt:83-92`; `app/src/main/AndroidManifest.xml:23-56`; `SettingsRecoveryPage.kt:168-180`; `BodyFatRepository.kt:61-83`.

**Reproduce:** install the release, open Settings > Recovery > Body fat sync, and tap Connect. The UI requests `READ_BODY_FAT` and `WRITE_BODY_FAT`, but neither appears in the source or merged release manifest. `canReadBodyFat` and `canWriteBodyFat` therefore remain false, so both import and write-back are permanent no-ops. Health Connect requires requested data-type permissions to be declared in the manifest first ([Android data-type guidance](https://developer.android.com/health-and-fitness/health-connect/data-types), [write guidance](https://developer.android.com/health-and-fitness/health-connect/write-data)).

**Impact:** both directions of an advertised Recovery feature are dead on real installs.

**Repair:** declare both permissions, update the in-app privacy/Play Health declarations, assert every exposed permission in the merged manifest, then run a real-device grant/read/write test.

#### H-05: the one-time “entire” weight import captures only the ordinary history window, then permanently latches complete

**Code:** `HealthConnectManager.kt:72-81, 236-269`; `BodyweightRepository.kt:124-151`; `HealthConnectViewModel.kt:159-173`.

**Reproduce:** connect a new Avex install to a smart scale with more than 30 days of records. Avex requests only ordinary weight read/write, calls `readWeightHistory(0L, now)`, imports the accessible recent window, then sets `hc_weight_history_imported=true` after any successful non-null read. Older data is silently omitted forever. Health Connect exposes a separate, feature-gated history permission because ordinary reads are limited to data from the 30 days before the first grant ([Android read guidance](https://developer.android.com/health-and-fitness/health-connect/read-data), [permission reference](https://developer.android.com/reference/androidx/health/connect/client/permission/HealthPermission)).

**Impact:** months or years of bodyweight history disappear from the migration path intended to preserve them, affecting trends and bodyweight-relative strength.

**Repair:** declare/request `READ_HEALTH_DATA_HISTORY` only when the provider reports support; latch “entire history imported” only while that grant is active. If declined/unavailable, disclose a partial window and retain a retry path.

#### H-06: personal volume caps compare raw e1RMs from different lifts

**Code:** `PersonalProfile.kt:86-139`; `ProgramRepository.kt:214-237`; `VolumeModel.kt:65-77, 105-129`; duplicated display logic in `InsightEngine.kt:446-489`.

**Reproduce:** alternate four high-volume chest weeks containing a flat 300 lb bench plus a flat 50 lb fly with four low-volume weeks containing only the unchanged 50 lb fly. The code stores the maximum raw e1RM per muscle/week, then subtracts those maxima as if they were one exercise. It invents roughly 250 lb strength swings and can reduce the next generated chest cap by 35 percent without either lift changing. Reverse the exercise mix and the cap can rise 35 percent.

**Impact:** exercise selection is misread as physiology, materially rewriting weekly training volume and presenting it as a learned personal response.

**Repair:** calculate normalized within-exercise change first, then aggregate comparable changes by muscle/week. Share the corrected implementation with `InsightEngine`. Add a constant-strength mixed-lift regression.

#### H-07: a configured weekday rest day is converted into “train today”

**Code:** `WeeklySchedule.kt:57-69`; `DirectiveRepository.kt:68-112`; `TodayDirective.kt:180-219`.

**Reproduce:** use weekday mode with Wednesday blank and Thursday assigned Upper B. Open Overview on Wednesday under otherwise train-ready conditions. `resolveNextUp` scans forward and returns Thursday's key, but drops its day offset. `TodayDirective` receives only a non-null key, emits `TRAIN`, and explains “It's what today's schedule calls for.”

**Impact:** the flagship daily answer can tell someone to train on a deliberately scheduled recovery day and opens the future workout as if it were today's.

**Repair:** return a structured schedule resolution containing key plus offset, or pass today's exact key separately. A blank current weekday must produce Rest; the future key can remain a distinct “next up” value.

#### H-08: Wear command deduplication is neither durable nor acknowledgement-replaying

**Code:** phone `CommandDeduper.kt:12-23`, `WearCommandHandler.kt:24-110`, `WearStatePublisher.kt:153-157, 218-225`; watch `WearDataRepository.kt:309-342`; `SessionScreen.kt:69-109, 228-253`.

**Reproduce A:** let the phone persist a watch set, then kill its process before the ack is published. The watch times out and resends the same command ID. The in-memory deduper was lost, so the same command executes again and logs a second set. **Reproduce B:** make the ack DataItem write fail after the mutation. A same-ID retry hits the live deduper and returns without replaying an ack, leaving the watch permanently at “Not logged”. Marking the ID before execution also means an exception during execution suppresses every same-process retry.

`MessageClient` only queues messages for connected nodes and is intended for ephemeral payloads; persistent delivery belongs on `DataClient` ([official `MessageClient` contract](https://developers.google.com/android/reference/com/google/android/gms/wearable/MessageClient)).

**Impact:** a wrist action can duplicate a logged set after phone process death, or succeed on the phone while the watch can never learn the result.

**Repair:** persist a command inbox/outbox and final result keyed by command ID, replay cached acks for duplicates, and couple command result persistence to the mutation where Room permits. Use durable Data Items for commands that must survive disconnect/process death.

#### H-09: configuration recreation erases and then overwrites an in-progress onboarding setup

**Code:** `OnboardingScreen.kt:136-177, 187-194`; `OnboardingViewModel.kt:40-67`; host in `MainActivity.kt:436-465`.

**Reproduce:** on a fresh install, complete several onboarding pages and wait for the 250 ms draft save. Rotate the phone or resize it in multi-window. The retained ViewModel still exposes only its one-time initial `draftLoad`, usually `Ready(null)`, while roughly twenty screen fields use plain `remember`. The flow returns to page one with defaults and the new blank snapshot is then debounced back to DataStore, overwriting the good draft.

**Impact:** every answer, custom set and generated-plan preview seed can be lost through a normal Android lifecycle event, including the persisted recovery copy.

**Repair:** make the latest draft the ViewModel's synchronous state of truth before queueing the disk write, or hold one `OnboardingDraft` in `SavedStateHandle`. Test composition recreation both before and after the debounce.

#### H-10: changing the onboarding weight unit silently changes the value already entered

**Code:** `OnboardingScreen.kt:147, 155, 214-225, 345-352`; `OnboardingExtras.kt:85-101, 175-212`.

**Reproduce:** on a pounds-default device, enter `170` bodyweight, then tap kg. The text stays `170`; only the suffix and parser unit change. Completing onboarding stores about 374.8 lb. The reverse path stores `80 kg` as 80 lb. Common values remain inside validation, so there is no warning.

**Impact:** the first durable bodyweight row is wrong by a factor of 2.205 and feeds bodyweight trends, relative-strength standards and Coach/readiness inputs.

**Repair:** make unit selection one conversion-aware transition: parse with the old unit, retain canonical pounds, and format in the new unit. Preserve an explicit error for invalid text. Add both direction transitions to state tests.

#### H-11: an open swap sheet can relabel a set arriving from the watch while retaining the old stats identity

**Code:** `DaySwapHandlers.kt:13-72`; `DayScreen.kt:200-219`; `DayViewModel.kt:157-187`; `WorkoutRepository.kt:754-768`; `DayViewModelBuilders.kt:57-68, 234-237`.

**Reproduce:** open a planned exercise's swap picker before any set exists. While it remains open, log set one from the watch, then confirm a different movement on the stale phone sheet, ideally across units. The repository transaction correctly refuses to change `exercise_id` after a set exists, but still writes the new `swappedName` and `swappedUnit`. History/UI now name the chosen movement while PRs, progression and stats remain keyed to the original; later input uses the new unit under the old identity.

**Impact:** performed work is durably mislabeled and can be interpreted in the wrong unit across history and training decisions.

**Repair:** when the transactional set count is nonzero, make `setSessionSwap` return failure without updating any field. Refresh/close the sheet and show the existing message. Persist a future default only after the current-session swap succeeds.

#### H-12: the freestyle Recent rail can append a duplicate ID and crash the logger

**Code:** `ExerciseBrowserScreen.kt:76-89, 136-138, 211-233`; `FreestyleLogScreen.kt:509, 566-576`.

**Reproduce:** add an exercise from the normal browser grid, reopen the browser, then choose the same exercise from Recently performed. The grid applies the `exclude` set but Recent does not, and confirmation blindly appends. Both lazy rows use `ex.libId` as their key, so Compose throws when the duplicate row is measured. The invalid list can also enter the resumable draft and crash again on restoration.

**Impact:** an ordinary freestyle selection path crashes the primary logger and can poison its recovery draft.

**Repair:** filter Recent against `exclude` and enforce uniqueness again at the append mutation boundary; add a Recent/exclusion test and a draft-restoration regression.

#### H-13: Android process recreation silently discards the entire unsaved Program Builder draft

**Code:** `ProgramBuilderViewModel.kt:34-58, 75-87, 115-207`; `ProgramBuilderScreen.kt:84-89, 125-149`; route in `ForgeNavHost.kt:215-226`; `ProgramBuilderDayDetail.kt:94-99, 239-287`.

**Reproduce:** build or edit a multi-day program without saving, background Avex, let Android kill the process while retaining the task, then return. Navigation restores the Program Builder route, but the new ViewModel has an empty/non-dirty heap draft and reloads the last saved program, or an empty blank builder. Every unsaved day, exercise, set/rep and reorder edit is gone, and no discard warning appears because `dirty` reset too. In-flow dialogs/sheets also use plain `remember`, so rotation alone loses their uncommitted substate.

**Impact:** a substantial program document can vanish through normal Android lifecycle recovery while the editor itself appears to restore successfully.

**Repair:** restore the document plus dirty/active-editor state through `SavedStateHandle` or a bounded on-device draft, use stable restored UI keys, and clear only after Save or explicit Discard. Add ViewModel process-recreation and Compose dialog restoration tests.

#### H-14: ForgeMotion applies Android's animator scale twice, so tween timing is squared

**Code:** `Motion.kt:25-26, 40-44, 80-92`; scale observer in `MainActivity.kt:205-215, 289-295`; representative `AvexIntro.kt:52-77` and root navigation callers.

**Reproduce:** set Developer options > Animator duration scale to 2x. Avex first turns a nominal 320 ms reveal into 640 ms, then Compose 1.8's `MotionDurationScale` applies 2x again, so it lasts about 1.28 seconds. At the valid 10x setting, a 240 ms navigation tween takes about 24 seconds, not 2.4; the cold intro spends roughly 56.7 seconds in scaled tweens. At 0.5x, tweens run around 0.25x nominal while springs receive only the platform factor.

**Impact:** the system setting affects every Forge tween quadratically and makes tween/spring pacing disagree across navigation, onboarding, workout logging, charts, Academy, Profile, Home and Coach. High scales make the app practically unusable.

**Repair:** pass nominal durations to Compose and let its motion context apply the platform scale once. Keep a separate observable gate only for raw delays, animated drawables and other non-Compose motion. Test 0.5x and 2x against an animation clock.

#### H-15: the PLATES warm-up suggester treats stored pounds as a machine-plate count

**Code:** `DayScreen.kt:288-307`; `TrainingHelpers.kt:55-90, 149-162`; storage conversion in `SetInputRow.kt:120, 255-263`; engine contract in `WarmupEngine.kt:154-231`.

**Reproduce:** configure 15 lb machine plates, log 4 plates, then open Warm-up sets for that movement. Storage correctly holds 60 lb, but the UI passes `weightLb` into an engine whose PLATES input scale is count. The helper displays **60 Working plates** and calculates the ramp from 60 plates instead of 4.

**Impact:** every PLATES movement with prior/proposed load receives a grossly inflated training prescription.

**Repair:** divide stored/suggested pounds by configured pounds-per-plate at the adapter and rename the parameter to make its scale explicit; test from a persisted set through displayed suggestions.

### Medium

#### M-01: raw Health Connect step rows double-count providers and mis-bucket intervals

**Code:** `HealthConnectManager.kt:451-466, 509-523`.

**Reproduce:** let phone and watch providers each write 1,000 overlapping steps for 10:00 to 11:00. Avex sums both raw rows as 2,000, ignoring Health Connect's source-priority de-duplication. A row spanning 10:30 to 11:30 is also assigned wholly to its start hour; a 23:55 to 00:05 row is assigned wholly to the start day.

**Impact:** Overview/Cardio movement is inflated and the same bad signal feeds readiness and deload logic.

**Repair:** use `StepsRecord.COUNT_TOTAL` aggregation, grouped by local period for days and duration for hours. Android explicitly recommends aggregate APIs for cumulative multi-origin data ([read/aggregate guidance](https://developer.android.com/health-and-fitness/health-connect/read-data)).

#### M-02: deleting or resetting Avex data leaves Avex-authored Health Connect copies behind

**Code:** Health writes in `HealthConnectManager.kt:272-392, 542-620`; only exercise-session deletion at `:623-637`; local-only deletion in `WorkoutRepository.kt:605-609` and `ResetRepository.kt:53-81`.

**Reproduce:** enable workout/calorie write-back, finish a watch-HR gym session, then delete it. Room deletes local rows while Health Connect retains the exercise, calorie and HR records. Factory reset likewise clears local cardio/weight/body-fat data but retains external mirrors. Weight/body-fat writes have no `clientRecordId`, so Avex cannot target them later.

**Impact:** sensitive fitness data survives a UI action labelled “Deletes ALL data”, and downstream Health apps keep counting orphan records.

**Repair:** assign stable client record IDs to every Avex-authored type, mirror individual deletes across all related types, and perform/report external cleanup before local reset keys disappear. Android's synchronization guidance says a client should delete Health Connect data when it is deleted at source ([sync guidance](https://developer.android.com/health-and-fitness/health-connect/sync-data)).

#### M-03: import duplicate detection ignores fields that change workout meaning

**Code:** `WorkoutImportRepository.kt:219-253, 287-372, 492-530`; semantic fields in `ImportModels.kt:17-58`.

**Reproduce:** import a set at timestamp T, then import a valid edited copy with identical exercise/reps/weight/duration but a changed assisted, AMRAP, failure, warm-up/set-type, RPE, difficulty, skipped, note, order, or session classification field. Both fingerprints collapse to the same numeric tuple, so the edited session is discarded as a duplicate.

**Impact:** valid incoming training semantics are silently lost. An assisted pull-up can remain PR-eligible, for example.

**Repair:** include every persisted meaning-changing field in a canonical fingerprint, or use stable source-session IDs for Avex exports and a complete content fingerprint for foreign imports.

#### M-04: a check-in saved across midnight can move or delete a day's existing row

**Code:** `CheckinRepository.kt:31-34, 45-66`; `CheckinDao.kt:14-18`; `CheckinEntry.kt:22-45`.

**Reproduce:** inject clock samples of 23:59:59.999, 00:00:00.000 and post-midnight into one `save`. The lookup can reuse yesterday's primary key while the new entity receives today's unique date key. `REPLACE` deletes conflicts before inserting, so yesterday moves to today or both existing rows collapse into one.

**Impact:** historical readiness input silently disappears.

**Repair:** sample one instant, derive one date key, and key the DAO upsert/update by that same date.

#### M-05: interrupted progress-photo import strands hidden private images that backups still copy

**Code:** `ProgressPhotoRepository.kt:142-190`; `BackupRepository.kt:676-687`.

**Reproduce:** kill the app after destination bytes validate but before the JSON index commits. The `pp_*.jpg` remains after process death, is absent from the gallery and has no UI delete route, yet the next backup ZIP includes every regular file in the photo directory.

**Impact:** sensitive imagery becomes hidden/undeletable in Avex and propagates into backups.

**Repair:** write to a temp and atomically publish with a reconciled index protocol; backup only safely resolved indexed names; delete or quarantine orphan files at startup/before backup.

#### M-06: deload state and generated program can permanently disagree

**Code:** `AdaptationRepository.kt:379-415`; `ProgramRepository.kt:182-200`.

**Reproduce:** fail program generation after `deloadWeekStartMs` is saved, leaving the old full-volume plan labelled as a deload. The inverse occurs when a normal Room program commits and the process dies before the old DataStore marker clears.

**Impact:** rotation and Coach suggestions can be suppressed while the actual workout has the wrong volume/label.

**Repair:** store active-program deload metadata in Room with the program transaction, or use a durable generation ID/pending state plus boot reconciliation.

#### M-07: concurrent workout starts can create multiple active sessions

**Code:** `WorkoutRepository.kt:137-173`; `Session.kt:8-18`; `SessionDao.kt:58-63`.

**Reproduce:** barrier two `startOrResumeSession` calls after both read no active session, then release both inserts. Two rows have `finished_at IS NULL`; the DAO's `LIMIT 1` hides one rather than enforcing the invariant.

**Impact:** a live workout becomes invisible and service/segment state can attach to different session IDs.

**Repair:** serialize selection/insertion in one transaction and enforce a database representation that makes two active rows impossible.

#### M-08: unperformed Coach swaps and rep shifts are recorded as successful and can earn auto-apply trust

**Code:** `OutcomeWatcher.kt:83-118`; `TrustLedger.kt:46-105`; persistence in `CoachRepository.kt:287-293, 389-404`.

**Reproduce:** apply a swap or rep-range shift, then do not perform that target slot for the entire 14-day observation window. The empty history falls through to `ok`; repeated no-exposure “successes” count toward earned trust and eventual automatic changes.

**Impact:** Coach escalates autonomy on fabricated evidence and reports an intervention successful without observing it.

**Repair:** require a post-apply non-skipped target bout (and valid post-change e1RM for rep shifts); otherwise record neutral “not followed” or retain a bounded pending state.

#### M-09: one expired historical layoff can suppress ordinary spacing readiness forever

**Code:** `LifeEvents.kt:183-200`; `ReadinessAdvisor.kt:76-101`.

**Reproduce:** use completed sessions 40, 38, 10 and 2 days ago. The 28-day historical gap returns a non-null layoff even though the return ramp expired. `ReadinessAdvisor` suppresses normal rest-spacing whenever the object exists, not only while `away || returning`, so later fresh/ease-in spacing signals remain disabled.

**Impact:** after one old break, readiness permanently loses a core input.

**Repair:** return null once the ramp expires, or at minimum suppress spacing only while the layoff is active.

#### M-10: failed Wear RPE and undo sends optimistically remove their only retry affordance

**Code:** watch `WearDataRepository.kt:236-248, 309-342`; `SessionScreen.kt:257-342`; `TimerView.kt:82-109`.

**Reproduce:** disconnect the phone, then tap Save RPE. The watch marks `rpeSent=true` before transport, closes the RPE screen, and ignores the eventual `sendWithRetry=false`; the rating is never queued and the rate row disappears. Undo clears `_lastLog` before delivery in the same way.

**Impact:** a user's explicit wrist edit is silently lost during an ordinary disconnect.

**Repair:** keep a pending durable command and remove/resolve the affordance only after a positive ack; surface terminal failure with a same-ID retry.

#### M-11: tagged releases omit the Wear AAB and Wear R8 mapping

**Code:** `.github/workflows/release.yml:149-155, 177-209`.

**Reproduce:** trigger a version tag. The workflow runs phone `bundleRelease`, both APK assemblies, verifies only the phone AAB, and archives only the phone mapping. A local `:wear:bundleRelease` succeeds and produces the missing 3.67 MB AAB plus a 25.3 MB mapping file.

**Impact:** the draft has no Wear bundle for an AAB-based Wear release and no way to deobfuscate this minified Wear build's production crashes. Play manages Wear releases on their own form-factor track and accepts Wear bundles/APKs independently ([Play track guidance](https://support.google.com/googleplay/android-developer/answer/13295490?hl=en-GB), [Wear packaging](https://developer.android.com/training/wearables/packaging)).

**Repair:** build, verify and archive `:wear:bundleRelease`; archive both mappings under unambiguous phone/Wear names; make missing artifacts fail the release.

#### M-12: rotating the cardio log discards every unsaved field

**Code:** `CardioLogSheet.kt:88-137`; host state in `CardioScreen.kt:117-140`; `CustomActivityDialog.kt:58-59`.

**Reproduce:** open a new cardio log or edit an existing one, fill several fields, then rotate or resize before Save. The ViewModel keeps only `sheetOpen` and the original persisted entity; activity, duration, distance, effort, note, time, intervals, HR zone, per-type fields, conditions, and the nested custom-activity draft all live in plain `remember`. A new log becomes blank/default and an edit silently reverts.

**Impact:** a normal configuration change loses unsaved input from a primary logging flow.

**Repair:** keep one saveable/ViewModel-owned `CardioLogDraft`, clear it only on explicit cancel or successful save, and add restoration tests for both a new row and an edit.

#### M-13: concurrent Start taps can create multiple active training blocks

**Code:** `CoachBlock.kt:75-79`; `CoachViewModel.kt:189-200`; `BlockRepository.kt:43-68, 96-99`; `TrainingBlockDao.kt:11-29`.

**Reproduce:** barrier two rapid Start-a-block calls so both `active()` checks return null before either insert. Both auto-ID rows are inserted with `ended_at = NULL`. `ORDER BY started_at DESC LIMIT 1` hides one. Ending the visible block ends only that row, and the hidden block reappears later.

**Impact:** the user can explicitly end a block while another unseen active block continues to drive phase behavior.

**Repair:** serialize start/advance/end at the repository boundary, disable the action while in flight, enforce or repair the singleton invariant in storage, and add a barrier-backed concurrency test.

#### M-14: daily check-in silently drops a nonblank invalid bodyweight and reports success

**Code:** `CheckinViewModel.kt:95, 101-139`; `CheckinSheet.kt:120-149`; parser in `OnboardingScreen.kt:105-123`.

**Reproduce:** enter valid check-in answers plus `8` in Weight, or another nonblank value outside the accepted range, then Save. The nullable parser returns null and the safe-call skips the bodyweight write. `runCatching` still succeeds, closes the sheet and marks today answered without an error.

**Impact:** a typo silently removes the user's weigh-in while the surrounding check-in appears successfully saved.

**Repair:** distinguish blank from invalid before launching writes; keep invalid input open with the existing range error and test blank, invalid, pounds, kilograms and stones paths.

#### M-15: Cardio's current day and week remain stale across calendar and time-zone boundaries

**Code:** `CardioViewModel.kt:87-121, 166-190, 488-539`; `CardioScreen.kt:79-115`; `CardioWeeksViewModel.kt:49-108`; `CardioWeeksScreen.kt:66-82, 112-120`; `CardioWeekDetail.kt:73-81`.

**Reproduce:** leave Cardio visible from Monday 23:59 into Tuesday. The daily signal is reduced to Monday's epoch, so equal values do not re-emit Tuesday through Sunday; Monday remains styled as today and Health Connect today-steps remain stale. Leave Weeks open across Sunday-to-Monday and the new current week is absent while the old one remains in-progress. A time-zone change is also ignored because the zone is remembered without a changing key.

**Impact:** day identity, steps, streak, current-week membership and judged averages can remain wrong until unrelated state changes or route recreation.

**Repair:** carry a local-day/zone signal into both ViewModels, derive today and Monday from it, refresh the bounded current-day Health read on that tick, and test daily, weekly and zone transitions with a fake clock.

#### M-16: a failed progress-camera repository save is presented as success, then its only copy is deleted

**Code:** `ProgressCameraViewModel.kt:43-50`; `ProgressCameraScreen.kt:116-127`; nullable failure contract in `ProgressPhotoRepository.kt:173-190`.

**Reproduce:** make `addCaptured` return null, as its existing corrupt-index test already does, or trigger copy/validation/index failure. CameraX has successfully produced its cache file, but the ViewModel ignores the nullable result and unconditionally invokes `onSaved`. The screen navigates back as if the photo exists; the repository has deleted the cache and failed destination.

**Impact:** a newly captured private progress photo is irrecoverably lost without an error or retry affordance.

**Repair:** navigate only on a non-null repository result, restore a retryable error state on null/exception, and retain the temp file until persistence succeeds.

#### M-17: supported imported image formats can display and export with the wrong orientation

**Code:** `BeforeAfterCardRenderer.kt:198-209`; `MirrorTestViewer.kt:516-527`; `ProgressPhotoImage.kt:80-91`; imports in `ProgressPhotoRepository.kt:137-166`.

**Reproduce:** import an orientation-tagged PNG or WebP on API 26-29. Avex accepts and copies the bytes unchanged but all three renderers use platform `android.media.ExifInterface`; framework support for those containers arrived only in Android 11/API 30. On every API, mirrored orientations 2, 4, 5 and 7 are also ignored because the switch handles only 3, 6 and 8.

**Impact:** thumbnails, full viewer/compare, camera ghost and exported share cards can all be sideways or incorrectly mirrored.

**Repair:** centralize an exact-size decoder using the already-declared AndroidX ExifInterface `rotationDegrees` and `isFlipped`. Cover orientations 1-8 across JPEG/PNG/WebP plus supported HEIF versions. The platform itself recommends AndroidX because the framework class has known issues ([platform reference](https://developer.android.com/reference/android/media/ExifInterface), [AndroidX reference](https://developer.android.com/reference/androidx/exifinterface/media/ExifInterface)).

#### M-18: “Remove folder” leaves Avex's persistent read/write access to that tree

**Code:** `SettingsBackupPage.kt:71-100`; `SettingsViewModel.kt:783-789`; grant acquisition in `BackupRepository.kt:541-552`.

**Reproduce:** choose backup folder A, confirm its read/write entry in `contentResolver.persistedUriPermissions`, then remove it or replace it with B. Avex clears only the DataStore URI; it never calls `releasePersistableUriPermission`, so A remains accessible across reboot and replaced destinations accumulate.

**Impact:** the app retains broader storage capability than its visible connected-folder state says it has.

**Repair:** acquire/persist a replacement first, then explicitly release the old URI's actual held flags; on removal, clear the preference and release the grant. Persisted grants remain until revoked or released ([ContentResolver contract](https://developer.android.com/reference/android/content/ContentResolver#releasePersistableUriPermission(android.net.Uri,int))).

#### M-19: invalid nonblank viewer weight clears a valid stored photo weight

**Code:** `MirrorTestViewer.kt:164-173, 369-390`; sink in `ProgressPhotoRepository.kt:248`.

**Reproduce:** open a photo with a stored bodyweight, enter `.` or `1..2`, then swipe away or close. The field filter permits the text, parsing returns null, and commit cannot distinguish invalid input from an intentional blank, so it writes null over the valid snapshot.

**Impact:** photo metadata and same-weight pairing/delta/share outputs are silently damaged by a typo.

**Repair:** make blank the only clear action; preserve the last committed value and show/gate invalid nonblank text.

#### M-20: editing photo metadata A to B and back to A in one viewer session leaves B on disk

**Code:** `MirrorTestViewer.kt:119-133, 145-190, 197-203`.

**Reproduce:** open a photo whose note is `start`, change it to `progress`, swipe away so that value persists, return, change it back to `start`, then close. Dirty checking still compares against the launch-time snapshot, decides there is no change, and leaves `progress` in the repository. Title and weight have the same A-to-B-to-A failure.

**Impact:** the viewer briefly confirms the requested value while durable metadata remains different.

**Repair:** keep a per-file last-committed baseline and update it after every dispatched mutation, or observe live repository state; test the three fields across page switches and dismissal.

#### M-21: rapid weekly-plan edits can cancel or overwrite another weekday's change

**Code:** `SettingsViewModel.kt:163-174, 183-194`; `SettingsProgramPage.kt:180-205`; whole-list storage in `SettingsRepository.kt:774-783`.

**Reproduce A:** change a weekday and immediately leave Settings. This setter bypasses the cancellation-safe write helper, so ViewModel teardown can cancel it. **Reproduce B:** choose Monday then Tuesday before the first write completes. Both coroutines can read the same seven-slot list and later whole-list write replaces the other weekday's change.

**Impact:** the visible fixed-week schedule can silently revert or persist only part of a multi-day setup.

**Repair:** mutate one weekday inside one repository DataStore edit transaction and invoke it through the durable write policy; add cancellation and barrier-backed two-day tests.

#### M-22: Recovery opt-out and backup-folder actions can be canceled with the destination

**Code:** `HealthConnectViewModel.kt:177-200`; `SettingsViewModel.kt:783-789`.

**Reproduce:** change wearable brand or a Health Connect outbound toggle and immediately leave Settings, using a delayed DataStore to make cancellation deterministic. These actions use bare `viewModelScope.launch`, so the prior value can survive. Folder removal has the same window. A user can therefore opt out visibly while later weights/sessions still mirror, or remove a backup target while scheduled backups retain it.

**Impact:** privacy-relevant preferences can report success in the UI but not survive navigation.

**Repair:** apply the same non-cancellable durable preference-write boundary used elsewhere, limiting it to the small grant/preference mutation rather than any long backup job.

#### M-23: read-only weight permission is presented as working two-way Health Connect sync

**Code:** `HealthConnectViewModel.kt:35-50, 116-140`; `SettingsRecoveryPage.kt:145-159`; permission split in `HealthConnectManager.kt:72-81, 205-211`.

**Reproduce:** grant Weight read but deny or revoke Weight write. `weightGranted` reflects only `canReadWeight`, so Recovery displays connected, claims both-way sync and exposes the outbound toggle. A local weigh-in then saves successfully while `BodyweightRepository` silently skips the Health Connect write.

**Impact:** the app tells users an enabled data mirror is working when its distinct permission is absent.

**Repair:** expose read/write states separately, retain import for read-only users, disable/uncheck outbound sync without write permission, and test the full grant/revocation matrix.

#### M-24: changing pose while a camera capture is in flight mis-tags the saved photo

**Code:** `ProgressCameraScreen.kt:116-127, 175-181`.

**Reproduce:** select Front, tap the shutter, then tap Back before a delayed CameraX `onImageSaved` callback. Pose controls stay enabled and the callback reads current mutable Compose state, so the already-taken Front image is stored as Back.

**Impact:** pose filtering, same-pose comparisons, ghost overlays and share labels classify the photo incorrectly.

**Repair:** snapshot pose with the shutter request and pass that immutable value into persistence; disable capture controls while in flight as a secondary guard.

#### M-25: saving a custom exercise discards its selected muscle

**Code:** `ExerciseBrowserScreen.kt:287-330`; temporary draft in `FreestyleDraft.kt:25-35, 64-70, 119-126`; persistence in `FreestyleLogScreen.kt:397-434` and `FreestyleLogViewModel.kt:35-45, 125-143`; aggregation in `StatsVolumeAggregations.kt:29-58`.

**Reproduce:** create Sled Push with Quads, log it and save. Finished-session persistence carries the ID/name/sets but no custom muscle. Muscle stats and anatomy omit those sets because the ID is absent from Program; reusing the past workout defaults the movement to the first enum value, Chest.

**Impact:** all saved custom movements lose user-selected classification and undercount or misclassify muscle volume.

**Repair:** persist a nullable custom-muscle code on the logged exercise, migrate it through input/template projections, and use it when no Program exercise exists.

#### M-26: lossy custom-exercise IDs merge distinct movements' history and PR state

**Code:** `ExerciseBrowserScreen.kt:92-111`; `FreestyleLogScreen.kt:579-584`; `FreestyleLogViewModel.kt:65-67, 125-143`.

**Reproduce:** create `!!!`, save it, then create `@@@`. Both punctuation-only names map to `custom-exercise`; two names sharing the same normalized first 40 characters collide too. The later movement inherits the earlier movement's last sets and PR frontier, and both cannot coexist in one log.

**Impact:** distinct custom exercises are durably aliased, merging history in a way that cannot later be reconstructed from the stored ID.

**Repair:** persist a UUID-backed custom exercise identity or append a stable digest of the full canonical name; retain the slug only as readable metadata and migrate/alias existing non-colliding IDs.

#### M-27: notification mutations can die with a throwaway destination ViewModel, making Clear all partial with no Undo

**Code:** `NotificationsScreen.kt:75-110`; `NotificationsViewModel.kt:25-42`; duplicate ownership in `ForgeNavHost.kt:141-150, 312-325`; sequential writes in `NotificationFeed.kt:400-404`.

**Reproduce:** seed two dismissible notices and suspend the second settings write. Tap Clear all, let the first finish, then Back before releasing the second. The destination creates its own `hiltViewModel` despite a root-scoped notifications VM already existing; popping cancels its scope. `runCatching` swallows cancellation, so one row disappears, later rows remain, and Undo is never published. Opening a Coach notice has the same cancel-after-immediate-pop boundary for its seen write.

**Impact:** an all-or-nothing user action silently becomes partial, and read state can reappear later.

**Repair:** pass the existing root ViewModel into the route, keep short persistence mutations alive through navigation, rethrow cancellation, and publish Undo only after every dismissal succeeds.

#### M-28: Android 8-12 users who block Avex notifications never get the recovery row

**Code:** recovery action in `NotificationsScreen.kt:96-101`; capability hardcode and hide rule in `NotificationFeed.kt:137-142, 265-284`.

**Reproduce:** on API 26-32, disable all Avex notifications in OS settings and resume the app. Versions below Android 13 unconditionally set `notificationsAllowed=true`, so the feed hides its Turn on notifications row even though every notification is blocked.

**Impact:** the supported pre-13 device range loses the only in-app recovery route after OS-level blocking.

**Repair:** use `NotificationManagerCompat.areNotificationsEnabled()` on every supported API, keeping the explicit “do not remind” preference separate ([AndroidX contract](https://developer.android.com/reference/androidx/core/app/NotificationManagerCompat#areNotificationsEnabled())).

#### M-29: monthly/yearly recap loses custom and late-swapped exercise identity

**Code:** `RecapViewModel.kt:79-82, 107-109`; aggregate query in `LoggedExerciseDao.kt:173-192`; canonical resolver in `Program.kt:320-337`.

**Reproduce:** make custom `Sled Push 2.0` the month's most frequent exercise. Persistence stores its exact `swapped_name`, but the aggregate returns only `exercise_id`, so Recap renders the slug fallback `Custom Sled Push 2 0`. As a downstream compatibility case, existing rows produced by H-11's late relabel are grouped under the preserved original ID and differently named rows can merge into one bucket. Fixing H-11 stops new instances of that second case but does not repair existing data.

**Impact:** both recap periods can name and count the user's most-trained movement incorrectly.

**Repair:** query range rows with `(sessionId, exerciseId, swappedName)`, resolve canonical display identity first, then count distinct sessions by normalized resolved name. Cover custom punctuation and multiple late swaps sharing a base ID.

#### M-30: widget deep links are dropped on repeated warm launches and a cold custom-program race

**Code:** `MainActivity.kt:82-88, 133-140, 271-276, 474-477`; handler in `ForgeNavHost.kt:98-120`; async program load in `ForgeApp.kt:46-54`; seed facade in `Program.kt:280-302`.

**Reproduce A:** tap a widget for `upper-a`, Back to Home, then tap the same widget again while the `singleTask` activity remains. Assigning the same sticky string produces no state change, so the value-keyed effect never runs twice. **Reproduce B:** cold-tap a widget for a custom builder day while delaying `ensureLoaded`. Validation sees only the seed split, rejects the custom key, and never retries when the unchanged string outlives program readiness.

**Impact:** the widget's primary start/resume action silently fails on ordinary repeated use and on a cold custom-program race.

**Repair:** model each intent as a sequenced event, await program readiness before validation, and consume only after routing or explicit rejection.

#### M-31: the Goals lens can hide every remaining goal and remove its own recovery control

**Code:** `GoalsScreen.kt:79-82, 89-116, 159-195`; retained route in `ForgeNavHost.kt:397-415`.

**Reproduce:** with live and reached goals, select Reached, open/delete the last reached goal, then return. The saved `chosenLens=REACHED` remains, but lens pills disappear because only one category exists. The stale filter renders “Nothing reached yet” while live goals exist, with no control to switch back. The inverse occurs when the last live goal disappears.

**Impact:** real goals become inaccessible until the route is recreated.

**Repair:** coerce the effective lens to whichever category remains; honor the saved choice only while both are populated.

#### M-32: weekly/monthly custom-goal progress remains in the expired period after rollover

**Code:** `GoalsViewModel.kt:70-105`; Home flow in `OverviewViewModel.kt:191-212, 283-284`; captions in `GoalsComponents.kt:97-117, 353-365` and `OverviewScreen.kt:225-271`; range computation in `ExtendedGoalRepository.kt:99-167`.

**Reproduce:** leave a completed weekly goal visible from Sunday into Monday without a related database write. Goals and Home keep the old `4 / 4` reached object instead of the new week's `0 / 4`; monthly rollover and time-zone changes behave the same. Captions also memoize wall-clock output by an unchanged goal object.

**Impact:** current value, reached state, ordering, lens membership and deadline copy can all describe an expired period.

**Repair:** include the local-day/zone signal in both progress flows and caption mapping, using the emitted time as explicit input; test week, month and zone transitions without database writes.

#### M-33: a bodyweight goal created before the first weigh-in never gets a stable baseline

**Code:** creation in `GoalEditorScreen.kt:198-203, 425-477`; `GoalsViewModel.kt:158-161`; fallback in `ExtendedGoalRepository.kt:59-78, 99-167`; fixed-start math in `CustomGoal.kt:72-84`.

**Reproduce:** with no weights, create a goal for 180 lb, then log 200 and later 190. The stored baseline remains null, so each read substitutes the latest weight as the start. The caption moves from 200 to 190 and progress stays 0% instead of 50%, then jumps directly to reached at 180.

**Impact:** an explicitly allowed new-user flow produces a progress meter that can never show intermediate progress and rewrites its claimed historical start.

**Repair:** atomically persist the first real weigh-in as the missing baseline before later calculations, or require a weight before creation; add a repository regression for 200 to 190 toward 180.

#### M-34: rotation permanently consumes a visible Undo action

**Code:** `SnackbarController.kt:23-42`; `SnackbarControllerHost.kt:41-57`; activity recreation contract in `AndroidManifest.xml:96-101`.

**Reproduce:** delete a goal/cardio/holiday/custom activity, clear notifications, or apply watch stats, then rotate while the Undo snackbar is visible. The singleton channel event was already consumed; disposal cancels `showSnackbar` and the new composition creates a fresh host with no event to replay. The mutation remains committed and Undo never returns.

**Impact:** all seven production Undo flows lose their only recovery action through normal configuration change.

**Repair:** retain the current event in replaying state with a stable ID/expiry until actual action or timeout; host detachment must not acknowledge it, and clear-by-ID must preserve newest-wins semantics.

#### M-35: live “Remove animations” changes do not stop Avex's custom timers/media

**Code:** non-observable field in `Motion.kt:25-39`; live observer in `MainActivity.kt:210-215, 289-295`; Academy gates at `AcademyScreen.kt:115-127` and `AcademyGallery.kt:515-524`; onboarding media in `PlanModeMedia.kt:84-99`.

**Reproduce:** open Academy with multiple start-here pointers, enable system Remove animations without killing Avex, then return. The observer updates a volatile field but invalidates no Compose state, so the seven-second promoted-card rotator keeps moving and pointers stay removed from their normal chapters. An active onboarding animated WebP likewise keeps playing until unrelated recomposition/route recreation.

**Impact:** a live accessibility setting is ignored by precisely the raw timer/drawable paths that Compose cannot stop itself.

**Repair:** expose the disabled/scale value as Compose-observable state at the app root and use it for non-Compose motion/reachability policy after H-14 removes tween pre-scaling.

#### M-36: day-log sheets cannot scroll, making later records unreachable

**Code:** `DayLog.kt:57-67, 90-133`; callers in `ProfileScreen.kt:459-465` and `StatsContent.kt:156-162`.

**Reproduce:** create/import enough workout/cardio rows on one date to exceed the viewport, roughly 10-12 compact rows or fewer at 200% font, then open that day from Profile Activity or Stats. The sheet eagerly renders an unbounded `forEach` inside a plain Column; expanding the modal does not make that child scroll, so later rows cannot be seen or opened.

**Impact:** both calendar entry points present an incomplete day and make valid sessions inaccessible from that sheet.

**Repair:** use a keyed `LazyColumn` for header and records, or at minimum a vertical scroll state; test overflow at normal and large font scale.

#### M-37: accepted custom accents can make text on accent-filled controls fail AA contrast

**Code:** foreground selection in `ForgeTheme.kt:102-112, 131-135`; accepted range in `AccentColorPicker.kt:63-74, 232-239, 508-517`; filled controls in `Capsules.kt:31-60, 147-188` and `NotificationBell.kt:103-130`.

**Reproduce:** in default Pearl, enter accepted custom accent `#777777`. Its luminance is about 0.1845, so the hard 0.18 threshold selects `#110F0C` as `onPrimary`; contrast is about **4.27:1**, below WCAG AA for normal text. The accepted 0.153-0.197 luminance interval contains colors for which neither existing Pearl foreground reaches 4.5:1.

**Impact:** labels, commit/Finish/Set controls, switches and small notification badges can become inaccessible under a supported user color.

**Repair:** calculate contrast against both candidates and fall back to black/white or reject/adjust the gap; property-test every accepted custom primary/onPrimary pair at 4.5:1.

### Low

#### L-01: folder auto-scan hides bodyweight-only and extras-only Avex exports

**Code:** `WorkoutImportRepository.kt:108-160`; `ForgeBodyweightCsvImporter.kt:4-49`.

The scanner counts only parsed workout sessions. The bodyweight CSV intentionally returns its data through `parseExtras`, so a valid remembered-folder export is cached as absent; cardio/goals/bodyweight-only JSON behaves likewise. Direct file picking works. Count extras and surface a source-appropriate summary whenever either workouts or extras exist.

#### L-02: Format “Reset this section” leaves distance and length unchanged

**Code:** `SettingsRepository.kt:20-44, 425-441`; `SettingsSubPages.kt:142-191`.

Set miles and centimetres, then reset the Format section. `USE_MILES` and `USE_CM` are missing from the section's key list, so both visible controls retain their explicit values. Add the two keys and a test that asserts every control owned by a resettable page returns to default while unrelated keys survive.

#### L-03: a custom cardio activity is renamed to “Other” in the weekly timeline

**Code:** `CardioWeekDetailComponents.kt:37-76`; correct sibling handling in `CardioEntryRow.kt:58-64` and `CardioSessionDetailSheet.kt:100-110`.

Create a custom activity named Padel, log it, then open Cardio > Weeks > that week. The timeline row maps every `custom_*` code through `CardioType.fromCode` and displays Other with the generic icon, although the main list and detail sheet resolve Padel correctly. Resolve the provided `CardioActivity` from `LocalCardioTypes` as the sibling surfaces do and cover a custom code in the row test.

#### L-04: rest-completion haptics ignore Off and are requested twice when enabled

**Code:** preference-aware effects in `DayScreen.kt:104-117`; duplicate effects in `RestTimerBubble.kt:103-129`; setting contract in `SettingsSubPages.kt:475-481`.

Set Feedback strength to Off and let a rest reach 10 seconds and zero. The bubble still requests haptics because it bypasses `forgeHaptic`. With feedback enabled, the screen and bubble each request the same two events. Keep event haptics in the preference-aware screen owner and remove them from the visual bubble; test zero calls for Off and one per threshold otherwise.

#### L-05: valid timed holds display `0 SETS` in their exercise card

**Code:** correct global predicate in `FreestyleLogScreen.kt:389-394`; card predicate at `:683-684, 727-733`.

Enter a 45-second Plank hold. Save is enabled and the session total increments, but the Plank card derives completion only from positive reps; timed sets legitimately store zero reps, so it says `0 SETS`. Share the timed-versus-rep validity predicate between both counts.

#### L-06: deleting a pinned goal leaves an orphan key that later evicts a live pin

**Code:** `GoalsViewModel.kt:125-175`; three-slot cap in `SettingsRepository.kt:292-308`; Home resolution in `OverviewScreen.kt:225-271`.

Pin A, B and C; delete C; then pin D. The raw preference remains `[A,B,C]`, so `takeLast(3)` produces `[B,C,D]`; Home skips orphan C after eviction and shows only B/D, silently dropping live A while leaving a slot empty. Remove pins with deleted rows, restore them with custom-goal Undo, and apply the cap after live-key resolution.

#### L-07: Home prints a custom cardio storage code instead of its activity name

**Code:** `OverviewUiStateMapper.kt:75-89`; rendering in `OverviewScreen.kt:165-210, 652-683`; resolver in `CardioActivity.kt:43-57`.

Log custom Padel and let it enter Home's recent rows. The mapper title-cases `custom_ab12cd34` into `Custom_ab12cd34` rather than resolving Padel; built-in `hiit` likewise becomes `Hiit`. Carry the type code to composition and resolve it through the already-provided `LocalCardioTypes`, with a deleted-definition fallback.

#### L-08: Trophies renders a false zero/empty account while its real snapshot is loading

**Code:** `TrophiesScreen.kt:48, 85-103`; `TrophiesComponents.kt:57-79`; `TrophiesUiState.kt:44-58`; `TrophiesViewModel.kt:39-73`.

On an account with earned trophies, delay one of the roughly 14 snapshot DAO reads and cold-open Trophies. State correctly says `isLoading=true`, but the screen ignores it and renders `0 EARNED`, an empty progress bar and “Nothing earned yet” until the snapshot resolves. Branch on loading before definitive numeric/empty claims and test a delayed populated snapshot. A snapshot exception is a separate uncaught crash path, not a persistent false-zero state.

## Meaningful performance opportunities

### P-01 (Medium): large history exports run CPU/file work on Main and full JSON performs N+1 queries

**Code:** `BackupRepository.kt:104-192, 200-322, 393-467`; `PdfExportRepository.kt:53-197`; `SettingsViewModel.kt:569-599`.

From Settings, `viewModelScope.launch` calls large export paths that never move the whole operation off Main. JSON construction, large `StringBuilder` work, PDF Canvas rendering and final writes therefore compete with UI frames. Full JSON also performs exactly `3 + 3S + E` Room queries for `S` sessions and `E` logged exercises, and retains the complete formatted document before writing.

Move the entire operation to IO, batch-load exercises/sets/moods/segments, group once, and stream JSON/CSV through buffered writers. Keep progress/cancellation explicit for multi-year histories.

### P-02 (Medium): every phone cold start computes a Wear glance before checking whether a watch exists

**Code:** `ForgeApp.kt:45-57`; `WearStatePublisher.kt:111-142`; nested reads in `DirectiveRepository.kt:62-65` and `AdaptationRepository.kt:127-205, 253-285`.

Even with no paired watch, startup computes readiness, weekly stats and today's directive before the Data Layer write discovers there is no consumer. On a cold cache this one path calls `sessionDao.allFinished()` seven times, plus repeated exercise/set/cardio/check-in reads over lifetime history.

Gate glance construction on a cached/queried Wear capability or reachable node, publish when capability connects, and construct one shared fact bundle per refresh rather than recursively re-querying it.

### P-03 (Medium): hot chronological Room queries lack supporting indices, and import duplicate checks become quadratic

**Code:** `Session.kt:16-42`; `CardioEntry.kt:19-41`; hot queries in `SessionDao.kt:58-114, 150-172, 197-207, 229-287, 305-346` and `CardioDao.kt:26-49`.

`EXPLAIN QUERY PLAN` on the v36 schema reports full scans plus temporary sort trees for recent session/cardio history, and a full session scan for each importer's `startRefsInRange` call. Repeating that lookup for every incoming workout makes the duplicate guard O(N²).

Directional desktop SQLite measurement with the exact relevant schema and 50,000 synthetic rows:

- recent session/cardio median: 7.188/6.940 ms without indices, 0.008/0.012 ms with them;
- 10,000-row import-window loop: 0.634 s without `started_at`, 0.013 s with it (47.1x).

These are not claimed Android timings. The query plans and scaling are platform-independent evidence. Add measured v37 indices on `session.started_at`, likely `session.finished_at`, and `cardio_entry.date`; benchmark realistic imports before adding extra composites because each index costs writes/storage.

### P-04 (Medium, battery): Wear keeps the display awake while idle and recomposes clocks far faster than their displayed precision

**Code:** `wear/MainActivity.kt:46-47`; `wear/ui/SessionScreen.kt:81-85, 115-116`; `wear/ui/TimerView.kt:47-63`.

Opening the Wear app unconditionally sets `FLAG_KEEP_SCREEN_ON`, including the idle/no-session screen, and never clears it. During a session, SetView writes `nowMs` every 250 ms (14,400 times/hour) although its elapsed figure changes by the minute and the undo/rate window only needs second precision. TimerView writes every 200 ms (18,000 times/hour) although it displays integer seconds.

Keep the screen awake only for the active interaction where it is genuinely required, clear it in idle/background, and tick on second/minute boundaries. Confirm savings with a Wear power trace.

### P-05 (Medium): the baseline-profile module is wired but no Avex profile is generated or shipped

**Code/config:** `app/build.gradle.kts:315-318`; `baselineprofile/build.gradle.kts:24-32`; `baselineprofile/src/main/.../BaselineProfileGenerator.kt`; CI/release workflows.

There is no generated source `baseline-prof.txt`, and no CI/release task invokes profile generation. The release merge contains 2,737 dependency profile rules and zero `Lcom/forge` or `Lcom/quietsoftware` rules before R8, so the existing Avex startup/CUJ generator contributes nothing to the shipped profile. Android's plugin copies generated rules into the profiled module only when the generation task runs, and recommends generating profiles for release-critical Compose paths ([Baseline Profile creation guidance](https://developer.android.com/topic/performance/baselineprofiles/create-baselineprofile)).

Generate/commit or deterministically generate the profile for releases, include startup plus measured hub/day-session CUJs, then benchmark TTID/TTFD and navigation on a physical device. The opportunity is confirmed; the size of Avex's gain is intentionally not guessed.

### P-06 (Medium): Coach blocks first paint on invisible future-feature work

**Code:** `CoachViewModel.kt:34-42, 60-87, 97-131, 171-245`; `CoachScreen.kt:130-132, 165-180, 203-233`; `CoachGoalRepository.kt:74-82`; `ProjectRepository.kt:29-62`.

Opening Coach loads fields that no production composable reads. `goalRepo.states()` and `conflicts()` independently perform two full adaptation assemblies, each loading full session/exercise/set histories, related repositories and Health Connect recovery. Project loading performs three active-project queries plus two history/scanner passes for invisible output. Academy new-count is also loaded but never rendered. The screen holds its first non-loading frame until this work finishes.

Delete the dead goal/project/new-lesson fields, loads, actions, injections and uncalled `GoalPickerDialog` until there is a real surface. If a side effect remains necessary, move it to its mutation boundary and reuse a cached snapshot. Add repository call counters and a seeded-history first-frame trace.

### P-07 (Medium): Academy synchronously decodes oversized raster covers during composition

**Code/resources:** `AcademyGallery.kt:107-147, 250-292, 325-330`; 35 `drawable-nodpi/cover_*.webp` assets.

Every newly composed gallery plate uses `painterResource`. Android documents that `painterResource` decodes raster images on the main thread ([Compose resource guidance](https://developer.android.com/develop/ui/compose/resources?hl=en)). Thirty-one covers are 1200x1600, roughly 7.3 MiB each as ARGB; three are 900x1200 and one is 1200x630. `drawable-nodpi` prevents density downsampling even though posters render around half-screen width and the opening crop is 2:1.

Generate display-sized local thumbnails for gallery/opening plates, retaining a larger reader source only where it can be shown. Measure cold Academy scroll frames and bitmap heap on a physical device before choosing final dimensions; no image-loader dependency is needed to establish the win.

### P-08 (Medium): one cardio-table change drives three Room observers and can repeat every full-history aggregate

**Code:** `CardioViewModel.kt:145-190`; `CardioRepository.kt:22-37`; `CardioDao.kt:26-49`.

The ViewModel already materializes `observeAll()` for streaks, records, pace series and week aggregation, but combines it with separate weekly-minutes and weekly-entry observers over the same table. Every insert/update/delete invalidates all three SQL queries; emissions can rerun full-history sorting and aggregation more than once and transiently mix new and old slices.

Derive the current-week slice, minutes, active days and cells once from the already-loaded full list plus the day/week anchor. Use a counting DAO test and seeded-history benchmark to verify query and recomputation reduction.

### P-09 (Medium, memory/reliability): compare and share decoders retain up to four times the requested source pixels

**Code:** `MirrorTestViewer.kt:478-514`; `BeforeAfterCardRenderer.kt:52-66, 186-196`; correct exact post-sample scaling in `ProgressPhotoImage.kt:62-78`.

Both decoders stop after power-of-two `inSampleSize`. The longest edge can therefore remain just under twice the request, nearly four times its pixel count. At a 1400 px viewer request, one 4:3 source can retain about 23.5 MB ARGB instead of 5.9 MB; two-source compare can retain about 47 MB. Running a 1200 px share render while those images remain composed has a deterministic representative bound near **87 MB** before rotation temporaries, PNG compression and the rest of the app. The share path converts failure to null and provides no user-visible error.

Reuse one exact post-sample resize/orientation decoder, size to actual destination cells, and surface render failure. Add awkward-dimension decoder bounds plus a low-memory compare-and-share device test. The allocation multiplier is confirmed from the decode math; an OOM threshold is intentionally not claimed without a device measurement.

### P-10 (Medium): every body heatmap recompiles the full anatomy SVG geometry

**Code:** `BodyHeatmap.kt:34-40, 73-97`; reusable precedent in `MuscleFigure.kt:44-63, 85-91`; 59,545-byte `BodyAnatomy.kt` with 155 path strings.

Each front/back `FigureColumn` creates a new `CompiledFigure` in composition-local `produceState`. Opening Stats, session summary, session detail and the enlarged map reparses both orientations and outlines for every new instance, allocates new Paths and initially draws blank. The comment claiming process-wide reuse is false; a sibling implementation already demonstrates top-level cached geometry.

Create thread-safe lazy compiled front/back figures off Main and reuse them across instances. A parse counter across multiple surfaces can make the eliminated compile count a regression test.

### P-11 (Low): freestyle template loading performs one set query per exercise

**Code:** `FreestyleTemplateViewModel.kt:104-121`; existing session-wide DAO query in `LoggedSetDao.kt:345`.

Selecting a past workout with 12 exercises performs one exercise query plus 12 `forLoggedExercise` set queries. Load `allForSession(sessionId)` once, group by `loggedExerciseId`, and assert the query count in a template-loader test.

### P-12 (Low, battery): a paused rest timer runs an unused infinite animation at frame cadence

**Code:** `RestTimerBubble.kt:75, 137-145`.

`rememberInfiniteTransition` always runs, but its value affects scale only after the timer finishes. An active or indefinitely paused bubble therefore advances an ignored animation every frame. Instantiate the transition only in the finished branch and verify a paused bubble becomes quiescent under a test frame clock.

### P-13 (Low): the lifetime sparkline rebuilds full-series paths on every reveal frame

**Code:** `SurfaceKit.kt:360-405`; unbounded caller in `ProfileSurfaceSections.kt:135-164`; source series in `ProfileRepository.kt:311-315`.

Profile passes one point per finished lifetime session with no cap. During the 900 ms reveal, roughly 54 frames at 60 Hz, every frame recomputes min/max and walks all `N` points to allocate/fill both line and area Paths. That is about `216N` point visits and 108 Path allocations for one reveal; at 5,000 sessions, roughly 1.08 million visits even though a phone-width chart cannot show 5,000 independent x positions.

Remember normalized extrema/data, reduce over-width series into pixel buckets preserving extrema, and cache immutable paths by data/size so animation updates only the clip frontier/dot. Benchmark 100/1,000/10,000-point entry allocations and frame time; no current device-jank number is claimed.

### P-14 (Medium): Home launches a full adaptation and Health Connect snapshot whose result is never rendered

**Code:** dead state in `OverviewUiState.kt:104-112`; unconditional load in `OverviewViewModel.kt:145-151, 257-264, 299-300, 350-413`; fan-out in `AdaptationRepository.kt:91-113, 127-205, 287-315`.

Every Overview ViewModel runs `reloadCoach`, even though no current Home composable reads `coach`, `coachLearning` or `coachFatigue`, and the associated apply/dismiss actions have no caller. The snapshot reads all finished sessions, logged exercises and sets, finished sessions again through life events, check-ins, cardio, restrictions, moods, bodyweight, swaps, preferences, cooldowns and Health Connect recovery, then emits three dead state flows.

Delete the removed Home Coach fields, loads, actions and dependencies. A counting cold-start test should assert zero adaptation/Health reads on Home; this is execution elimination, not a speculative cache.

### P-15 (Medium): Home keeps an unbounded all-session scan and unused observers alive for removed cards

**Code:** abandoned state in `OverviewUiState.kt:118-129`; scan in `OverviewViewModel.kt:39-44, 87-119, 285-292`; unused observers at `:177-190, 214-248`; unbounded query in `SessionDao.kt:305-307`.

While Home is collected, `weeklyVolumeFlow` materializes every finished session and loops the whole history to fill two fields no composable reads. It repeats on each session-table invalidation. Unrendered trophy IDs, weekly cardio distance/target/day-strip and identity inputs keep additional Room/DataStore observers and combine emissions alive.

Delete the dead volume/identity/card inputs and trim the top-level combine to render consumers. With `N` historical sessions, a subscription plus one update should perform no `observeAllFinishedSessions` query or O(N) bucketing.

### P-16 (Low): Profile and Recap skeletons run one shimmer clock per placeholder

**Code:** `ForgeShimmer.kt:25-50`; 18-instance `ProfileSkeleton.kt:29-93`; 16-instance `RecapScreen.kt:157-178`.

Every placeholder creates its own infinite transition and, on each draw, allocates a three-color list plus moving gradient. At 60 Hz, Profile drives about 1,080 independent animated-state updates, lists and brushes per second while loading; Recap drives about 960 of each, doubled on 120 Hz displays.

Own one phase animation per skeleton/host, pass it into a pure modifier, remember stable colors, and use draw caching if allocation traces still justify it.

### P-17 (Low): themed startup wordmarks recreate a native RenderEffect every frame

**Code:** `AvexWordmark.kt:149-173, 180-234`; lifetime in `AvexIntro.kt:40-89`.

On API 33+, four themed wordmarks remember one mutable RuntimeShader but call `createRuntimeShaderEffect(...).asComposeRenderEffect()` every frame even though the shader and input name do not change. The 1.26-1.45 second cold overlay creates roughly 75-90 native effects/wrappers at 60 Hz per launch.

Remember the Compose RenderEffect once per shader and continue updating only its uniforms; verify one factory call per wordmark instance.

## Falsified or excluded candidates

The review kept a rejection log rather than promoting suspicious patterns on shape alone. Notable non-findings:

- **Room migration gaps:** all 59 migration statements from v12 through v36 were independently applied and every target matched the exported Room schema under Room semantics. The v1-v11 destructive fallback is explicit compatibility policy, not evidence of a broken declared chain.
- **Restore ZIP traversal/bomb and progress-photo path traversal:** canonical-path checks, entry/count/size caps, safe generated-name resolution and hostile-index tests block the suspected paths. Photo imports also cap bytes and remove partial files.
- **Swap re-key corruption:** the repository transaction prevents a concurrently arriving set from being moved to a different `exercise_id`. H-11 is the narrower, reproduced name/unit relabel that remains after that safeguard.
- **Freestyle partial save, rapid double logging and note-debounce loss:** current transaction/`NonCancellable`, synchronous in-flight guard and dispose-time note flush respectively prevent those candidate failures.
- **Pre-session ramp omission:** current screenshot tests explicitly record its intentional removal. Only the live PLATES adapter mismatch in H-15 was retained.
- **Onboarding's immediate completion callback:** the production host passes a no-op and navigation follows the durable done preference, so it is not an early-navigation bug. The configuration-loss and unit-reinterpretation paths remain independently reproducible.
- **Platform EXIF as a security exploit:** current official guidance supports replacing the framework parser, but no exploitable Avex path was established. M-17 is ranked only for confirmed rendering/export correctness.
- **Domain-shaped dead paths:** `SessionAdaptor`'s budget path has no production caller, conditioning outcomes E-B/E-D have no production constructor, and the timed-hold nullable assertion is unreachable for current built-ins. None were reported as user bugs.
- **Unbounded UI algorithm claims:** bounded chart scans, off-main/capped same-weight matching, generated-preview work and the broad Settings combine did not have enough measured or deterministic cost to retain. Performance entries above have a concrete query/parse/tick/allocation count or a measurement recipe.
- **Lint/style output:** warnings, hints, doctrine deviations, copy, spacing and purely cosmetic inconsistencies were excluded unless they directly proved one of the correctness or cost findings above.
