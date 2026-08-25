# Area 02 — Backup / Restore / Data Import

Scope: `data/repo/BackupRepository.kt`, `data/importer/*`, `ForgeApp.applyPendingRestore`,
`service/AutoBackupWorker.kt`, plus the export/share writers. Round-trip traced field-by-field
for `exportFullDataJson` → `ForgeJsonImporter` and for `backupToUri` → `restoreFromUri` →
`applyPendingRestore`.

Test coverage today: **only** `app/src/test/java/com/forge/app/data/importer/ImporterTest.kt`
(126 lines, 10 tests: CSV quoting/BOM/semicolon, 5 name-match asserts, one happy-path parse per
importer). There is **zero** test coverage for `BackupRepository` (backup, restore, validation,
staging), `ForgeApp.applyPendingRestore`, `WorkoutImportRepository.insert` (dup guard, nonce,
FK remap, volume), `ForgeJsonImporter`, locale decimals, CRLF, short rows, or any export→import
round trip.

---

## [CRITICAL] A restore that reports failure still replaces the live database at the next launch

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:740-746`, `:782-788`;
`app/src/main/java/com/forge/app/ForgeApp.kt:67-89`

**What:** `restoreFromIncoming` stages the four restore components one after another *without a
rollback*:

```kotlin
val pendingDb = File(context.filesDir, "pending_restore.db")
if (pendingDb.exists()) pendingDb.delete()
dbFile.copyTo(pendingDb, overwrite = true)          // line 742 — succeeds

val pendingPrefs = File(context.filesDir, "pending_restore_prefs.pb")
if (pendingPrefs.exists()) pendingPrefs.delete()
prefsFile?.copyTo(pendingPrefs, overwrite = true)   // line 746 — throws IOException
```

The `catch (e: java.io.IOException)` at line 782 returns `IO_ERROR`, and the `finally` at line 785
deletes only the **cacheDir** temps (`temps`, `photoStage`). `pending_restore.db` — already written
into `filesDir` — is left behind. Nothing anywhere else in the codebase deletes it (verified:
`grep -rn "pending_restore"` matches only BackupRepository's own writes and ForgeApp's swap).

`SettingsViewModel.restoreDatabase` (line 639-642) only sets `_restoreSucceeded` on `SUCCESS`, so
the app does **not** restart; the user sees "Couldn't read that file. Try again, or pick a
different copy." and carries on. On the next cold start `ForgeApp.applyPendingRestore()` (called
first thing in `onCreate`, line 41) sees `pending_restore.db`, swaps it over `forge.db`, and
deletes the WAL/SHM sidecars.

The same hole exists for the photo block (`:753-762`, `copyTo` can throw) and the avatar block
(`:764-768`).

**Scenario:** Storage is nearly full. User restores a backup; the DB copy (say 8 MB) succeeds, the
prefs copy fails with ENOSPC. UI says the restore failed. User frees space, trains for two weeks,
logs 8 workouts. Two weeks later the process is cold-started (OEM kill, reboot, update) →
`applyPendingRestore` swaps in the two-week-old staged DB. Those 8 workouts, and every PR, photo
link and streak since, are gone, with no user action and no message. The staged file is the *only*
copy of the old state and the live DB is overwritten via rename, so there is nothing to recover.

**Fix:** Wrap the staging block so any failure deletes everything already staged:
```kotlin
val staged = mutableListOf<File>()
try { ...stage each, staged.add(it)... }
catch (e: Exception) { staged.forEach { it.delete() }; throw e }
```
and add a `pending_restore.db` sanity gate in `applyPendingRestore` (e.g. only apply when a
sibling `pending_restore.ready` marker written *last* by the repository exists). Also clear all
`pending_restore*` files at the start of every `restoreFromIncoming` so a previously-abandoned
stage can't be resurrected by an unrelated later restore.

---

## [CRITICAL] Auto-backup deletes the user's only off-device backup before writing its replacement, and swallows the failure

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:440-446`, call site `:428`

**What:**
```kotlin
private fun writeZipToFolder(folderUri: Uri, snap: File) {
    val tree = DocumentFile.fromTreeUri(context, folderUri) ?: return
    tree.findFile(AUTO_BACKUP_NAME)?.delete()                       // 443 — good copy destroyed
    val doc = tree.createFile("application/zip", AUTO_BACKUP_NAME) ?: return   // 444 — may return null
    context.contentResolver.openOutputStream(doc.uri)?.use { out -> writeBackupZip(out, snap) }  // 445
}
```
The previous, known-good backup is deleted *first*. If `createFile` returns null, if
`openOutputStream` returns null, or if `writeBackupZip` throws mid-stream (disk full, SD card
ejected, cloud provider offline, permission revoked), the folder is left with **no backup or a
0-byte / truncated one**.

The call site makes it silent:
```kotlin
if (folderUri != null) runCatching { writeZipToFolder(folderUri, snap) }   // 428 — result discarded
```
`autoBackup` then returns normally, `AUTO_BACKUP_FAILED_MARKER` is *deleted* (line 435),
`autoBackupFailed()` reports false, and `hasAnyBackup()` reports true. The user is told they are
protected.

**Scenario:** User points the backup folder at a Google Drive / SD-card tree (the GYMAP-67
feature, whose whole point per the comment on line 426 is "so the backup survives an uninstall").
Weekly worker fires while the SD card is unmounted or the cloud provider's SAF process has been
killed. `findFile(...)?.delete()` succeeds against the stale document tree, `createFile` returns
null → early `return`. The folder now holds nothing. Two months later the user reinstalls the app
after a phone swap, opens the folder, and there is no `forge_auto_backup.zip` — the only copy that
survived the uninstall was deleted by the backup itself.

**Fix:** Write to a temp name in the tree first (`forge_auto_backup.zip.tmp`), verify the stream
closed without error and the resulting length is > 0, then delete the old file and rename the temp
into place. Propagate the failure to the caller so `recordAutoBackupFailure()` runs instead of
`AUTO_BACKUP_FAILED_MARKER.delete()`.

---

