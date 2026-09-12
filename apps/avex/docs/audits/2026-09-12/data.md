# Avex whole-app audit: data, persistence, export/import, privacy, Health Connect

Reviewed current worktree HEAD `9637809` on 2026-09-12. This is a read-only review of the whole assigned production surface, not the recent commit range. No production files changed and no Gradle commands run by this reviewer. Root owns build/test validation and the combined release report.

Prior audit context checked against current source: `docs/AUDIT_DEFERRED.md` and `docs/audit-fixes-2026-09-07.md`. Existing fixed work was verified where relevant: staged restore/rollback/startup gating, Room validation before restore, WAL snapshot fallback, shared timed/assisted JSON set serializer, tracked-only statistics queries, idempotent Wear ledgers, preferences protection fallback, and persisted-folder replacement/release. Their former failures are not re-reported here. Known normal-session Health Connect postcommit crash gap and intentionally parked conditioning code remain existing deferrals.

## Confirmed findings

### D1. P1: Backup restore can lower the gallery lock without authenticating

- Primary locations: `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:968-970`; `app/src/main/java/com/forge/app/ui/settings/SettingsViewModel.kt:712-716` and `:811-815`.
- Supporting locations: `ui/settings/SettingsScreen.kt:329,364`; `ui/settings/SettingsDialogs.kt:132`; `data/prefs/SettingsRepository.kt:152-158`; `RestoreApply.kt:175-208`.
- Trigger: enable gallery lock while leaving whole-app lock off. Restore a valid backup whose preferences explicitly contain `gallery_lock_enabled=false`. No credential check guards either selected-file restore or local auto-backup restore. An ordinary older backup made with that setting off is enough to expose the photos it contains after restart.
- Stronger case: a valid ZIP containing an Avex database and preferences with the explicit false flag, but no photo members, preserves the current gallery. `RestoreApply` only replaces photos when the pending photo directory exists. The current photos therefore survive while the incoming preferences turn their lock off.
- Evidence: restore copies any parseable incoming preferences verbatim to the pending preferences file. `SettingsRepository.protection` gives an explicit stored false priority over `ProtectionSentinel`. Restore confirmation is only an ordinary confirmation dialog; it does not call authentication. This circumvents the dedicated authenticated gallery-disable path in `ProtectedSettingsActions`.
- Impact: someone using the unlocked phone can bypass the app's separately protected gallery. No raw filesystem access or database corruption is needed with an existing suitable backup. The crafted archive case requires constructing an otherwise valid archive and makes the current protected gallery accessible.
- Fix: enforce this boundary centrally. Preserve the current privacy/app/gallery protection settings across unauthenticated restore, or require successful current-device authentication before a restore can lower any active protection. Cover both manual and local automatic restore, including archives without photos and explicit false keys.
- Validation: end-to-end source trace; no biometric/device reproduction. This is independent of the already fixed export/folder and direct settings-disable boundaries.

### D2. P2: Factory reset leaves complete private backup and export copies in app storage

- Primary location: `app/src/main/java/com/forge/app/data/repo/ResetRepository.kt:100-109`.
- Supporting locations: `data/repo/BackupRepository.kt:538-570,641-658`; `core/io/ExportFiles.kt:20-26`; `ui/settings/SettingsScreen.kt:152`.
- Trigger: make a local automatic backup and/or JSON/PDF export, then choose Factory reset.
- Evidence: the reset clears Room tables, the live photo directory, avatar, Health Connect records and preferences. It never deletes `filesDir/forge_auto_backup.zip`, `filesDir/exports/`, or the backup status files. The backup ZIP contains the old database, preferences and photos. `restoreFromAutoBackup` remains available because the ZIP still exists. The factory reset copy promises "Deletes ALL data" and "cannot be undone."
- Impact: the apparent wipe retains a recoverable complete copy of sensitive history/photos plus plaintext export files. Re-entering the app can offer the old auto-backup for restoration. Resetting the lock flags makes the remaining unprotected exports especially inconsistent with the promise.
- Fix: define reset ownership of app-private generated files and erase backups, exports and status markers as part of factory reset, with backup/reset work serialized so a concurrent backup cannot recreate them. External user-owned exported documents need a separate explicit policy; this finding concerns private files the app itself retains.
- Validation: all reset callees and export/backup paths inspected; no destructive runtime reset performed.

