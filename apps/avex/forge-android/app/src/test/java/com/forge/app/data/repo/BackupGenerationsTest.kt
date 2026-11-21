package com.forge.app.data.repo

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.crypto.BackupCrypto
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.loggedExercise
import com.forge.app.data.db.loggedSet
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
 * The auto-backup keeps several copies, not one: a damaged or unwanted newest copy must leave an
 * older one to fall back on, and rapid "Back up now" taps must not push the older ones out.
 */
@RunWith(RobolectricTestRunner::class)
class BackupGenerationsTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val clock = Clock { 1_700_000_000_000L }
    private val settings = SettingsRepository(context, clock)
    private val encryption = BackupEncryption(context, FakeDeviceKeyWrapper())
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
        encryption = encryption
    )

    private val day = 24L * 60 * 60 * 1000
    private fun slot(g: Int) = File(context.filesDir, BackupRepository.generationName(g))
    private val pendingDb get() = File(context.filesDir, "pending_restore.db")

    @After
    fun tearDown() = db.close()

    private suspend fun logWorkout(startedAt: Long) {
        val sessionId = db.sessionDao().insert(session(startedAt = startedAt, finishedAt = startedAt + 3_600_000L))
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = "bench"))
        db.loggedSetDao().insert(loggedSet(loggedExerciseId = exId, weightLb = 185.0, reps = 5))
    }

    /** Make the newest copy look [daysOld] days old, as if the weekly job had run back then. */
    private fun ageNewest(daysOld: Int) {
        slot(0).setLastModified(clock.nowMs() - daysOld * day)
    }

    private fun stagedSessionCount(): Int =
        SQLiteDatabase.openDatabase(pendingDb.path, null, SQLiteDatabase.OPEN_READONLY).use { database ->
            database.rawQuery("SELECT COUNT(*) FROM session", null).use { it.moveToFirst(); it.getInt(0) }
        }

    @Test
    fun spacedBackupsKeepTheNewestThreeCopies() = runTest {
        for (week in 1..4) {
            logWorkout(startedAt = 1_600_000_000_000L + week * day)
            repo.autoBackup()
            ageNewest(daysOld = 7)
        }
        assertTrue(slot(0).exists())
        assertTrue(slot(1).exists())
        assertTrue(slot(2).exists())
        assertFalse(File(context.filesDir, "forge_auto_backup.3.zip").exists())
        assertEquals(BackupRepository.BACKUP_GENERATIONS, repo.autoBackupCopies().size)
    }

    @Test
    fun backupsWithinADayRefreshTheNewestInsteadOfRotating() = runTest {
        logWorkout(startedAt = 1_600_000_000_000L)
        repo.autoBackup()
        ageNewest(daysOld = 7)
        repo.autoBackup() // a week later: rotates
        repo.autoBackup() // "Back up now", same day: refreshes slot 0 only
        repo.autoBackup()
        assertEquals(2, repo.autoBackupCopies().size)
        assertFalse(slot(2).exists())
    }

    @Test
    fun anOlderCopyRestoresWhatItHeld() = runTest {
        logWorkout(startedAt = 1_600_000_000_000L)
        repo.autoBackup()
        ageNewest(daysOld = 7)
        logWorkout(startedAt = 1_600_000_000_000L + 7 * day)
        repo.autoBackup()

        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromAutoBackup(generation = 1))
        assertEquals("the older copy holds one workout", 1, stagedSessionCount())
        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromAutoBackup(generation = 0))
        assertEquals("the newest holds both", 2, stagedSessionCount())
    }

    @Test
    fun aDamagedNewestCopyLeavesTheOlderOneRestorable() = runTest {
        logWorkout(startedAt = 1_600_000_000_000L)
        repo.autoBackup()
        ageNewest(daysOld = 7)
        repo.autoBackup()

        val bytes = slot(0).readBytes()
        bytes.fill(0, bytes.size / 3, bytes.size / 3 + 64)
        slot(0).writeBytes(bytes)

        assertTrue(repo.restoreFromAutoBackup(generation = 0) != BackupRepository.RestoreOutcome.SUCCESS)
        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromAutoBackup(generation = 1))
    }

    @Test
    fun passwordProtectedCopiesRotateAndRestoreToo() = runTest {
        encryption.setPassword("deadlift doubles".toCharArray())
        logWorkout(startedAt = 1_600_000_000_000L)
        repo.autoBackup()
        ageNewest(daysOld = 7)
        repo.autoBackup()

        assertTrue(BackupCrypto.isEncrypted(slot(0)))
        assertTrue(BackupCrypto.isEncrypted(slot(1)))
        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromAutoBackup(generation = 1))
    }

    @Test
    fun aMissingGenerationIsReportedNotGuessed() = runTest {
        logWorkout(startedAt = 1_600_000_000_000L)
        repo.autoBackup()
        assertEquals(BackupRepository.RestoreOutcome.NO_BACKUP_FILE, repo.restoreFromAutoBackup(generation = 2))
        assertEquals(BackupRepository.RestoreOutcome.NO_BACKUP_FILE, repo.restoreFromAutoBackup(generation = 9))
    }

    @Test
    fun aFactoryResetRemovesEveryCopy() = runTest {
        logWorkout(startedAt = 1_600_000_000_000L)
        repo.autoBackup()
        ageNewest(daysOld = 7)
        repo.autoBackup()
        repo.deleteLocalCopies()
        assertTrue(repo.autoBackupCopies().isEmpty())
    }
}
