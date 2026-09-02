package com.forge.app.data.repo

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * M-18: what Avex can still reach after "Remove folder", and after a folder is replaced.
 *
 * A persisted URI grant lives until it is revoked or released — nothing expires it. Avex cleared
 * only the DataStore uri, so removing a backup folder reported that it was no longer connected
 * while the app kept read and write access to that tree across reboots, and every replacement
 * added another grant that was never given up. The visible connected-folder state has to be the
 * whole truth about what the app can open.
 */
@RunWith(RobolectricTestRunner::class)
class BackupFolderGrantTest {

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
        db = db,
        clock = clock
    )

    @After
    fun tearDown() = db.close()

    private val folderA: Uri = Uri.parse("content://com.android.externalstorage.documents/tree/primary%3ABackupsA")
    private val folderB: Uri = Uri.parse("content://com.android.externalstorage.documents/tree/primary%3ABackupsB")

    private fun heldTrees(): List<Uri> =
        context.contentResolver.persistedUriPermissions.map { it.uri }

    /** What the picker's result would leave behind before the repository is told about it. */
    private fun grantFromPicker(uri: Uri) {
        context.contentResolver.takePersistableUriPermission(
            uri, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        )
    }

    @Test
    fun connectingAFolderHoldsExactlyThatOne() = runTest {
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)

        assertEquals(listOf(folderA), heldTrees())
        assertEquals(folderA.toString(), settings.backupFolderUri.first())
    }

    @Test
    fun removingTheFolderGivesUpItsAccessToo() = runTest {
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)

        repo.forgetBackupFolder()

        assertNull("the preference is cleared", settings.backupFolderUri.first())
        assertTrue("and so is the grant behind it", heldTrees().isEmpty())
    }

    @Test
    fun replacingTheFolderDoesNotAccumulateGrants() = runTest {
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)

        grantFromPicker(folderB)
        repo.rememberBackupFolder(folderB)

        assertEquals("only the folder the app is pointing at", listOf(folderB), heldTrees())
        assertEquals(folderB.toString(), settings.backupFolderUri.first())
    }

    @Test
    fun rePickingTheSameFolderKeepsIt() = runTest {
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)
        repo.rememberBackupFolder(folderA)

        assertEquals(listOf(folderA), heldTrees())
        assertEquals(folderA.toString(), settings.backupFolderUri.first())
    }

    @Test
    fun aTreeTheImporterIsStillPointingAtIsLeftAlone() = runTest {
        // Downloads is the obvious folder to pick for both. The importer holds its own read grant
        // on the same tree under its own setting, and removing the BACKUP folder must not take the
        // folder scan with it.
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)
        settings.setImportFolderUri(folderA.toString())

        repo.forgetBackupFolder()

        assertNull("the backup destination is gone", settings.backupFolderUri.first())
        assertEquals("but the importer's tree is not", listOf(folderA), heldTrees())
    }

    @Test
    fun removingWhenNothingWasConnectedIsHarmless() = runTest {
        repo.forgetBackupFolder()

        assertNull(settings.backupFolderUri.first())
        assertTrue(heldTrees().isEmpty())
    }
}
