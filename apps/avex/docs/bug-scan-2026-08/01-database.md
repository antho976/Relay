# Persistence layer audit — forge-android (Room / migrations / DAOs)

Scope: `app/src/main/java/com/forge/app/data/db/**` plus the repository call sites that determine
whether a persistence-layer defect is actually reachable.

## Verification performed (what is NOT broken)

Before the findings, the things I mechanically proved correct, because a pre-release audit needs the
negative result as much as the positive:

- **Migration chain is complete and exact.** DB version is 36 (`ForgeDatabase.kt:171`). Migrations
  12→13 … 35→36 all exist, all are registered in `ALL_MIGRATIONS` in ascending order
  (`Migrations.kt:506-531`), and the array has no gaps and no duplicates.
- **Every migration step reproduces its exported schema byte-for-byte.** I rebuilt v12 from
  `app/schemas/.../12.json`, replayed each migration's `execSQL` in SQLite, and diffed the resulting
  `PRAGMA table_info` / index / foreign-key state against `13.json` … `36.json` at *every* step.
  Zero mismatches in columns, affinities, NOT NULL, primary keys, indices (including uniqueness) and
  foreign keys. The only diff reported was `sqlite_sequence`, which SQLite creates for AUTOINCREMENT
  and Room ignores.
- **No entity/schema drift.** Every `@Entity` field maps to a column in `36.json` with matching
  column name, affinity and nullability, and every schema column exists on its entity. The one
  apparent extra (`CheckinEntry.hasAnswers`) is a computed getter in the class body, not a column.
- **No unqualified `fallbackToDestructiveMigration()`.** `DatabaseModule.kt:48` uses
  `fallbackToDestructiveMigrationFrom(true, 1..11)` only — the documented pre-lock range — and
  `fallbackToDestructiveMigrationOnDowngrade` is gated on `BuildConfig.DEBUG` (`DatabaseModule.kt:57`).
- **Type converters are lossless.** `Converters.kt:13-18` stores `EffortRating` by its `code` string,
  not ordinal, and `EffortRating.fromCode` (`types/EffortRating.kt:20-21`) returns null rather than
  throwing on an unknown code. Enum reordering is safe. There are no list/JSON/date converters at all
  (comma-joined fields like `session.tags`, `checkin_entry.sore_muscles` and `cardio_entry.conditions`
  are plain `String` columns joined in Kotlin from closed code vocabularies, not free user text).
- **Backup/restore is well hardened.** `BackupRepository.restoreFromIncoming` rejects a backup below
  `MIN_RESTORABLE_VERSION = 12` precisely because the destructive-fallback range would drop tables
  (`BackupRepository.kt:735`), and `ForgeApp.applyPendingRestore` deletes the `-wal`/`-shm` sidecars
  after the DB swap (`ForgeApp.kt:87-90`) so stale WAL frames can't be replayed over the restored file.

Everything below is in the DAO / call-site layer. **There are no migration bugs.**

---

## [CRITICAL] Health Connect import silently destroys the user's bodyweight note

**File:** `app/src/main/java/com/forge/app/data/repo/BodyweightRepository.kt:75`
(mechanism: `app/src/main/java/com/forge/app/data/db/dao/BodyweightDao.kt:13-14`,
unique index from `app/src/main/java/com/forge/app/data/db/Migrations.kt:57-60`,
`note` column added in `Migrations.kt:258-262`)

**What:** `bodyweight_entry` has a UNIQUE index on `date_key` and `BodyweightDao.upsert` is
`@Insert(onConflict = OnConflictStrategy.REPLACE)`. In SQLite, `INSERT OR REPLACE` resolves a unique
conflict by **deleting the conflicting row and inserting a new one** — it is not a column-wise merge.
`importLatestFromHealthConnect` constructs the entity positionally with `note` left at its default
`null`:

```kotlin
dao.upsert(BodyweightEntry(dateKey = dateKey, weightLb = hc.weightLb, recordedAt = hc.timeMs))
```

so the existing row for that day, *including the freeform note the user typed* (GYMAP-54), is dropped.
The row's autoincrement `id` also changes.

