package com.forge.app.data.repo

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.RestoreManifest
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
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Password-protected backups through the real repository: what a protected backup looks like on
 * disk, that it opens on the phone that made it without typing, that anywhere else it needs the
 * password, and that a wrong or missing password stages nothing.
 */
@RunWith(RobolectricTestRunner::class)
class EncryptedBackupRestoreTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val clock = Clock { 1_700_000_000_000L }
    private val settings = SettingsRepository(context, clock)
    private val wrapper = FakeDeviceKeyWrapper()
    private val encryption = BackupEncryption(context, wrapper)
    private val repo = repoWith(encryption)

    private fun repoWith(enc: BackupEncryption) = BackupRepository(
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
        encryption = enc
    )

    private val password = "squat day every day".toCharArray()
    private val pendingDb get() = File(context.filesDir, "pending_restore.db")

    @After
    fun tearDown() = db.close()

    private suspend fun seedOneWorkout() {
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = "bench"))
        db.loggedSetDao().insert(loggedSet(loggedExerciseId = exId, weightLb = 225.0, reps = 3))
    }

    private fun stagedSessionCount(): Int =
        SQLiteDatabase.openDatabase(pendingDb.path, null, SQLiteDatabase.OPEN_READONLY).use { database ->
            database.rawQuery("SELECT COUNT(*) FROM session", null).use { it.moveToFirst(); it.getInt(0) }
        }

    private fun assertNothingStaged() {
        assertFalse("nothing may be staged", pendingDb.exists())
        assertFalse(RestoreManifest.verify(context.filesDir))
    }

    /** A protected backup, made on "another phone": the key this test's repo holds is gone. */
    private suspend fun backupFromAnotherPhone(): File {
        seedOneWorkout()
        encryption.setPassword(password.copyOf())
        val archive = temporaryFolder.newFile("protected.zip")
        repo.backupToUri(Uri.fromFile(archive))
        encryption.clear()
        return archive
    }

    @Test
    fun aProtectedBackupIsNeitherAZipNorReadable() = runTest {
        seedOneWorkout()
        encryption.setPassword(password.copyOf())
        val archive = temporaryFolder.newFile("protected.zip")
        repo.backupToUri(Uri.fromFile(archive))

        assertTrue(BackupCrypto.isEncrypted(archive))
        val head = archive.readBytes().copyOf(4)
        assertFalse("must not start with the ZIP signature", head.contentEquals(byteArrayOf(0x50, 0x4B, 0x03, 0x04)))
        assertFalse("the entry name must not be visible", String(archive.readBytes(), Charsets.ISO_8859_1).contains("database.db"))
    }

    @Test
    fun thePhoneThatMadeItRestoresItWithoutAskingForThePassword() = runTest {
        seedOneWorkout()
        encryption.setPassword(password.copyOf())
        val archive = temporaryFolder.newFile("protected.zip")
        repo.backupToUri(Uri.fromFile(archive))

        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromUri(Uri.fromFile(archive)))
        assertEquals(1, stagedSessionCount())
    }

    @Test
    fun elsewhereItAsksForThePasswordAndStagesNothing() = runTest {
        val archive = backupFromAnotherPhone()
        assertEquals(BackupRepository.RestoreOutcome.NEEDS_PASSWORD, repo.restoreFromUri(Uri.fromFile(archive)))
        assertNothingStaged()
    }

    @Test
    fun aWrongPasswordIsSaidSoAndStagesNothing() = runTest {
        val archive = backupFromAnotherPhone()
        val outcome = repo.restoreFromUri(Uri.fromFile(archive), "squat day every day?".toCharArray())
        assertEquals(BackupRepository.RestoreOutcome.WRONG_PASSWORD, outcome)
        assertNothingStaged()
    }

    @Test
    fun theRightPasswordRestoresItOnAnotherPhone() = runTest {
        val archive = backupFromAnotherPhone()
        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromUri(Uri.fromFile(archive), password.copyOf()))
        assertEquals(1, stagedSessionCount())
        assertTrue(RestoreManifest.verify(context.filesDir))
    }

    @Test
    fun aDamagedProtectedBackupIsCorruptAndStagesNothing() = runTest {
        val archive = backupFromAnotherPhone()
        val bytes = archive.readBytes()
        bytes[bytes.size / 2] = (bytes[bytes.size / 2].toInt() xor 0x01).toByte()
        archive.writeBytes(bytes)
        assertEquals(BackupRepository.RestoreOutcome.CORRUPT, repo.restoreFromUri(Uri.fromFile(archive), password.copyOf()))
        assertNothingStaged()
    }

    @Test
    fun theWeeklySlotIsProtectedAndRestoresOnTheSamePhone() = runTest {
        seedOneWorkout()
        encryption.setPassword(password.copyOf())
        val slot = repo.autoBackup()
        assertTrue(BackupCrypto.isEncrypted(slot))
        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromAutoBackup())
        assertEquals(1, stagedSessionCount())
    }

    @Test
    fun plainBackupsFromBeforeThePasswordStillRestore() = runTest {
        seedOneWorkout()
        val archive = temporaryFolder.newFile("plain.zip")
        repo.backupToUri(Uri.fromFile(archive))
        encryption.setPassword(password.copyOf())

        assertFalse(BackupCrypto.isEncrypted(archive))
        assertEquals(BackupRepository.RestoreOutcome.SUCCESS, repo.restoreFromUri(Uri.fromFile(archive)))
    }

    @Test
    fun anUnreadableKeyFailsTheBackupInsteadOfWritingItInTheClear() = runTest {
        seedOneWorkout()
        encryption.setPassword(password.copyOf())
        wrapper.destroy() // the keystore entry is gone; the key file is still there

        assertEquals(BackupEncryption.State.Unavailable, encryption.state())
        val archive = temporaryFolder.newFile("never.zip")
        try {
            repo.backupToUri(Uri.fromFile(archive))
            fail("the backup must not go ahead")
        } catch (e: BackupRepository.BackupPasswordUnavailableException) {
            // expected
        }
        assertEquals("nothing may be written", 0L, archive.length())
        try {
            repo.autoBackup()
            fail("the weekly backup must not go ahead either")
        } catch (e: BackupRepository.BackupPasswordUnavailableException) {
            // expected
        }
        assertFalse(File(context.filesDir, "forge_auto_backup.zip").exists())
    }

    @Test
    fun removingThePasswordMakesLaterBackupsPlain() = runTest {
        seedOneWorkout()
        encryption.setPassword(password.copyOf())
        encryption.clear()
        assertEquals(BackupEncryption.State.Off, encryption.state())
        val archive = temporaryFolder.newFile("plain.zip")
        repo.backupToUri(Uri.fromFile(archive))
        assertFalse(BackupCrypto.isEncrypted(archive))
    }

    @Test
    fun aFactoryResetForgetsThePassword() = runTest {
        encryption.setPassword(password.copyOf())
        assertTrue(encryption.isConfigured())
        repo.deleteLocalCopies()
        assertFalse(encryption.isConfigured())
        assertTrue(wrapper.destroyed)
    }
}
