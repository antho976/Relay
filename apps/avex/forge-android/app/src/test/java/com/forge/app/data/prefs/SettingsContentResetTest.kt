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
        repo.resetSettingsOnly()
        val after = context.forgePreferences.data.first()
        keys.forEach { assertEquals("saved-${it.name}", after[it]) }
    }
}