### D3. P2: Single-session and weekly JSON exports silently turn untracked sessions into tracked sessions on import

- Primary locations: `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:154-172,400-415`.
- Supporting locations: full exporter correctly writes the flag at `BackupRepository.kt:292`; `data/importer/ForgeJsonImporter.kt:138`; `data/importer/WorkoutImportRepository.kt:334`; tracked queries in `data/db/dao/SessionDao.kt:120,137,155` and many others.
- Trigger: mark a completed workout as untracked, export that session or the week as JSON, then import into a fresh app.
- Evidence: the full JSON exporter includes `isUntracked`; the other two session serializers omit it. The importer explicitly defaults a missing key to false and persists that value. Weekly export uses `finishedByFinishTimeInRange`, which includes untracked sessions. The single-session route loads by ID.
- Impact: an excluded workout becomes included in training counts/progression/aggregate statistics after transfer. This is session semantics loss despite the prior fix that correctly preserved timed/assisted set semantics. The weekly serializer also omits `sessionType`, which reinforces the drift between the three separate serializers.
- Fix: share session field serialization across all JSON routes, preserving `isUntracked` and `sessionType`, while allowing route-specific envelope fields. Add one export/import round trip for an untracked session to the existing timed/assisted export test.
- Validation: exporter/importer/persistence source trace. Existing `BackupRestoreTest` exercises timed/assisted fields for all JSON routes but not this session-level flag.

### D4. P2: Weekly JSON import drops distinct same-day cardio entries

- Primary locations: `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:203`; `app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:443-444`.
- Supporting locations: `data/importer/ForgeJsonImporter.kt:216-220`; `data/db/dao/CardioDao.kt:64-65`.
- Trigger: log a 30-minute run at 08:00 for 4 km and another 30-minute run at 18:00 for 5 km on the same date. Export the week and import into an empty app.
- Evidence: weekly export truncates both original instants to the same `yyyy-MM-dd` string. The importer resolves both to local midnight. Extra-row deduplication checks only `(date,type,duration_min)`, ignoring distance and all other distinguishing fields, so the second entry is skipped.
- Impact: an ordinary export/import round trip loses the second workout, duration and distance without identifying a conflict. The same loose key also affects genuinely distinct full-export rows sharing an instant/type/duration.
- Fix: retain exact cardio timestamps in weekly exports and use a semantic fingerprint or durable import identity rather than this three-column key. Retain support for legacy date-only files without treating distinct content as duplicate.
- Validation: deterministic in-memory Python/SQLite reproduction of the exact export date transform and DAO query. Input dates `1789041600000` and `1789077600000` (2026-09-10 08:00/18:00 America/Toronto) both became date `2026-09-10`; only the 4 km row was inserted. This was not an Android runtime test.

### D5. P2: PDF workout exports misrepresent timed sets as zero-repetition sets

- Primary location: `app/src/main/java/com/forge/app/data/repo/PdfExportRepository.kt:115-119`.
- Supporting location: `data/db/entities/LoggedSet.kt:75` stores `durationSeconds`; JSON uses `ExportSetFields` correctly.
- Trigger: log a timed exercise such as a 90-second hold, then export the session as a PDF.
- Evidence: every set is rendered as `weight × reps`, with no branch reading `durationSeconds`. The app represents timed sets with their duration in that field, often with reps zero.
- Impact: the shared/exported workout report says e.g. `BW × 0` instead of the actual 90-second performance. This remained outside the recent shared JSON set serializer fix.
- Fix: use a shared set presentation model that distinguishes timed and repetition-based work. Exercise order/sets should preserve the corresponding timed label and duration in PDFs.
- Validation: source trace, no rendered PDF generated.

