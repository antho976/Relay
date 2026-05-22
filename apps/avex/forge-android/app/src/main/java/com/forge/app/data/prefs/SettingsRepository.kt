package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Small typed wrapper over the app's DataStore. Phase 4 only uses [lastDeloadAtSessionCount];
 * future phases (welcome flow, rest-timer default duration, notification toggle, etc.)
 * add Flow + setter pairs alongside.
 */
@Singleton
class SettingsRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    val lastDeloadAtSessionCount: Flow<Int> = context.forgePreferences.data
        .map { prefs -> prefs[PreferenceKeys.LAST_DELOAD_AT_SESSION_COUNT] ?: 0 }

    suspend fun setLastDeloadAtSessionCount(count: Int) {
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.LAST_DELOAD_AT_SESSION_COUNT] = count
        }
    }

    val welcomed: Flow<Boolean> = context.forgePreferences.data
        .map { prefs -> prefs[PreferenceKeys.WELCOMED] ?: false }

    suspend fun setWelcomed(value: Boolean) {
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.WELCOMED] = value
        }
    }
}