This is the exact class of bug `CustomizationRepository.clearSwap` was already fixed for — see its own
comment at `CustomizationRepository.kt:64`: *"The old unconditional DELETE silently wiped the rest
timer and pinned note."* The bodyweight path never got the same treatment; every other write in that
repository that must preserve sibling fields uses `existing.copy(...)`, this one does not.

**Scenario:** 07:00 — user opens the app and logs a weigh-in by hand: 182.4 lb with the note
*"post-vacation, felt bloated"*. Row is `(id=1, '2026-08-25', 182.4, recorded_at=07:00, note='post-vacation, felt bloated')`.
07:05 — the smart scale's reading syncs into Health Connect. On next app open,
`importLatestFromHealthConnect` reads it; `BodyweightSync.shouldImport(hcTimeMs=07:05, …, localLatestMs=07:00)`
returns true because the HC reading is strictly newer (`domain/health/BodyweightSync.kt:22`). The upsert
runs and the row becomes `(id=2, '2026-08-25', 182.6, recorded_at=07:05, note=NULL)`.
The note is gone permanently, with no prompt, no undo, and no trace in the UI. I confirmed the exact
SQLite behaviour against the real DDL:

```
before: [(1, '2026-08-25', 182.4, 1000, 'post-vacation, felt bloated')]
after : [(2, '2026-08-25', 182.6, 1300, None)]
```