### D6. P2: An unsuccessful configured backup-folder write is reported as backup success

- Primary locations: `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:561,569,574-597`.
- Supporting location: `service/AutoBackupWorker.kt:36-38`.
- Trigger: choose an external backup folder, then revoke/unmount its provider or exhaust its capacity before the next automatic backup.
- Evidence: the local ZIP succeeds first; the folder write is wrapped in ignored `runCatching`. The folder helper also returns normally for unavailable tree, failed `createFile`, null output stream and copy failure. `autoBackup` then clears the failure marker and the worker returns success. The code now preserves the previous external ZIP until replacement is complete, correctly fixing prior data-loss behavior, but the current external-copy failure still is never surfaced.
- Impact: the user sees a fresh successful automatic backup even when their selected recoverable folder copy is missing or stale. Losing app-private storage then loses work they reasonably expected in the chosen folder.
- Fix: return structured local/folder results and show partial failure for the configured destination. Preserve the successful local ZIP and previous external ZIP while retaining a destination failure/retry signal. Do not equate the local snapshot timestamp with external-destination success.
- Validation: all early returns and caller result flow inspected; provider fault injection not performed. Distinct from the already fixed "delete previous backup before replacement exists" issue.

## Lower priority confirmed edge cases and cleanup

1. **P3, large Health Connect weight history truncation is treated as complete.** `data/health/HealthConnectManager.kt:321` stops after 5,000 records while retaining no continuation/partial information; `ui/settings/HealthConnectViewModel.kt:229-235` marks any non-null result complete when history permission exists. A history over the cap remains partly unimported while the one-time completion flag suppresses further backfill. Keep the bound, but return a partial result or stream/deduplicate pages by day. Source-confirmed boundary, realistic frequency unknown; do not represent this as a common device failure.

2. **P3, reset settings forgets folder preferences while retaining their SAF permissions.** `data/prefs/SettingsRepository.kt:1301-1337` clears backup/import tree URIs without going through `PersistedTreeGrants`, and factory reset's `resetAll` does the same. Unlike normal folder replacement/removal, these reset routes never release the old persisted grants. Android retains invisible read/write grants after the UI shows no connected folder. Centralize folder-owner cleanup during reset or preserve connected-folder state for a preferences-only reset. No demonstrated data disclosure from the unused grant by itself.

3. **P3, PDF singleton freezes the default timezone.** `data/repo/PdfExportRepository.kt:45` captures `ZoneId.systemDefault()` once while the JSON repository correctly uses a getter. Change device timezone without process death and PDF session date/weekly cardio range uses the old zone. Use a current/injected zone at export invocation.

4. **Optional dead-code removal, do not label a live statistics bug.** `data/repo/StatsEffortAggregations.kt:47-67` has an unused `buildEffortDistribution` helper matching uppercase enum names while `LoggedExerciseDao.effortRatingsSince` returns persisted lowercase codes (`easy`, `just-right`, etc.). Both have no production call sites. Delete the unused pair, or fix the conversion before wiring it. Prior audit already deliberately retained some unused stats/DAO helpers, so this is optional simplification.

5. **Useful simplification tied to actual bugs:** centralize the duplicated session/cardio JSON field writers in `BackupRepository`. The set writer was already centralized and tested; session flags and cardio timestamps still drift across full/weekly/single output routes, producing D3/D4. This is a targeted reason to shorten code, not a broad refactor before release.

## Candidates not promoted without more evidence

