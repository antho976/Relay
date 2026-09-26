package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.flow.first
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
        clock = clock
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
        db.close()
    }

    @Test
    fun aResetKilledAfterTheTableWipeIsFinishedAtBoot() = runBlocking {
        // The state the process left: tables gone, preferences still saying "onboarded".
        settings.completeOnboarding("Sam", com.forge.app.domain.units.WeightUnit.KG, "build_muscle", null)
        marker.createNewFile()

        repo.finishInterruptedFactoryReset()

        assertFalse("back through onboarding", settings.onboardingDone.first())
        assertFalse(marker.exists())
    }

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
