package com.tally.app.data.prefs

import android.content.Context
import androidx.compose.runtime.Immutable
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.tally.core.BudgetPeriod
import com.tally.core.MoneyFormatter
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
    /** Relay's ground: "matte" (the default), "dark" or "oled" (ui/theme/Theme.kt). */
    val palette: String = "matte",
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

/** One setting the PC sync carries, as the PC stores it: a string [value] and when it changed. */
@Immutable
data class SyncedSetting(val key: String, val value: String, val updatedAt: Long) {
    companion object {
        const val CURRENCY = "currency"
        const val MONTH_START_DAY = "month_start_day"
        const val WEEK_STARTS_MONDAY = "week_starts_monday"
    }
}

@Immutable
data class SyncedSettings(val all: List<SyncedSetting>)

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
        val palette = stringPreferencesKey("palette")
        val backupAuto = booleanPreferencesKey("backup_auto")
        val backupFolder = stringPreferencesKey("backup_folder")
        val backupLastAt = longPreferencesKey("backup_last_at")
        val backupFailed = booleanPreferencesKey("backup_failed")
        val backupFolderFailed = booleanPreferencesKey("backup_folder_failed")
        // When each synced setting last changed, for the PC sync's newest-wins (epoch millis).
        val currencyAt = longPreferencesKey("currency_at")
        val monthStartAt = longPreferencesKey("month_start_day_at")
        val weekMondayAt = longPreferencesKey("week_starts_monday_at")
        // The last PC sync's watermark: no synced setting is stamped under it (raiseSyncFloor).
        val syncFloor = longPreferencesKey("sync_floor")
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
                palette = p[Keys.palette] ?: if (p[Keys.amoled] == true) "oled" else "matte",
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

    suspend fun setCurrency(code: String) = store.edit {
        it[Keys.currency] = code
        it.stamp(Keys.currencyAt)
    }
    suspend fun setMonthStartDay(day: Int) = store.edit {
        it[Keys.monthStart] = day.coerceIn(1, BudgetPeriod.MAX_START_DAY)
        it.stamp(Keys.monthStartAt)
    }
    suspend fun setWeekStartsMonday(monday: Boolean) = store.edit {
        it[Keys.weekMonday] = monday
        it.stamp(Keys.weekMondayAt)
    }

    /**
     * A change here is newer than the one it replaces even when this phone's clock runs behind
     * the PC's that stamped it: `max(now, previous + 1)`, as every ledger row is stamped. Nor is
     * it older than the last sync's watermark, or that sync's successor would not send it.
     */
    private fun MutablePreferences.stamp(key: Preferences.Key<Long>) {
        this[key] = maxOf(System.currentTimeMillis(), (this[key] ?: 0L) + 1, this[Keys.syncFloor] ?: 0L)
    }

    /**
     * Holds every later stamp at or above [watermark], the one a PC sync just took (PcSync): a
     * setting changed after it, with the clock set back since, is still sent by the next sync.
     * It never comes down; the ledger's rows have the same floor (LedgerSync.mark).
     */
    suspend fun raiseSyncFloor(watermark: Long) = store.edit {
        if (watermark > (it[Keys.syncFloor] ?: 0L)) it[Keys.syncFloor] = watermark
    }
    suspend fun setAccent(accent: Accent) = store.edit { it[Keys.accent] = accent.key }
    suspend fun setAccentEnabled(enabled: Boolean) = store.edit { it[Keys.accentEnabled] = enabled }
    suspend fun setAmoled(amoled: Boolean) = store.edit { it[Keys.amoled] = amoled }
    suspend fun setPalette(name: String) = store.edit { it[Keys.palette] = name }
    suspend fun setOnboarded(done: Boolean) = store.edit { it[Keys.onboarded] = done }
    suspend fun setDefaultAccount(id: Long) = store.edit { it[Keys.defaultAccount] = id }
    suspend fun setSampleLoaded(loaded: Boolean) = store.edit { it[Keys.sampleLoaded] = loaded }

    /** Erase-all keeps the look (accent, theme) and forgets everything about the money. */
    suspend fun resetData() = store.edit {
        it.remove(Keys.onboarded)
        it.remove(Keys.defaultAccount)
        it.remove(Keys.sampleLoaded)
        it.remove(Keys.monthStart)
        // Back to the 1st is a change too, so the paired PC hears about it.
        it.stamp(Keys.monthStartAt)
    }

    /** The settings the PC sync carries, each with when it last changed (0: never set here). */
    suspend fun synced(): SyncedSettings {
        val p = store.data.catch { if (it is IOException) emit(emptyPreferences()) else throw it }.first()
        val s = current()
        return SyncedSettings(
            listOf(
                SyncedSetting(SyncedSetting.CURRENCY, s.currency, p[Keys.currencyAt] ?: 0L),
                SyncedSetting(SyncedSetting.MONTH_START_DAY, s.monthStartDay.toString(), p[Keys.monthStartAt] ?: 0L),
                SyncedSetting(SyncedSetting.WEEK_STARTS_MONDAY, if (s.weekStartsMonday) "1" else "0", p[Keys.weekMondayAt] ?: 0L),
            )
        )
    }

    /**
     * Takes a setting from the PC when its change is newer than this phone's, keeping the PC's
     * change time; [force] takes it whatever the stamps, with the PC's whole ledger. True when it
     * was taken. Values that do not read are refused, not guessed.
     *
     * A currency is only ever relabelled here, never converted. The PC changes it only between
     * currencies with the same decimals, so one with other decimals is refused and the phone's
     * stands, unless the PC's whole ledger comes with it (its amounts are already in it).
     */
    suspend fun applySynced(key: String, value: String, at: Long, force: Boolean = false): Boolean {
        var taken = false
        store.edit { p ->
            fun newer(stamp: Preferences.Key<Long>) = force || (p[stamp] ?: 0L) < at
            when (key) {
                SyncedSetting.CURRENCY -> {
                    val incoming = runCatching { MoneyFormatter(value, Locale.ROOT).fractionDigits }.getOrNull()
                    val mine = MoneyFormatter(p[Keys.currency] ?: Settings.defaultCurrency(), Locale.ROOT).fractionDigits
                    if (newer(Keys.currencyAt) && incoming != null && (force || incoming == mine)) {
                        p[Keys.currency] = value
                        p[Keys.currencyAt] = at
                        taken = true
                    }
                }
                SyncedSetting.MONTH_START_DAY -> {
                    val day = value.trim().toIntOrNull()?.takeIf { it in 1..BudgetPeriod.MAX_START_DAY }
                    if (newer(Keys.monthStartAt) && day != null) {
                        p[Keys.monthStart] = day
                        p[Keys.monthStartAt] = at
                        taken = true
                    }
                }
                SyncedSetting.WEEK_STARTS_MONDAY -> {
                    val monday = when (value.trim().lowercase()) {
                        "1", "true" -> true
                        "0", "false" -> false
                        else -> null
                    }
                    if (newer(Keys.weekMondayAt) && monday != null) {
                        p[Keys.weekMonday] = monday
                        p[Keys.weekMondayAt] = at
                        taken = true
                    }
                }
            }
        }
        return taken
    }
}