- `WorkoutRepository.setSessionTags:632-635` reads a full Session and writes it with `@Update`, unlike newer column-scoped completion/classification mutations. Concurrent finish/untracked changes can be overwritten if the tag writer holds an older snapshot. Need root/UI reviewer to establish the actual live call overlap. Proposed narrow `UPDATE session SET tags = ... WHERE id = ...` avoids the risk regardless.
- `ProgramRepository.rerollDay:374-406` does not take `mutationMutex` used by whole-program generation/save. Its own transaction is atomic, but it can derive from and publish stale state across a simultaneous full mutation. Need a reachable concurrent UI/Coach path before promoting a release finding.
- `BlockRepository.advanceForWeek` persists the advanced block before applying scheduled deload; its best-effort deload call can fail and leave no same-week retry. This is an error-injection/retry boundary to verify, not a reproduced normal-user failure.
- PDF per-exercise outer overflow disclosure does not check individual set rows. Very many sets for one exercise can be clipped without contributing to the omitted-exercise count. Not promoted alongside D5 because frequency and intended one-page scope need an explicit decision.

## Independent confirmation of root privacy findings

These are root-owned findings and should not be counted a second time in the combined report.

- **App lock overlay does not dismiss/cover existing separate Dialog windows.** `MainActivity.kt:498-558` keeps `ForgeNavHost` composed and draws `AppLockScreen` as a sibling in the activity window. `ui/profile/MirrorTestViewer.kt:205` and `MirrorTestCompare.kt:142` use Compose `Dialog`, which owns its own Android window. `ui/nav/ForgeNavHost.kt:454-469` gates gallery content only on `galleryLocked`. With app lock ON and gallery lock OFF, an already open gallery viewer stays composed when returning from background locked. The Dialog therefore remains above the activity's lock overlay. Verified in source, no device/window test performed.
- **Snackbar actions remain above the app lock.** `MainActivity.kt:558` draws `SnackbarControllerHost` after the lock overlay; its callback executes a taken action without consulting lock state. Deleting a goal/cardio row and backgrounding/reopening before the short Undo duration expires permits the action while locked. Root should supply final snackbar callback line references and runtime qualification.

## Validation limits and coverage

Source inspection covered all production Kotlin files under `data/`, `security/`, and `domain/health/`, plus export-file ownership, auto-backup work, Health Connect settings flow, and restore/authentication call sites. The inventory below enumerates those complete directories and the additional files inspected. Full-app root review owns unrelated UI/domain/build code. Existing test names and relevant cases were inspected to identify gaps; test execution belongs to root. No physical provider, backup-folder fault, lock window, biometric, migration-on-device, or rendered PDF validation was performed here.

### Complete file inventory (157 production files)

All files below were source-inspected in full. Additional focused call-site inspection: RestoreApply.kt, MainActivity.kt lock composition, SettingsScreen.kt/SettingsDialogs.kt restore/reset actions, SettingsViewModel.kt export/restore/reset/protected actions, ForgeNavHost.kt gallery lock branches, MirrorTestViewer.kt and MirrorTestCompare.kt Dialog ownership.

