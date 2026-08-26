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
        settingsRepo = SettingsRepository(context, Clock { 1_700_000_000_000L })
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
}
