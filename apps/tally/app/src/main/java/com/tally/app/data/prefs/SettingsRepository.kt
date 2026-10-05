package com.tally.app.data.prefs

import android.content.Context
import androidx.compose.runtime.Immutable
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.tally.core.BudgetPeriod
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import java.io.IOException
import java.util.Currency
import java.util.Locale
import javax.inject.Inject
import javax.inject.Singleton

/** The accent choices. [key] is what is stored, so it never changes once shipped. */
enum class Accent(val key: String, val label: String, val argb: Long) {
    EMBER("ember", "Ember", 0xFFD4761F),
    RED("red", "Red", 0xFFE23D3D),
    SAGE("sage", "Sage", 0xFF7FB27A),
    TEAL("teal", "Teal", 0xFF4FA9A0),
    SKY("sky", "Sky", 0xFF6A9FD8),
    IRIS("iris", "Iris", 0xFF8C87D9),
    ROSE("rose", "Rose", 0xFFD9768E),
    GOLD("gold", "Gold", 0xFFD9A441);

    companion object {
        val DEFAULT = EMBER
        fun of(key: String?): Accent = entries.firstOrNull { it.key == key } ?: DEFAULT
    }
}

@Immutable
data class Settings(
    val currency: String,
    val monthStartDay: Int = 1,
    val weekStartsMonday: Boolean = true,
    val accent: Accent = Accent.DEFAULT,
    val accentEnabled: Boolean = true,
    val amoled: Boolean = false,
    val onboarded: Boolean = false,
    val defaultAccountId: Long = 0,
    val sampleLoaded: Boolean = false,
) {
    fun periodFor(date: java.time.LocalDate): BudgetPeriod = BudgetPeriod.containing(date, monthStartDay)

    companion object {
        /** The device's own currency, or CAD where the locale has none (a bare "fr"). */
        fun defaultCurrency(locale: Locale = Locale.getDefault()): String =
            runCatching { Currency.getInstance(locale).currencyCode }.getOrNull() ?: "CAD"
    }
}

/**
 * The automatic backup: on or off, the folder a copy also goes to (a Storage Access Framework
 * tree the owner picked, so it outlives an uninstall), and how the last run went.
 */
@Immutable
data class BackupPrefs(
    val auto: Boolean = true,
    val folderUri: String? = null,
    /** When the last backup was written, in epoch millis; 0 before the first. */
    val lastAt: Long = 0L,
    val lastFailed: Boolean = false,
    /** The folder copy failed while the copy kept on the phone was written. */
    val folderFailed: Boolean = false,
)

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "settings")

@Singleton
class SettingsRepository @Inject constructor(@ApplicationContext context: Context) {

    private val store = context.dataStore

    private object Keys {
        val currency = stringPreferencesKey("currency")
        val monthStart = intPreferencesKey("month_start_day")
        val weekMonday = booleanPreferencesKey("week_starts_monday")
        val accent = stringPreferencesKey("accent")
        val accentEnabled = booleanPreferencesKey("accent_enabled")
        val amoled = booleanPreferencesKey("amoled")
        val onboarded = booleanPreferencesKey("onboarded")
        val defaultAccount = longPreferencesKey("default_account")
        val sampleLoaded = booleanPreferencesKey("sample_loaded")
        val backupAuto = booleanPreferencesKey("backup_auto")
        val backupFolder = stringPreferencesKey("backup_folder")
        val backupLastAt = longPreferencesKey("backup_last_at")
        val backupFailed = booleanPreferencesKey("backup_failed")
        val backupFolderFailed = booleanPreferencesKey("backup_folder_failed")
    }

    val settings: Flow<Settings> = store.data
        // A corrupt or unreadable file reads as defaults instead of crashing the app at launch.
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { p ->
            Settings(
                currency = p[Keys.currency] ?: Settings.defaultCurrency(),
                monthStartDay = (p[Keys.monthStart] ?: 1).coerceIn(1, BudgetPeriod.MAX_START_DAY),
                weekStartsMonday = p[Keys.weekMonday] ?: true,
                accent = Accent.of(p[Keys.accent]),
                accentEnabled = p[Keys.accentEnabled] ?: true,
                amoled = p[Keys.amoled] ?: false,
                onboarded = p[Keys.onboarded] ?: false,
                defaultAccountId = p[Keys.defaultAccount] ?: 0,
                sampleLoaded = p[Keys.sampleLoaded] ?: false,
            )
        }
        .distinctUntilChanged()

    suspend fun current(): Settings = settings.first()

    /** Kept apart from [settings], so a backup run never recomposes the theme at the root. */
    val backup: Flow<BackupPrefs> = store.data
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { p ->
            BackupPrefs(
                auto = p[Keys.backupAuto] ?: true,
                folderUri = p[Keys.backupFolder],
                lastAt = p[Keys.backupLastAt] ?: 0L,
                lastFailed = p[Keys.backupFailed] ?: false,
                folderFailed = p[Keys.backupFolderFailed] ?: false,
            )
        }
        .distinctUntilChanged()

    suspend fun currentBackup(): BackupPrefs = backup.first()

    suspend fun setBackupAuto(on: Boolean) = store.edit { it[Keys.backupAuto] = on }

    suspend fun setBackupFolder(uri: String?) = store.edit {
        if (uri == null) it.remove(Keys.backupFolder) else it[Keys.backupFolder] = uri
        it.remove(Keys.backupFolderFailed)
    }

    /** How a backup run ended: written at [at] (with or without its folder copy), or failed. */
    suspend fun recordBackup(at: Long, failed: Boolean, folderFailed: Boolean) = store.edit {
        if (!failed) it[Keys.backupLastAt] = at
        it[Keys.backupFailed] = failed
        it[Keys.backupFolderFailed] = folderFailed
    }

    suspend fun setCurrency(code: String) = store.edit { it[Keys.currency] = code }
    suspend fun setMonthStartDay(day: Int) = store.edit { it[Keys.monthStart] = day.coerceIn(1, BudgetPeriod.MAX_START_DAY) }
    suspend fun setWeekStartsMonday(monday: Boolean) = store.edit { it[Keys.weekMonday] = monday }
    suspend fun setAccent(accent: Accent) = store.edit { it[Keys.accent] = accent.key }
    suspend fun setAccentEnabled(enabled: Boolean) = store.edit { it[Keys.accentEnabled] = enabled }
    suspend fun setAmoled(amoled: Boolean) = store.edit { it[Keys.amoled] = amoled }
    suspend fun setOnboarded(done: Boolean) = store.edit { it[Keys.onboarded] = done }
    suspend fun setDefaultAccount(id: Long) = store.edit { it[Keys.defaultAccount] = id }
    suspend fun setSampleLoaded(loaded: Boolean) = store.edit { it[Keys.sampleLoaded] = loaded }

    /** Erase-all keeps the look (accent, theme) and forgets everything about the money. */
    suspend fun resetData() = store.edit {
        it.remove(Keys.onboarded)
        it.remove(Keys.defaultAccount)
        it.remove(Keys.sampleLoaded)
        it.remove(Keys.monthStart)
    }
}