- `app/src/main/java/com/forge/app/data/db/Converters.kt`
- `app/src/main/java/com/forge/app/data/db/ForgeDatabase.kt`
- `app/src/main/java/com/forge/app/data/db/ForgeDatabaseFactory.kt`
- `app/src/main/java/com/forge/app/data/db/Migrations.kt`
- `app/src/main/java/com/forge/app/data/db/SessionWrites.kt`
- `app/src/main/java/com/forge/app/data/db/SqliteSnapshot.kt`
- `app/src/main/java/com/forge/app/data/db/dao/AdviceEventDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/ArticleEventDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/BodyFatDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/BodyMeasurementDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/BodyweightDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/CardioDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/CheckinDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/CoachDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/CoachGoalDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/CoachProjectDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/DayNameOverrideDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/ExerciseCustomizationDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/ExerciseGoalDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/ExtendedGoalDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/InjuryRestrictionDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/LeanMassDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/LessonEventDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/LoggedExerciseDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/LoggedSetDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/MoodDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/ProgramCustomizationDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/ProgramDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/RestDayDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/RestEventDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/SessionBreakDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/SessionDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/SessionHrSampleDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/SessionSegmentDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/SuggestionOutcomeDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/TrainingBlockDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/TrophyNearMissDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/UnlockedTrophyDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/VacationDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/WarmupRoutineDao.kt`
- `app/src/main/java/com/forge/app/data/db/dao/WearCommandDao.kt`
- `app/src/main/java/com/forge/app/data/db/entities/AdviceEvent.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ArticleEvent.kt`
- `app/src/main/java/com/forge/app/data/db/entities/BodyFatEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/BodyMeasurementEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/BodyweightEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/CardioEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/CheckinEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/CoachGoal.kt`
- `app/src/main/java/com/forge/app/data/db/entities/CoachPass.kt`
- `app/src/main/java/com/forge/app/data/db/entities/CoachProject.kt`
- `app/src/main/java/com/forge/app/data/db/entities/DayNameOverride.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ExerciseCustomization.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ExerciseGoal.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ExtendedGoal.kt`
- `app/src/main/java/com/forge/app/data/db/entities/InjuryRestriction.kt`
- `app/src/main/java/com/forge/app/data/db/entities/LeanMassEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/LessonEvent.kt`
- `app/src/main/java/com/forge/app/data/db/entities/LoggedExercise.kt`
- `app/src/main/java/com/forge/app/data/db/entities/LoggedSet.kt`
- `app/src/main/java/com/forge/app/data/db/entities/MoodEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/OverlaySource.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ProgramCustomization.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ProgramDay.kt`
- `app/src/main/java/com/forge/app/data/db/entities/ProgramSlot.kt`
- `app/src/main/java/com/forge/app/data/db/entities/RestDayEntry.kt`
- `app/src/main/java/com/forge/app/data/db/entities/RestEvent.kt`
- `app/src/main/java/com/forge/app/data/db/entities/Session.kt`
- `app/src/main/java/com/forge/app/data/db/entities/SessionBreak.kt`
- `app/src/main/java/com/forge/app/data/db/entities/SessionHrSample.kt`
- `app/src/main/java/com/forge/app/data/db/entities/SessionSegment.kt`
- `app/src/main/java/com/forge/app/data/db/entities/SuggestionOutcome.kt`
- `app/src/main/java/com/forge/app/data/db/entities/TrainingBlock.kt`
- `app/src/main/java/com/forge/app/data/db/entities/TrophyNearMiss.kt`
- `app/src/main/java/com/forge/app/data/db/entities/UnlockedTrophy.kt`
- `app/src/main/java/com/forge/app/data/db/entities/VacationPeriod.kt`
- `app/src/main/java/com/forge/app/data/db/entities/WarmupRoutineItem.kt`
- `app/src/main/java/com/forge/app/data/db/entities/WearCommand.kt`
- `app/src/main/java/com/forge/app/data/db/projections/StatsProjections.kt`
- `app/src/main/java/com/forge/app/data/db/types/EffortRating.kt`
- `app/src/main/java/com/forge/app/data/health/HcExerciseTypes.kt`
- `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt`
- `app/src/main/java/com/forge/app/data/importer/CsvParser.kt`
- `app/src/main/java/com/forge/app/data/importer/ExerciseNameMatcher.kt`
- `app/src/main/java/com/forge/app/data/importer/FitNotesImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/ForgeBodyweightCsvImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/ForgeJsonImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/GenericCsvImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/GymImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/HevyImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/ImportFingerprint.kt`
- `app/src/main/java/com/forge/app/data/importer/ImportModels.kt`
- `app/src/main/java/com/forge/app/data/importer/StrongImporter.kt`
- `app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt`
- `app/src/main/java/com/forge/app/data/prefs/PreferencesDataStore.kt`
- `app/src/main/java/com/forge/app/data/prefs/SettingsRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/AcademyRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/AdaptationRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/AvatarRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/BlockRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/BodyFatRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/BodyMeasurementRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/BodyweightRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/CardioRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/CheckinRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/CoachGoalRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/CoachRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/ConditioningRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/CustomizationRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/DirectiveRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/EngineInputSignals.kt`
- `app/src/main/java/com/forge/app/data/repo/ExportSetFields.kt`
- `app/src/main/java/com/forge/app/data/repo/ExtendedGoalRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/GoalRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/LeanMassRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/LibraryRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/NotificationFeed.kt`
- `app/src/main/java/com/forge/app/data/repo/PdfExportRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/PersistedTreeGrants.kt`
- `app/src/main/java/com/forge/app/data/repo/ProfileRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/ProgramCustomizationRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/ProgramRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/ProgressPhotoRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/ProjectRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/ResetRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/RestDayRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/SampleDataSeeder.kt`
- `app/src/main/java/com/forge/app/data/repo/StatsChartAggregations.kt`
- `app/src/main/java/com/forge/app/data/repo/StatsEffortAggregations.kt`
- `app/src/main/java/com/forge/app/data/repo/StatsRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/StatsStrengthAggregations.kt`
- `app/src/main/java/com/forge/app/data/repo/StatsVolumeAggregations.kt`
- `app/src/main/java/com/forge/app/data/repo/StorageRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/TrophyRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/VacationRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/WarmupRepository.kt`
- `app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt`
- `app/src/main/java/com/forge/app/domain/health/ActiveCalorieEstimator.kt`
- `app/src/main/java/com/forge/app/domain/health/BodyFatSync.kt`
- `app/src/main/java/com/forge/app/domain/health/BodyweightSync.kt`
- `app/src/main/java/com/forge/app/domain/health/HcRecordKeys.kt`
- `app/src/main/java/com/forge/app/domain/health/LeanMassSync.kt`
- `app/src/main/java/com/forge/app/domain/health/MetCalories.kt`
- `app/src/main/java/com/forge/app/domain/health/RouteMatching.kt`
- `app/src/main/java/com/forge/app/domain/health/SessionHrAnalysis.kt`
- `app/src/main/java/com/forge/app/domain/health/StepBucketing.kt`
- `app/src/main/java/com/forge/app/domain/health/WatchWorkout.kt`
- `app/src/main/java/com/forge/app/domain/health/WearableBrand.kt`
- `app/src/main/java/com/forge/app/security/AppLockManager.kt`
- `app/src/main/java/com/forge/app/security/BiometricAuthenticator.kt`
- `app/src/main/java/com/forge/app/security/LocalAppLock.kt`
- `app/src/main/java/com/forge/app/security/ProtectedSettingsActions.kt`
- `app/src/main/java/com/forge/app/security/ProtectionSentinel.kt`
- `app/src/main/java/com/forge/app/core/io/ExportFiles.kt`
- `app/src/main/java/com/forge/app/service/AutoBackupWorker.kt`
- `app/src/main/java/com/forge/app/ui/settings/HealthConnectViewModel.kt`

### Additional root request: reachable Coach input

Supported JSON can import five finished tracked bouts for `barbell-bench-press` with `weightLb:100`, `reps:0`, `durationSeconds:60`. ExerciseLibrary.kt:556-557 identifies it as WEIGHT. ForgeJsonImporter.kt:77-78 recognizes the ID, :90-106 reads the three fields independently without exercise-unit validation, :138 defaults tracked. WorkoutImportRepository.kt:244-245 only rejects zero set count; :324-334 writes finished tracked sessions; :358-367 preserves the catalogue ID; :385-396 writes the three fields verbatim. Five distinct session dates avoid dedupe. This establishes the import producer for the domain reviewer's `evaluate` NPE probe; the domain reviewer owns the crash finding and its run evidence.