## [CRITICAL] Auto-backup truncates the internal backup slot in place — a failed write destroys the previous good backup

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:421-437`

**What:**
```kotlin
val file = File(context.filesDir, AUTO_BACKUP_NAME)   // forge_auto_backup.zip
val snap = snapshotDatabase()
try {
    file.outputStream().use { out -> writeBackupZip(out, snap) }   // 425
    ...
} finally { snap.delete() }
...
File(context.filesDir, AUTO_BACKUP_FAILED_MARKER).delete()         // 435
```
`File.outputStream()` opens with truncation, so the moment line 425 runs the previous week's
good ZIP is gone. If `writeBackupZip` throws part-way (ENOSPC — very likely, since it has just
written a full DB snapshot to `cacheDir` in `snapshotDatabase()` and is now writing a second full
copy to `filesDir`), the slot is left as a truncated ZIP.

`autoBackupSavedAtMs()` (line 462) reports the file's `lastModified()` — which the failed write
just refreshed — so Settings shows a recent "last backed up" date for an unrestorable file. The
`AutoBackupWorker` catch only writes the failure marker after `runAttemptCount >= 3`
(`AutoBackupWorker.kt:44-47`), and any later successful run deletes the marker; meanwhile the
damage to the slot is already done.

`restoreFromAutoBackup()` will then hit `isZip` → true, `nextEntry` → `ZipException` → `CORRUPT`
("That backup is corrupted or incomplete. Try a different copy or re-export it.") — but there is
no other copy.

**Scenario:** Phone at 98% storage. Weekly worker runs. `snapshotDatabase()` writes a 40 MB VACUUM
copy into cache, then `writeBackupZip` fills the disk 60% of the way through the ZIP. The user's
one-and-only automatic backup — which held six months of training — is now a 24 MB fragment. The
UI says "Last backup: today".

**Fix:** Same temp-then-rename pattern: write `forge_auto_backup.zip.tmp`, close it, verify size,
then `renameTo` the live slot. Do not touch the failure marker unless the rename succeeded.

---

## [HIGH] One unescaped double-quote in a CSV cell silently discards the rest of the file

**File:** `app/src/main/java/com/forge/app/data/importer/CsvParser.kt:49-63` (specifically `:55`)

**What:** The parser enters quote mode on *any* `"` seen outside quotes, not just at the start of
a field:
```kotlin
c == '"' -> inQuotes = true    // line 55 — fires mid-field
```
Once set, `inQuotes` only clears at the next `"` (line 52). With an odd number of quotes in the
file, everything from that quote to EOF is accumulated into a single cell, and every `,` and `\n`
after it stops being a delimiter. `parseWith` then flushes one giant row at line 67-70 and the
importer's `for (row in rows.drop(1))` never sees the remaining rows.

The class KDoc claims "a malformed line yields the cells it can rather than throwing, since a
single bad row in a multi-year export shouldn't sink the whole import" — but it sinks everything
after the bad row.

**Scenario:** FitNotes/Strong CSV where one comment cell holds an inches mark. Input:
```
Date,Exercise,Category,Weight,Reps,Comment
2024-01-02,Bench Press,Chest,225,5,paused 2" off chest
2024-01-04,Squat,Legs,315,5,
2024-01-06,Deadlift,Back,405,3,
... 4 more years of rows ...
```
The `"` after `2` turns quote mode on and it is never turned off. Result: **2 rows parsed**, the
Jan-04 and Jan-06 sessions and every row after them are dropped. The user is told
`"Imported 1 workout from FitNotes · 1 sets."` and has no way to know 4 years went missing.
(Because the sniffers also read the raw text, `canParse` still succeeds, so the file is accepted.)

**Fix:** Only enter quote mode when the current cell is empty *and* the previous character was a
delimiter/row start (`if (cell.isEmpty()) inQuotes = true else cell.append('"')`). Optionally
report the unbalanced-quote condition as `skippedRows` so the summary is honest.

---

## [HIGH] Fuzzy exercise matcher permanently files history under the wrong equipment and, in one case, the wrong muscle group

**File:** `app/src/main/java/com/forge/app/data/importer/ExerciseNameMatcher.kt:83-93`, `:112`,
`:42-59`; library at `app/src/main/java/com/forge/app/program/ExerciseLibrary.kt`

**What:** `JACCARD_FLOOR = 0.66` means a 2-token user name matches any 3-token library name that
shares both tokens (2/3 = 0.667 ≥ 0.66) — which is exactly the shape "\<equipment\> + \<user's
name\>". Ties are broken by `if (score > bestScore)` (strict `>`), so the **first** library entry
in declaration order wins, and the dumbbell variants are declared first (`ExerciseLibrary.kt:83`
`db-bench-press` precedes `barbell-bench-press`). I reproduced the matcher against the real
115-entry library; confirmed wrong results:

| Source name (as exported by Strong/Hevy/FitNotes) | Matched to | Should be |
|---|---|---|
| `Bench Press` | `db-bench-press` "DB Bench Press" (0.667) | `barbell-bench-press` |
| `Incline Bench Press` | `incline-db-bench-press` (0.750) | `incline-barbell-bench` (exists! but its name lacks "press", scoring 0.5) |
| `Reverse Fly (Dumbbell)` | `db-fly` "DB Fly" — **CHEST** (0.667) | `bw-prone-reverse-fly` — rear delt (0.5) |
| `Good Morning` | `bw-good-morning` "Bodyweight Good Morning" (0.667) | barbell movement |
| `Bicep Curl` | `mwm-seated-bicep-curl` "Seated Bicep Curl" — machine (0.667) | `db-curl` (0.333) |
| `Hip Thrust` | `barbell-hip-thrust` (0.667) | equipment asserted, not stated |
| `Step Up` / `Glute Bridge` / `Lateral Raise` | `db-*` variants (0.667) | equipment asserted |

Two curated entries (`:42-59`) are wrong outright:
- `"crunch" -> "cable-crunch"` — a plain bodyweight crunch, the single most common ab entry in
  every gym app, is filed as a **weighted cable** movement.
- `"shoulder press" -> "barbell-overhead-press"` — FitNotes' generic "Shoulder Press" (usually
  dumbbell) is filed as barbell. (Note the ironic asymmetry: `"Shoulder Press (Dumbbell)"` scores
  0.5 and is correctly left unmatched, while the *less* specific bare name is force-matched.)

**Scenario:** A FitNotes user with 3 years of `Bench Press` at 185-245 lb imports their export.
Every one of those sets is written with `exerciseId = "db-bench-press"` and `swappedName = null`
(`WorkoutImportRepository.kt:172-174`), i.e. indistinguishable from a real dumbbell press. Their
existing 70 lb-per-hand DB bench history is now merged with 245 lb barbell sets in the same
exercise: the Hall of Fame PR for "DB Bench Press" reads 245 lb, the strength curve is a cliff,
e1RM projections and the coach's progression suggestions are computed off a 3.5x-inflated load —
and because `swappedName` is null there is no record anywhere of what the row originally said.
The `Reverse Fly (Dumbbell)` → `DB Fly` case additionally moves rear-delt volume into the CHEST
muscle group, skewing the volume-balance charts the app is built around.