Secondary effect from the same line: the `id` changed from 1 to 2, so any UI still holding the old id
(the bodyweight list's delete action calls `BodyweightDao.delete(id)`, `BodyweightDao.kt:35-36`)
silently no-ops against a row that no longer exists.

Note `importHistoryFromHealthConnect` (`BodyweightRepository.kt:101-105`) is **not** affected — it
filters against `existing` date keys first, so it never overwrites a day that already has a row. Only
the "latest" path is broken.

**Fix:** Read-modify-write like every other repo in this package:

```kotlin
val existing = dao.forDate(dateKey)          // add a by-date read to BodyweightDao
dao.upsert(
    existing?.copy(weightLb = hc.weightLb, recordedAt = hc.timeMs)
        ?: BodyweightEntry(dateKey = dateKey, weightLb = hc.weightLb, recordedAt = hc.timeMs)
)
```

Preserving `id` as well as `note` also fixes the stale-id delete. Longer term, switch `BodyweightDao`
(and the other `date_key`-unique tables: `body_fat`, `lean_mass`, `body_measurement`, `checkin_entry`)
from `@Insert(REPLACE)` to `@Upsert`, or to an explicit `@Query("UPDATE … WHERE date_key = …")` +
insert-if-zero-rows, so a future nullable column added to any of them can't be silently erased the same way.

---

## [HIGH] Assisted sets are excluded from PR detection but counted in every max-weight surface

**File:** `app/src/main/java/com/forge/app/data/db/dao/LoggedSetDao.kt:114-121` (`personalBestSet`),
`:135-140` (`maxWeightForExercise`), `:143-148` (`maxWeightAcrossExercises`),
`:155-162` (`maxWeightPerExercise`), `:310-319` (`topLift`)

**What:** `repMaxFrontierForExercise` (`LoggedSetDao.kt:46-57`) carries `AND s.is_assisted = 0` with an
explicit comment — *"Assisted sets are excluded, matching isPr's filter"* — and `PrDetector` is
guarded again in Kotlin at `DayViewModelBuilders.kt:285` and `WorkoutRepository.kt:246`
(`if (!set.isAssisted && PrDetector.isPr(...))`). None of the five max-weight queries above carry that
filter. So a set the PR engine deliberately refuses to recognise still sets the personal best, fills a
goal's progress bar, and unlocks strength trophies.

Consumers: `DayViewModelBuilders.kt:83` (day-screen PB), `GoalRepository.kt:46` (goal progress and the
`achieved` flag), `TrophyRepository.kt:75-76` (Bench Club / Squat Club), `OverviewViewModel.kt:467`
(profile "top lift").

**Scenario:** User sets a goal "Pull-ups → 25 lb weighted" (`exercise_goal`). They train pull-ups on
an assisted machine, dialling in 40 lb of assistance, and log the set as weight `40` then tap the
ASSIST toggle (`DayExerciseHandlers.kt:161-163` → `LoggedSetDao.setAssisted`, which only flips
`is_assisted` on the already-stored weight). Now:

- `GoalRepository.goalsWithProgress()` calls `maxWeightPerExercise(["pull_up"])`, gets `40.0`, and
  `fraction = 40/25 → 1.0`, `achieved = true`. The Goals screen reports the goal **complete**.
- `personalBestSet("pull_up")` returns that set, so the day screen shows "PB 40 lb" for a lift the
  user has only ever done *assisted*.
- Meanwhile the PR system correctly ignores the set entirely, so no PR is flagged and the PRs subtab
  shows nothing — the app contradicts itself on the same data on two adjacent screens.

**Fix:** Add `AND s.is_assisted = 0` to all five queries, matching `repMaxFrontierForExercise`. If any
surface genuinely wants assisted lifts included, that should be an explicit separate query, not the
absence of a filter.

---

## [HIGH] Untracked sessions leak into trophies, goal progress and the PR frontier

**File:** `app/src/main/java/com/forge/app/data/db/dao/LoggedSetDao.kt:46-57`
(`repMaxFrontierForExercise`), `:143-148`, `:155-162`, `:207-212`, `:297-307` (`maxSessionVolume`)

**What:** `Session.isUntracked` is documented as *"session is untracked — excluded from streak,
trophies, suggestions (#110)"* (`entities/Session.kt:30-31`). The stats/adaptation reads honour it —
`observeAllFinishedSetsWithSession` (`LoggedSetDao.kt:176-186`), `bestE1rmLbSince` (`:194-204`),
`allForFinishedSessions` (`:286-294`), `observeRecentPrs` (`LoggedExerciseDao.kt:79-87`) all carry
`s.is_untracked = 0`. The max-weight / max-volume / max-reps family does not.

`TrophyRepository.snapshot()` makes the inconsistency visible inside a single function: at
`TrophyRepository.kt:87-89` it explicitly builds `trackedSessions = allSessions.filter { !it.isUntracked }`
for lifetime tonnage and first-session date, but at `:75-78` it feeds `maxBenchLb`, `maxSquatLb`,
`maxSessionVolumeLb` and `maxSingleExerciseReps` from queries that see untracked rows.

Same gap in `repMaxFrontierForExercise`, which is the *all-time PR bar*.

**Scenario A (unearned trophy):** User does a one-off max-out at a friend's gym, marks the session
untracked so it won't pollute their stats, and benches 245. `maxWeightAcrossExercises(BENCH_EXERCISE_IDS)`
ignores the flag, returns 245, and the "Bench Club 225" trophy unlocks permanently
(`UnlockedTrophyDao.unlock` is IGNORE-on-conflict, so it is never re-evaluated away). The PRs subtab
shows nothing for that session because `observeRecentPrs` filters untracked out — the trophy appears
with no visible lift behind it.

**Scenario B (suppressed real PR):** Same 245 lb untracked bench. Three weeks later the user benches
240 in a normal tracked session — a genuine all-time best for their tracked history.
`repMaxFrontierForExercise("bench", …)` includes the untracked 245, `PrDetector.isPr` returns false,
`wasPr` stays 0, and no PR is recorded or celebrated. The user cannot see why, because the 245 is
hidden from every PR surface.

**Fix:** Join `session` and add `AND s.is_untracked = 0` to `repMaxFrontierForExercise`,
`maxWeightAcrossExercises`, `maxWeightPerExercise`, `maxWeightForExercise`, `personalBestSet`,
`maxSessionVolume` and `maxRepsSummedPerExercise` — the same predicate the stats reads already use.
`maxWeightForExercise`, `personalBestSet`, `maxRepsAnySet` and `maxRepsSummedPerExercise` currently
have no `session` join at all and need one.

---

## [MEDIUM] `lastLoggedBefore` orders by row id, so a history import corrupts "last time" and the weight suggestion

**File:** `app/src/main/java/com/forge/app/data/db/dao/LoggedExerciseDao.kt:40-45`

**What:**

```sql
SELECT * FROM logged_exercise
WHERE exercise_id = :exerciseId AND session_id != :excludeSessionId
ORDER BY id DESC LIMIT 1
```

`id` is an AUTOINCREMENT insertion counter, not a time. It equals chronological order only while every
session is created live. `WorkoutImportRepository` (`data/importer/WorkoutImportRepository.kt:146-183`)
inserts *backdated* sessions — `startedAt` comes from the source file (`:130`) while the session and its
`logged_exercise` rows get fresh, highest-yet ids. After an import, `ORDER BY id DESC` returns the
last row the importer wrote, regardless of when that workout actually happened.

Consumers: `WorkoutRepository.lastPerformanceSets` (`:226`, the freestyle logger's "copy last time"
panel), `WorkoutRepository.lastLoggedExerciseBefore` (`:594-595`) → `DayViewModelBuilders.kt:67`, and
`WatchSessionMirror.lastPerformanceWeightText` (`:149`).

**Scenario:** User trains in Avex for six months (last bench: 205 × 5, three days ago). They then
import three years of Strong history to backfill. The importer writes those sessions with ids above
every existing row; the newest bench in the import file is from just before they switched, 185 × 5.
Open the day screen:

- `prevLE`/`prevSets` resolve to the imported 185 × 5 entry, so the helper text reads
  **"Last: 185 × 5"** (`DayViewModelBuilders.kt:91`) instead of 205 × 5.
- Worse, that stale row is passed straight into `ProgressionAdvisor.suggestNextLoad` as `prevSets` and
  `prevEffort = prevLE?.difficulty` (`DayViewModelBuilders.kt:112-113`), so the *suggested working
  weight* is computed off a six-month-old performance and regresses the user by ~20 lb.

**Fix:** Order by the session's actual time, not the row id:

```sql
SELECT le.* FROM logged_exercise le
INNER JOIN session s ON le.session_id = s.id
WHERE le.exercise_id = :exerciseId AND le.session_id != :excludeSessionId
ORDER BY s.started_at DESC, le.id DESC LIMIT 1
```

(and while adding the join, consider `s.finished_at IS NOT NULL AND s.is_untracked = 0 AND le.skipped = 0`
to match the rest of the day-screen reads).

---

## [MEDIUM] `trophy_near_miss` REPLACE is a no-op — the table grows without bound and truncates the near-miss list

**File:** `app/src/main/java/com/forge/app/data/db/dao/TrophyNearMissDao.kt:13-14`,
entity `app/src/main/java/com/forge/app/data/db/entities/TrophyNearMiss.kt:11-18`

**What:** The DAO declares `@Insert(onConflict = OnConflictStrategy.REPLACE)`, which reads as an
upsert-by-trophy. It is not: the entity's only key is `@PrimaryKey(autoGenerate = true) val id`, and
there is **no unique index on `trophy_id`** (confirmed against `36.json` — `trophy_near_miss` has no
indices at all, and no migration ever adds one). REPLACE therefore never fires, and every evaluation
pass appends a fresh row per near-miss trophy.

`TrophyRepository.recordNearMisses` (`TrophyRepository.kt:138-156`) inserts one row for *every* locked
trophy at ≥ 80 % progress, and `evaluateAndUnlockNew()` runs on every session finish
(`ui/gym/train/DaySessionHandlers.kt:98` and `:173`) and on every cardio log
(`ui/cardio/CardioViewModel.kt:384`). The only pruning is `deleteForTrophies` for trophies that have
since *unlocked* (`TrophyRepository.kt:130`); a trophy that sits at 80–99 % is never pruned.

**Scenario:** A user with 8 trophies parked in the 80–99 % band who trains 4×/week and logs 2 cardio
sessions/week writes 8 × 6 = 48 rows per week — ~2,500 rows/year, forever, all near-duplicates.
The read is `observeRecent()` = `ORDER BY recorded_at DESC LIMIT 50` (`TrophyNearMissDao.kt:16-17`),
so after a single week the 50-row window holds barely one week of passes. Both consumers degrade:

- `TrophiesViewModel` does `.distinctBy { it.trophyName }.take(10)` (`ui/trophies/TrophiesViewModel.kt:54-56`),
  so if 3 trophies dominate the last 50 rows the "how close you came" list shows 3 entries instead of 10,
  and genuinely-close trophies recorded in an earlier pass are invisible.
- Every row in one pass is written with the same `recordedAt = now` (`TrophyRepository.kt:139`), so
  `ORDER BY recorded_at DESC` ties across a whole pass and SQLite's tiebreak decides which of them
  survive the `LIMIT 50` — when a pass writes more than 50 rows the window is an arbitrary slice of a
  single pass and `OverviewViewModel`'s "Up next" pick (`ui/overview/OverviewViewModel.kt:277-281`,
  `maxByOrNull { progress/target }`) can miss the actually-closest trophy.

**Fix:** Make the REPLACE mean what it says — add `indices = [Index(value = ["trophy_id"], unique = true)]`
to the entity plus a migration that de-dupes existing rows and creates the index (the same shape as
`MIGRATION_14_15` for `bodyweight_entry`, `Migrations.kt:51-62`). One live row per trophy then makes
the `LIMIT 50` window meaningful and removes the `distinctBy` workaround.

---

## [MEDIUM] "Reset session data" leaves the coach's learned state pointing at a wiped history

**File:** `app/src/main/java/com/forge/app/data/repo/ResetRepository.kt:24`
(`suspend fun resetSessions() = sessionDao.deleteAll()` → `SessionDao.kt:122-123`)

**What:** `DELETE FROM session` cascades only to tables that declare a foreign key on `session.id`:
`logged_exercise` → `logged_set`, `rest_event`, `session_segment`, `session_break`, `session_hr_sample`
(and SET NULL on `mood_entry`). Several tables that are *derived from* session history have no FK at
all and survive untouched:

- `suggestion_outcome` — read by `SuggestionOutcomeDao.recent(500)`, the weight-chip step calibrator.
- `coach_decision` / `coach_pass` — `CoachDao.allDecisions()` (`CoachDao.kt:114-115`) feeds
  `CoachGenBias.from(...)` at `ProgramRepository.kt:112-113`, which biases *every future program
  generation*, and TrustLedger.
- `advice_event`, `trophy_near_miss`, `rest_day_entry`.
- `mood_entry` rows survive with `session_id = NULL` (SET NULL is deliberate per
  `entities/MoodEntry.kt:13-15` for a *single* session delete) and are still returned by
  `MoodDao.since(...)`, which feeds readiness at `AdaptationRepository.kt:175` and `:220`.

The confirmation dialog tells the user this is total: *"Deletes all sessions, sets, and exercises
logged. Cannot be undone."* (`ui/settings/SettingsScreen.kt:146`).

**Scenario:** User's coach has been steadily wrong, so they reset session data to start clean. Their
workouts, sets and PRs are gone. But `coach_decision` still holds 40 applied/failed decisions judged
against those deleted sessions; the very next program generation calls `coachBias()` and applies
`volumeBias` / `prefer` / `avoid` / `repBias` derived entirely from a history the user just erased.
Readiness still reads mood entries for workouts that no longer exist. The user is handed a "fresh"
program that is silently shaped by the data they asked to delete.

**Fix:** Make `resetSessions()` transactional over the full session-derived set rather than relying on
CASCADE reach:

```kotlin
suspend fun resetSessions() = db.withTransaction {
    sessionDao.deleteAll()
    moodDao.deleteAll()            // add
    suggestionOutcomeDao.deleteAll()
    adviceEventDao.deleteAll()
    restEventDao.deleteAll()
    coachDao.deleteAllDecisions(); coachDao.deleteAllPasses()
    nearMissDao.deleteAll()
}
```

(all of those `deleteAll()` methods already exist). Alternatively narrow the dialog copy — but silently
retaining generation-affecting state past an explicit reset is the worse of the two.

---

## [MEDIUM] Trophy and PB maxima include the in-progress session

**File:** `app/src/main/java/com/forge/app/data/db/dao/LoggedSetDao.kt:135-140`
(`maxWeightForExercise`), `:114-121` (`personalBestSet`), `:155-162` (`maxWeightPerExercise`),
`:207-208` (`maxRepsAnySet`), `:211-212` (`maxRepsSummedPerExercise`)

**What:** These five queries have no `session` join and therefore no `finished_at IS NOT NULL`
predicate, unlike their siblings `maxSessionVolume` (`:297-307`) and `topLift` (`:310-319`), which do
filter finished sessions. They read the live, unfinished workout's sets.

**Scenario:** Mid-workout the user types `2255` into the weight field instead of `225` and logs the
set, then immediately notices and deletes it. Between the log and the delete, finishing any exercise
triggers `evaluateAndUnlockNew()` (`ui/gym/train/DaySessionHandlers.kt:98`), whose snapshot calls
`maxWeightAcrossExercises(BENCH_EXERCISE_IDS)`. The typo's 2255 is visible, every bench trophy
unlocks, and `UnlockedTrophyDao.unlock` is `OnConflictStrategy.IGNORE` (`UnlockedTrophyDao.kt:14`) —
so deleting the set does *not* re-lock them. The trophies are permanent. The same set also shows as
the day screen's PB (`personalBestSet` → `DayViewModelBuilders.kt:83`) and completes any bench goal
(`maxWeightPerExercise` → `GoalRepository.kt:46`) until it is deleted.

**Fix:** Add the `INNER JOIN session ss ON le.session_id = ss.id AND ss.finished_at IS NOT NULL`
these queries are missing (folding in naturally with the `is_untracked` and `is_assisted` fixes above,
since all three want the same join).

---

## [LOW] `perDayTypeStats` computes duration from wall-clock, bypassing `active_seconds`

**File:** `app/src/main/java/com/forge/app/data/db/dao/SessionDao.kt:211-219`

**What:** `AVG((finished_at - started_at) / 60000.0) AS avg_duration_min`. Migration 19→20 added
`session.active_seconds` precisely so duration surfaces stop reading wall-clock
(`Migrations.kt:167-172`), and `Session.durationMinutes()` (`entities/Session.kt:54-62`) is documented
as the single funnel: *"All display surfaces (session detail, overview, history rows, the weekly-duration
chart) read through this so they can never disagree on a session's length."* This SQL is a duration
surface that does not.

