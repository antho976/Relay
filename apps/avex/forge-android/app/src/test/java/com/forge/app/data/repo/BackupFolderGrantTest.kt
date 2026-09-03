package com.forge.app.data.repo

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.datastore.preferences.core.edit
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.prefs.PreferenceKeys
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.prefs.forgePreferences
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
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
        grants = PersistedTreeGrants(context, settings),
        db = db,
        clock = clock
    )

    /**
     * Both halves of the shared state this class mutates, reset before every test.
     *
     * `forgePreferences` is a file-level property delegate, so one DataStore file is shared by
     * every test in the JVM, and `persistedUriPermissions` is real device state that outlives an
     * Application. Teardown closed the database and nothing else, so a folder connected by one test
     * was still connected — in the preference AND as a held grant — for the next, and
     * `aTreeTheImporterIsStillPointingAtIsLeftAlone` failed on a tree it never granted.
     */
    @Before
    fun setUp(): Unit = runBlocking {
        context.contentResolver.persistedUriPermissions.forEach {
            runCatching {
                context.contentResolver.releasePersistableUriPermission(
                    it.uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                )
            }
        }
        context.forgePreferences.edit {
            it.remove(PreferenceKeys.BACKUP_FOLDER_URI)
            it.remove(PreferenceKeys.IMPORT_FOLDER_URI)
        }
    }

    private val importRepo = com.forge.app.data.importer.WorkoutImportRepository(
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

    /** A grants layer that can refuse a take, which Robolectric's resolver never does. */
    private class RefusingGrants(context: Context, settings: SettingsRepository) :
        PersistedTreeGrants(context, settings) {
        var refuse = false
        override suspend fun take(treeUri: Uri, write: Boolean): Boolean =
            if (refuse) false else super.take(treeUri, write)
    }

    private val refusingGrants = RefusingGrants(context, settings)

    private val refusingRepo = BackupRepository(
        context = context,
        sessionDao = db.sessionDao(),
        loggedExerciseDao = db.loggedExerciseDao(),
        loggedSetDao = db.loggedSetDao(),
        cardioDao = db.cardioDao(),
        coachGoalDao = db.coachGoalDao(),
        settingsRepo = settings,
        photoRepo = ProgressPhotoRepository(context, db.bodyweightDao()),
        avatarRepo = AvatarRepository(context, settings),
        grants = refusingGrants,
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

    // ── The two ways the first pass still leaked, and the failure it swallowed ──

    /**
     * The other end of the shared tree. Removing backup correctly left A for import; CHANGING
     * import to B did not release A, because the import path released nothing at all. No setting
     * then named A and Avex kept read access to it indefinitely — the same invisible retained grant
     * "Remove folder" was fixed for, reached by replacing instead of removing.
     */
    @Test
    fun aSharedTreeIsGivenUpOnceTheLastSettingLetsGoOfIt() = runTest {
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)
        importRepo.rememberFolder(folderA)

        repo.forgetBackupFolder()
        assertEquals("import still points at it", listOf(folderA), heldTrees())

        grantFromPicker(folderB)
        importRepo.rememberFolder(folderB)

        assertEquals("and now nothing does", listOf(folderB), heldTrees())
        assertEquals(folderB.toString(), settings.importFolderUri.first())
    }

    /** And the reverse pairing: import lets go first, backup keeps the tree it still names. */
    @Test
    fun changingTheImportFolderLeavesABackupTreeAlone() = runTest {
        grantFromPicker(folderA)
        repo.rememberBackupFolder(folderA)
        importRepo.rememberFolder(folderA)

        grantFromPicker(folderB)
        importRepo.rememberFolder(folderB)

        assertEquals(
            "backup still points at A, so A stays",
            listOf(folderA, folderB), heldTrees().sortedBy { it.toString() }
        )
    }

    /**
     * A grant that could not be taken is not a connected folder.
     *
     * The take was best-effort and its failure swallowed: the preference moved to the new folder
     * and the old grant was released, so once the picker's own transient permission expired Avex
     * could open NEITHER — and the backup that ran immediately afterwards succeeded on that
     * transient grant, so the first sign of it was a weekly job silently writing nowhere.
     *
     * Robolectric's content resolver accepts every persistable take, so the refusal is injected.
     */
    @Test
    fun aFolderWhoseGrantCannotBeTakenReplacesNothing() = runTest {
        grantFromPicker(folderA)
        assertTrue(repo.rememberBackupFolder(folderA))

        refusingGrants.refuse = true
        assertFalse("not connected", refusingRepo.rememberBackupFolder(folderB))

        assertEquals("the working folder is still the one on file", folderA.toString(), settings.backupFolderUri.first())
        assertEquals("and Avex still has access to it", listOf(folderA), heldTrees())
    }
}