**Fix:** (a) Raise the floor above 0.667 (0.72 leaves the genuine 3-of-4 matches at 0.75 intact
while rejecting all the 2-of-3 equipment-inference cases). (b) Add an equipment-conflict guard:
if the library name contains an equipment token (`barbell`/`dumbbell`/`cable`/`machine`/
`bodyweight`/`smith`) that the source name does not, refuse the fuzzy match. (c) Break ties
deterministically toward the *barbell* variant rather than declaration order, and make ties
(`score == bestScore` across >1 id) a non-match. (d) Drop or re-target `crunch` and
`shoulder press` in `CURATED`. (e) Always set `swappedName = ex.name` on import so the original
label survives even for matched rows, making a bad match recoverable.

---

## [HIGH] `parseWeight` reads a US thousands separator as a decimal point — a 1,250 lb set becomes 1.25 lb

**File:** `app/src/main/java/com/forge/app/data/importer/GymImporter.kt:80-91`

**What:**
```kotlin
val normalised = when {
    s.contains(',') && s.contains('.') -> s.replace(",", "")   // "1,234.5" → thousands
    s.contains(',') -> s.replace(',', '.')                     // "100,5"   → decimal
    else -> s
}
```
The second branch assumes any lone comma is a European decimal separator. A US-locale value with
a thousands separator and no fractional part hits it. Verified:

| input | result |
|---|---|
| `"82,5"` | 82.5 ✓ (intended) |
| `"1,234.5"` | 1234.5 ✓ |
| `"1,250"` | **1.25** ✗ |
| `"12,345"` | **12.345** ✗ |

**Scenario:** A user imports a spreadsheet/Jefit CSV whose Weight column is comma-formatted:
`2024-03-11,Leg Press,"1,250",8`. `CsvParser` correctly yields the cell `1,250`; `parseWeight`
returns 1.25; `roundWeight` → 1.3 lb; the set is stored as `weightLb = 1.3`, `weightText = "1.3"`,
and the session's denormalised `totalVolumeLb` (`WorkoutImportRepository.kt:137-139`) is computed
from that. A 10,000 lb leg-press session lands in history as a 10 lb one, dragging the volume
chart and every weekly-total aggregate down permanently. The same conversion silently damages any
"Volume"-style column in a generic CSV.

**Fix:** Only treat a lone comma as a decimal point when it is followed by 1-2 digits **and** is
the last separator (`Regex("^\\d+,\\d{1,2}$")`); a comma followed by exactly 3 digits, or more than
one comma, is a thousands separator. Better: decide once per *file* by sampling the weight column,
rather than per cell.

---

## [HIGH] `...T...Z` timestamps are parsed as local time — imported workouts land on the wrong day

**File:** `app/src/main/java/com/forge/app/data/importer/GymImporter.kt:35-40`, `:59-73`

**What:** `DATE_TIME_FORMATS` includes `"yyyy-MM-dd'T'HH:mm:ss'Z'"` (line 36), where the `'Z'` is
a **literal** to be consumed and discarded, not a zone. The result is parsed as a `LocalDateTime`
and then stamped in the device zone:
```kotlin
return LocalDateTime.parse(s, fmt).atZone(zone).toInstant().toEpochMilli()   // :64
```
`zone` is `ZoneId.systemDefault()` (line 56). So a UTC instant is reinterpreted as a wall-clock
time in the user's zone, shifting the session by the full UTC offset.

**Scenario:** A Sydney user (UTC+11 in November) imports an export containing
`start_time,2024-11-05T18:30:00Z`. That instant is 2024-11-**06** 05:30 local — a Wednesday
morning session. The importer stores `startedAt` = 2024-11-05 18:30 *Sydney* = 07:30 UTC, i.e.
Tuesday evening, **11 hours early and on the previous calendar day**. Every downstream surface
keyed on the local date — the weekly volume chart, the training-days streak, "rest day" detection,
the calendar heatmap — attributes the workout to the wrong day, and a Sunday-evening UTC session
imports as Sunday for a UTC+11 user but as *Saturday* for a UTC-8 user re-importing the same file.
Round-tripping through Avex's own weekly export (`BackupRepository.kt:89`, `ForgeJsonImporter.kt:79`)
has the mirror problem: the date is written in local time and re-read at local midnight, so the
time-of-day is destroyed entirely.

**Fix:** Handle zone-bearing formats with `OffsetDateTime`/`Instant.parse` before falling through
to the `LocalDateTime` list; only apply `systemDefault()` to genuinely zone-less strings. Record
the source's offset in `ImportedSession` so a re-import on a different device is stable.

---

## [HIGH] Duplicate guard silently drops a *different* workout that happens to share a start instant

**File:** `app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:118-134`;
`data/db/dao/SessionDao.kt:118-119`

**What:** Date-only sources stamp midnight, so the code nudges collisions forward by `nth * 1000L`
and then de-dupes on the exact instant:
```kotlin
val nth = startNonce.getOrDefault(session.startedAtMs, 0)
startNonce[session.startedAtMs] = nth + 1
val startedAt = session.startedAtMs + nth * 1000L
if (sessionDao.countAtStart(startedAt) > 0) { duplicates++; continue }
```
`startNonce` is **per-import-run**, but `countAtStart` queries the **whole table**. Any session
already at that instant — from a different file, a different app, or an earlier partial import —
matches, and the incoming workout is discarded with the message "1 already in your log was
skipped."

**Scenario:** A migrating user imports `fitnotes_export.csv` first. FitNotes has no workout
grouping (`FitNotesImporter.kt:46`), so 2024-01-15 becomes one session at local midnight,
`startedAt = T`. Later they import `strong_2023_2024.csv`, whose Date column in their edited copy
is date-only. Its 2024-01-15 "Push" workout gets `nth = 0` → `startedAt = T` → `countAtStart(T) > 0`
→ **silently dropped**. The two files describe entirely different workouts (they used FitNotes in
the morning and Strong in the evening), but only one survives, and the UI reports it as an
already-present duplicate. The same thing happens on any second import of a date-only file when a
same-day session already exists: the nonce restarts at 0 for the new run, so workout #2 of a
2-workout day collides with workout #1 already in the DB.

