package com.forge.app.data.repo

import android.content.Context
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.importer.ImportResult
import com.forge.app.data.importer.WorkoutImportRepository
import com.forge.app.data.prefs.SettingsRepository
import java.io.File
import java.time.LocalDateTime
import java.time.ZoneId
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
 * Avex JSON out, Avex JSON back in (audit 2026-09-26, 08 P2 and 09 P2).
 *
 * The exports dropped `wasPr`, `hitFullTarget`, `supersetGroup` and three cardio fields, the weekly
 * and session files dropped `isUntracked`, and the weekly file wrote cardio as a bare date. And the
 * duplicate guard printed every stored field, so importing an export onto the device that wrote it
 * duplicated any workout the export did not reproduce exactly.
 */
@RunWith(RobolectricTestRunner::class)
class ExportImportRoundTripTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val zone = ZoneId.systemDefault()
    private fun at(day: Int, hour: Int) =
        LocalDateTime.of(2026, 9, day, hour, 0).atZone(zone).toInstant().toEpochMilli()

    /** Thursday 24 September 2026, 20:00: the weekly export covers Monday 21st onwards. */
    private val clock = Clock { at(24, 20) }
    private val settings = SettingsRepository(context, clock)

    private val source: ForgeDatabase = inMemoryForgeDb()
    private val target: ForgeDatabase = inMemoryForgeDb()

    private fun backupOf(db: ForgeDatabase) = BackupRepository(
        context = context,
        sessionDao = db.sessionDao(),
        loggedExerciseDao = db.loggedExerciseDao(),
        loggedSetDao = db.loggedSetDao(),
        cardioDao = db.cardioDao(),
        coachGoalDao = db.coachGoalDao(),
        settingsRepo = settings,
        photoRepo = ProgressPhotoRepository(context, db.bodyweightDao()),
        avatarRepo = AvatarRepository(context, settings),
        grants = PersistedTreeGrants(context, settings),
        db = db,
        clock = clock
    )

    private fun importerOf(db: ForgeDatabase) = WorkoutImportRepository(
        context = context,
        db = db,
        sessionDao = db.sessionDao(),
        loggedExerciseDao = db.loggedExerciseDao(),
        loggedSetDao = db.loggedSetDao(),
        moodDao = db.moodDao(),
        cardioDao = db.cardioDao(),
        coachGoalDao = db.coachGoalDao(),
        bodyweightDao = db.bodyweightDao(),
        settingsRepo = settings,
        grants = PersistedTreeGrants(context, settings)
    )

    @After
    fun tearDown() {
        source.close()
        target.close()
    }

    /** A copy outside the exports dir, so a second export cannot overwrite what is being imported. */
    private fun asUri(file: File): Uri =
        Uri.fromFile(file.copyTo(temporaryFolder.newFile(), overwrite = true))

    /**
     * One untracked deload workout on Wednesday 23rd: a PR cable fly in superset A, a swapped pec
     * deck, a skipped exercise and a custom movement, logged with no stamped active time. Plus two
     * equal runs that day, morning and evening.
     */
    private suspend fun seed(db: ForgeDatabase): Long {
        val start = at(23, 18)
        val sessionId = db.sessionDao().insert(
            Session(
                dayKey = "push", startedAt = start, finishedAt = start + 3_600_000L,
                isUntracked = true, sessionType = "deload", prCount = 1, activeSeconds = 0
            )
        )
        suspend fun exercise(le: LoggedExercise, vararg reps: Int) {
            val id = db.loggedExerciseDao().insert(le)
            reps.forEachIndexed { i, r ->
                db.loggedSetDao().insert(
                    LoggedSet(
                        loggedExerciseId = id, setIndex = i, weightText = "100", weightLb = 100.0,
                        reps = r, completedAt = start + 60_000L * (i + 1)
                    )
                )
            }
        }
        exercise(
            LoggedExercise(sessionId = sessionId, exerciseId = "cable-fly", orderIndex = 0,
                wasPr = true, hitFullTarget = true, supersetGroup = "A"),
            12, 12
        )
        exercise(
            LoggedExercise(sessionId = sessionId, exerciseId = "pec-deck", orderIndex = 1,
                swappedName = "Cable Crossover", supersetGroup = "A"),
            15
        )
        exercise(LoggedExercise(sessionId = sessionId, exerciseId = "machine-chest-press", orderIndex = 2, skipped = true))
        exercise(LoggedExercise(sessionId = sessionId, exerciseId = "custom-landmine-press", orderIndex = 3, note = ""), 10)

        listOf(at(23, 8), at(23, 18)).forEach { date ->
            db.cardioDao().insert(
                CardioEntry(date = date, type = "run", durationMin = 30, distanceKm = 5.0,
                    intervalCount = 6, hrZone = "4", conditions = "hot,wind")
            )
        }
        return sessionId
    }

    private suspend fun assertEverythingArrived(db: ForgeDatabase) {
        val session = db.sessionDao().allFinished().single()
        assertTrue("untracked stays excluded", session.isUntracked)
        assertEquals("deload", session.sessionType)

        val exercises = db.loggedExerciseDao().forSession(session.id).associateBy { it.orderIndex }
        val fly = exercises.getValue(0)
        assertEquals("cable-fly", fly.exerciseId)
        assertTrue("the PR survives", fly.wasPr)
        assertTrue(fly.hitFullTarget)
        assertEquals("A", fly.supersetGroup)
        val pec = exercises.getValue(1)
        assertEquals("pec-deck", pec.exerciseId)
        assertEquals("Cable Crossover", pec.swappedName)
        assertEquals("A", pec.supersetGroup)
        assertTrue("the skip survives", exercises.getValue(2).skipped)

        val cardio = db.cardioDao().since(0L)
        assertEquals("two equal runs, morning and evening, are two runs", listOf(at(23, 8), at(23, 18)), cardio.map { it.date }.sorted())
        cardio.forEach {
            assertEquals(6, it.intervalCount)
            assertEquals("4", it.hrZone)
            assertEquals("hot,wind", it.conditions)
        }
    }

    @Test
    fun theFullExportRoundTripsEveryField() = runTest {
        seed(source)

        val result = importerOf(target).import(asUri(backupOf(source).exportFullDataJson()))

        assertTrue("got $result", result is ImportResult.Success)
        assertEverythingArrived(target)
    }

    @Test
    fun theWeeklyExportRoundTripsEveryField() = runTest {
        seed(source)

        importerOf(target).import(asUri(backupOf(source).exportWeeklyJson()))

        assertEverythingArrived(target)
    }

    @Test
    fun theSessionExportKeepsTheUntrackedFlag() = runTest {
        val id = seed(source)

        importerOf(target).import(asUri(backupOf(source).exportSessionJson(id)!!))

        assertTrue(target.sessionDao().allFinished().single().isUntracked)
    }

    @Test
    fun importingAnExportOntoTheDeviceThatWroteItAddsNothing() = runTest {
        seed(source)
        val importer = importerOf(source)

        listOf(
            backupOf(source).exportFullDataJson(),
            backupOf(source).exportWeeklyJson()
        ).map { asUri(it) }.forEach { uri ->
            val result = importer.import(uri)
            assertTrue("got $result", result is ImportResult.NothingToImport)
        }

        assertEquals(1, source.sessionDao().allFinished().size)
        assertEquals(2, source.cardioDao().since(0L).size)
    }
}