The query also omits the `is_untracked = 0` filter that its immediate neighbours
`avgMaxVolumeByDayKey` (`:189-193`) and `lifetimeAggregate` (`:197-201`) both carry.

**Scenario (latent):** A session started at 18:00, trained for 40 minutes, left, and resumed and
finished at 22:30 has `active_seconds ≈ 2400` but `finished_at - started_at = 270 min`. Any screen
wired to `avg_duration_min` would report a 4½-hour average for that day type.

Currently unreachable: `perDayTypeStats()` has **no callers** outside the DAO (the Recap screen's
`avgDurationMin` at `ui/recap/RecapViewModel.kt:93` is computed elsewhere). It is dead code that will
produce wrong numbers the moment someone wires it up.

**Fix:** Either delete it, or change the expression to
`AVG(CASE WHEN active_seconds > 0 THEN active_seconds / 60.0 ELSE (finished_at - started_at) / 60000.0 END)`
and add `AND is_untracked = 0`, mirroring `durationMinutes()`.

---

## [LOW] `session_break` is write-only

**File:** `app/src/main/java/com/forge/app/data/db/dao/SessionBreakDao.kt:7-11`

**What:** The DAO exposes `@Insert` and nothing else — no query, no delete. `WorkoutRepository.kt:663`
writes a row per logged break; no code anywhere reads `session_break`. Rows accumulate for the life of
the install (they do CASCADE away with their session, so this is bounded by session count, not
unbounded).

