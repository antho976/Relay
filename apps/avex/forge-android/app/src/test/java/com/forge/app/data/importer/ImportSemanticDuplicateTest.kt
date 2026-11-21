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
 * What a re-import of an Avex JSON export counts as new work (audit 2026-09-26, 09 P2).
 *
 * The guard used to print every field the row holds (M-03), so a copy that differed in an
 * annotation, an end time or a mood landed BESIDE the original: the same pull-up twice in the log.
 * An import merges and never overwrites, so an annotation-only difference is now the same workout
 * and is skipped. A difference in what the source states about the work itself (reps, load, hold,
 * RPE, warm-up) is still a different workout.
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
    private fun avexExport(
        name: String,
        setExtras: String = "",
        exerciseExtras: String = "",
        /** Session-level fields to add or override — `finishedAt`, `prCount`, `mood` (M-03). */
        sessionOverrides: Map<String, String> = emptyMap()
    ): Uri {
        val file = temporaryFolder.newFile(name)
        val session = LinkedHashMap<String, String>().apply {
            put("startedAt", "$startedAt")
            put("finishedAt", "${startedAt + 3_600_000L}")
            putAll(sessionOverrides)
        }
        val fields = session.entries.joinToString(",") { (k, v) -> "\"$k\":$v" }
        file.writeText(
            """
            {"exportVersion":1,"sessions":[{
              $fields,
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
    fun anAnnotationOnlyDifferenceIsTheSameWorkout() = runTest {
        repo.import(avexExport("plain.json"))

        val copies = listOf(
            avexExport("assisted.json", setExtras = ""","isAssisted":true"""),
            avexExport("failure.json", setExtras = ""","toFailure":true"""),
            avexExport("note.json", exerciseExtras = """"note":"left side only","""),
            avexExport("end.json", sessionOverrides = mapOf("finishedAt" to "${startedAt + 4_500_000L}")),
            avexExport("pr.json", sessionOverrides = mapOf("prCount" to "2")),
            avexExport("mood.json", sessionOverrides = mapOf("mood" to "\"strong\"")),
            avexExport("untracked.json", sessionOverrides = mapOf("isUntracked" to "true"))
        ).map { repo.import(it) }

        copies.forEach { assertTrue("got $it", it is ImportResult.NothingToImport) }
        assertEquals("one workout, however it was annotated", 1, storedSessions().size)
    }

    @Test
    fun aDifferentRpeOrWarmUpIsDifferentWork() = runTest {
        repo.import(avexExport("plain.json"))

        val rpe = repo.import(avexExport("rpe.json", ""","rpe":9.0"""))
        val warmup = repo.import(avexExport("warmup.json", ""","setType":"warmup""""))

        listOf(rpe, warmup).forEach {
            assertEquals("got $it", 1, (it as ImportResult.Success).sessions)
        }
        assertEquals(3, storedSessions().size)
    }

    @Test
    fun aSkippedExerciseIsImportedAndKeptSkipped() = runTest {
        val file = temporaryFolder.newFile("skip.json")
        file.writeText(
            """
            {"exportVersion":1,"sessions":[{"startedAt":$startedAt,"finishedAt":${startedAt + 3_600_000L},
              "exercises":[
                {"name":"Pull Up","orderIndex":0,"sets":[{"weightLb":0,"reps":8}]},
                {"name":"Dip","orderIndex":1,"skipped":true,"sets":[]}
              ]}]}
            """.trimIndent()
        )

        repo.import(Uri.fromFile(file))

        val session = storedSessions().single()
        val exercises = db.loggedExerciseDao().forSession(session.id)
        assertEquals(2, exercises.size)
        assertTrue("the skip survives the migration", exercises.single { it.orderIndex == 1 }.skipped)
    }
}
