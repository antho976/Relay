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
    private val marker = File(context.filesDir, "factory_reset_pending")

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

    @Test
    fun withNoMarkerTheBootTouchesNothing() = runBlocking {
        db.sessionDao().insert(session())

        repo.finishInterruptedFactoryReset()

        assertEquals(1, db.sessionDao().allFinished().size)
    }
}