**Scenario:** A user who takes 5 breaks per workout over two years accumulates ~2,000 rows that are
written, backed up, restored and migrated, and never read by anything.

**Fix:** Either add the read the feature was intended to have, or drop the write at
`WorkoutRepository.kt:663` and retire the table in a future migration. Not a correctness bug — flagging
it so the pre-release DB isn't carrying a dead write path.

---

## [LOW] Column defaults exist on the upgrade path but not on a fresh install

**File:** `app/src/main/java/com/forge/app/data/db/Migrations.kt:159-163, 176, 200-201, 397-399, 419-421`

**What:** Several migrations add columns with SQL defaults — `coach_decision.day_key DEFAULT ''`,
`coach_decision.outcome DEFAULT 'pending'`, `coach_decision.scope DEFAULT 'week'`,
`session.active_seconds DEFAULT 0`, `program_customization.source DEFAULT 'user'`,
`exercise_customization.source DEFAULT 'user'`, and `checkin_entry`'s `sick` / `sore_muscles` /
`skipped`. The corresponding entities declare Kotlin defaults but **no `@ColumnInfo(defaultValue = …)`**,
so `36.json` records no `defaultValue` and a fresh install creates those columns as NOT NULL with no
SQL default.

Room's `TableInfo.Column.equals` only validates a default when the *entity* declares one, so this
passes schema validation and `MigrationTest` today — it is not a live bug.