**Fix:** De-dupe on a content fingerprint (source + start day + exercise/set signature), not on the
raw start instant. Seed `startNonce` from `sessionDao.countAtStart(base)` so the nudge continues
past what is already stored, and never count a match as a duplicate unless the set contents also
agree.

---

## [HIGH] Round-tripping through the JSON export turns timed holds into rep sets and assisted sets into genuine PRs

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:200-210` (export);
`app/src/main/java/com/forge/app/data/importer/ForgeJsonImporter.kt:46-52`,
`app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:180-192` (import);
entity at `data/db/entities/LoggedSet.kt:93-125`

**What:** `LoggedSet` has 13 columns. `exportFullDataJson` writes six of them
(`weightText`, `weightLb`, `reps`, `rpe`, `completedAt`, `difficultyTag`) and omits
**`durationSeconds`, `isAssisted`, `isAmrap`, `toFailure`, `setType`, `dropAnnotation`**.
`ForgeJsonImporter` then reads only three (`weightLb`, `reps`, `rpe`) and
`WorkoutImportRepository` writes the rest at their entity defaults (`durationSeconds = null`,
`isAssisted = false`, `toFailure = false`, `setType = null`).

Both omissions have semantic consequences that the entity doc spells out:
- `durationSeconds` — "When set, `reps` is not a meaningful count and this set is **excluded from
  every weight×reps aggregate (volume, e1RM, PR)** so it can't pollute strength stats"
  (`LoggedSet.kt:119-124`).
- `isAssisted` — "bands / spotter — **excluded from all-time PR comparison**" (`LoggedSet.kt:105`).

The `ForgeJsonImporter` KDoc explicitly offers this path as a migration route: "it lets a user move
history between two installs via the plain export".

**Scenario:** User moves to a new phone via `forge_export.json`. A 90-second plank was stored as
`durationSeconds = 90, reps = 90, weightLb = 45` (weighted plank). Export drops `durationSeconds`;
import restores it as `null`. The plank is now a **90-rep, 45 lb set**: it contributes
90 × 45 = 4050 lb to session volume (`WorkoutImportRepository.kt:137-139`), and its Epley e1RM
computes as a world record. It becomes the top entry in the Hall of Fame and anchors every
subsequent progression suggestion for that movement. Simultaneously, every band-assisted pull-up
(`isAssisted = true`) comes back as `false` and is now eligible for PR comparison.

**Fix:** Export every `LoggedSet` / `LoggedExercise` / `Session` column (or generate the JSON from
the entity via reflection/serialization so a new column can't be forgotten), and read them back in
`ForgeJsonImporter` → `ImportedSet`. At minimum add `durationSeconds` and `isAssisted` to both
sides, and skip timed-hold sets from the volume sum on import.

---

## [HIGH] Restore validation never checks database integrity — a bit-rotted backup is accepted and then deleted by Room

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:719-724`, `:797-806`

**What:** Validation is:
```kotlin
dbFile.rawQuery("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN " +
    "('session','logged_exercise','logged_set')", null).use { c -> c.moveToFirst() && c.getInt(0) >= 3 }
```
This reads page 1 (the schema) and nothing else. A backup whose *data* pages were damaged in
transit — emailed, synced through a flaky cloud provider, copied off a failing SD card — passes,
because `sqlite_master` lives at the front of the file. There is no `PRAGMA integrity_check` /
`quick_check`, and no checksum or size record in the ZIP.

