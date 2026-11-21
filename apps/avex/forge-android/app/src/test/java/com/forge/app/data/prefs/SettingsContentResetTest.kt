package com.forge.app.data.prefs

import android.app.Application
import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class SettingsContentResetTest {
    @Test fun `settings reset retains user content stored beside preferences`() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val repo = SettingsRepository(context, Clock { 1000 })
        val keys = listOf(PreferenceKeys.CUSTOM_EXERCISES, PreferenceKeys.CUSTOM_CARDIO_TYPES, PreferenceKeys.FREESTYLE_DRAFT)
        context.forgePreferences.edit { prefs -> keys.forEach { prefs[it] = "saved-${it.name}" } }
        repo.setGalleryLockEnabled(true)
        repo.resetSettingsOnly()
        assertTrue(repo.galleryLockEnabled.first())
        val after = context.forgePreferences.data.first()
        keys.forEach { assertEquals("saved-${it.name}", after[it]) }
    }

    /** Audit 2026-09-26: the reset kept 16 hand-picked keys and lost the state stored beside them. */
    @Test fun `settings reset keeps latches, program state and schedule but resets real settings`() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val repo = SettingsRepository(context, Clock { 1000 })
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.DELOAD_WEEK_START_MS] = 777L
            prefs[PreferenceKeys.PROGRAM_GENERATION_SEED] = 42L
            prefs[PreferenceKeys.SHOWN_MILESTONES] = setOf("first_pr")
            prefs[PreferenceKeys.SCHEDULE_WEEKLY] = "upper-a,,lower-a,,,,"
            prefs[PreferenceKeys.LIKED_EXERCISES] = setOf("bench")
            prefs[PreferenceKeys.warmupKey("upper-a")] = "band pull-aparts"
            prefs[PreferenceKeys.ACCENT_COLOR_HEX] = "#EF4444"
            prefs[PreferenceKeys.dayColorKey("upper-a")] = "#FF5733"
        }
        repo.resetSettingsOnly()
        val after = context.forgePreferences.data.first()
        assertEquals(777L, after[PreferenceKeys.DELOAD_WEEK_START_MS])
        assertEquals(42L, after[PreferenceKeys.PROGRAM_GENERATION_SEED])
        assertEquals(setOf("first_pr"), after[PreferenceKeys.SHOWN_MILESTONES])
        assertEquals("upper-a,,lower-a,,,,", after[PreferenceKeys.SCHEDULE_WEEKLY])
        assertEquals(setOf("bench"), after[PreferenceKeys.LIKED_EXERCISES])
        assertEquals("band pull-aparts", after[PreferenceKeys.warmupKey("upper-a")])
        assertNull(after[PreferenceKeys.ACCENT_COLOR_HEX])
        assertNull(after[PreferenceKeys.dayColorKey("upper-a")])
    }
}