**Scenario (latent):** An upgraded device and a fresh install end up with structurally different
tables. Any future raw `execSQL` insert or partial-column insert that omits, say, `checkin_entry.sick`
succeeds on every upgraded device and throws `NOT NULL constraint failed` on every fresh install — a
bug class that will only reproduce on new users' phones.

**Fix:** Add `@ColumnInfo(defaultValue = "0")` / `"''"` / `"'pending'"` / `"'week'"` / `"'user'"` to
the matching entity fields and regenerate the schema, so both creation paths converge. This is a
schema-version bump, so it can wait for the next migration rather than gating this release.

---

## Verification note (independently re-checked)

The CRITICAL bodyweight-note finding was re-verified from the primary sources rather than
taken on trust. All four links in the chain hold:

1. `BodyweightEntry.kt` declares `note: String? = null` — a **defaulted** parameter, and
   `@PrimaryKey(autoGenerate = true) val id: Long = 0`, under a `unique = true` index on
   `date_key`.
2. `BodyweightDao.upsert` is `@Insert(onConflict = OnConflictStrategy.REPLACE)` — which Room
   compiles to SQLite `INSERT OR REPLACE`, i.e. **DELETE the conflicting row, then INSERT**.
   It is not an UPDATE and does not preserve unlisted columns.
