package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/** A factory reset the process died in the middle of is finished at the next boot (audit 2026-09-26). */
@RunWith(RobolectricTestRunner::class)
class InterruptedFactoryResetTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val clock = Clock { 1_700_000_000_000L }
    private val settings = SettingsRepository(context, clock)
    private val marker = File(context.filesDir, ResetRepository.PENDING_FACTORY_RESET)

    private val backupRepo = BackupRepository(
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
        encryption = com.forge.app.security.BackupEncryption(context, com.forge.app.security.FakeDeviceKeyWrapper())
    )

    private val repo = ResetRepository(
        sessionDao = db.sessionDao(),
        trophyDao = db.unlockedTrophyDao(),
        cardioDao = db.cardioDao(),
        moodDao = db.moodDao(),
        suggestionOutcomeDao = db.suggestionOutcomeDao(),
        adviceEventDao = db.adviceEventDao(),
        restEventDao = db.restEventDao(),
        restDayDao = db.restDayDao(),
        coachDao = db.coachDao(),
        nearMissDao = db.trophyNearMissDao(),
        settingsRepo = settings,
        photoRepo = ProgressPhotoRepository(context, db.bodyweightDao()),
        avatarRepo = AvatarRepository(context, settings),
        backupRepo = backupRepo,
        health = HealthConnectManager(context),
        clock = clock,
        db = db,
        context = context
    )

    @After
    fun tearDown() {
        marker.delete()
        // Persisted grants are device state that outlives the Application; never leak one.
        context.contentResolver.persistedUriPermissions.forEach {
            runCatching {
                context.contentResolver.releasePersistableUriPermission(
                    it.uri,
                    android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or
                        android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                )
            }
        }
        db.close()
    }

    /**
     * The boot path runs before the startup gate opens, and Room's open helper and the settings
     * DataStore both wait on that gate: the live reset, called from here, waited for itself and
     * left every later launch on "Preparing Avex…" (2026-09-28 scan). So it must finish the reset
     * from the files alone. This database fails on open in place of the shut gate.
     */
    @Test
    fun aResetKilledAfterTheTableWipeIsFinishedAtBootWithoutOpeningTheStores() = runBlocking {
        // The state the process left: the database file and preferences still saying "onboarded".
        val dbFile = context.getDatabasePath("forge.db").apply { parentFile?.mkdirs(); writeText("tables") }
        val wal = File(dbFile.path + "-wal").apply { writeText("frames") }
        val prefsFile = File(context.filesDir, com.forge.app.RestoreApply.PREFS_PATH)
            .apply { parentFile?.mkdirs(); writeText("onboarded") }
        marker.createNewFile()
        grantFromPicker(backupFolder)

        val shut = shutDatabase()
        try {
            resetRepository(shut).finishInterruptedFactoryReset()
        } finally {
            shut.close()
        }

        assertFalse("the database opens empty", dbFile.exists())
        assertFalse(wal.exists())
        assertFalse("back through onboarding", prefsFile.exists())
        assertFalse(marker.exists())
        assertTrue("the folder grant goes with the setting that named it", heldTrees().isEmpty())
    }

    /**
     * The preference wipe forgets the backup and import folders, but a persisted grant outlives
     * the setting that named it (M-18): the reset has to give the trees back too.
     */
    @Test
    fun aFactoryResetReleasesTheFolderGrants() = runBlocking {
        grantFromPicker(backupFolder)
        grantFromPicker(importFolder)

        repo.factoryReset()

        assertTrue(heldTrees().isEmpty())
    }

    private val backupFolder: android.net.Uri =
        android.net.Uri.parse("content://com.android.externalstorage.documents/tree/primary%3ABackups")
    private val importFolder: android.net.Uri =
        android.net.Uri.parse("content://com.android.externalstorage.documents/tree/primary%3AImports")

    private fun heldTrees(): List<android.net.Uri> =
        context.contentResolver.persistedUriPermissions.map { it.uri }

    /** What the folder picker's result leaves behind. */
    private fun grantFromPicker(uri: android.net.Uri) {
        context.contentResolver.takePersistableUriPermission(
            uri,
            android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or
                android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        )
    }

    /** Stands in for the shut startup gate: opening this database fails the test. */
    private fun shutDatabase(): ForgeDatabase =
        androidx.room.Room.inMemoryDatabaseBuilder(context, ForgeDatabase::class.java)
            .openHelperFactory(object : androidx.sqlite.db.SupportSQLiteOpenHelper.Factory {
                override fun create(
                    configuration: androidx.sqlite.db.SupportSQLiteOpenHelper.Configuration
                ): androidx.sqlite.db.SupportSQLiteOpenHelper {
                    val helper = androidx.sqlite.db.framework.FrameworkSQLiteOpenHelperFactory().create(configuration)
                    return object : androidx.sqlite.db.SupportSQLiteOpenHelper by helper {
                        override val writableDatabase: androidx.sqlite.db.SupportSQLiteDatabase
                            get() = error("the boot path opened the database before the startup gate")
                        override val readableDatabase: androidx.sqlite.db.SupportSQLiteDatabase
                            get() = error("the boot path opened the database before the startup gate")
                    }
                }
            })
            .build()

    private fun resetRepository(database: ForgeDatabase) = ResetRepository(
        sessionDao = database.sessionDao(),
        trophyDao = database.unlockedTrophyDao(),
        cardioDao = database.cardioDao(),
        moodDao = database.moodDao(),
        suggestionOutcomeDao = database.suggestionOutcomeDao(),
        adviceEventDao = database.adviceEventDao(),
        restEventDao = database.restEventDao(),
        restDayDao = database.restDayDao(),
        coachDao = database.coachDao(),
        nearMissDao = database.trophyNearMissDao(),
        settingsRepo = settings,
        photoRepo = ProgressPhotoRepository(context, database.bodyweightDao()),
        avatarRepo = AvatarRepository(context, settings),
        backupRepo = backupRepo,
        health = HealthConnectManager(context),
        clock = clock,
        db = database,
        context = context
    )

    /**
     * "Deletes ALL data" has to mean the copies too (2026-09-26 audit, D2). The weekly ZIP holds the
     * whole database, the preferences and every photo, and it used to survive the reset alongside
     * exports, crash logs and a staged restore that the next boot would have swapped back in.
     */
    @Test
    fun aFactoryResetErasesTheBackupExportsCrashLogsAndAStagedRestore() = runBlocking {
        val files = context.filesDir
        val left = listOf(
            File(files, "forge_auto_backup.zip"),
            File(files, "forge_auto_backup.zip.tmp"),
            File(files, "forge_auto_backup.json"),
            File(files, "auto_backup_failed"),
            File(files, "manual_backup_done"),
            File(files, "exports/avex_sessions.csv"),
            File(files, "crashes/crash_1.txt"),
            File(files, "pending_restore.db"),
            File(files, "pending_restore_prefs.pb"),
            File(files, "pending_restore_avatar.jpg"),
            File(files, "pending_restore_photos/p1.jpg"),
            File(files, com.forge.app.RestoreManifest.NAME),
            File(context.cacheDir, "forge_snapshot_1.db"),
            File(context.cacheDir, "forge_restore_in_1")
        )
        left.forEach { it.parentFile?.mkdirs(); it.writeText("old data") }
        val unrelatedCache = File(context.cacheDir, "cap_1.jpg").apply { writeText("camera temp") }

        repo.factoryReset()

        left.forEach { assertFalse("${it.name} survived the reset", it.exists()) }
        assertFalse(File(files, "exports").exists())
        assertFalse(File(files, "crashes").exists())
        assertFalse("nothing staged for the next boot to apply", com.forge.app.RestoreManifest.anyPending(files))
        assertFalse("Settings stops offering a restore", backupRepo.hasAnyBackup())
        assertFalse("the reset finished, so no marker", marker.exists())
        // The sweep is scoped to backup scratch; other cache files are not its business.
        assertTrue(unrelatedCache.exists())
        unrelatedCache.delete()
        Unit
    }

    @Test
    fun anAutoBackupRacingAResetDoesNotLandInTheSlot() = runBlocking {
        marker.createNewFile()

        val refused = runCatching { backupRepo.autoBackup() }

        // Refused by the reset guard specifically, not by some earlier failure in the snapshot.
        assertEquals("factory reset in progress", refused.exceptionOrNull()?.message)
        assertFalse(File(context.filesDir, "forge_auto_backup.zip").exists())
        assertFalse(File(context.filesDir, "forge_auto_backup.zip.tmp").exists())
    }

    @Test
    fun withNoMarkerTheBootTouchesNothing() = runBlocking {
        db.sessionDao().insert(session())

        repo.finishInterruptedFactoryReset()

        assertEquals(1, db.sessionDao().allFinished().size)
    }
}
