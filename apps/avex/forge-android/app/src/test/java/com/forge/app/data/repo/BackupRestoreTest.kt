package com.forge.app.data.repo

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.RestoreManifest
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.loggedExercise
import com.forge.app.data.db.loggedSet
import com.forge.app.data.db.session
import com.forge.app.data.prefs.SettingsRepository
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Backup and restore — 1,157 lines that had no tests, guarding the one operation in the app whose
 * failure is unrecoverable.
 *
 * Everything else here can be wrong and fixed in the next release. A restore that accepts a file it
 * should have rejected replaces a real training history with rubbish, and the user reached for it
 * precisely because something had already gone wrong. So the property under test throughout is not
 * "does a good backup restore" — it is **a bad one must change nothing**.
 *
 * The design already takes this seriously: a restore does not swap anything in place, it STAGES
 * pending files that `ForgeApp.applyPendingRestore` applies atomically at the next boot. That is why
 * these tests assert on the staged pending set rather than on the live database, and why "nothing
 * was staged" is the assertion that matters on every rejection path.
 */
@RunWith(RobolectricTestRunner::class)
class BackupRestoreTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

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
        clock = clock
    )

    private val pendingDb get() = File(context.filesDir, "pending_restore.db")

    @After
    fun tearDown() = db.close()

    private suspend fun seedOneWorkout(exerciseId: String = "bench", weightLb: Double = 225.0) {
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = exerciseId))
        db.loggedSetDao().insert(loggedSet(loggedExerciseId = exId, weightLb = weightLb, reps = 3))
    }

    private fun outFile(name: String = "backup.zip"): File = temporaryFolder.newFile(name)

    /** Rows in the STAGED database, read back the way the next boot would see it. */
    private fun stagedSessionCount(): Int {
        val staged = SQLiteDatabase.openDatabase(pendingDb.path, null, SQLiteDatabase.OPEN_READONLY)
        return staged.use { database ->
            database.rawQuery("SELECT COUNT(*) FROM session", null).use {
                it.moveToFirst()
                it.getInt(0)
            }
        }
    }

    // ── The good path ───────────────────────────────────────────────────────────────────────────

    @Test
    fun aBackupRoundTripsBackIntoAStagedDatabaseCarryingTheSameRows() = runTest {
        seedOneWorkout()
        val archive = outFile()

        repo.backupToUri(Uri.fromFile(archive))
        assertTrue("the archive should have been written", archive.length() > 0)

        val outcome = repo.restoreFromUri(Uri.fromFile(archive))

        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, outcome)
        assertTrue("a pending database should be staged for the next boot", pendingDb.isFile)
        assertEquals("the staged database carries the session that was backed up", 1, stagedSessionCount())
        assertTrue(
            "the manifest is what lets the next boot apply the set",
            RestoreManifest.verify(context.filesDir)
        )
    }

    @Test
    fun theArchiveIsAZipCarryingTheDatabaseUnderTheAgreedEntryName() = runTest {
        // ForgeApp and BackupRepository agree on "database.db" by constant. Renaming the entry on
        // one side only would produce archives that back up fine and restore as NOT_A_BACKUP —
        // discovered, at the earliest, by someone who has already lost their data.
        seedOneWorkout()
        val archive = outFile()
        repo.backupToUri(Uri.fromFile(archive))

        val entries = java.util.zip.ZipFile(archive).use { zip ->
            zip.entries().toList().map { it.name }
        }
        assertTrue("database.db missing from $entries", "database.db" in entries)
    }

    @Test
    fun makingABackupClearsTheStaleAutoBackupFailureNotice() = runTest {
        // The notice asks the user to make a backup. Making one has to dismiss it, or it nags
        // forever and stops meaning anything.
        repo.recordAutoBackupFailure()
        assertTrue(repo.autoBackupFailed())

        seedOneWorkout()
        repo.backupToUri(Uri.fromFile(outFile()))

        assertFalse("a successful manual backup answers the warning", repo.autoBackupFailed())
        assertTrue("and counts as having a backup", repo.hasAnyBackup())
    }

    // ── Rejection: nothing may be staged ────────────────────────────────────────────────────────

    @Test
    fun aFileThatIsNotABackupIsRejectedAndStagesNothing() = runTest {
        seedOneWorkout()
        val notABackup = temporaryFolder.newFile("holiday-photo.jpg")
        notABackup.writeText("this is not a database")

        val outcome = repo.restoreFromUri(Uri.fromFile(notABackup))

        assertEquals(BackupRepository.RestoreOutcome.NOT_A_BACKUP, outcome)
        assertFalse("nothing may be staged from a rejected file", pendingDb.exists())
    }

    @Test
    fun aZipWithoutADatabaseEntryIsRejected() = runTest {
        // A real ZIP, so it passes the magic-byte sniff and gets as far as extraction — the case a
        // signature check alone would wave through.
        val archive = temporaryFolder.newFile("prefs-only.zip")
        ZipOutputStream(archive.outputStream()).use { zip ->
            zip.putNextEntry(ZipEntry("settings.preferences_pb"))
            zip.write("not a database".toByteArray())
            zip.closeEntry()
        }

        val outcome = repo.restoreFromUri(Uri.fromFile(archive))

        assertEquals(BackupRepository.RestoreOutcome.NOT_A_BACKUP, outcome)
        assertFalse(pendingDb.exists())
    }

    @Test
    fun anEmptyFileIsRejected() = runTest {
        val empty = temporaryFolder.newFile("empty.zip")
        val outcome = repo.restoreFromUri(Uri.fromFile(empty))
        assertEquals(BackupRepository.RestoreOutcome.NOT_A_BACKUP, outcome)
        assertFalse(pendingDb.exists())
    }

    @Test
    fun aTruncatedSqliteFileIsReportedAsCorruptRatherThanRestored() = runTest {
        // Starts with the SQLite magic, so it is recognisably OUR file — and is still unusable.
        // The distinction matters to the user: "this isn't a backup" and "your backup is damaged"
        // are different problems with different next steps.
        val truncated = temporaryFolder.newFile("half-a-backup.db")
        truncated.writeBytes("SQLite format 3 ".toByteArray(Charsets.US_ASCII) + ByteArray(64))

        val outcome = repo.restoreFromUri(Uri.fromFile(truncated))

        assertEquals(BackupRepository.RestoreOutcome.CORRUPT, outcome)
        assertFalse(pendingDb.exists())
    }

    @Test
    fun aRejectedRestoreLeavesTheLiveDatabaseUntouched() = runTest {
        seedOneWorkout(weightLb = 225.0)
        val before = db.loggedSetDao().maxWeightForExercise("bench")

        val junk = temporaryFolder.newFile("junk.bin")
        junk.writeBytes(ByteArray(2048) { 0x7F })
        repo.restoreFromUri(Uri.fromFile(junk))

        assertEquals(
            "the live history must survive a rejected restore intact",
            before, db.loggedSetDao().maxWeightForExercise("bench")
        )
    }

    @Test
    fun aRejectedRestoreLeavesNoTempFilesBehindInTheCache() = runTest {
        // Staging copies the candidate into cacheDir first. A rejection path that returns without
        // cleaning up leaves a full copy of a would-be backup on the device every time someone
        // picks the wrong file.
        val junk = temporaryFolder.newFile("junk2.bin")
        junk.writeBytes(ByteArray(4096) { 0x11 })

        repo.restoreFromUri(Uri.fromFile(junk))

        val leftovers = context.cacheDir.listFiles().orEmpty().filter { it.name.startsWith("forge_restore") }
        assertTrue("cache still holds ${leftovers.map { it.name }}", leftovers.isEmpty())
    }

    @Test
    fun restoringWithNoAutoBackupSlotSaysSoInsteadOfFailing() = runTest {
        assertEquals(
            BackupRepository.RestoreOutcome.NO_BACKUP_FILE,
            repo.restoreFromAutoBackup()
        )
        assertFalse(pendingDb.exists())
    }

    // ── The nudge that tells a user they have no safety net ──────────────────────────────────────

    @Test
    fun theNoBackupWarningOnlyFiresOnceThereIsSomethingToLose() = runTest {
        assertFalse("a brand-new install has nothing to back up yet", repo.shouldWarnNoBackup())

        seedOneWorkout()
        // seedOneWorkout leaves the session finished, so there is now real history and no backup.
        assertTrue("history with no backup should warn", repo.shouldWarnNoBackup())

        repo.backupToUri(Uri.fromFile(outFile("first.zip")))
        assertFalse("and stop warning once a backup exists", repo.shouldWarnNoBackup())
    }

    @Test
    fun aSameVersionSqliteFileThatOnlyNamesOurTablesIsRefusedBeforeItIsStaged() = runTest {
        // Three empty tables called session, logged_exercise and logged_set, at this build's
        // schema version. That passed the header check, the table-name check, quick_check and both
        // version floors, was swapped over the live database at boot, and only THEN failed Room's
        // validation, once the pre-restore snapshot had been discarded. The staged file is now
        // opened through the production Room builder before anything is kept.
        val impostor = temporaryFolder.newFile("impostor.db")
        impostor.delete()
        SQLiteDatabase.openOrCreateDatabase(impostor, null).use { sql ->
            sql.execSQL("CREATE TABLE session (x INTEGER)")
            sql.execSQL("CREATE TABLE logged_exercise (x INTEGER)")
            sql.execSQL("CREATE TABLE logged_set (x INTEGER)")
            sql.version = db.openHelper.readableDatabase.version
        }

        val outcome = repo.restoreFromUri(Uri.fromFile(impostor))

        assertEquals(BackupRepository.RestoreOutcome.CORRUPT, outcome)
        assertFalse("nothing may be staged from a file Room cannot open", pendingDb.exists())
        assertFalse(RestoreManifest.file(context.filesDir).exists())
        val probes = context.getDatabasePath("forge_restore_probe.db").parentFile?.listFiles().orEmpty()
            .filter { it.name.startsWith("forge_restore_probe") }
        assertTrue("the probe is cleaned up: ${probes.map { it.name }}", probes.isEmpty())
    }

    @Test
    fun aRejectedRestoreLeavesNoManifestBehind() = runTest {
        // A stale READY marker beside nothing, or beside the next attempt's half-staged files, is
        // exactly the state the marker exists to rule out.
        val junk = temporaryFolder.newFile("junk3.bin")
        junk.writeBytes(ByteArray(2048) { 0x7F })
        repo.restoreFromUri(Uri.fromFile(junk))
        assertFalse(RestoreManifest.file(context.filesDir).exists())
    }
}