Once staged and swapped in at boot, Room opens the file. The corruption surfaces on the first read
that touches a bad page, and `SupportSQLiteOpenHelper.Callback.onCorruption` (Room's default) **deletes
the database file**. The original `forge.db` was already replaced by `swapStagedFile`'s atomic
rename (`ForgeApp.kt:141-153`), so there is nothing left.

**Scenario:** User's `forge_auto_backup.zip` is copied off a dying SD card; a 4 KB page inside
`database.db` is zeroed. `isZip` ✓, `sawDb` ✓, `isForgeDatabase` ✓ (schema page intact),
`databaseUserVersion` ✓ (header intact), `incomingVersion == currentVersion` ✓ → SUCCESS, app
restarts, DB swapped. Room opens it, hits the zeroed page while loading the session list,
`onCorruption` deletes `forge.db`, Room recreates an empty schema. The user now has **zero
sessions**, and the pre-restore database is gone.

**Fix:** Run `PRAGMA quick_check(1)` (or `integrity_check`) inside `isForgeDatabase` and return
`CORRUPT` on anything other than `ok`. Additionally, store a SHA-256 of `database.db` as a ZIP
comment or a `manifest.json` entry at backup time and verify it before staging.

---

## [HIGH] Cardio / distance-only rows create phantom 0 lb × 0 rep sets and phantom sessions

**File:** `app/src/main/java/com/forge/app/data/importer/StrongImporter.kt:41`,
`HevyImporter.kt:33`, `FitNotesImporter.kt:40`, `GenericCsvImporter.kt:31`;
`GymImporter.kt:76`

**What:** The guard intended to skip non-resistance rows is:
```kotlin
if (reps == null && (weightRaw == null || weightRaw == 0.0)) continue
```
but `parseReps` returns `0` — not `null` — for the string `"0"`:
```kotlin
fun parseReps(raw: String): Int? = raw.trim().toDoubleOrNull()?.toInt()?.takeIf { it >= 0 }
```
`"0".toDouble().toInt() = 0`, and `0 >= 0`, so `reps = 0`, `reps == null` is false, and the guard
never fires. The row is added as `ImportedSet(weightLb = null, reps = 0)`.

**Scenario:** Strong and Hevy write `0` (not blank) into Weight and Reps for cardio/distance
exercises. A Strong row:
```
2024-06-02 07:15:00,Morning Cardio,45m,Running,1,0,kg,0,,5,km,1800,,
```
produces a session titled "Morning Cardio" containing an exercise "Running" with one set of
`weightText = "BW"`, `reps = 0`, `weightLb = null`. `ExerciseNameMatcher.match("Running")` returns
null, so it is stored under `exerciseId = "ext-running"` with `swappedName = "Running"`. A year of
running logs becomes ~150 junk sessions and ~150 junk zero-rep "exercises" in the lifting history,
each inflating `setCount` and the session count that drives streaks, trophies and the
"workouts this week" figure — while contributing zero volume, so the average-volume-per-session
statistic collapses. There is no bulk-delete for imported sessions.

**Fix:** Treat `reps <= 0` as absent for the purpose of this guard:
`val hasReps = reps != null && reps > 0`, then `if (!hasReps && (weightRaw == null || weightRaw <= 0.0)) continue`.
Count the skipped rows into `ImportResult.Success.skippedRows` (currently hard-coded to 0 at
`WorkoutImportRepository.kt:210`) so the user is told.

---

## [HIGH] Forge-JSON import builds the entire 25 MB document as a JSONObject two or three times

**File:** `app/src/main/java/com/forge/app/data/importer/ForgeJsonImporter.kt:22`, `:26`;
`WorkoutImportRepository.kt:62`, `:89-90`, `:241-255`

**What:** `readBounded` allows up to `MAX_IMPORT_BYTES = 25 * 1024 * 1024` and materialises the
whole file as a Java `String` (UTF-16 → ~50 MB for a 25 MB file, plus the 25 MB
`ByteArrayOutputStream` and its `toByteArray()` copy — ~100 MB peak before parsing starts).
`canParse` then does `JSONObject(text)` (line 22) purely to test for a `"sessions"` key and
throws the tree away; `parse` builds a *second* full tree (line 26). `scanFolder` calls
`canParse` + `parse` for every candidate (line 89-90) and then `import` re-reads and re-parses
the file the user taps.

`org.json.JSONObject` allocates a `HashMap` + boxed value per field; a pretty-printed
(`toString(2)`) Avex export of a multi-year history is dominated by whitespace and short keys, so
the in-memory tree runs 5-10x the text size.

**Scenario:** Power user with 5 years / ~600 sessions / ~25,000 sets. `exportFullDataJson`
(`BackupRepository.kt:260`) writes ~12 MB of pretty-printed JSON. They share it to the new phone
and tap import. `readBounded` holds ~24 MB of `String`; `canParse` builds a ~100 MB JSONObject
tree; on a 192 MB-heap device this is already tight, and `parse` immediately builds another.
`OutOfMemoryError` is **not** an `Exception` subclass caught by
`runCatching { importer.parse(...) }` in the sense that matters — `runCatching` catches
`Throwable`, so the import silently returns `NothingToImport`: "No new workouts found in that
file." The user concludes the export is empty and abandons the migration.

`exportFullDataJson` has the mirror problem: `root.toString(2)` (line 260) builds the whole
document as one String on top of the already-complete JSONObject tree.

**Fix:** Cheap sniff in `canParse` (regex/`contains("\"sessions\"")` on the first few KB) instead
of a full parse; parse once and pass the tree; stream with `JsonReader` for large files; drop
`MAX_IMPORT_BYTES` for JSON to something a JSONObject tree can survive, or stream-write the export
with `JsonWriter` and stop pretty-printing it.

---

## [MEDIUM] Import clamps every session's duration to 60 s – 6 h, rewriting real finish times

**File:** `app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:140-144`

**What:**
```kotlin
val activeSec = (session.finishedAtMs?.let { ((it - startedAt) / 1000L).toInt() }
    ?: (totalSets * SECONDS_PER_SET)).coerceIn(60, 6 * 3600)
val finishedAt = startedAt + activeSec * 1000L
```
The source's real `finishedAt` is discarded and recomputed from a clamped wall-clock delta. The
app explicitly supports sessions spanning days (`BackupRepository.kt:339-340`: "a 'resume later'
session spanning days must report real training time") and stores `activeSeconds` separately
precisely so away-time isn't counted — but the importer reintroduces the wall-clock conflation and
then clamps it.

**Scenario:** Round-tripping through `forge_export.json`: a session started Friday 18:00 and
finished Sunday 10:00 (resumed twice), with `activeSeconds = 4200` (70 real minutes). The export
writes both `finishedAt` and `activeSeconds` (`BackupRepository.kt:178-179`); the importer reads
only `finishedAt` (`ForgeJsonImporter.kt:32`), computes 144,000 s, clamps to 21,600, and writes
`activeSeconds = 21600, finishedAt = startedAt + 6h`. The session now claims **6 hours of
training** instead of 70 minutes, corrupting the weekly-duration chart and the average-session-length
figure. Conversely a 45-second finisher session clamps *up* to 60 s.

**Fix:** Read `activeSeconds` from the Forge JSON when present and pass it through
`ImportedSession`; keep the source `finishedAtMs` verbatim and only synthesise one when the source
has none. Clamp only the synthesised value.

---

## [MEDIUM] The JSON export→import round trip drops most of what it exports; cardio, goals and bodyweight are never read back at all

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:171-256` vs
`app/src/main/java/com/forge/app/data/importer/ForgeJsonImporter.kt:25-72`

**What:** Field-by-field trace of `exportFullDataJson` → `ForgeJsonImporter` → `WorkoutImportRepository`:

*Session:* `startedAt` ✓ · `finishedAt` ✓ (then clamped, above) · `journal` ✓ ·
`dayKey` ✗ (forced to `Program.FREESTYLE_DAY_KEY`, `WorkoutImportRepository.kt:148`) ·
`activeSeconds` ✗ · `totalVolumeLb` ✗ (recomputed) · `prCount` ✗ (forced 0) ·
`setCount` ✗ (recomputed) · `sessionType` ✗ (forced "normal") · `intensity` ✗ (default "normal") ·
`isUntracked` ✗ (default false) · `tags` ✗ · `mood` ✗ · `segments` ✗ · `id` ✗.

*LoggedExercise:* `exerciseId` ✓ (only when `ExerciseLibrary.byId` resolves) · `swappedName` ✓ ·
`note` ✓ · `orderIndex` ✗ (array position used) · `difficulty` ✗ · `skipped` ✗ ·
`supersetGroup`/`slotId`/`hitFullTarget`/`wasPr` ✗ (never exported).

*LoggedSet:* `weightLb` ✓ · `reps` ✓ · `rpe` ✓ · `weightText` ✗ (regenerated) ·
`completedAt` ✗ (forced to the session's `finishedAt`) · `difficultyTag` ✗ ·
`durationSeconds`/`isAssisted`/`isAmrap`/`toFailure`/`setType`/`dropAnnotation` ✗ (never exported).

*Top-level:* the `cardio` array (`:220-236`) and `coachGoals` array (`:241-255`) are written and
**never read by any importer**. `exportBodyweightCsv` (`:371-379`) has no importer either.
`exportVersion` (`:156`) is written and never checked.

**Scenario:** User migrates to a new phone via the JSON export as the KDoc invites. Every session
lands as a freestyle workout with no program day, no tags, no mood, no session type, PR count 0,
per-set difficulty tags gone, and all 400 cardio entries and every coach goal simply absent — with
no warning; the summary reads "Imported 612 workouts from Avex export · 24,918 sets."

**Fix:** Either (a) mark the JSON export as strictly human/AI-readable in the UI and route
device-to-device migration exclusively through the ZIP backup, removing `ForgeJsonImporter`'s
migration claim, or (b) extend `ImportedSession`/`ImportedExercise`/`ImportedSet` with the missing
fields, read them, and add cardio/goal/bodyweight import. Read `exportVersion` and refuse an
unknown one instead of silently mis-parsing it.

---

## [MEDIUM] Weekly and full exports of the same session produce different start instants, so importing both double-counts everything

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:89` (weekly `date`) vs
`:177` (full `startedAt`); `ForgeJsonImporter.kt:75-81`

**What:** `sessionStartMillis` prefers `startedAt` (an exact epoch) and falls back to `date`
(a `yyyy-MM-dd` string re-read at local midnight). The weekly export writes only `date`; the full
export writes only `startedAt`. Both files pass `canParse` (both have a `sessions` key), and both
are `.json` files that `scanFolder` will list side by side for one-tap import
(`WorkoutImportRepository.kt:78-95`, `IMPORTABLE_EXTENSIONS` includes `.json`).

**Scenario:** User exports both `forge_export.json` and `forge_weekly_export.json` into Downloads
to share with a coach, then later grants Avex the Downloads folder for the import scan. Both files
appear in the "found imports" list. They tap both. The full export imports session S at
`startedAt = 1730829600000`; the weekly export imports the *same* session at local midnight of the
same date — a different instant — so `countAtStart` does not match and **S is inserted a second
time**. Every session from the last 7 days is now duplicated, doubling that week's volume, set
count and training-day count.

**Fix:** Give the weekly export the real `startedAt` too (it costs nothing and it is what the
importer prefers), and de-dupe on content rather than instant (see the duplicate-guard finding).

---

## [MEDIUM] `applyPendingRestore` swaps DB, prefs, photos and avatar independently — a partial failure leaves the user's live data paired with the backup's settings

**File:** `app/src/main/java/com/forge/app/ForgeApp.kt:67-124`

**What:** The KDoc on `restoreFromIncoming` (`BackupRepository.kt:665-668`) promises the four
components are swapped "in atomically at next boot — DB, prefs and photos together, so a kill or
copy failure can never leave the live DB and photo folder from different backups." The
implementation is four independent `swapStagedFile` calls, each with its own success flag; a
failing one is merely recorded in `anyFailed` and **kept for a retry on the next boot** while the
others have already been applied.

The prefs blob is also never validated: `restoreFromIncoming` extracts whatever is under
`settings.preferences_pb` (`:688-693`) with no format check, unlike the DB which gets `isSqlite` +
`isForgeDatabase` + version gating.

**Scenario A (order-dependent half-restore):** The DB swap fails (`deleteOrThrow` on a locked
`forge.db-wal`, which a still-running `WorkoutSessionService` can hold) but the prefs swap
succeeds. `applied = false`, so no "restored" confirmation is shown — yet the live preferences,
including the whole program-generation config, have been **irreversibly replaced** by the backup's
(there is no copy of the pre-restore prefs anywhere). The user's current program now describes days
that none of their live sessions' `dayKey`s match. If the DB swap keeps failing, this is permanent.

**Scenario B (corrupt prefs):** A ZIP whose `settings.preferences_pb` entry was damaged in transit
is staged unvalidated and swapped over the live prefs. DataStore throws `CorruptionException` on
first read — after the DB has already been replaced, so both the settings and the recovery path
are gone in the same boot.

**Fix:** Stage into a single `pending_restore/` directory and apply it as one unit: swap all four,
and on any failure restore the aside-moved originals (the photo block at `:97-116` already does
this correctly — apply the same move-aside/roll-back to the DB and prefs). Validate the prefs blob
by parsing it with `PreferencesSerializer` before staging.

---

## [MEDIUM] Generic CSV import parses the whole file twice; folder scan parses up to 60 files twice each

**File:** `app/src/main/java/com/forge/app/data/importer/GenericCsvImporter.kt:14`, `:19`;
`WorkoutImportRepository.kt:62`, `:87-90`

**What:** `canParse` is `resolve(CsvParser.parse(text)) != null` — a full parse into
`List<List<String>>` whose only purpose is to look at the header row, discarded immediately.
`parse` (line 19) then parses again. The in-file comment at `:17-18` notes an earlier fix that
removed *one* of three passes, but the `canParse` pass remains.

`scanFolder` runs the whole detector chain plus a full `parse` over every `.csv/.json/.txt` in the
granted folder (`MAX_SCAN_FILES = 60`), each up to 25 MB, on every visit to the Import screen and
again after every successful import (`SettingsViewModel.importData` calls `scanImportFolder()`).

**Scenario:** User grants Downloads, which holds 60 CSVs including a 20 MB bank statement and
several large exports. Each Import-screen visit reads 60 files into memory and fully CSV-parses
each unrecognised one twice. A 20 MB CSV parses to roughly 1.5 M `String` cells (~150 MB of heap)
— twice — before being rejected. The screen janks for many seconds and can OOM; the
`runCatching { ... }.getOrDefault(emptyList())` at `SettingsViewModel.kt:616` turns the OOM into a
silently empty "found imports" list.

**Fix:** Make `canParse` header-only (parse just the first line). Cap `scanFolder` by file size
(e.g. skip > 2 MB during the sniff), cache results keyed by uri + `lastModified`, and don't
re-scan after every import.

---

## [MEDIUM] Import assumes UTF-8; a UTF-16 export imports as garbage or not at all

**File:** `app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:253`

**What:** `Read.Ok(String(out.toByteArray(), Charsets.UTF_8))` — the charset is hard-coded and no
BOM sniff happens at the byte level. `CsvParser.parse` strips a UTF-8 BOM (`U+FEFF`, verified as
`EF BB BF` in the source) but that is after decoding, so it cannot help a UTF-16 file. Excel on
Windows offers "Unicode Text (*.txt)" which is UTF-16LE, and several gym apps' Windows companion
tools emit UTF-16.

**Scenario:** User opens their Strong CSV in Excel to tidy it and saves as "Unicode Text". The
bytes are `FF FE 44 00 61 00 74 00 65 00 ...`. Decoded as UTF-8 this becomes replacement
characters interleaved with NULs. `firstLine(text)` contains no `"workout name"`, every `canParse`
fails, and the user gets "That file isn't a recognised gym-app export" for a file that visibly
contains their data.

**Fix:** Sniff the first bytes for `FF FE` / `FE FF` / `EF BB BF` and pick UTF-16LE / UTF-16BE /
UTF-8 accordingly; fall back to UTF-8 with `CodingErrorAction.REPLACE`.

---

## [MEDIUM] Nothing cleans up abandoned snapshot / restore temp files

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:514`, `:626`, `:650`,
`:679`, `:689`, `:698-700`, `:705`

**What:** Every temp is named with `System.currentTimeMillis()`
(`forge_snapshot_<ms>.db`, `forge_restore_in_<ms>`, `forge_restore_db_<ms>.db`,
`forge_restore_prefs_<ms>.pb`, `forge_restore_photos_<ms>/`, `forge_restore_avatar_<ms>.jpg`) and
is deleted only by the `finally` blocks of the call that created it. If the process dies mid-backup
or mid-restore — which is exactly what happens when the failure was ENOSPC or an OEM kill — the
temps survive with unique names, so they accumulate rather than being reused. There is no
startup sweep.

**Scenario:** Auto-backup fails three weeks running on a full device. `cacheDir` accumulates three
`forge_snapshot_*.db` files, each a full copy of the database (40 MB each = 120 MB), making the
storage pressure that caused the failure worse and further reducing the chance the next backup
succeeds. Android will reclaim `cacheDir` only under system-wide pressure.

**Fix:** Sweep `cacheDir` for `forge_snapshot_*` / `forge_restore_*` at the start of
`snapshotDatabase()` and `restoreFromIncoming()`, or use fixed names so each run reuses one slot.

---

## [MEDIUM] ZIP extraction during restore is unbounded

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:682-714`

**What:** `copyAtMost` caps the *incoming* file at `MAX_RESTORE_BYTES` (2 GiB), but the entry
extraction loop has no cap at all: `exDb.outputStream().use { zin.copyTo(it) }` (line 687) and the
photo/prefs/avatar copies write until the entry ends. A ZIP's compression ratio for zero-filled or
highly repetitive data is > 1000:1.

Separately, a 2 GiB write into `cacheDir` is itself hostile on a budget device — `copyAtMost`
writes the full 2 GiB *before* returning false.

**Scenario:** User is sent a "backup" by someone in a forum thread, or a genuine backup gets
mangled. A 4 MB ZIP whose `database.db` entry decompresses to 40 GB fills the device's internal
storage during restore. The `finally` at `:785` deletes the temp afterwards, but the device has
already hit "Storage full" and may have killed other apps' data in the meantime.

**Fix:** Wrap each `zin` copy in the same `copyAtMost` bound (a real Avex DB is bounded by
`dbSizeBytes()` — a few hundred MB at the absolute outside), reject the archive when any entry
exceeds it, and lower `MAX_RESTORE_BYTES` to something proportionate (e.g. 512 MB) or stream the
size check before writing.

---

## [LOW] Weekly export writes the raw exercise id as the human `name`, so seed-split history round-trips as "Ua1"

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:104`;
`ForgeJsonImporter.kt:84-89`; `Program.kt:341-345`

**What:** `put("name", ex.swappedName ?: ex.exerciseId)` — unlike `exportSessionJson`
(`:297`), which correctly uses `Program.exerciseDisplayName(...)`. For any non-swapped exercise the
`name` field is a raw id. `ForgeJsonImporter.exerciseName` prefers `name` when non-blank
(`:85`), so the id text is what flows through when `exerciseId` doesn't resolve in
`ExerciseLibrary`.

**Scenario:** A long-time user's early history uses the seed-split ids (`ua1`..`lb6`), which are
deliberately *not* in `ExerciseLibrary` (they resolve only on the display path, `Program.kt:336`).
Weekly export writes `name: "ua1"`, `exerciseId: "ua1"`. On re-import `ExerciseLibrary.byId("ua1")`
is null → no `catalogueId`; `ExerciseNameMatcher.match("ua1")` is null; `syntheticId("ua1")` →
`"ext-ua1"` with `swappedName = "ua1"`. Their early bench-press history now displays as **"Ua1"**
(via `humanizeExerciseId`) and is de-linked from Barbell Bench Press stats. It also degrades the
export's stated purpose (AI analysis) — the model sees `barbell-bench-press` instead of
`Barbell Bench Press`.

**Fix:** Use `Program.exerciseDisplayName(ex.exerciseId, ex.swappedName)` in `exportWeeklyJson`,
matching `exportSessionJson`.

---

## [LOW] The per-session export cannot be re-imported by the app that wrote it

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:276` (key `"session"`);
`ForgeJsonImporter.kt:22` (requires `"sessions"`)

**What:** `exportSessionJson` nests everything under a singular `"session"` object.
`ForgeJsonImporter.canParse` requires `JSONObject(text).has("sessions")`, and no other importer
recognises JSON. `forge_session_<id>.json` therefore always yields `UnrecognisedFormat`:
"That file isn't a recognised gym-app export."

**Scenario:** User taps "save this workout's data" on a session, shares the file to themselves,
and later shares it back into Avex to restore that one workout. Rejected.

**Fix:** In `ForgeJsonImporter.canParse`/`parse`, accept a root `"session"` object by wrapping it
in a one-element list.

---

## [LOW] CR-only line endings collapse the whole file into one row

**File:** `app/src/main/java/com/forge/app/data/importer/CsvParser.kt:57`

**What:** `c == '\r' -> { /* swallow; the paired \n ends the row */ }`. Classic-Mac CSVs (still
produced by some older spreadsheet exports) use `\r` alone as the terminator, so no row ever ends.
The whole file becomes one row; `rows.size < 2` → every importer returns `emptyList()`.

**Scenario:** A CR-terminated export yields "No new workouts found in that file." with no
indication that the line endings are the problem.

**Fix:** Treat `\r` as a row terminator, consuming a following `\n` if present.

---

## [LOW] Non-numeric weight text ("BW", "2 plates") does not survive the JSON round trip

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:203` (exports
`weightText`); `ForgeJsonImporter.kt:46-52` (ignores it);
`WorkoutImportRepository.kt:216-220` (regenerates it)

**What:** `LoggedSet.weightText` is documented as "what the user typed verbatim ('BW', '2 plates',
'45') and is what's shown back in the UI" (`LoggedSet.kt:74-77`). It is exported but never read
back; the importer regenerates it from `weightLb` as a bare lb number, or `"BW"` when the weight is
null/zero.

**Scenario:** A set logged as `"2 plates"` (parsed to 135 lb) round-trips as `weightText = "135"`.
Cosmetic but it is the field the UI shows, so the user's own notation is silently rewritten across
their entire history.

**Fix:** Carry `weightText` through `ImportedSet` when the source provides it.

---

## [LOW] Exported JSON mixes numeric and empty-string types for the same field

**File:** `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:226`, `:231-233`,
`:246`, `:249-250`; also `:91`, `:111`, `:113`, `:178`, `:204`, `:206`

**What:** Nullable numbers are written as either a number or `""`
(`put("elevationM", c.elevationM ?: "")`, `put("completedAt", g.completedAt ?: "")`), and nullable
doubles are written as the integer `0` (`put("weightLb", set.weightLb ?: 0)`), conflating
"bodyweight / not recorded" with "zero". Any consumer using `optDouble`/`optLong` on the
empty-string form gets the default rather than a signal that the value was absent.

**Fix:** Use `JSONObject.NULL` for absent values, and `0.0` (not `0`) for genuine zero doubles.

---

## [LOW] `isWarmup` is parsed and then discarded

**File:** `app/src/main/java/com/forge/app/data/importer/ImportModels.kt:22-23`;
`HevyImporter.kt:51`; `WorkoutImportRepository.kt:180-192`

**What:** `HevyImporter` sets `isWarmup` from Hevy's `set_type` column and `ImporterTest` asserts
it (`ImporterTest.kt:94`), but `WorkoutImportRepository` never maps it onto
`LoggedSet.setType = "warmup"`, which exists for exactly this. The KDoc acknowledges it
("kept so we could filter later; imported as normal today").

**Fix:** One-line: `setType = if (s.isWarmup) "warmup" else null` in the `LoggedSet` construction.

---

## Test-coverage gaps (what a regression suite must add)

`ImporterTest.kt` covers only happy paths. Nothing exercises:

1. **`BackupRepository` end to end** — no test at all. Needs: backup→restore round trip;
   `restoreFromIncoming` staging cleanup on IO failure (the CRITICAL above); each `RestoreOutcome`
   branch (`NOT_A_BACKUP`, `NEWER_VERSION`, `TOO_OLD`, `CORRUPT`, `TOO_LARGE`); a v12 backup
   restored onto v36 actually migrating; `writeZipToFolder` failure not destroying the prior file.
2. **`ForgeApp.applyPendingRestore`** — no test. Needs: partial-failure behaviour, the photo
   move-aside rollback, WAL/SHM sidecar deletion.
3. **`WorkoutImportRepository.insert`** — no test. Needs: the duplicate guard, `startNonce` across
   two separate import runs, FK remapping (sets → correct logged exercise → correct session),
   `totalVolumeLb` agreeing with the app's own `VolumeCalculator` (which is imported at
   `WorkoutImportRepository.kt:16` but never used).
4. **`ForgeJsonImporter`** — no test (the suite comment says it is "exercised in the app").
5. **CSV edge cases** — unbalanced quote, CRLF, CR-only, quoted newline, row shorter than the
   header, row longer than the header, trailing empty column, duplicate header names, UTF-16.
6. **Numeric/locale** — `"82,5"` (covered by no test), `"1,250"`, `"1,234.5"`, `"10.0"` reps,
   `"0"` reps + `"0"` weight, negative values.
7. **Timezone** — `...T...Z` parsing, and a date-only import producing the same instant on two
   devices in different zones.
8. **Round trip** — one property test: `sessions == import(export(sessions))` for every field the
   export claims to carry.

---

## Verification note (independently re-checked)

The #1 CRITICAL — the orphaned staged restore — was re-verified from primary sources and holds
exactly as reported.

**The staging is deliberate and well-reasoned.** `BackupRepository.kt:737-746` explains itself:

> "Don't close Room and swap the file here — that races with any flow still reading the DB until
> the process is killed. Stage the files instead; ForgeApp.applyPendingRestore swaps them in at
> next boot, before Room/DataStore open."

That design is sound, and the surrounding guards are genuinely careful: `NEWER_VERSION` /
`TOO_OLD` refusals with a comment about `fallbackToDestructiveMigrationFrom(1..11)` dropping all
tables, three-table validation so another app's SQLite file can't pass, and `deleteOrThrow` on
the WAL/`-shm` sidecars so SQLite can't replay stale frames over the restored file.

**The defect is one missing line on the error path.** Staging writes
`filesDir/pending_restore.db` and `filesDir/pending_restore_prefs.pb` at `:740-746`. If anything
after that throws `IOException` — the photo copy, an unreadable ZIP entry, a full disk — control
reaches the handler at `:783-785`, which returns `RestoreOutcome.IO_ERROR`. The `finally` at
`:786-789` cleans up only `temps` and `photoStage`:

```kotlin
} finally {
    temps.forEach { it.delete() }
    photoStage?.deleteRecursively()
}
```

`pendingDb` and `pendingPrefs` live in `filesDir`, not `temps`, and nothing anywhere deletes them
on the failure path.

`ForgeApp.applyPendingRestore()` then runs unconditionally at every cold start and asks only
whether the file exists — it has no way to know the restore that produced it failed:

```kotlin
if (!pending.exists() && !pendingPrefs.exists() && ...) return
```

**Concrete failing scenario:** user restores a backup from three weeks ago. The photo copy fails
on a full disk → `IO_ERROR` → the UI reports the restore failed. The user shrugs, keeps training,
logs eight sessions over the next fortnight. Then the OS evicts the process and they cold-start
the app. `applyPendingRestore` finds the orphaned `pending_restore.db`, swaps it over the live
DB, deletes the WAL sidecars — and all eight sessions are gone, with no backup of the newer state
and no prompt. The staged prefs blob lands too, so their program config reverts as well.

**Fix:** delete `pendingDb`/`pendingPrefs`/`pendingPhotos`/`pendingAvatar` in the `finally` on
every non-`SUCCESS` outcome, or stage under a temp name and rename to `pending_restore.*` only as
the last action of a fully successful restore. The rename-last variant is the safer shape — it
makes "the staged file exists" and "the restore succeeded" the same fact.