3. `BodyweightRepository.importLatestFromHealthConnect()` constructs
   `BodyweightEntry(dateKey = ..., weightLb = ..., recordedAt = ...)` — omitting `note`, so it
   is `null` at the moment of the REPLACE.
4. The guard `BodyweightSync.shouldImport` only compares **timestamps**, not date keys. So it
   permits exactly the collision case.

**Concrete failing scenario:** user types a weigh-in at 07:00 with the note "fasted". At 18:00
the same day their smart scale posts a reading to Health Connect. `shouldImport` sees
18:00 > 07:00 → true. `dateKey` resolves to the same calendar day → unique-index conflict →
the 07:00 row is **deleted** and replaced by a note-less row. The note is gone permanently,
with no undo, and the row's `id` changes (autoGenerate assigns a fresh rowid), so any UI or
pending edit holding the old id now points at nothing.

This directly contradicts the method's own KDoc, which promises "so a typed weigh-in is never
overwritten and re-importing is idempotent". The guard does protect the *weight value* from
older readings; it does not protect the *note* from a newer same-day one.

**Fix:** read the existing row and carry the note forward, mirroring the pattern already
applied in `CustomizationRepository.clearSwap` (see its comment at `CustomizationRepository.kt:64`):
`dao.findByDateKey(dateKey)?.let { existing -> entry.copy(id = existing.id, note = existing.note) }`
— or switch the DAO to `@Upsert` / an explicit UPDATE that names only the columns HC owns.
