package com.forge.app.data.importer

import android.content.Context
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.loggedExercise
import com.forge.app.data.db.loggedSet
import com.forge.app.data.db.session
import com.forge.app.data.prefs.SettingsRepository
import java.io.File
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The half of importing that touches the database.
 *
 * The PARSING half is well covered already — 27 tests across CSV quoting, delimiter detection,
 * BOMs, fuzzy headers, per-vendor grouping and timezone handling. What had no tests is what happens
 * to those parsed sessions on the way into the DB: the duplicate guard, the same-instant slot
 * search, and the promise in the class KDoc that an import MERGES and never replaces.
 *
 * That guard is easy to get wrong in both directions, and both directions are silent. Too loose and
 * scanning a Downloads folder twice doubles someone's entire training history. Too tight — which is
 * the bug the slot search exists to fix — and a genuinely new workout is dropped because a
 * date-only source stamped it at the same midnight as one already stored.
 */
@RunWith(RobolectricTestRunner::class)
class WorkoutImportRepositoryTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()

    private val repo = WorkoutImportRepository(
        context = context,
        db = db,
        sessionDao = db.sessionDao(),
        loggedExerciseDao = db.loggedExerciseDao(),
        loggedSetDao = db.loggedSetDao(),
        moodDao = db.moodDao(),
        cardioDao = db.cardioDao(),
        coachGoalDao = db.coachGoalDao(),
        bodyweightDao = db.bodyweightDao(),
        settingsRepo = SettingsRepository(context, Clock { 1_700_000_000_000L }),
        grants = com.forge.app.data.repo.PersistedTreeGrants(
            context, SettingsRepository(context, Clock { 1_700_000_000_000L })
        )
    )

    @After
    fun tearDown() = db.close()

    /**
     * A FitNotes export. Date-only, one set per row, no workout grouping — which is precisely the
     * shape that makes two distinct same-day workouts collide on one start instant.
     */
    private fun fitNotesFile(name: String, vararg rows: String): Uri {
        val file = temporaryFolder.newFile(name)
        file.writeText(
            buildString {
                appendLine("Date,Exercise,Category,Weight,Weight Unit,Reps,Distance,Distance Unit,Time,Comment")
                rows.forEach { appendLine(it) }
            }
        )
        return Uri.fromFile(file)
    }

    private fun row(date: String, exercise: String, weightLb: Int, reps: Int) =
        "$date,$exercise,Barbell,$weightLb,lbs,$reps,,,,"

    /** Strong groups by (Date, Workout Name), so one file can carry several workouts on one date. */
    private fun strongFile(name: String, vararg rows: String): Uri {
        val file = temporaryFolder.newFile(name)
        file.writeText(
            buildString {
                appendLine("Date,Workout Name,Duration,Exercise Name,Set Order,Weight,Weight Unit,Reps")
                rows.forEach { appendLine(it) }
            }
        )
        return Uri.fromFile(file)
    }

    private fun strongRow(date: String, workout: String, exercise: String, weightLb: Int, reps: Int) =
        "$date,$workout,60m,$exercise,1,$weightLb,lbs,$reps"

    private suspend fun storedSessionCount(): Int = db.sessionDao().allFinished().size

    private suspend fun storedSetCount(): Int =
        db.sessionDao().allFinished().sumOf { s ->
            db.loggedExerciseDao().forSession(s.id).sumOf { db.loggedSetDao().countForLoggedExercise(it.id) }
        }

    // ── The duplicate guard ─────────────────────────────────────────────────────────────────────

    @Test
    fun aFirstImportLandsItsWorkout() = runTest {
        val uri = fitNotesFile(
            "first.csv",
            row("2026-01-05", "Bench Press", 225, 3),
            row("2026-01-05", "Bench Press", 225, 3)
        )

        val result = repo.import(uri) as ImportResult.Success

        assertEquals(1, result.sessions)
        assertEquals(2, result.sets)
        assertEquals(0, result.duplicatesSkipped)
        assertEquals(1, storedSessionCount())
    }

    @Test
    fun importingTheSameFileTwiceAddsNothingTheSecondTime() = runTest {
        // The whole point of the guard: the Import screen lists files found in Downloads, and
        // re-running a scan is the most natural thing a user does. Without this, every re-import
        // doubles their history — with no visible signal until the totals look absurd.
        val uri = fitNotesFile(
            "same.csv",
            row("2026-01-05", "Bench Press", 225, 3),
            row("2026-01-05", "Bench Press", 225, 3)
        )
        repo.import(uri)
        val setsAfterFirst = storedSetCount()

        val second = repo.import(uri)

        // NothingToImport, not Success(sessions = 0): a wholly-duplicate file takes an early return.
        //
        // Worth knowing: the duplicate COUNT is tallied and then discarded on this path, so a user
        // re-importing their whole history is told "No new workouts found in that file" — the same
        // sentence an empty or broken export produces. The data handling is right; only the
        // sentence is ambiguous. Pinned as-is because it is current behaviour, not a defect this
        // suite should quietly change.
        assertTrue("got $second", second is ImportResult.NothingToImport)
        assertEquals("no session duplicated", 1, storedSessionCount())
        assertEquals("no sets duplicated", setsAfterFirst, storedSetCount())
    }

    @Test
    fun aFileMixingOldAndNewWorkoutsImportsOnlyTheNewOne() = runTest {
        // The partial case, and the one where the duplicate counter actually reaches the user: a
        // fresh export from the same app contains everything already imported plus whatever has
        // been logged since.
        repo.import(fitNotesFile("jan5.csv", row("2026-01-05", "Bench Press", 225, 3)))

        val second = repo.import(
            fitNotesFile(
                "jan5-and-jan6.csv",
                row("2026-01-05", "Bench Press", 225, 3),
                row("2026-01-06", "Back Squat", 315, 5)
            )
        ) as ImportResult.Success

        assertEquals("only the new day lands", 1, second.sessions)
        assertEquals("and the old one is reported as already present", 1, second.duplicatesSkipped)
        assertEquals(2, storedSessionCount())
    }

    @Test
    fun aDifferentWorkoutOnAnAlreadyOccupiedDayStillLands() = runTest {
        // The bug the slot search fixes. Both files stamp their workout at the same midnight
        // because neither source records a time. The second is DIFFERENT work, so it must take the
        // next slot rather than be waved off as a duplicate — it used to be dropped in silence.
        repo.import(
            fitNotesFile("morning.csv", row("2026-01-05", "Bench Press", 225, 3))
        )
        val evening = repo.import(
            fitNotesFile("evening.csv", row("2026-01-05", "Back Squat", 315, 5))
        ) as ImportResult.Success

        assertEquals("the evening workout is not a duplicate", 0, evening.duplicatesSkipped)
        assertEquals(1, evening.sessions)
        assertEquals("both same-day workouts are stored", 2, storedSessionCount())

        val startTimes = db.sessionDao().allFinished().map { it.startedAt }.sorted()
        assertEquals("they must not share an instant", 2, startTimes.toSet().size)
        assertTrue(
            "the nudge should be seconds, not a different day",
            startTimes[1] - startTimes[0] < 60_000L
        )
    }

    @Test
    fun theGuardComparesContentNotJustTheStartInstant() = runTest {
        // Same day, same exercise, different work. Matching on the start instant alone would call
        // this a duplicate; the guard also compares set count and volume.
        repo.import(fitNotesFile("light.csv", row("2026-01-05", "Bench Press", 135, 10)))
        val heavy = repo.import(
            fitNotesFile("heavy.csv", row("2026-01-05", "Bench Press", 315, 1))
        ) as ImportResult.Success

        assertEquals(0, heavy.duplicatesSkipped)
        assertEquals(2, storedSessionCount())
    }

    @Test
    fun twoDifferentLiftsWithIdenticalArithmeticAreNotTheSameWorkout() = runTest {
        // Set count and total volume are a fingerprint of the NUMBERS, not of the work. Bench 3×10
        // at 100 and Row 3×10 at 100 are three sets and 3,000 lb either way — so the second was
        // waved off as a duplicate and dropped, silently, on the one path whose entire promise is
        // that it merges rather than loses. Date-only sources make this the common case, not a
        // contrived one: every workout they carry starts at the same midnight.
        repo.import(
            fitNotesFile(
                "bench.csv",
                row("2026-01-05", "Bench Press", 100, 10),
                row("2026-01-05", "Bench Press", 100, 10),
                row("2026-01-05", "Bench Press", 100, 10)
            )
        )
        val barbellRow = repo.import(
            fitNotesFile(
                "row.csv",
                row("2026-01-05", "Barbell Row", 100, 10),
                row("2026-01-05", "Barbell Row", 100, 10),
                row("2026-01-05", "Barbell Row", 100, 10)
            )
        ) as ImportResult.Success

        assertEquals("a different lift is a different workout", 0, barbellRow.duplicatesSkipped)
        assertEquals(1, barbellRow.sessions)
        assertEquals(2, storedSessionCount())
    }

    @Test
    fun theSameWorkoutIsStillRecognisedWhenTheNumbersAreIdenticalToAnothers() = runTest {
        // The other side of the same coin: tightening the fingerprint must not stop it matching.
        val uri = fitNotesFile("bench.csv", row("2026-01-05", "Bench Press", 100, 10))
        repo.import(uri)
        repo.import(fitNotesFile("row.csv", row("2026-01-05", "Barbell Row", 100, 10)))

        val again = repo.import(uri)

        assertTrue("got $again", again is ImportResult.NothingToImport)
        assertEquals("still two sessions, not three", 2, storedSessionCount())
    }

    /**
     * The same two workouts, re-imported in the OPPOSITE order.
     *
     * Strong groups rows into workouts by (Date, Workout Name), so ONE file can hold two distinct
     * sessions that both start at the same midnight — a date-only export has no time to separate
     * them with. The second takes the next free slot a second later.
     *
     * The slot search used to begin at a per-run counter and only move forward. On the reversed
     * pass "Evening" matched itself at +1s and pushed the counter to +2s; "Morning" then began its
     * search at +2s, never looked at the midnight slot where it was already stored, and was
     * inserted a second time.
     *
     * Re-running an import is the one thing this path promises changes nothing, and the order of
     * rows in a file is not something a user controls — two exports of the same data can differ.
     */
    @Test
    fun reImportingTheSameDayInTheOppositeOrderAddsNothing() = runTest {
        repo.import(
            strongFile(
                "forwards.csv",
                strongRow("2026-01-05", "Morning", "Bench Press", 100, 10),
                strongRow("2026-01-05", "Evening", "Barbell Row", 90, 8)
            )
        )
        assertEquals("two workouts on one date", 2, storedSessionCount())

        val again = repo.import(
            strongFile(
                "backwards.csv",
                strongRow("2026-01-05", "Evening", "Barbell Row", 90, 8),
                strongRow("2026-01-05", "Morning", "Bench Press", 100, 10)
            )
        )

        assertTrue("got $again", again is ImportResult.NothingToImport)
        assertEquals("both were already stored", 2, storedSessionCount())
    }

    @Test
    fun aGenuinelyNewWorkoutOnAnAlreadyCrowdedDayStillLands() = runTest {
        // The guard must stay loose in the other direction: scanning every occupied slot is for
        // finding a match, not for refusing anything that collides on a date-only midnight.
        repo.import(
            strongFile(
                "existing.csv",
                strongRow("2026-01-05", "Morning", "Bench Press", 100, 10),
                strongRow("2026-01-05", "Evening", "Barbell Row", 90, 8)
            )
        )
        assertEquals(2, storedSessionCount())

        repo.import(strongFile("third.csv", strongRow("2026-01-05", "Night", "Squat", 200, 5)))

        assertEquals("the new workout takes the next free slot", 3, storedSessionCount())
    }

    // ── Merge, never replace ────────────────────────────────────────────────────────────────────

    @Test
    fun anImportMergesIntoExistingHistoryRatherThanReplacingIt() = runTest {
        // The KDoc's promise, and the difference between an import and a .zip restore. A user
        // bringing in years of history from another app must not lose what they already logged here.
        val sessionId = db.sessionDao().insert(session(startedAt = 1_600_000_000_000L))
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = "deadlift"))
        db.loggedSetDao().insert(loggedSet(loggedExerciseId = exId, weightLb = 405.0, reps = 1))

        repo.import(fitNotesFile("incoming.csv", row("2026-01-05", "Bench Press", 225, 3)))

        assertEquals("the pre-existing session survives", 2, storedSessionCount())
        assertEquals(
            "and its sets are untouched",
            405.0, db.loggedSetDao().maxWeightForExercise("deadlift")!!, 0.001
        )
    }

    // ── Files that should import nothing ─────────────────────────────────────────────────────────

    @Test
    fun anUnrecognisedFileImportsNothingAndSaysSo() = runTest {
        val file = temporaryFolder.newFile("shopping-list.csv")
        file.writeText("apples,bananas\n3,4\n")

        val result = repo.import(Uri.fromFile(file))

        assertTrue("got $result", result is ImportResult.UnrecognisedFormat)
        assertEquals(0, storedSessionCount())
    }

    @Test
    fun anEmptyFileImportsNothing() = runTest {
        val file = temporaryFolder.newFile("empty.csv")
        val result = repo.import(Uri.fromFile(file))
        assertTrue("got $result", result is ImportResult.NothingToImport)
        assertEquals(0, storedSessionCount())
    }

    @Test
    fun aRecognisedFileWithNoUsableRowsImportsNothing() = runTest {
        // Header only: the format is recognised, so this is distinct from UnrecognisedFormat, and
        // the user needs to be told the file was empty rather than unreadable.
        val uri = fitNotesFile("headers-only.csv")
        val result = repo.import(uri)
        assertTrue("got $result", result is ImportResult.NothingToImport)
        assertEquals(0, storedSessionCount())
    }

    @Test
    fun importedSessionsAreStoredAsFinishedSoTheyCountTowardHistory() = runTest {
        // An imported workout with no finished_at would be invisible to every stats and PR query in
        // the app — see the exclusion contract in LoggedSetDaoTest — and would also read as a live
        // session on next launch.
        repo.import(fitNotesFile("done.csv", row("2026-01-05", "Bench Press", 225, 3)))

        val stored = db.sessionDao().allFinished().single()
        assertTrue("an imported workout is finished", stored.finishedAt != null)
        assertEquals(
            "and reaches the strength maxima",
            225.0, db.loggedSetDao().maxWeightForExercise("bench_barbell") ?: run {
                // The catalogue id depends on the name matcher; assert via whatever id it resolved.
                val ex = db.loggedExerciseDao().forSession(stored.id).single()
                db.loggedSetDao().maxWeightForExercise(ex.exerciseId)!!
            },
            0.001
        )
    }

    // ── Re-imports that used to duplicate history (audit 2026-09-26, 09 P2) ──────────────────────

    @Test
    fun reImportingAfterEditingTheImportedWorkoutAddsNothing() = runTest {
        val uri = strongFile("strong.csv", strongRow("2026-01-05 10:00:00", "Push", "Bench Press", 225, 5))
        repo.import(uri)
        // The user marks it untracked and annotates the exercise on this device.
        val stored = db.sessionDao().allFinished().single()
        db.sessionDao().update(stored.copy(isUntracked = true, journal = "travel gym"))
        val ex = db.loggedExerciseDao().forSession(stored.id).single()
        db.loggedExerciseDao().update(ex.copy(note = "paused reps"))

        val again = repo.import(
            strongFile("strong-next.csv", strongRow("2026-01-05 10:00:00", "Push", "Bench Press", 225, 5))
        )

        assertTrue("got $again", again is ImportResult.NothingToImport)
        assertEquals(1, db.sessionDao().allFinished().size)
    }

    @Test
    fun aMovementTheMatcherNowKnowsDoesNotDuplicateTheOlderImport() = runTest {
        // Written by an earlier build whose matcher did not know "Cable Fly": synthetic id + label.
        val start = java.time.LocalDateTime.of(2026, 1, 5, 10, 0)
            .atZone(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli()
        val sessionId = db.sessionDao().insert(session(startedAt = start, finishedAt = start + 3_600_000L, dayKey = "freestyle"))
        val leId = db.loggedExerciseDao().insert(
            com.forge.app.data.db.entities.LoggedExercise(
                sessionId = sessionId, exerciseId = "ext-cable-fly", orderIndex = 0, swappedName = "Cable Fly"
            )
        )
        db.loggedSetDao().insert(loggedSet(loggedExerciseId = leId, weightLb = 40.0, reps = 12))
        assertEquals("the premise: today's matcher resolves it", "cable-fly", ExerciseNameMatcher.match("Cable Fly"))

        val result = repo.import(strongFile("fly.csv", strongRow("2026-01-05 10:00:00", "Chest", "Cable Fly", 40, 12)))

        assertTrue("got $result", result is ImportResult.NothingToImport)
        assertEquals(1, storedSessionCount())
    }

    @Test
    fun strongRunsImportAsCardioAndOnlyOnce() = runTest {
        fun runs(name: String): Uri {
            val file = temporaryFolder.newFile(name)
            file.writeText(
                "Date,Workout Name,Duration,Exercise Name,Set Order,Weight,Weight Unit,Reps,Distance,Distance Unit,Seconds\n" +
                    "2026-01-05 07:00:00,Run,30m,Running,1,0,kg,0,5,km,1800\n" +
                    // A second, identical run in the same file is a second run.
                    "2026-01-06 07:00:00,Run,30m,Running,1,0,kg,0,5,km,1800\n" +
                    "2026-01-06 07:00:00,Run,30m,Running,2,0,kg,0,5,km,1800\n"
            )
            return Uri.fromFile(file)
        }

        val first = repo.import(runs("runs.csv")) as ImportResult.Success
        val again = repo.import(runs("runs-again.csv"))

        assertEquals(0, first.sessions)
        assertEquals(3, first.cardioEntries)
        assertTrue("got $again", again is ImportResult.NothingToImport)
        assertEquals("no phantom lifting sessions", 0, storedSessionCount())
        assertEquals(3, db.cardioDao().since(0L).size)
    }

    @Test
    fun aLongNameAnEarlierImportFiledUnderTheTruncatedIdKeepsThatId() = runTest {
        val name = "Incline Dumbbell Bench Press (Neutral Grip)"
        // What the old 40-character cut produced.
        val legacy = "ext-incline-dumbbell-bench-press-neutral-gri"
        val start = java.time.LocalDateTime.of(2026, 1, 5, 10, 0)
            .atZone(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli()
        val sessionId = db.sessionDao().insert(session(startedAt = start, finishedAt = start + 3_600_000L, dayKey = "freestyle"))
        db.loggedExerciseDao().insert(
            com.forge.app.data.db.entities.LoggedExercise(
                sessionId = sessionId, exerciseId = legacy, orderIndex = 0, swappedName = name
            )
        )

        repo.import(strongFile("later.csv", strongRow("2026-02-05 10:00:00", "Push", name, 60, 8)))
        repo.import(
            strongFile("paused.csv", strongRow("2026-02-06 10:00:00", "Push", "\"$name, Paused\"", 50, 8))
        )

        val ids = db.sessionDao().allFinished().filter { it.id != sessionId }
            .map { s -> db.loggedExerciseDao().forSession(s.id).single().exerciseId }.toSet()
        assertTrue("the old id is reused for the same name: $ids", legacy in ids)
        assertEquals("and the paused variant gets its own", 2, ids.size)
    }

    @Test
    fun aFileWhoseRowsAreAllUnreadableSaysSo() = runTest {
        val result = repo.import(strongFile("bad.csv", strongRow("someday", "Push", "Bench Press", 100, 5)))

        assertTrue("got $result", result is ImportResult.NoReadableRows)
        assertEquals(1, (result as ImportResult.NoReadableRows).skippedRows)
    }

    // ── Re-imports after a fix or a move (audit 2026-09-26 follow-up) ───────────────────────────

    private fun midnight(date: String) =
        java.time.LocalDate.parse(date).atStartOfDay(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli()

    /** A workout as an earlier build's importer stored it: [sets] of (weight, reps, hold seconds). */
    private suspend fun storeOldImport(startedAt: Long, vararg exercises: Pair<String, List<Triple<Double?, Int, Int?>>>): Long {
        val sessionId = db.sessionDao().insert(session(startedAt = startedAt, finishedAt = startedAt + 3_600_000L, dayKey = "freestyle"))
        exercises.forEachIndexed { i, (name, sets) ->
            val leId = db.loggedExerciseDao().insert(
                loggedExercise(sessionId = sessionId, exerciseId = "ext-${name.lowercase()}", orderIndex = i)
                    .copy(swappedName = name, note = if (i == 0) "felt strong" else null)
            )
            db.loggedSetDao().insertAll(sets.mapIndexed { j, (w, r, hold) ->
                loggedSet(loggedExerciseId = leId, setIndex = j, weightLb = w, reps = r, durationSeconds = hold)
            })
        }
        return sessionId
    }

    @Test
    fun aWeightlessOldImportIsFilledInNotDuplicated() = runTest {
        // A Hevy lb export read by the old parser: the right sets, no loads.
        val id = storeOldImport(midnight("2026-01-05"), "Bench Press" to listOf(Triple(null, 3, null), Triple(null, 3, null)))

        val result = repo.import(
            fitNotesFile("fixed.csv", row("2026-01-05", "Bench Press", 225, 3), row("2026-01-05", "Bench Press", 225, 3))
        ) as ImportResult.Success

        assertEquals(0, result.sessions)
        assertEquals(1, result.workoutsCorrected)
        assertEquals(1, storedSessionCount())
        val sets = db.loggedSetDao().allForSession(id)
        assertEquals(listOf(225.0, 225.0), sets.map { it.weightLb })
        assertEquals("the user's note survives", "felt strong", db.loggedExerciseDao().forSession(id).single().note)
    }

    @Test
    fun aPhantomCardioHoldInAnOldImportIsDropped() = runTest {
        val id = storeOldImport(
            midnight("2026-01-05"),
            "Bench Press" to listOf(Triple(225.0, 3, null)),
            "Running" to listOf(Triple(null, 1800, 1800))
        )

        val result = repo.import(fitNotesFile("fixed.csv", row("2026-01-05", "Bench Press", 225, 3))) as ImportResult.Success

        assertEquals(1, result.workoutsCorrected)
        assertEquals(1, storedSessionCount())
        assertEquals(1, db.loggedExerciseDao().forSession(id).size)
    }

    @Test
    fun aDifferentWorkoutIsNotMistakenForALossyCopy() = runTest {
        storeOldImport(midnight("2026-01-05"), "Bench Press" to listOf(Triple(null, 5, null)))

        val result = repo.import(fitNotesFile("other.csv", row("2026-01-05", "Bench Press", 225, 3))) as ImportResult.Success

        assertEquals("different reps: a new workout", 1, result.sessions)
        assertEquals(0, result.workoutsCorrected)
        assertEquals(2, storedSessionCount())
    }

    @Test
    fun reImportingAfterAZoneChangeAddsNothing() = runTest {
        val original = java.util.TimeZone.getDefault()
        try {
            java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("America/New_York"))
            val uri = strongFile("strong.csv", strongRow("2026-01-05 18:00:00", "Push", "Bench Press", 225, 5))
            repo.import(uri)
            java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("Europe/Berlin"))

            val second = repo.import(uri)

            assertTrue("got $second", second is ImportResult.NothingToImport)
            assertEquals(1, storedSessionCount())
        } finally {
            java.util.TimeZone.setDefault(original)
        }
    }
}
