package com.forge.app.data.importer

import android.content.Context
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.prefs.SettingsRepository
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
 * M-03 end to end: a corrected re-export must land, and an unchanged one must not.
 *
 * The Avex JSON export is the source that actually carries the semantic fields, and the migration
 * path it exists to serve is exactly where the loss hurt: fix an assisted flag in the source app,
 * export again, import — and the corrected workout was discarded as a duplicate of the one it was
 * correcting, silently, leaving an assisted pull-up PR-eligible.
 */
@RunWith(RobolectricTestRunner::class)
class ImportSemanticDuplicateTest {

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

    private val startedAt = 1_767_600_000_000L

    /** One Avex-JSON workout: a single pull-up set, with whatever set fields [setExtras] adds. */
    private fun avexExport(name: String, setExtras: String = "", exerciseExtras: String = ""): Uri {
        val file = temporaryFolder.newFile(name)
        file.writeText(
            """
            {"exportVersion":1,"sessions":[{
              "startedAt":$startedAt,
              "finishedAt":${startedAt + 3_600_000L},
              "exercises":[{
                "name":"Pull Up",
                $exerciseExtras
                "sets":[{"weightLb":0,"reps":8$setExtras}]
              }]
            }]}
            """.trimIndent()
        )
        return Uri.fromFile(file)
    }

    private suspend fun storedSessions() = db.sessionDao().allFinished()

    @Test
    fun anUnchangedReExportIsStillRecognisedAsAlreadyImported() = runTest {
        repo.import(avexExport("first.json"))

        val second = repo.import(avexExport("again.json"))

        assertTrue("got $second", second is ImportResult.NothingToImport)
        assertEquals(1, storedSessions().size)
    }

    @Test
    fun aCorrectedAssistedFlagIsNotADuplicate() = runTest {
        repo.import(avexExport("before.json"))

        val corrected = repo.import(
            avexExport("after.json", setExtras = ""","isAssisted":true""")
        ) as ImportResult.Success

        assertEquals("the corrected copy is different work", 0, corrected.duplicatesSkipped)
        assertEquals(1, corrected.sessions)
        assertEquals(2, storedSessions().size)
    }

    @Test
    fun aCorrectedSetTypeRpeOrFailureFlagIsNotADuplicate() = runTest {
        repo.import(avexExport("plain.json"))

        val warmup = repo.import(avexExport("warmup.json", ""","setType":"warmup""""))
        val rpe = repo.import(avexExport("rpe.json", ""","rpe":9.0"""))
        val failure = repo.import(avexExport("failure.json", ""","toFailure":true"""))

        listOf(warmup, rpe, failure).forEach {
            assertEquals("got $it", 1, (it as ImportResult.Success).sessions)
        }
        assertEquals("each correction is its own workout", 4, storedSessions().size)
    }

    @Test
    fun aSkippedExerciseIsNotTheSameAsAPerformedOne() = runTest {
        repo.import(avexExport("performed.json"))

        val skipped = repo.import(
            avexExport("skipped.json", exerciseExtras = """"skipped":true,""")
        ) as ImportResult.Success

        assertEquals(0, skipped.duplicatesSkipped)
        assertEquals(2, storedSessions().size)
    }
}
