package com.forge.app.data.importer

import android.content.Context
import android.database.sqlite.SQLiteFullException
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.prefs.SettingsRepository
import java.time.Instant
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Import hardening audit, 2026-09-27: what a hostile or corrupted file can do on the way in.
 *
 * Every case here used to reach the database unchanged: a weight of Infinity, a two-million-rep
 * set, a journal larger than a CursorWindow, a weigh-in of NaN, a workout dated 2400, and a JSON
 * document nested deep enough to overflow the parser's stack, which escaped import() altogether.
 */
@RunWith(RobolectricTestRunner::class)
class ImportHardeningTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()

    private val now = Instant.parse("2026-09-27T12:00:00Z").toEpochMilli()
    private val clock = Clock { now }

    private fun repoWith(cardioDao: CardioDao = db.cardioDao()) = WorkoutImportRepository(
        context = context,
        db = db,
        sessionDao = db.sessionDao(),
        loggedExerciseDao = db.loggedExerciseDao(),
        loggedSetDao = db.loggedSetDao(),
        moodDao = db.moodDao(),
        cardioDao = cardioDao,
        coachGoalDao = db.coachGoalDao(),
        bodyweightDao = db.bodyweightDao(),
        settingsRepo = SettingsRepository(context, clock),
        grants = com.forge.app.data.repo.PersistedTreeGrants(context, SettingsRepository(context, clock)),
        clock = clock
    )

    private val repo = repoWith()

    @After
    fun tearDown() = db.close()

    private val start = Instant.parse("2026-01-05T18:00:00Z").toEpochMilli()

    private fun file(name: String, text: String): Uri {
        val f = temporaryFolder.newFile(name)
        f.writeText(text)
        return Uri.fromFile(f)
    }

    /** One Avex-JSON workout of a single exercise holding [sets]; [extra] adds session fields. */
    private fun sessionJson(
        startedAt: Long,
        sets: String = """{"weightLb":100,"reps":5}""",
        extra: String = "",
        exercise: String = "Pull Up"
    ) = "{\"startedAt\":$startedAt,\"finishedAt\":${startedAt + 3_600_000L}$extra," +
        "\"exercises\":[{\"name\":\"$exercise\",\"sets\":[$sets]}]}"

    private fun avexFile(name: String, vararg sessions: String, cardio: String = "") =
        file(name, """{"exportVersion":1,"sessions":[${sessions.joinToString(",")}]$cardio}""")

    // ── Numbers ─────────────────────────────────────────────────────────────────────────────────

    @Test
    fun nonFiniteWeightsAndRepsAreNotNumbers() {
        listOf("Infinity", "-Infinity", "NaN", "1e999").forEach {
            assertNull("weight $it", ImportParsing.parseWeight(it))
            assertNull("reps $it", ImportParsing.parseReps(it))
        }
        assertEquals("an ordinary weight still parses", 45.5, ImportParsing.parseWeight("45.5 kg")!!, 0.0)
        assertEquals(8, ImportParsing.parseReps("8"))
    }

    @Test
    fun importedSetsTakeTheLoggingPathsClamps() = runTest {
        val sets = listOf(
            """{"weightLb":"Infinity","reps":5}""",
            """{"weightLb":"NaN","reps":5}""",
            """{"weightLb":"1e999","reps":5}""",
            """{"weightLb":100,"reps":-5}""",
            """{"weightLb":100,"reps":5000000}""",
            """{"weightLb":1000000000,"weightText":"1000000000","reps":1}"""
        ).joinToString(",")

        val result = repo.import(avexFile("numbers.json", sessionJson(start, sets)))

        assertTrue("got $result", result is ImportResult.Success)
        val session = db.sessionDao().allFinished().single()
        val stored = db.loggedSetDao().allForSession(session.id).sortedBy { it.setIndex }
        assertEquals(listOf(null, null, null, 100.0, 100.0, 2000.0), stored.map { it.weightLb })
        assertEquals(listOf(5, 5, 5, 0, 999, 1), stored.map { it.reps })
        assertNotEquals("a clamped weight takes its text with it", "1000000000", stored.last().weightText)
        // 100 x 0 + 100 x 999 + 2000 x 1, from the stored values rather than the file's.
        assertEquals(101_900.0, session.totalVolumeLb!!, 0.001)
    }

    // ── Text ────────────────────────────────────────────────────────────────────────────────────

    @Test
    fun aHugeJournalAndNameAreCappedNotStoredWhole() = runTest {
        val journal = "x".repeat(3 * 1024 * 1024)
        val longName = "Zqxv " + "y".repeat(300)

        repo.import(
            avexFile("long.json", sessionJson(start, extra = ",\"journal\":\"$journal\",\"tags\":\"${"t".repeat(500)}\"", exercise = longName))
        )

        val session = db.sessionDao().allFinished().single()
        assertEquals(ImportBounds.MAX_LONG_TEXT, session.journal.length)
        assertEquals(ImportBounds.MAX_SHORT_TEXT, session.tags.length)
        val exercise = db.loggedExerciseDao().forSession(session.id).single()
        assertEquals(ImportBounds.MAX_SHORT_TEXT, exercise.swappedName!!.length)
    }

    @Test
    fun aCapNeverSplitsASurrogatePair() {
        val s = "a".repeat(ImportBounds.MAX_SHORT_TEXT - 1) + "💪" // flexed biceps straddles the cap
        val capped = ImportBounds.shortText(s)!!
        assertEquals(ImportBounds.MAX_SHORT_TEXT - 1, capped.length)
        assertTrue(capped.none { Character.isSurrogate(it) })
    }

    // ── Weigh-ins ───────────────────────────────────────────────────────────────────────────────

    @Test
    fun badWeighInsAreSkippedAndGoodOnesSurvive() {
        val csv = """
            date,weightLb
            2026-01-05,180
            2026-01-06,NaN
            2026-13-45,181
            2026-01-07,Infinity
            2026-01-08,2000
            2026-01-09,0
            2026-01-10,182.5
        """.trimIndent()

        val read = ForgeBodyweightCsvImporter().read(csv, assumeKg = false)

        assertEquals(listOf("2026-01-05", "2026-01-10"), read.extras.bodyweight.map { it.dateKey })
        assertEquals(5, read.skippedRows)
    }

    @Test
    fun aWeighInFileReportsItsSkippedRows() = runTest {
        val result = repo.import(file("bw.csv", "date,weightLb\n2026-01-05,180\n2026-02-30,181\n"))

        result as ImportResult.Success
        assertEquals(1, result.bodyweightEntries)
        assertEquals(1, result.skippedRows)
        assertEquals(180.0, db.bodyweightDao().byDateKey("2026-01-05")!!.weightLb, 0.0)
    }

    // ── Dates ───────────────────────────────────────────────────────────────────────────────────

    @Test
    fun farFutureAndPre2000WorkoutsAreSkippedAsRows() = runTest {
        val result = repo.import(
            avexFile(
                "dates.json",
                sessionJson(start),
                sessionJson(Instant.parse("1990-01-01T10:00:00Z").toEpochMilli()),
                sessionJson(Instant.parse("2400-01-01T10:00:00Z").toEpochMilli()),
                // Two days past "now": outside the one day of slack.
                sessionJson(now + 2 * 86_400_000L)
            )
        )

        result as ImportResult.Success
        assertEquals(1, result.sessions)
        assertEquals(3, result.skippedRows)
        assertEquals(listOf(start), db.sessionDao().allFinished().map { it.startedAt })
    }

    @Test
    fun aCsvOfOnlyOutOfRangeDatesSaysItsRowsCouldNotBeRead() = runTest {
        val csv = "Date,Exercise,Category,Weight,Weight Unit,Reps,Distance,Distance Unit,Time,Comment\n" +
            "2099-01-05,Bench Press,Barbell,225,lbs,5,,,,\n" +
            "1970-01-02,Bench Press,Barbell,225,lbs,5,,,,\n"

        val result = repo.import(file("fitnotes.csv", csv))

        assertTrue("got $result", result is ImportResult.NoReadableRows)
        assertEquals(0, db.sessionDao().allFinished().size)
    }

    @Test
    fun aFinishInstantOutOfRangeRejectsTheWorkout() = runTest {
        val bad = """{"startedAt":$start,"finishedAt":9223372036854775000,"exercises":[{"name":"Pull Up","sets":[{"weightLb":0,"reps":8}]}]}"""

        val result = repo.import(avexFile("finish.json", bad, sessionJson(start + 86_400_000L)))

        result as ImportResult.Success
        assertEquals(1, result.sessions)
        assertEquals(1, result.skippedRows)
    }

    // ── Malformed documents ─────────────────────────────────────────────────────────────────────

    @Test
    fun deeplyNestedJsonIsAParseFailureNotACrash() = runTest {
        // org.json recurses once per level; two million levels overflow any thread's stack.
        val nested = """{"exportVersion":1,"sessions":""" + "[".repeat(2_000_000)

        val result = repo.import(file("nested.json", nested))

        assertEquals(ImportResult.ParseFailed(ImportSource.FORGE_JSON), result)
    }

    // ── Atomicity ───────────────────────────────────────────────────────────────────────────────

    @Test
    fun aFailureWritingExtrasLeavesNoWorkoutsBehind() = runTest {
        val real = db.cardioDao()
        val failing = object : CardioDao by real {
            override suspend fun insert(entry: CardioEntry): Long = throw SQLiteFullException("disk full")
        }
        val cardio = ""","cardio":[{"date":$start,"type":"run","durationMin":30}]"""

        val outcome = runCatching { repoWith(failing).import(avexFile("atomic.json", sessionJson(start), cardio = cardio)) }

        assertTrue("the write failed: $outcome", outcome.isFailure)
        assertEquals("the workouts rolled back with it", 0, db.sessionDao().allFinished().size)
        assertEquals(0, db.loggedSetDao().allForFinishedSessions().size)
    }

    @Test
    fun cardioOutOfBoundsKeepsTheEntryAndDropsTheMeasure() {
        val json = """{"sessions":[],"cardio":[{"date":$start,"type":"run","durationMin":99999,"distanceKm":"Infinity","inclinePct":"NaN","elevationM":1e6}]}"""

        val entry = ForgeJsonImporter().parseExtras(json, assumeKg = false).cardio.single()

        assertEquals(ImportBounds.MAX_CARDIO_MINUTES, entry.durationMin)
        assertNull(entry.distanceKm)
        assertNull(entry.inclinePct)
        assertNull(entry.elevationM)
    }
}
