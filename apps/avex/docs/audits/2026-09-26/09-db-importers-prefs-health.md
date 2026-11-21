# Room database, importers, preferences, Health Connect

Scope: `data/db/**` (database, migrations, 35 DAOs, 37 entities, projections, types), `data/importer/**`, `data/prefs/**`, `data/health/**`, plus `app/schemas/`. 96 files, all read in full. The migration chain was replayed in sqlite3 against all 32 exported schemas.
Paths are relative to `forge-android/app/src/main/java/com/forge/app/data/` unless noted.

**Counts:** P1 1 · P2 5 · P3 9 · P4 10.

**Confirmed OK:** migration chain is contiguous 12→38; SQL, indices and foreign keys match each exported schema (only column DEFAULTs differ); destructive fallback limited to versions 1–11; schema export and migration tests in place; converters store codes, not ordinals; no REPLACE insert cascade-deletes children; no `!!`, TODO or logging in scope; Health Connect calls are permission-gated and fail soft.

## P1

### [P1] Hevy imports from pound-based accounts come in with no weights
- Category: bug (wrong training data saved)
- Location: `importer/HevyImporter.kt:13-17, 32, 37` (KDoc at `:5` says "Weight is always metric")
- Problem: `canParse` accepts a header with `exercise_title` plus *either* `weight_kg` or `start_time`, but `parse` only reads `weight_kg`. Hevy writes the account's unit into the header, so an imperial account's export has `weight_lbs` (and `distance_miles`). For such a file every row's weight cell is "", every set is stored as bodyweight (`weightLb = null`), session volume is 0, and the rows are kept because reps > 0. Years of history arrive weightless with no bulk undo; PRs, goals and trophies read empty. *Code path re-checked by the coordinator.*
- Fix: resolve the weight column like FitNotes does: `weight_kg` → kg→lb, else `weight_lbs` as-is, else `assumeKg`. Add a fixture test for both headers.
- Confidence: medium (code path certain; the `weight_lbs` header comes from knowledge of Hevy's format, not a fixture)

## P2

### [P2] Strong and Hevy cardio rows become phantom lifting sessions again, as "timed holds"
- Category: bug (regression)
- Location: `importer/StrongImporter.kt:41-43`; `importer/HevyImporter.kt:33-35`
- Problem: the prior audit's exact row `2024-06-02 07:15:00,Morning Cardio,45m,Running,1,0,kg,0,,5,km,1800,,` now passes the filter (it also requires `durationSeconds == null`, and Seconds = 1800), becoming an `ext-running` exercise with one 1800-second hold and the 5 km dropped. Hevy cardio rows take the same path. A year of runs becomes ~150 finished strength sessions, inflating workout counts, streaks, trophies and "workouts this week". Regression of the fix in `docs/bug-scan-2026-08/02-backup-import.md`.
- Fix: a row is a hold only when distance is blank/0; rows with a distance are skipped (and counted) or routed to `ImportedCardio`.
- Confidence: high

### [P2] Generic CSV drops unpadded, slash-plus-time and short ISO dates; ambiguous dates decided row by row
- Category: bug
- Location: `importer/GymImporter.kt:66-71, 85-92, 97-117`
- Problem: tested on JDK 21: `"5/4/2024"` (Excel/Sheets US default; `dd`/`MM` need two digits), `"05/04/2024 18:30"`, `"2024-05-06T18:30"`, `"2024-05-06T18:30:00.123"` all return null and hit `?: continue` silently; a US spreadsheet imports nothing and says "No new workouts found". Day/month order is chosen per row, so a UK file on an en-US phone reads `13/05` correctly but `04/05` as April 5.
- Fix: `M/d/yyyy`, `d/M/yyyy` + time variants; `ISO_LOCAL_DATE_TIME`; decide day/month order once per file by scanning for a first component > 12.
- Confidence: high

### [P2] Import duplicate check duplicates history on common re-imports
- Category: bug
- Location: `importer/WorkoutImportRepository.kt:280-301, 564-612, 615-658`; `importer/ImportFingerprint.kt:15-17`
- Problem: a stored session matches only on exact fingerprint equality, and the fingerprint includes user-editable fields and matcher-resolved ids; any mismatch inserts again at start + 1 s.
  - (a) import a Strong file, mark a session untracked or annotate an exercise, re-import the next export → a second tracked copy;
  - (b) an app update's matcher now resolves "Cable Fly" (previously `ext-cable-fly`) → every session with that lift is duplicated;
  - (c) importing Avex's own JSON onto the device that made it duplicates overlapping sessions that have a skipped exercise (dropped at `ForgeJsonImporter.kt:114`), a session swap on a library exercise (`swappedName` forced null, `:369`/`:588`), `activeSeconds = 0` (export writes wall-clock, `BackupRepository.kt:99-101`), a non-library id, or a `""` note.
- Fix: build identity only from source-provided facts (start slot, source exercise name, reps/weight/hold/RPE/warm-up), or store an `import_key` hash of the raw source workout (migration 38→39). For Avex JSON, normalize both sides.
- Confidence: high

### [P2] "Reset app settings" wipes one-shot latches and user data
- Category: bug
- Location: `prefs/SettingsRepository.kt:1316-1354`, called from `repo/ResetRepository.kt:93` (UI copy: "Does not delete your data.")
- Problem: `prefs.clear()` then restores 16 hand-picked keys. Lost: `SHOWN_MILESTONES`/`UNREAD_MILESTONES` (milestones re-fire, `OverviewUiStateMapper.kt:163-179`); dismissal/announcement sets and `COACH_ADVANCED_PROMPT_AFTER`, `LAST_SEEN_COACH_WEEK_ID`; `DELOAD_WEEK_START_MS`, `PROGRAM_GENERATION_INTENT`, `PROGRAM_GENERATION_SEED` (reset mid-deload loses the "deload already running" guard, `CoachRepository.kt:691-695`, and the deload never ends; see `04-settings…`); the weekly schedule, liked/disliked/pinned/favourite exercises, problem areas, priority muscles, `HC_WEIGHT_HISTORY_IMPORTED`; import/backup folder URIs without releasing their grants (still open, data.md lower #2).
- Fix: clear only an allow-list of real settings (`SettingsSection.entries.flatMap { it.keys }` + other true settings); release grants via `PersistedTreeGrants`.
- Confidence: high

### [P2] Cardio duplicate check drops distinct entries — still open from data.md D4
- Category: bug
- Location: `db/dao/CardioDao.kt:63-65`; `importer/WorkoutImportRepository.kt:444`; `importer/ForgeJsonImporter.kt:216-222`
- Problem: `existsAt(date, type, durationMin)` is the whole identity; weekly exports write date-only (local midnight), so a 30-min 4 km run and a 30-min 5 km run the same day collapse. `intervalCount`, `hrZone`, `conditions` are neither exported nor imported.
- Fix: dedupe on a semantic key or import identity; exact timestamps; round-trip the three fields.
- Confidence: high

## P3

### [P3] Skipped exercises dropped when importing Avex's own JSON
- Location: `importer/ForgeJsonImporter.kt:83, 114-122`
- Problem: skipped exercises export with `"sets": []` but are kept only `if (sets.isNotEmpty())`; skip history (and the honesty %) is lost after a device migration.
- Fix: keep an exercise when `skipped` is true.

### [P3] Synthetic ids for unmatched exercises collide after 40 characters
- Location: `importer/WorkoutImportRepository.kt:660-668`
- Problem: `.take(40)` makes "Incline Dumbbell Bench Press (Neutral Grip)" and "…(Neutral Grip, Paused)" the same id; PRs, prefill and charts merge.
- Fix: append a short stable hash when truncating (as `ExerciseBrowserScreen.kt:126-128` does); trim a trailing `-`.
- Confidence: medium

### [P3] Unparseable rows and importer exceptions both reported as "No new workouts" — still open (02-backup-import.md)
- Location: `importer/WorkoutImportRepository.kt:85-91, 425`; `importer/ImportModels.kt:174-184`
- Problem: bad rows are skipped silently and `skippedRows = 0` is hard-coded; a `parse` exception becomes `emptyList()` → `NothingToImport`; `skippedRows`, `exercises`, `matchedExercises` are never read.
- Fix: count skipped rows and show them in `userMessage()`; map exceptions to a parse error.

### [P3] Date format, 12h/24h and Timezone settings do nothing
- Category: bug / dead-code
- Location: `prefs/SettingsRepository.kt:584-598`; `prefs/PreferencesDataStore.kt:185-190`; via `MainActivity.kt:462-471` into `ui/theme/ForgeUiSettings.kt:22-23`
- Problem: saved and shown in Settings → Format, but nothing reads `dateFormat`/`timeFormat24h`, and `timezone` is read only by `SettingsViewModel:312`. The `DATE_FORMAT` KDoc disagrees with its default. (Same finding from the UI side in `04-…` and `06-…`.)
- Fix: wire in (shared formatter, injected `ZoneId`) or remove.

### [P3] Health Connect recovery reads cut to the oldest 1000 records
- Location: `health/HealthConnectManager.kt:491-522` (compare `readAllPages` at `:581-595`)
- Problem: sleep, resting HR and HRV use one ascending page of 1000 over a ≥90-day window (`repo/AdaptationRepository.kt:73-77`); a provider writing >~11 records/day loses the newest weeks, silently emptying the trend windows.
- Fix: `readAllPages`, or read descending.
- Confidence: medium

### [P3] Weight-history backfill capped at 5000 records, then marked complete — still open (data.md lower #1)
- Location: `health/HealthConnectManager.kt:300-324, 1180`; `ui/settings/HealthConnectViewModel.kt:224-235`
- Problem: ascending reads lose the *newest* readings past the cap, then "history imported" is set true so it never retries.
- Fix: return a truncation marker and don't set the flag, or reduce to one reading per day while paging.

### [P3] Day colours and the default weekly schedule go stale when the program changes
- Location: `prefs/SettingsRepository.kt:285-290, 862-866, 886-888`
- Problem: both read `Program.dayKeys` inside `pref {}` (plain in-memory state), so they recompute only when DataStore emits; combining with `programRepository.revision` (`DayListViewModel.kt:84-88`) doesn't recompute them. After cold-start load (`ForgeApp.kt:63`) or regeneration the day list shows stale values.
- Fix: read all `day_color_*` keys from `prefs.asMap()`; apply `defaultFor(dayKeys)` downstream combined with the revision flow.
- Confidence: medium

### [P3] FitNotes timed holds dropped silently
- Location: `importer/FitNotesImporter.kt:38-40` (KDoc `:7-8` lists a `Time` column); `importer/GenericCsvImporter.kt:36-38`
- Problem: a plank row (Weight 0, Reps 0, Time) is dropped uncounted because `Time` is never read.
- Fix: parse `time` via `parseDurationToMillis` into `durationSeconds`; exclude rows with a distance.
- Confidence: medium (Time format not verified against a real export)

### [P3] Date parsing throws per format and rebuilds formatters every row
- Category: simplify / perf
- Location: `importer/GymImporter.kt:83-117`
- Problem: `DATE_ONLY_FORMATS` is a getter rebuilding 8 formatters per call; `yyyy-MM-dd` fails 13 formats by exception before matching. ~0.9–1.1 s per 50k rows on desktop JVM, several times slower on a phone.
- Fix: memoize per raw string, build the list once, try ISO first, use `parseUnresolved` to avoid throwing.

## P4

- **bug:** Hevy `failure` and `dropset` set types discarded; only `warmup` is read (`importer/HevyImporter.kt:39, 53`). (medium)
- **dead-code:** ~30 DAO methods with no production caller (receiver-aware grep across app/wear/shared main+test): `AdviceEventDao.forAdvice`; `CoachGoalDao.update`, `byId`; `CheckinDao.all`, `delete`; `ExtendedGoalDao.update`, `observeForExercise`, `markComplete`, `deleteAll`; `InjuryRestrictionDao.all`, `delete`; `LoggedExerciseDao.delete`, `observePrCount`, `observeTotalLogged`, `observeHeatmapTimestamps` (+ `HeatmapTimestamp`), `observeAllPrs`, `prDatesPerExercise`, `effortRatingsSince`; `LoggedSetDao.maxRepsAnySet`, `topLift`; `MoodDao.observeRecent`, `observeAll`; `ProgramDao.observeDays`, `observeSlots`; `SessionDao.distinctDayKeysTrained` (test only), `observeFinishedInRange`, `observeMoodOverTime`, `lifetimeAggregate`, `perDayTypeStats`; `SessionHrSampleDao.deleteForSession`; `TrainingBlockDao.active`; `deleteAll` on BodyFat, BodyMeasurement, Bodyweight, LeanMass, LessonEvent, Vacation, WarmupRoutine DAOs (factory reset uses `clearAllTables`).
- **dead-code:** the retired `article_event` table still has a DAO and Hilt provider (`db/dao/ArticleEventDao.kt`, `db/ForgeDatabase.kt:190`, `di/DatabaseModule.kt:65`); remove now, drop the table at the next schema bump.
- **dead-code — prefs:** unreferenced `ROTATION_UNIT` (`:318`), `LAST_ROTATED_AT_MS` (`:321`); write-only `WELCOMED`; read-but-never-written `FONT_CHOICE`/`fontChoice` (still in the APPEARANCE reset list), `QUIET_HOURS_START/_END` (legacy seed), `USER_AGE_YEARS`/`MAX_HR_OVERRIDE` (setters have no callers, so `ConditioningRepository.kt:50-51` always sees 0); no-caller members `monthlyPrTarget` (+ setter/key), `coachBriefIntroSeen` (+ setter/key), `rotationCounter`, `setExerciseLiked`, `setWeeklySchedule` (test only); stale "reserved for UI pass" comments (`:170`, `:174`).
- **bad-code — stale docs:** `db/ForgeDatabase.kt:77-111` says "Schema is v31" (it's 38); `:110` and `db/Migrations.kt:9` point to `DatabaseModule` for the destructive fallback (now `ForgeDatabaseFactory.kt:27`); `LoggedExerciseDao.kt:273-282` stacked KDocs; `RestEventDao.kt:14` doc on the wrong method; `TrainingBlockDao.kt:38` "Finished blocks" returns every block; `WorkoutImportRepository.kt:219-226, 303-313, 523-530, 754-755`; `HevyImporter.kt:5`.
- **bad-code:** column DEFAULTs differ between upgraded and fresh installs (confirmed by sqlite replay; `db/Migrations.kt:159, 162, 176, 200-201, 397-398, 419-421`; still open, 01-database.md LOW). Add `@ColumnInfo(defaultValue = …)` at the next bump.
- **bug (minor):** `wear_command` grows without bound (`db/dao/WearCommandDao.kt`); add a ~7-day retention delete.
- **bug (minor):** bodyweight CSV importer accepts impossible dates (`2026-02-30` passes the regex; stored with `recordedAt = now`) (`importer/ForgeBodyweightCsvImporter.kt:43, 52`; `WorkoutImportRepository.kt:502-505`).
- **bad-code / simplify:** an import commits sessions and extras in separate transactions (`WorkoutImportRepository.kt:242` vs `:442`); `ForgeJsonImporter` parses the full JSON tree separately in `parse`, `parseExtras` and the folder scan (`:149`, `:160`); `ExerciseNameMatcher.kt:138-139` redundant `replace`s; `HealthConnectManager.kt:967-968` makes a permissions call per record in `toWatchWorkout`.

## Files reviewed (96)
- `db/`: Converters, ForgeDatabase, ForgeDatabaseFactory, Migrations, SessionWrites, SqliteSnapshot
- `db/dao/` (35): AdviceEvent, ArticleEvent, BodyFat, BodyMeasurement, Bodyweight, Cardio, Checkin, Coach, CoachGoal, CoachProject, DayNameOverride, ExerciseCustomization, ExerciseGoal, ExtendedGoal, InjuryRestriction, LeanMass, LessonEvent, LoggedExercise, LoggedSet, Mood, ProgramCustomization, Program, RestDay, RestEvent, SessionBreak, Session, SessionHrSample, SessionSegment, SuggestionOutcome, TrainingBlock, TrophyNearMiss, UnlockedTrophy, Vacation, WarmupRoutine, WearCommand
- `db/entities/` (37): AdviceEvent, ArticleEvent, BodyFatEntry, BodyMeasurementEntry, BodyweightEntry, CardioEntry, CheckinEntry, CoachGoal, CoachPass, CoachProject, DayNameOverride, ExerciseCustomization, ExerciseGoal, ExtendedGoal, InjuryRestriction, LeanMassEntry, LessonEvent, LoggedExercise, LoggedSet, MoodEntry, OverlaySource, ProgramCustomization, ProgramDay, ProgramSlot, RestDayEntry, RestEvent, Session, SessionBreak, SessionHrSample, SessionSegment, SuggestionOutcome, TrainingBlock, TrophyNearMiss, UnlockedTrophy, VacationPeriod, WarmupRoutineItem, WearCommand
- `db/projections/StatsProjections`, `db/types/EffortRating`
- `health/`: HcExerciseTypes, HealthConnectManager
- `importer/`: CsvParser, ExerciseNameMatcher, FitNotesImporter, ForgeBodyweightCsvImporter, ForgeJsonImporter, GenericCsvImporter, GymImporter, HevyImporter, ImportFingerprint, ImportModels, StrongImporter, WorkoutImportRepository
- `prefs/`: PreferencesDataStore, SettingsRepository
- Also checked: all 32 `app/schemas` files, MigrationChainTest, WatchCommandMigrationTest, MigrationTest, DatabaseModule.
