# Data repositories (incl. backup, export, restore)

Scope: `data/repo/**`. 41 files, all read in full.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/data/repo/` unless noted.

**Counts:** P1 3 · P2 8 · P3 10 · P4 13. Five P2s are still open from `docs/audits/2026-09-12/data.md` (D2–D6).

## P1

### [P1] Background program regeneration deletes the in-progress workout with no confirm
- Category: bug (data loss)
- Location: `BlockRepository.kt:129, 141-145`; `CoachRepository.kt:231`; `WorkoutRepository.kt:373, 420-435, 596-629`; `ProgramRepository.kt:243-252, 534-536`; `AdaptationRepository.kt:417`
- Problem: `generate()` always calls `discardActiveSession()` inside its transaction, which CASCADE-deletes the active session's exercises and sets. Every UI caller wraps generation in `ProgramChangeGuard`; two repository paths don't. *Re-checked by the coordinator.*
  1. `ensureWeeklyPass` calls `blockRepository.advanceForWeek` before the vacation check and regardless of `coachEnabled`. The pass runs on every ON_RESUME (`NotificationFeed.refresh` → `pendingBanner`) and from `WeeklyRecapWorker`. When the block enters DELOAD (the default 5-week block always ends in one), `serveScheduledDeload` → `applyDeloadWeek` → `generate` deletes the session. Scenario: a workout started Sunday night and resumed after midnight Monday in the deload week, or a session left unfinished last week, is deleted silently where `resolveOrphanSession` would have preserved it.
  2. `saveFreestyleDraft` → `finishAfterCommit` → `maybeRotateProgram` runs `restoreAfterDeload` on the first finish after the deload window, or `rerollAll` every Nth session. `FreestyleLogViewModel` never checks for an active program session.
- Fix: background callers check `sessionDao.getActiveSession()` and defer (leaving the marker/counter intact so the next pass retries), or pass an `onActiveSession = DEFER | FINISH_TO_HISTORY` policy into `generate`. Gate `advanceForWeek` on `coachEnabled`.
- Confidence: high

### [P1] Coach off→on mid-week makes every later pass that week delete and re-mint the week's decisions
- Category: bug (wrong training data)
- Location: `CoachRepository.kt:203-209, 217, 280, 285`; `data/db/dao/CoachDao.kt:141-154`
- Problem: `coachOffPassWeekId` is set at `:280` and never cleared (the only writer), so `recordedWhileOff` stays true all week. Every `ensureWeeklyPass` (each resume, each Coach open) then calls `clearPass` → `DELETE FROM coach_decision WHERE week_id=?` for **all statuses** (the DAO KDoc assumes only shadow rows exist). *Re-checked by the coordinator.*
- Scenario: first pass runs with the coach off. The user turns the coach on Tuesday and applies "Bench 3→4 sets". On the next resume the APPLIED row and its undo data are deleted while the overlay stays live; the planner no longer sees it (no pending lock, no `CoachGenBias` drift), re-proposes, and applying re-derives 4→5. In auto mode `previousPassRanAt` is read *after* the clear (`:217`), so `autoApplyTooSoon` is false and it auto-applies again on every resume. Skipped proposals come back too.
- Fix: `if (won && !coachOff) settings.setCoachOffPassWeekId("")` (or store the off flag on the `coach_pass` row); restrict `clearPass` to shadow/proposed rows and never regenerate a week that has applied, folded or skipped decisions.
- Confidence: high

### [P1] Restore swaps in backup prefs verbatim and can lower the gallery lock without authentication — still open from data.md D1
- Category: bug (security)
- Location: `BackupRepository.kt:968-970`; callers `ui/settings/SettingsViewModel.kt:714-719, 813-818`
- Problem: a backup with `gallery_lock_enabled=false` unlocks the gallery after restart; the restore is confirmed with a plain dialog.
- Fix: overlay the current protection keys onto the incoming prefs, or require authentication when the restore would lower protection. Same for `restoreFromAutoBackup`.
- Confidence: high

## P2

### [P2] A coach swap relabels the user's rest-timer / pinned-note row as coach-owned; the next regenerate deletes it
- Category: bug (data loss)
- Location: `CustomizationRepository.kt:75-82, 104-118`; `ProgramRepository.kt:123, 523`; `data/db/dao/ExerciseCustomizationDao.kt:23-24`
- Problem: `exercise_customization` holds swap, rest-timer override and pinned note in one row with a single `source`. `upsertSwap` sets `source = COACH` (from `CoachRepository.kt:637`); `clearSwap` (coach undo, `:779`) keeps COACH; `clearCoachSwaps` (`DELETE WHERE source='coach'`) runs on every regenerate and `saveCustomProgram`. A user's "seat notch 4" note and 150 s rest on Bench vanish at the next rotation/re-roll/deload after any coach swap, defeating fixes #59/#112.
- Fix: in a transaction, for each coach row with a rest timer or pinned note, upsert it with the swap blanked and `source=USER`; delete the rest. `clearSwap` resets `source` to USER when it keeps the row.
- Confidence: high

### [P2] Generation params assembled three times; the Settings paths ignore `personalCaps`
- Category: bug / bad-code
- Location: `AdaptationRepository.kt:386-412`; `ProgramRepository.kt:327-341`; `ui/settings/SettingsViewModel.kt:447-459`
- Problem: the comment says "keep the two in sync", but there are three and they've drifted. Settings generate/re-roll/deload use population caps; rotation, coach/Overview deload and `restoreAfterDeload` use personal caps (±35%). A Settings-generated program comes back from deload→restore with different per-muscle sets despite `keepPicks`' "same program at full volume" promise.
- Fix: make `ProgramRepository.currentParams(days)` public and use it everywhere; `applyDeloadWeek` becomes `generate(currentParams().copy(deload = true), …, keepPicks = true)`.
- Confidence: high

### [P2] Full JSON export drops fields, and import loses them
- Category: bug (data loss on round trip)
- Location: `BackupRepository.kt:306-313, 336-351, 219-224`; `data/importer/ImportModels.kt:135-146`; `WorkoutImportRepository.kt:363-373`
- Problem: cardio rows omit `intervalCount`, `hrZone`, `conditions` (the CSV export has them; `intervalCount` feeds `ConditioningLoad` and the Academy unlock). Exercises omit `wasPr`, `hitFullTarget`, `supersetGroup`: after import `was_pr` is 0 everywhere, so recent PRs, `prCount` (trophies, milestones) and `prSessionStartTimes` read zero while session `prCount` still says N. The KDoc "nothing reads it back in" is stale: `ForgeJsonImporter` reads this file.
- Fix: export and import these fields; fix the KDoc; add a round-trip test.
- Confidence: high

### [P2] Weekly and single-session JSON drop `isUntracked` (weekly also drops `sessionType`) — still open, D3
- Location: `BackupRepository.kt:154-172, 400-415`
- Problem: the importer defaults a missing `isUntracked` to false, so excluded sessions come back counted.
- Fix: one shared `sessionFields()` writer for all three exporters, like `exportSetFields`.

### [P2] Weekly cardio exported date-only; a same-day duplicate is dropped on import and per-type fields are lost — still open, D4
- Location: `BackupRepository.kt:200-210`; `WorkoutImportRepository.kt:444`; `data/db/dao/CardioDao.kt:64-65`
- Problem: an 08:00 run and an 18:00 run of equal duration both resolve to midnight and `existsAt(date, type, dur)` skips the second. Rest reason, note, incline etc. are omitted.
- Fix: export epoch `date` and all fields, reusing the full-export cardio writer.

### [P2] PDF prints timed holds as "× 0" reps — still open, D5
- Location: `PdfExportRepository.kt:115-119`
- Problem: a 90 s weighted plank renders "45 lb × 0"; `durationSeconds` is never read.
- Fix: render `durationSeconds` as "m:ss hold".

### [P2] Folder-backup failure reported as success; a failed rename can lose the only folder copy — still open, D6
- Location: `BackupRepository.kt:561, 574-595`
- Problem: `runCatching` + silent early returns clear the failure marker; `tmp.renameTo` is unchecked, and the next run's `findFile(FOLDER_TMP_NAME)?.delete()` (`:582`) deletes the stranded `.part` copy.
- Fix: structured folder result surfaced to the user; keep `.part` until a new write succeeds.

### [P2] Factory reset keeps the auto-backup ZIP, exports and markers — still open, D2
- Location: `ResetRepository.kt:100-110`
- Fix: `BackupRepository.deleteLocalCopies()` (auto slot, tmp, markers, `pending_restore_*`, exports dir) called from `factoryReset`.

## P3

### [P3] Cancelled restore still gets staged
- Location: `BackupRepository.kt:970, 1006-1008, 1157-1173`; `ui/settings/SettingsViewModel.kt:714-718`
- Problem: `isPreferencesBlob`'s `runCatching` swallows the `CancellationException` from `data.first()`; the restore continues, prefs are skipped, the manifest is published (`stagedOk = true`) and the VM reports IO_ERROR. The next boot applies a restore the user saw fail, without their prefs. (Companion to the Settings P1 in `04-settings-security-notifications.md`.)
- Fix: rethrow `CancellationException`; `ensureActive()` before `RestoreManifest.publish`.
- Confidence: medium

### [P3] `updateSet` clamps the weight but keeps the typed text
- Location: `WorkoutRepository.kt:911-912` (vs `893-897`)
- Problem: editing a set to "1000000000" stores 2000 lb with the text still "1000000000" — the desync `logSet` documents and avoids.
- Fix: a shared `sanitized(set)` that also rewrites `weightText` and clamps `durationSeconds`.

### [P3] `resolveOrphanSession` finishes via a stale whole-row `update`
- Location: `WorkoutRepository.kt:677-714` (vs `379-416`)
- Problem: bypasses `finishIfUnfinished`/`setFinishTotals`; a concurrent wrist finish can double-stamp the session and double-mirror to Health Connect, and the write rewrites tags, journal and untracked from the old read.
- Fix: `closeDanglingSegments`, then `finishInTransaction`; mirror only on `Won`.
- Confidence: medium

### [P3] Next-up inputs assembled three ways with different day anchors
- Location: `DirectiveRepository.kt:88-93` (startedAt) vs `StatsRepository.kt:172-176` and `ui/gym/train/DayListViewModel.kt:102-106` (finishedAt)
- Problem: a 23:40→00:30 session counts as trained today for Stats but not the directive; Overview shows both (`:389`, `:488`).
- Fix: one `NextUpInputs.from(sessions, zone, today)`.
- Confidence: medium

### [P3] `snapshotCached()` caches nothing
- Category: perf / bad-code
- Location: `AdaptationRepository.kt:246-247`; false comments `CoachRepository.kt:890-891, 1008-1009`; callers `AcademyRepository.kt:77`, `NotificationFeed.kt:138`, `ProjectRepository.kt:39, 57`, `BlockRepository.kt:118`, `DirectiveRepository.kt:74`
- Problem: every call is a full-history fan-out plus a Health Connect `readRecovery` IPC; that runs on every ON_RESUME via `syncCoachMoments`, and twice back-to-back in `CoachViewModel.kt:137-140`.
- Fix: a real revision-keyed cache, or rename it, fix the comments and throttle `syncCoachMoments`.

### [P3] `ProjectRepository.proposal()` duplicates `proposals().firstOrNull()`, costing two snapshots
- Location: `ProjectRepository.kt:37-63`; `ui/coach/CoachViewModel.kt:324-325`
- Fix: delete `proposal()`; use `options.firstOrNull()`.

### [P3] Five writers with zero callers; their tables are only read
- Category: dead-code
- Location: `CheckinRepository.kt:47-48, 89-114`; `CustomizationRepository.kt:126-130`; `WarmupRepository.kt:17, 26-38`; `RestDayRepository.kt` (never injected); `ProgramCustomizationRepository.kt:190-224`
- Problem: live readers remain (`LifeEvents` + `AdaptationRepository.kt:140-144` restrictions, `EngineInputSignals.kt:33`, `DayViewModel.kt:200` custom warmups, day-name reads, `observeCustomExercises`/`customSiblingIds`, the USER-lock scan `CoachRepository.kt:428-440, 495-502`), so injury restrictions, day names, custom warmups and custom exercises are unreachable features. `setRepRange`/`setSetsOverride` are only called with COACH.
- Fix: wire the UI, or delete writers and readers.

### [P3] Health Connect weight backfill does up to 5,000 autocommit upserts after loading the whole table
- Category: perf
- Location: `BodyweightRepository.kt:157-174`
- Fix: one `withTransaction`, or a bulk `insertAll(IGNORE)`.

### [P3] Bodyweight CSV export drops the weigh-in note
- Location: `BackupRepository.kt:488-496`
- Fix: add `note` (and `recordedAt`) and read it back in `ForgeBodyweightCsvImporter`.

### [P3] PDF export freezes the time zone at construction — still open (data.md lower-priority #3)
- Location: `PdfExportRepository.kt:45`
- Fix: `get() = ZoneId.systemDefault()`.

## P4

- **PDF polish:** footer prints raw epoch millis; `doc.close()` not in `finally`; per-exercise set query (N+1) (`PdfExportRepository.kt:196, 202-203, 110`).
- **dead / inconsistent:** relative strength computed twice with different rounding (`StatsRepository.kt:516-524` `.toInt()` vs `StatsStrengthAggregations.kt:60-63` `roundToInt`) and never rendered.
- **dead-code — unused public API** (zero refs in app/wear/shared main+test): `AdaptationRepository.coachRecommendations` (331); `BodyFatRepository.latestPercent` (32); `BodyweightRepository.latestWeightLb` (31); `LeanMassRepository.latestLb` (27); `CardioRepository.observeSince`, `observeDistanceKmSince` (28, 38); `StatsRepository.observeAllFinishedSessions` (296); `GoalRepository.observe` (20); `TrophyRepository.observeUnlockedIds` (45); `BlockRepository.observeActive`, `history` (46, 60); `ProjectRepository.observeActive`, `history`, `dueForReview` (27, 31, 97); `CoachGoalRepository.observeActive`, `all`, `archive`, `complete`, `delete`, `reorder`, `harvestCompleted`, `promotionCandidates` (32-98; coach goals can be added but never removed; `reorder` is non-transactional); `CoachRepository.history` (316, 1+N queries); `AcademyRepository.unlocked`, `upcoming` (36-40); `NotificationFeed.unreadCount`, `academyUnreadCount` (313, 317); `WorkoutRepository.inTransaction` (219), `createFreestyleSession` (221), `reLogSession` (244, UI caller removed in d72f020), `sessionSegments` (593), `loggedExerciseForSlot` (746), `bestHoldSecondsForExercise` (929), `maxWeightForExercise` (940); `setSessionType`/`setUntracked`/`setIntensity` (645-648) reachable only from never-dispatched `DayUiEvent`s.
- **dead-code:** 17 stats helpers with no production caller (13 unreferenced, 4 test-only) in `StatsEffortAggregations.kt`, `StatsStrengthAggregations.kt`, `StatsVolumeAggregations.kt`; `buildEffortDistribution` compares uppercase names to lowercase stored codes (still open, data.md lower-priority #4; parked in AUDIT_DEFERRED.md).
- **dead-code:** `SampleDataSeeder` is unreachable (callers `OverviewViewModel.kt:303`, `SettingsViewModel.kt:420` never invoked) and would be non-transactional with no empty-DB guard if wired.
- **dead-code:** unused constructor deps `DirectiveRepository.kt:32, 34` (`workoutRepository`, `clock`); `SampleDataSeeder.kt:37` (`clock`).
- **bad-code — stale KDoc:** `StatsRepository.kt:60-63, 437-442`; `WorkoutRepository.kt:203-212, 321-326`; `ProgramRepository.kt:354-358`; `ProgressPhotoRepository.kt:324-327, 458-466`; `BackupRepository.kt:219-224`.
- **bug (minor):** `setSessionTags` does a whole-row read-modify-write (`WorkoutRepository.kt:632-635`); use a column-scoped UPDATE.
- **bug (minor):** current vs longest streak use different anchors (`StatsRepository.kt:226-250` finishedAt vs `TrophyRepository.kt:181-197` startedAt); Profile can show "2 days, longest 1".
- **bad-code:** `TrophyRepository.kt:247` `LocalDate.now` bypasses the injected clock; `checkVarietyPack` is O(d²) (`:262-283`).
- **bug (minor):** `StorageRepository.kt:66-75` duplicates `AUTO_BACKUP_NAME`; "Clear cache" deletes the running backup snapshot and the `cap_*.jpg` camera temp kept as the only copy for a retry.
- **bad-code:** swallowed failures and cancellation: `ProgressPhotoRepository.kt:330-428` (mutation results ignored); `NotificationFeed.kt:138-142` (a cancelled refresh flips `wearableConnected` false); `runCatching` around suspend calls in `ProjectRepository.kt:39, 57`, `BlockRepository.kt:70, 95-98, 117-119, 144`. Add a shared `runCatchingNonCancel`.
- **bug (minor):** `rerollDay` bypasses `mutationMutex` (`ProgramRepository.kt:387-426`).

## Files reviewed (41)
AcademyRepository, AdaptationRepository, AvatarRepository, BackupRepository, BlockRepository, BodyFatRepository, BodyMeasurementRepository, BodyweightRepository, CardioRepository, CheckinRepository, CoachGoalRepository, CoachRepository, ConditioningRepository, CustomizationRepository, DirectiveRepository, EngineInputSignals, ExportSetFields, ExtendedGoalRepository, GoalRepository, LeanMassRepository, NotificationFeed, PdfExportRepository, PersistedTreeGrants, ProfileRepository, ProgramCustomizationRepository, ProgramRepository, ProgressPhotoRepository, ProjectRepository, ResetRepository, RestDayRepository, SampleDataSeeder, StatsChartAggregations, StatsEffortAggregations, StatsRepository, StatsStrengthAggregations, StatsVolumeAggregations, StorageRepository, TrophyRepository, VacationRepository, WarmupRepository, WorkoutRepository.
