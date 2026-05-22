package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.preferencesDataStore

/**
 * Single DataStore instance for app-wide preferences. Property delegate is the
 * recommended way to declare a DataStore — it manages the file path, schema migration,
 * and prevents accidentally creating multiple stores for the same file.
 */
val Context.forgePreferences: DataStore<Preferences> by preferencesDataStore(name = "forge_settings")

/** Centralised keys so misspellings are caught at compile time. */
object PreferenceKeys {
    val LAST_DELOAD_AT_SESSION_COUNT = intPreferencesKey("last_deload_at_session_count")
    val WELCOMED = androidx.datastore.preferences.core.booleanPreferencesKey("welcomed")
}
