package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import com.forge.app.core.time.Clock
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.map
import java.io.IOException
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The settings sub-pages that support a scoped "reset to defaults" (#544). Each carries EXACTLY the
 * preference keys its page owns, so a reset clears that page (and only that page) back to defaults.
 * Keeping the key list on the enum — the single place that maps a page to its keys — is what stops
 * the drift the old stringly-typed when() invited (it silently dropped USER_SEX from "format").
 */
enum class SettingsSection(val keys: List<Preferences.Key<*>>) {
    APPEARANCE(
        listOf(
            PreferenceKeys.AMOLED_MODE, PreferenceKeys.COMPACT_SET_LOGGING,
            PreferenceKeys.ACCENT_COLOR_HEX, PreferenceKeys.ACCENT_ENABLED, PreferenceKeys.FONT_CHOICE,
            PreferenceKeys.THEMED_LAUNCH_INTRO
            // APP_ICON is deliberately excluded — the enabled activity-alias is the real state, so
            // clearing the pref alone would desync the ringed choice from the on-device icon.
        )
    ),
    FORMAT(
        listOf(
            PreferenceKeys.WEIGHT_UNIT, PreferenceKeys.USE_KG, PreferenceKeys.USER_SEX, PreferenceKeys.DATE_FORMAT,
            PreferenceKeys.TIMEZONE,
            PreferenceKeys.TIME_FORMAT_24H, PreferenceKeys.FIRST_DAY_MONDAY
            // FAVORITE_TIMEZONES is the user's curated star list (data, not a format default), so a
            // scoped "reset Format" must NOT wipe it — it's only cleared by a full settings reset.
        )
    ),
    SESSION(
        listOf(
            PreferenceKeys.HAPTIC_STRENGTH, PreferenceKeys.KEEP_SCREEN_ON,
            PreferenceKeys.REST_COMPOUND_SECONDS,
            PreferenceKeys.REST_ISOLATION_SECONDS, PreferenceKeys.NOTE_TEMPLATES
        )
    ),
    NOTIFICATIONS(
        listOf(
            PreferenceKeys.QUIET_HOURS_ENABLED, PreferenceKeys.QUIET_HOURS_START, PreferenceKeys.QUIET_HOURS_END,
            PreferenceKeys.QUIET_HOURS_SCHEDULE,
            PreferenceKeys.TRAINING_REMINDER_ENABLED, PreferenceKeys.TRAINING_REMINDER_HOUR,
            // The notifications feed (2026-07-27). Clearing these is the ONLY way back for the two
            // one-shot invites: both are dismissed for good, and neither had an un-dismiss before —
            // the old cardio banner's × was a one-way door too. Resetting the section now restores
            // every switch on this page, which is what "reset this section" should have meant.
            PreferenceKeys.DISABLED_NOTICE_KINDS, PreferenceKeys.CARDIO_WEARABLE_HINT_DISMISSED,
            PreferenceKeys.NOTIF_PERM_ASKED
        )
    )
}

/**
 * Small typed wrapper over the app's DataStore. Each setting is a Flow + setter pair.
 * (The old lastDeloadAtSessionCount counter was retired by the adaptation engine's
 * DeloadAdvisor — deload timing now comes from real fatigue signals, and the last deload
 * is read from Session rows, not a pref.)
 */
@Singleton
class SettingsRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val clock: Clock
) {
    /**
     * Every read below goes through this, not through `context.forgePreferences.data` directly.
     *
     * DataStore's `data` throws on a read failure — IOException for an unreadable file,
     * CorruptionException for an unparseable one — and none of the ~100 flows in this file handled
     * it. MainActivity resolves several of them before the first frame, so one bad read propagated
     * out of onCreate as an uncaught exception on EVERY launch, with no in-app recovery: the user's
     * only move was Clear Data, which destroys the Room database and the photos too. The store's
     * ReplaceFileCorruptionHandler covers a corrupt file; this covers the rest, and degrading to
     * defaults is recoverable in a way that a launch loop is not.
     */
    private val allPreferences: Flow<Preferences> = context.forgePreferences.data
        .catch { e -> if (e is IOException) emit(emptyPreferences()) else throw e }

    /**
     * Every derived preference flow in this class goes through here, never through `map` on the raw
     * DataStore flow.
     *
     * `DataStore.data` re-emits the COMPLETE `Preferences` object on every write, whatever key
     * changed. Without a dedupe, all ~100 flows below re-emitted an identical value each time one
     * unrelated key was written — so the Overview's fourteen-stage combine (goal progress, top-lift
     * lookups, weekly-stats formatting, each with database reads behind it) re-ran roughly twice a
     * second while the freestyle logger autosaved its draft, for a preference that screen does not
     * read. Continuous background work during the two moments the app most needs to be responsive.
     */
    private fun <T> pref(read: (Preferences) -> T): Flow<T> =
        allPreferences.map(read).distinctUntilChanged()

    val shownMilestones: Flow<Set<String>> = pref { prefs -> prefs[PreferenceKeys.SHOWN_MILESTONES] ?: emptySet() }

    /** Fire a milestone: it can never fire again, and it waits in the notifications feed until cleared. */
    suspend fun markMilestoneShown(milestoneId: String) {
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.SHOWN_MILESTONES] ?: emptySet()
            prefs[PreferenceKeys.SHOWN_MILESTONES] = current + milestoneId
            val unread = prefs[PreferenceKeys.UNREAD_MILESTONES] ?: emptySet()
            prefs[PreferenceKeys.UNREAD_MILESTONES] = unread + milestoneId
        }
    }

    // ─── Notifications tab ────────────────────────────────────────────────────

    /** Milestones that have fired but are still waiting in the notifications feed. */
    val unreadMilestones: Flow<Set<String>> = pref { prefs -> prefs[PreferenceKeys.UNREAD_MILESTONES] ?: emptySet() }

    /** Clear (or, on undo, re-queue) one milestone's place in the feed. [SHOWN_MILESTONES] is left
     *  alone either way — a cleared milestone must never fire a second time. */
    suspend fun setMilestoneUnread(milestoneId: String, unread: Boolean) {
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.UNREAD_MILESTONES] ?: emptySet()
            prefs[PreferenceKeys.UNREAD_MILESTONES] =
                if (unread) current + milestoneId else current - milestoneId
        }
    }

    /** A one-shot result line waiting in the notifications feed. */
    data class SystemNotice(val id: String, val text: String)

    /** Pending one-shot result lines (leftover session resolved, import finished, backup restored). */
    val systemNotices: Flow<List<SystemNotice>> = pref { prefs ->
            (prefs[PreferenceKeys.SYSTEM_NOTICES] ?: emptySet())
                .mapNotNull { entry ->
                    val id = entry.substringBefore('|', missingDelimiterValue = "")
                    val text = entry.substringAfter('|', missingDelimiterValue = "")
                    if (id.isBlank() || text.isBlank()) null else SystemNotice(id, text)
                }
                // Stored as a Set, so sort for a stable order rather than whatever the set iterates.
                .sortedBy { it.id }
        }

    /** Queue a result line, replacing any earlier one with the same [id] — a second import result
     *  supersedes the first rather than stacking two "here's what came in" rows. */
    suspend fun addSystemNotice(id: String, text: String) =
        context.forgePreferences.edit { prefs ->
            val kept = (prefs[PreferenceKeys.SYSTEM_NOTICES] ?: emptySet())
                .filterNot { it.startsWith("$id|") }
            prefs[PreferenceKeys.SYSTEM_NOTICES] = (kept + "$id|$text").toSet()
        }

    /** Lessons whose feed row the user cleared without reading. Never written to the ledger. */
    val dismissedLessonNotices: Flow<Set<String>> = pref { prefs -> prefs[PreferenceKeys.DISMISSED_LESSON_NOTICES] ?: emptySet() }

    /** Clear (or, on undo, restore) one lesson's place in the feed. */
    suspend fun setLessonNoticeDismissed(lessonId: String, dismissed: Boolean) =
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.DISMISSED_LESSON_NOTICES] ?: emptySet()
            prefs[PreferenceKeys.DISMISSED_LESSON_NOTICES] =
                if (dismissed) current + lessonId else current - lessonId
        }

    /** Lessons whose arrival banner has already played. */
    val announcedLessonNotices: Flow<Set<String>> = pref { prefs -> prefs[PreferenceKeys.ANNOUNCED_LESSON_NOTICES] ?: emptySet() }

    /** Mark banners as played. Additive and never cleared: an announcement happens once, ever. */
    suspend fun markLessonNoticesAnnounced(lessonIds: Set<String>) {
        if (lessonIds.isEmpty()) return
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.ANNOUNCED_LESSON_NOTICES] =
                (prefs[PreferenceKeys.ANNOUNCED_LESSON_NOTICES] ?: emptySet()) + lessonIds
        }
    }

    /** `NoticeKind.key`s switched off — those rows never reach the feed. */
    val disabledNoticeKinds: Flow<Set<String>> = pref { prefs -> prefs[PreferenceKeys.DISABLED_NOTICE_KINDS] ?: emptySet() }

    suspend fun setNoticeKindEnabled(key: String, enabled: Boolean) =
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.DISABLED_NOTICE_KINDS] ?: emptySet()
            prefs[PreferenceKeys.DISABLED_NOTICE_KINDS] =
                if (enabled) current - key else current + key
        }

    suspend fun removeSystemNotice(id: String) =
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.SYSTEM_NOTICES] =
                (prefs[PreferenceKeys.SYSTEM_NOTICES] ?: emptySet()).filterNot { it.startsWith("$id|") }.toSet()
        }

    // ─── Per-day accent color (#65) ───────────────────────────────────────────

    suspend fun setDayColor(dayKey: String, hex: String?) =
        context.forgePreferences.edit { prefs ->
            if (hex == null) prefs.remove(PreferenceKeys.dayColorKey(dayKey))
            else prefs[PreferenceKeys.dayColorKey(dayKey)] = hex
        }

    fun observeAllDayColors(): kotlinx.coroutines.flow.Flow<Map<String, String>> =
        pref { prefs ->
            com.forge.app.program.Program.dayKeys.mapNotNull { key ->
                prefs[PreferenceKeys.dayColorKey(key)]?.let { color -> key to color }
            }.toMap()
        }

    // ─── Overview tile order (#64) ────────────────────────────────────────────

    val overviewTileOrder: Flow<List<String>> = pref { (it[PreferenceKeys.OVERVIEW_TILE_ORDER] ?: "gym,cardio,trophies").split(",") }
    suspend fun setOverviewTileOrder(order: List<String>) =
        context.forgePreferences.edit { it[PreferenceKeys.OVERVIEW_TILE_ORDER] = order.joinToString(",") }

    // ─── Pinned goals (Home, 2026-08-16) ─────────────────────────────────────

    /** Keys of the goals pinned to Home, in pin order. Home renders at most the first three. */
    val pinnedGoals: Flow<List<String>> = pref { prefs ->
            prefs[PreferenceKeys.PINNED_GOALS]
                ?.split(",")
                ?.filter { it.isNotBlank() }
                .orEmpty()
        }

    /** Adds or removes [key], preserving pin order. Capped so Home's three slots stay meaningful. */
    suspend fun toggleGoalPin(key: String, max: Int = 3) =
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.PINNED_GOALS]
                ?.split(",")?.filter { it.isNotBlank() }.orEmpty()
            val next = if (key in current) current - key else (current + key).takeLast(max)
            prefs[PreferenceKeys.PINNED_GOALS] = next.joinToString(",")
        }

    // ─── Custom warmup (#120) ────────────────────────────────────────────────

    /** Returns the custom warmup list for [dayKey], or null if the user hasn't overridden it. */
    fun getCustomWarmup(dayKey: String): kotlinx.coroutines.flow.Flow<List<String>?> =
        pref { prefs ->
            prefs[PreferenceKeys.warmupKey(dayKey)]
                ?.split("\n")
                ?.filter { it.isNotBlank() }
                ?.takeIf { it.isNotEmpty() }
        }

    suspend fun setCustomWarmup(dayKey: String, items: List<String>) =
        context.forgePreferences.edit { prefs ->
            if (items.isEmpty()) prefs.remove(PreferenceKeys.warmupKey(dayKey))
            else prefs[PreferenceKeys.warmupKey(dayKey)] = items.joinToString("\n")
        }

    // ─── Compact set logging (#35c) ───────────────────────────────────────────

    val compactSetLogging: Flow<Boolean> = pref { it[PreferenceKeys.COMPACT_SET_LOGGING] ?: false }
    suspend fun setCompactSetLogging(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.COMPACT_SET_LOGGING] = v }

    // ─── Overview tile visibility (#121) ─────────────────────────────────────

    val hiddenOverviewTiles: Flow<Set<String>> = pref { it[PreferenceKeys.HIDDEN_OVERVIEW_TILES] ?: emptySet() }
    suspend fun setTileHidden(tileId: String, hidden: Boolean) {
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.HIDDEN_OVERVIEW_TILES] ?: emptySet()
            prefs[PreferenceKeys.HIDDEN_OVERVIEW_TILES] = if (hidden) current + tileId else current - tileId
        }
    }

    // ─── Note templates (#113) ────────────────────────────────────────────────

    private val defaultNoteTemplates = setOf("form felt: ", "energy: ", "pain/discomfort: ", "focus cue: ")

    val noteTemplates: Flow<Set<String>> = pref { it[PreferenceKeys.NOTE_TEMPLATES] ?: defaultNoteTemplates }

    /** Add a user-defined note template (materializes the default set on first edit). Blank = no-op. */
    suspend fun addNoteTemplate(template: String) {
        val t = template.trim()
        if (t.isBlank()) return
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.NOTE_TEMPLATES] ?: defaultNoteTemplates
            prefs[PreferenceKeys.NOTE_TEMPLATES] = cur + t
        }
    }

    suspend fun removeNoteTemplate(template: String) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.NOTE_TEMPLATES] ?: defaultNoteTemplates
            prefs[PreferenceKeys.NOTE_TEMPLATES] = cur - template
        }

    // ─── Units (#2, GYMAP-72) ───────────────────────────────────────────────────

    /** The weight display unit (lb | kg | st). Reads the tri-state key, falling back to the legacy
     *  [USE_KG] boolean for installs that predate it so their kg/lb choice carries over. */
    val weightUnit: Flow<com.forge.app.domain.units.WeightUnit> = pref { prefs ->
            prefs[PreferenceKeys.WEIGHT_UNIT]?.let { com.forge.app.domain.units.WeightUnit.fromKey(it) }
                ?: com.forge.app.domain.units.WeightUnit.ofKg(prefs[PreferenceKeys.USE_KG] ?: false)
        }
    suspend fun setWeightUnit(unit: com.forge.app.domain.units.WeightUnit) =
        context.forgePreferences.edit { prefs ->
            val stone = com.forge.app.domain.units.WeightUnit.ST
            val oldKg = prefs[PreferenceKeys.WEIGHT_UNIT]
                ?.let { com.forge.app.domain.units.WeightUnit.fromKey(it) == com.forge.app.domain.units.WeightUnit.KG }
                ?: (prefs[PreferenceKeys.USE_KG] ?: false)
            val newKg = unit == com.forge.app.domain.units.WeightUnit.KG
            // A kg⇄st switch flips the USE_KG mirror, which would otherwise silently flip the DERIVED
            // cardio distance (mi/km) and body-measurement length (in/cm) too — surprising when the user
            // only meant to change a weight preference. Pin those to their pre-switch value first, but
            // only when the user never made an explicit pick. (lb⇄kg still flips as the deliberate
            // metric/imperial bundle; lb⇄st never flips USE_KG, so it's already stable.)
            val stonesInvolved = unit == stone ||
                prefs[PreferenceKeys.WEIGHT_UNIT] == stone.label
            if (stonesInvolved && oldKg != newKg) {
                if (prefs[PreferenceKeys.USE_MILES] == null) prefs[PreferenceKeys.USE_MILES] = !oldKg
                if (prefs[PreferenceKeys.USE_CM] == null) prefs[PreferenceKeys.USE_CM] = oldKg
            }
            prefs[PreferenceKeys.WEIGHT_UNIT] = unit.label
            // Keep the legacy mirror in sync (true only for kg) so the derived distance/length
            // defaults + backup, which read USE_KG, stay correct — stones reads as "not kg" = lb-like.
            prefs[PreferenceKeys.USE_KG] = newKg
        }

    val useKg: Flow<Boolean> = pref { it[PreferenceKeys.USE_KG] ?: false }
    /** Legacy boolean setter — routes through [setWeightUnit] so both keys stay consistent. */
    suspend fun setUseKg(value: Boolean) =
        setWeightUnit(com.forge.app.domain.units.WeightUnit.ofKg(value))

    /** Persisted folder the user granted Import to auto-scan (#GYMAP-17); null when none granted yet. */
    val importFolderUri: Flow<String?> = pref { it[PreferenceKeys.IMPORT_FOLDER_URI]?.takeIf { s -> s.isNotBlank() } }
    suspend fun setImportFolderUri(uri: String?) =
        context.forgePreferences.edit {
            if (uri.isNullOrBlank()) it.remove(PreferenceKeys.IMPORT_FOLDER_URI)
            else it[PreferenceKeys.IMPORT_FOLDER_URI] = uri
        }

    // ─── Backup (GYMAP-67) ────────────────────────────────────────────────────
    /** Weekly auto-backup master switch. Default ON. */
    val autoBackupEnabled: Flow<Boolean> = pref { it[PreferenceKeys.AUTO_BACKUP_ENABLED] ?: true }
    suspend fun setAutoBackupEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.AUTO_BACKUP_ENABLED] = value }

    /** User-picked folder the auto-backup also writes to (survives uninstall); null when none picked. */
    val backupFolderUri: Flow<String?> = pref { it[PreferenceKeys.BACKUP_FOLDER_URI]?.takeIf { s -> s.isNotBlank() } }
    suspend fun setBackupFolderUri(uri: String?) =
        context.forgePreferences.edit {
            if (uri.isNullOrBlank()) it.remove(PreferenceKeys.BACKUP_FOLDER_URI)
            else it[PreferenceKeys.BACKUP_FOLDER_URI] = uri
        }

    /**
     * Cardio distance unit. When the user never made an explicit choice, it follows the weight unit
     * (lb→miles, kg→km) so a single "pounds + miles" / "kilos + km" mental model holds by default —
     * this is also what existing users and the skip-onboarding path get for free.
     */
    val useMiles: Flow<Boolean> = pref { it[PreferenceKeys.USE_MILES] ?: !(it[PreferenceKeys.USE_KG] ?: false) }
    suspend fun setUseMiles(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.USE_MILES] = value }

    /**
     * Body-measurement length unit (GYMAP-52). When the user never made an explicit choice, it
     * follows the weight unit (kg→cm, lb→in) so one metric/imperial mental model holds by default;
     * an explicit pick in Settings breaks the tie.
     */
    val useCm: Flow<Boolean> = pref { it[PreferenceKeys.USE_CM] ?: (it[PreferenceKeys.USE_KG] ?: false) }
    suspend fun setUseCm(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.USE_CM] = value }

    // ─── Health Connect bodyweight sync (HC-3) ────────────────────────────────

    /** Mirror weigh-ins to Health Connect. Off by default — write-back is strictly opt-in. */
    val hcWriteBodyweight: Flow<Boolean> = pref { it[PreferenceKeys.HC_WRITE_BODYWEIGHT] ?: false }
    suspend fun setHcWriteBodyweight(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.HC_WRITE_BODYWEIGHT] = value }

    /** Mirror body-fat entries to Health Connect (GYMAP-62). Off by default — write-back is opt-in. */
    val hcWriteBodyFat: Flow<Boolean> = pref { it[PreferenceKeys.HC_WRITE_BODY_FAT] ?: false }
    suspend fun setHcWriteBodyFat(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.HC_WRITE_BODY_FAT] = value }

    /** Write each finished session's estimated active calories to Health Connect (HC-4). Opt-in, off by default. */
    val hcWriteCalories: Flow<Boolean> = pref { it[PreferenceKeys.HC_WRITE_CALORIES] ?: false }
    suspend fun setHcWriteCalories(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.HC_WRITE_CALORIES] = value }

    /** Watch workouts the user dismissed from the cardio import suggestions (W5), by HC record id. */
    val hcDismissedWatchImports: Flow<Set<String>> = pref { it[PreferenceKeys.HC_DISMISSED_WATCH_IMPORTS] ?: emptySet() }
    suspend fun addDismissedWatchImports(ids: Set<String>) =
        context.forgePreferences.edit {
            it[PreferenceKeys.HC_DISMISSED_WATCH_IMPORTS] =
                (it[PreferenceKeys.HC_DISMISSED_WATCH_IMPORTS] ?: emptySet()) + ids
        }

    /** Write each finished gym + cardio session to Health Connect (W0). Opt-in, off by default. */
    val hcWriteSessions: Flow<Boolean> = pref { it[PreferenceKeys.HC_WRITE_SESSIONS] ?: false }
    suspend fun setHcWriteSessions(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.HC_WRITE_SESSIONS] = value }

    /** Whether the one-time HC weight-history backfill has run (GYMAP-63). Default false. */
    val hcWeightHistoryImported: Flow<Boolean> = pref { it[PreferenceKeys.HC_WEIGHT_HISTORY_IMPORTED] ?: false }
    suspend fun setHcWeightHistoryImported(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.HC_WEIGHT_HISTORY_IMPORTED] = value }

    /** Which watch the user wears ([com.forge.app.domain.health.WearableBrand] key; "" = never
     *  asked). Advisory only — tailors Recovery's setup pointers, never gates a read. */
    val wearableBrand: Flow<String> = pref { it[PreferenceKeys.WEARABLE_BRAND] ?: "" }
    suspend fun setWearableBrand(value: String) =
        context.forgePreferences.edit { it[PreferenceKeys.WEARABLE_BRAND] = value }

    // ─── Appearance (#35a) ────────────────────────────────────────────────────

    val amoledMode: Flow<Boolean> = pref { it[PreferenceKeys.AMOLED_MODE] ?: false }
    suspend fun setAmoledMode(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.AMOLED_MODE] = value }

    val accentColorHex: Flow<String> = pref { it[PreferenceKeys.ACCENT_COLOR_HEX] ?: "" }
    suspend fun setAccentColorHex(hex: String) =
        context.forgePreferences.edit { it[PreferenceKeys.ACCENT_COLOR_HEX] = hex }

    /** When false the accent is suppressed app-wide (monochrome highlights). Default on. */
    val accentEnabled: Flow<Boolean> = pref { it[PreferenceKeys.ACCENT_ENABLED] ?: true }
    suspend fun setAccentEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.ACCENT_ENABLED] = value }

    /** Selected launcher-icon enum name; "" = default emblem. Deliberately NOT in the APPEARANCE
     *  reset set — the enabled activity-alias is the real state, so clearing this pref alone would
     *  desync the ringed choice from the icon actually on the home screen. */
    val appIcon: Flow<String> = pref { it[PreferenceKeys.APP_ICON] ?: "" }
    suspend fun setAppIcon(key: String) =
        context.forgePreferences.edit { it[PreferenceKeys.APP_ICON] = key }

    /** Theme the cold-launch Avex intro to the chosen app icon's family (default on). Off = the plain
     *  black-and-white Avex settle, no icon-family effect. */
    val themedLaunchIntro: Flow<Boolean> = pref { it[PreferenceKeys.THEMED_LAUNCH_INTRO] ?: true }
    suspend fun setThemedLaunchIntro(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.THEMED_LAUNCH_INTRO] = value }

    val fontChoice: Flow<String> = pref { it[PreferenceKeys.FONT_CHOICE] ?: "default" }

    // ─── Locale (#116) ────────────────────────────────────────────────────────

    val dateFormat: Flow<String> = pref { it[PreferenceKeys.DATE_FORMAT] ?: "MMM d, yyyy" }
    suspend fun setDateFormat(pattern: String) =
        context.forgePreferences.edit { it[PreferenceKeys.DATE_FORMAT] = pattern }

    val timeFormat24h: Flow<Boolean> = pref { it[PreferenceKeys.TIME_FORMAT_24H] ?: false }
    suspend fun setTimeFormat24h(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.TIME_FORMAT_24H] = value }

    val firstDayMonday: Flow<Boolean> = pref { it[PreferenceKeys.FIRST_DAY_MONDAY] ?: true }
    suspend fun setFirstDayMonday(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.FIRST_DAY_MONDAY] = value }

    val timezone: Flow<String> = pref { it[PreferenceKeys.TIMEZONE] ?: java.util.TimeZone.getDefault().id }
    suspend fun setTimezone(id: String) =
        context.forgePreferences.edit { it[PreferenceKeys.TIMEZONE] = id }

    /** IANA zone ids the user has starred, pinned to the top of the timezone picker. */
    val favoriteTimezones: Flow<Set<String>> = pref { it[PreferenceKeys.FAVORITE_TIMEZONES] ?: emptySet() }
    suspend fun toggleFavoriteTimezone(id: String) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.FAVORITE_TIMEZONES] ?: emptySet()
            prefs[PreferenceKeys.FAVORITE_TIMEZONES] = if (id in cur) cur - id else cur + id
        }

    // ─── Feel (#118) ──────────────────────────────────────────────────────────

    val hapticStrength: Flow<String> = pref { it[PreferenceKeys.HAPTIC_STRENGTH] ?: "strong" }
    suspend fun setHapticStrength(value: String) =
        context.forgePreferences.edit { it[PreferenceKeys.HAPTIC_STRENGTH] = value }

    // Keep-screen-on while logging (GYMAP-74) — default on so a session never locks mid-rest.
    val keepScreenOn: Flow<Boolean> = pref { it[PreferenceKeys.KEEP_SCREEN_ON] ?: true }
    suspend fun setKeepScreenOn(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.KEEP_SCREEN_ON] = v }

    // ─── Notifications (#122) ─────────────────────────────────────────────────

    val quietHoursEnabled: Flow<Boolean> = pref { it[PreferenceKeys.QUIET_HOURS_ENABLED] ?: false }
    suspend fun setQuietHoursEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.QUIET_HOURS_ENABLED] = value }

    /** Per-day quiet windows (GYMAP-75). Seeds from the legacy single window (START/END) until the
     *  user first edits a day, after which the JSON schedule is authoritative. */
    val quietHoursSchedule: Flow<com.forge.app.domain.notify.QuietHoursSchedule> =
        pref { readQuietSchedule(it) }

    private fun readQuietSchedule(prefs: androidx.datastore.preferences.core.Preferences) =
        com.forge.app.domain.notify.QuietHoursSchedule.fromJson(
            prefs[PreferenceKeys.QUIET_HOURS_SCHEDULE],
            prefs[PreferenceKeys.QUIET_HOURS_START] ?: com.forge.app.domain.notify.QuietHoursSchedule.DEFAULT_START,
            prefs[PreferenceKeys.QUIET_HOURS_END] ?: com.forge.app.domain.notify.QuietHoursSchedule.DEFAULT_END
        )

    /** Sets one weekday's window, persisting the whole schedule as JSON (read-modify-write so a partial
     *  edit never drops the other days). */
    suspend fun setQuietWindow(day: java.time.DayOfWeek, start: Int, end: Int) =
        context.forgePreferences.edit { prefs ->
            val next = readQuietSchedule(prefs)
                .withWindow(day, com.forge.app.domain.notify.QuietWindow(start, end))
            prefs[PreferenceKeys.QUIET_HOURS_SCHEDULE] =
                com.forge.app.domain.notify.QuietHoursSchedule.toJson(next)
        }

    /** Daily training reminder (engagement) — opt-in, default OFF so a new user is never nagged. */
    val trainingReminderEnabled: Flow<Boolean> = pref { it[PreferenceKeys.TRAINING_REMINDER_ENABLED] ?: false }
    suspend fun setTrainingReminderEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.TRAINING_REMINDER_ENABLED] = value }

    /** Whether the one-time "how your coach learns" card has been dismissed (CO6). */
    val coachBriefIntroSeen: Flow<Boolean> = pref { it[PreferenceKeys.COACH_BRIEF_INTRO_SEEN] ?: false }
    suspend fun setCoachBriefIntroSeen() =
        context.forgePreferences.edit { it[PreferenceKeys.COACH_BRIEF_INTRO_SEEN] = true }

    /** Hour-of-day (0–23) the reminder fires; default 18 (6pm). */
    val trainingReminderHour: Flow<Int> = pref { it[PreferenceKeys.TRAINING_REMINDER_HOUR] ?: 18 }
    suspend fun setTrainingReminderHour(hour: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.TRAINING_REMINDER_HOUR] = hour.coerceIn(0, 23) }

    /** Weekly "your week in numbers" recap notification (N2). On by default. */
    val weeklyRecapEnabled: Flow<Boolean> = pref { it[PreferenceKeys.WEEKLY_RECAP_ENABLED] ?: true }
    suspend fun setWeeklyRecapEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.WEEKLY_RECAP_ENABLED] = value }

    /** Rest-timer "done" alert — buzz + notification when the app is backgrounded (N2). On by default. */
    val restTimerAlertEnabled: Flow<Boolean> = pref { it[PreferenceKeys.REST_TIMER_ALERT_ENABLED] ?: true }
    suspend fun setRestTimerAlertEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.REST_TIMER_ALERT_ENABLED] = value }

    /** Whether the user has dealt with the notification-permission ask (N1) — it was a one-time
     *  launch dialog until 2026-07-27, and is now a clearable row in the notifications feed.
     *  Settable both ways so clearing that row stays undoable (DESIGN §12). */
    val notifPermAsked: Flow<Boolean> = pref { it[PreferenceKeys.NOTIF_PERM_ASKED] ?: false }
    suspend fun setNotifPermAsked(asked: Boolean = true) =
        context.forgePreferences.edit { it[PreferenceKeys.NOTIF_PERM_ASKED] = asked }

    /** True once the first workout is finished — gates the first-touch onboarding cards so they never
     *  reappear for a returning user (survives a DB wipe; see [PreferenceKeys.FIRST_WORKOUT_DONE]). */
    val firstWorkoutDone: Flow<Boolean> = pref { it[PreferenceKeys.FIRST_WORKOUT_DONE] ?: false }
    suspend fun setFirstWorkoutDone() =
        context.forgePreferences.edit { it[PreferenceKeys.FIRST_WORKOUT_DONE] = true }

    // ─── Monthly PR target (#84) ──────────────────────────────────────────────

    val monthlyPrTarget: Flow<Int> = pref { it[PreferenceKeys.MONTHLY_PR_TARGET] ?: 0 }
    suspend fun setMonthlyPrTarget(target: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.MONTHLY_PR_TARGET] = target }

    // ─── Equipment context (#44) ──────────────────────────────────────────────

    val availableEquipment: Flow<Set<String>> = pref { it[PreferenceKeys.AVAILABLE_EQUIPMENT] ?: emptySet() }
    suspend fun setAvailableEquipment(codes: Set<String>) =
        context.forgePreferences.edit { it[PreferenceKeys.AVAILABLE_EQUIPMENT] = codes }

    /**
     * Curated/frozen exercise pool (a preset such as the Developer's preset). Null/absent = no
     * curation (ordinary equipment filtering). Drives generation, the swap picker and the
     * like/dislike screen so they all show the same locked set.
     */
    val frozenExerciseIds: Flow<Set<String>?> = pref { it[PreferenceKeys.FROZEN_EXERCISE_IDS] }
    suspend fun setFrozenExerciseIds(ids: Set<String>?) =
        context.forgePreferences.edit {
            if (ids == null) it.remove(PreferenceKeys.FROZEN_EXERCISE_IDS)
            else it[PreferenceKeys.FROZEN_EXERCISE_IDS] = ids
        }

    // ─── Program generation (program-unlock) ──────────────────────────────────

    val daysPerWeek: Flow<Int> = pref { it[PreferenceKeys.DAYS_PER_WEEK] ?: 4 }
    suspend fun setDaysPerWeek(n: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.DAYS_PER_WEEK] = n.coerceIn(1, 7) }

    /** Default rest base (seconds) per movement type — what the rest timer starts at before personal
     *  tuning + the brutal bonus. Defaults to the canonical 180 / 90; clamped to a sane 30s–10min. */
    val restCompoundSeconds: Flow<Int> = pref { it[PreferenceKeys.REST_COMPOUND_SECONDS] ?: 180 }
    val restIsolationSeconds: Flow<Int> = pref { it[PreferenceKeys.REST_ISOLATION_SECONDS] ?: 90 }
    suspend fun setRestCompoundSeconds(s: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.REST_COMPOUND_SECONDS] = s.coerceIn(30, 600) }
    suspend fun setRestIsolationSeconds(s: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.REST_ISOLATION_SECONDS] = s.coerceIn(30, 600) }

    val programEmphasis: Flow<String> = pref { it[PreferenceKeys.PROGRAM_EMPHASIS] ?: "balanced" }
    suspend fun setProgramEmphasis(v: String) =
        context.forgePreferences.edit { it[PreferenceKeys.PROGRAM_EMPHASIS] = v }

    val likedExercises: Flow<Set<String>> = pref { it[PreferenceKeys.LIKED_EXERCISES] ?: emptySet() }
    val dislikedExercises: Flow<Set<String>> = pref { it[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet() }

    /** Like is mutually exclusive with dislike (and vice-versa) — setting one clears the other. */
    suspend fun setExerciseLiked(libId: String, liked: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.LIKED_EXERCISES] ?: emptySet()
            prefs[PreferenceKeys.LIKED_EXERCISES] = if (liked) cur + libId else cur - libId
            if (liked) prefs[PreferenceKeys.DISLIKED_EXERCISES] =
                (prefs[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet()) - libId
        }

    suspend fun setExerciseDisliked(libId: String, disliked: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet()
            prefs[PreferenceKeys.DISLIKED_EXERCISES] = if (disliked) cur + libId else cur - libId
            if (disliked) prefs[PreferenceKeys.LIKED_EXERCISES] =
                (prefs[PreferenceKeys.LIKED_EXERCISES] ?: emptySet()) - libId
        }

    /**
     * Batch absolute set — a custom exercise spans several `custom_…` ids (one per day it's on), so the
     * post-swap dislike prompt hides every copy in one edit. Mutual exclusion still holds. (Likes/dislikes
     * driven by the picker chips go through the [toggleExercisesLiked]/[toggleExercisesDisliked] toggles.)
     */
    suspend fun setExercisesDisliked(ids: Set<String>, disliked: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet()
            prefs[PreferenceKeys.DISLIKED_EXERCISES] = if (disliked) cur + ids else cur - ids
            if (disliked) prefs[PreferenceKeys.LIKED_EXERCISES] =
                (prefs[PreferenceKeys.LIKED_EXERCISES] ?: emptySet()) - ids
        }

    /**
     * Toggle variants — read-decide-write inside one DataStore edit so the decision uses the freshly
     * persisted set, not a (possibly stale) UI snapshot. A rapid double-tap therefore cycles the chip
     * correctly (on→off) instead of two taps computing the same target and the second no-op'ing.
     * "On" is keyed on whether ANY id is currently set (matching how the custom-exercise row displays).
     */
    suspend fun toggleExercisesLiked(ids: Set<String>) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.LIKED_EXERCISES] ?: emptySet()
            if (ids.any { it in cur }) {
                prefs[PreferenceKeys.LIKED_EXERCISES] = cur - ids
            } else {
                prefs[PreferenceKeys.LIKED_EXERCISES] = cur + ids
                prefs[PreferenceKeys.DISLIKED_EXERCISES] =
                    (prefs[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet()) - ids
            }
        }

    suspend fun toggleExercisesDisliked(ids: Set<String>) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet()
            if (ids.any { it in cur }) {
                prefs[PreferenceKeys.DISLIKED_EXERCISES] = cur - ids
            } else {
                prefs[PreferenceKeys.DISLIKED_EXERCISES] = cur + ids
                prefs[PreferenceKeys.LIKED_EXERCISES] =
                    (prefs[PreferenceKeys.LIKED_EXERCISES] ?: emptySet()) - ids
            }
        }

    /** After a "Make default" swap, offer to dislike the swapped-out exercise (default ON). */
    val swapDislikePromptEnabled: Flow<Boolean> = pref { it[PreferenceKeys.SWAP_DISLIKE_PROMPT_ENABLED] ?: true }
    suspend fun setSwapDislikePromptEnabled(enabled: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.SWAP_DISLIKE_PROMPT_ENABLED] = enabled }

    /** Rotation cadence: "never" | "every_n" (count = finished sessions). */
    val rotationCadence: Flow<String> = pref { it[PreferenceKeys.ROTATION_CADENCE] ?: "never" }
    suspend fun setRotationCadence(v: String) =
        context.forgePreferences.edit { it[PreferenceKeys.ROTATION_CADENCE] = v }

    val rotationEveryN: Flow<Int> = pref { it[PreferenceKeys.ROTATION_EVERY_N] ?: 4 }
    suspend fun setRotationEveryN(n: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.ROTATION_EVERY_N] = n.coerceAtLeast(1) }

    val rotationCounter: Flow<Int> = pref { it[PreferenceKeys.ROTATION_COUNTER] ?: 0 }
    suspend fun setRotationCounter(n: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.ROTATION_COUNTER] = n }

    /**
     * Count one finished session toward the next program re-roll, and report whether it lands on
     * [limit]. Returns true exactly once per [limit] sessions, having already reset the counter.
     *
     * The whole compare-and-set happens inside one `edit {}` because the caller used to read,
     * increment and write as three separate suspending steps. Two finishes racing — a double-tapped
     * FINISH, or a finish and an orphan recovery — both read the same value, so either the increment
     * was lost (the re-roll the user expects after four workouts arrives after five) or both saw the
     * limit and started two full program generations with different seeds, the loser's writes
     * discarded after the user had already seen the program change.
     */
    suspend fun countSessionTowardRotation(limit: Int): Boolean {
        val n = limit.coerceAtLeast(1)
        var reached = false
        context.forgePreferences.edit { prefs ->
            val next = (prefs[PreferenceKeys.ROTATION_COUNTER] ?: 0) + 1
            reached = next >= n
            prefs[PreferenceKeys.ROTATION_COUNTER] = if (reached) 0 else next
        }
        return reached
    }

    /** When the active deload week began (epoch-ms); 0 = not in a deload week (auto-coach seam, #18). */
    val deloadWeekStartMs: Flow<Long> = pref { it[PreferenceKeys.DELOAD_WEEK_START_MS] ?: 0L }
    suspend fun setDeloadWeekStartMs(ms: Long) =
        context.forgePreferences.edit { it[PreferenceKeys.DELOAD_WEEK_START_MS] = ms }

    // ─── Day-aware scheduling (weekly plan vs legacy sequence) ────────────────
    /** "sequence" (default — day after the last finished) or "weekday" (fixed Mon..Sun plan). */
    val scheduleMode: Flow<String> = pref { it[PreferenceKeys.SCHEDULE_MODE] ?: com.forge.app.domain.schedule.WeeklySchedule.MODE_SEQUENCE }
    suspend fun setScheduleMode(v: String) =
        context.forgePreferences.edit { it[PreferenceKeys.SCHEDULE_MODE] = v }

    /** The 7-slot weekly schedule (Mon..Sun; "" = rest). Defaults to program days on the first weekdays. */
    val weeklySchedule: Flow<List<String>> = pref {
            it[PreferenceKeys.SCHEDULE_WEEKLY]
                ?.let { stored -> com.forge.app.domain.schedule.WeeklySchedule.parse(stored) }
                ?: com.forge.app.domain.schedule.WeeklySchedule.defaultFor(com.forge.app.program.Program.dayKeys)
        }
    suspend fun setWeeklySchedule(slots: List<String>) =
        context.forgePreferences.edit {
            it[PreferenceKeys.SCHEDULE_WEEKLY] = com.forge.app.domain.schedule.WeeklySchedule.encode(slots)
        }

    // ─── Cardio weekly-minutes goal (cardio tab — NOT a program day) ──────────
    val cardioWeeklyTargetMin: Flow<Int> = pref { it[PreferenceKeys.CARDIO_WEEKLY_TARGET_MIN] ?: 0 }
    suspend fun setCardioWeeklyTargetMin(min: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.CARDIO_WEEKLY_TARGET_MIN] = min.coerceAtLeast(0) }

    /** Whether the user has dismissed the "connect a watch/ring" hint for good (cardio screen). */
    val cardioWearableHintDismissed: Flow<Boolean> = pref { it[PreferenceKeys.CARDIO_WEARABLE_HINT_DISMISSED] ?: false }
    /** [dismissed] is settable both ways so clearing the invite from the notifications feed stays
     *  undoable (DESIGN §12: undo over confirm). */
    suspend fun setCardioWearableHintDismissed(dismissed: Boolean = true) =
        context.forgePreferences.edit { it[PreferenceKeys.CARDIO_WEARABLE_HINT_DISMISSED] = dismissed }

    /** The last cardio activity code logged (GYMAP-40) — the log sheet's new-entry default. Null until
     *  the first non-rest session; the stored code is resolved to an activity at the call site. */
    val lastCardioType: Flow<String?> = pref { it[PreferenceKeys.LAST_CARDIO_TYPE] }
    suspend fun setLastCardioType(code: String) =
        context.forgePreferences.edit { it[PreferenceKeys.LAST_CARDIO_TYPE] = code }

    // ─── Custom cardio activity types (GYMAP-37) ──────────────────────────────
    /** The user's defined cardio activities, decoded from the JSON blob (empty when none). */
    val customCardioTypes: Flow<List<com.forge.app.domain.cardio.CustomCardioType>> =
        pref { com.forge.app.domain.cardio.CustomCardioType.listFromJson(it[PreferenceKeys.CUSTOM_CARDIO_TYPES]) }

    /** Append a new activity (deduped by code — its code is freshly minted so this is just a guard). */
    suspend fun addCustomCardioType(type: com.forge.app.domain.cardio.CustomCardioType) =
        context.forgePreferences.edit { prefs ->
            val cur = com.forge.app.domain.cardio.CustomCardioType.listFromJson(prefs[PreferenceKeys.CUSTOM_CARDIO_TYPES])
            prefs[PreferenceKeys.CUSTOM_CARDIO_TYPES] =
                com.forge.app.domain.cardio.CustomCardioType.listToJson(cur.filter { it.code != type.code } + type)
        }

    /** Replace an existing activity in place (rename / change glyph) — matched by its stable code. */
    suspend fun updateCustomCardioType(type: com.forge.app.domain.cardio.CustomCardioType) =
        context.forgePreferences.edit { prefs ->
            val cur = com.forge.app.domain.cardio.CustomCardioType.listFromJson(prefs[PreferenceKeys.CUSTOM_CARDIO_TYPES])
            prefs[PreferenceKeys.CUSTOM_CARDIO_TYPES] =
                com.forge.app.domain.cardio.CustomCardioType.listToJson(cur.map { if (it.code == type.code) type else it })
        }

    /** Forget an activity. Sessions already logged against its code keep the code and render as "Other". */
    suspend fun deleteCustomCardioType(code: String) =
        context.forgePreferences.edit { prefs ->
            val cur = com.forge.app.domain.cardio.CustomCardioType.listFromJson(prefs[PreferenceKeys.CUSTOM_CARDIO_TYPES])
            prefs[PreferenceKeys.CUSTOM_CARDIO_TYPES] =
                com.forge.app.domain.cardio.CustomCardioType.listToJson(cur.filter { it.code != code })
        }

    // ─── Plate weight (machine/cable plate-loaded exercises) ──────────────────
    /** Weight of one plate in lb. Plate-loaded exercises are entered/shown as a plate count. */
    val plateWeightLb: Flow<Double> = pref { it[PreferenceKeys.PLATE_WEIGHT_LB] ?: 15.0 }
    suspend fun setPlateWeightLb(lb: Double) =
        context.forgePreferences.edit { it[PreferenceKeys.PLATE_WEIGHT_LB] = lb.coerceIn(1.0, 200.0) }

    /**
     * Heaviest dumbbell the user owns, in lb (adjustable sets max out). Null = no ceiling set.
     * Drives the generator's heavy-slot stack bias and caps the progression chip's DB targets
     * (auto-coach Phase 0).
     */
    val maxDbWeightLb: Flow<Double?> = pref { prefs -> prefs[PreferenceKeys.MAX_DB_WEIGHT_LB]?.takeIf { it > 0.0 } }
    suspend fun setMaxDbWeightLb(lb: Double?) =
        context.forgePreferences.edit {
            if (lb == null || lb <= 0.0) it.remove(PreferenceKeys.MAX_DB_WEIGHT_LB)
            else it[PreferenceKeys.MAX_DB_WEIGHT_LB] = lb.coerceIn(5.0, 200.0)
        }

    /**
     * Coach mode (auto-coach Phase 4): "suggest" = every change waits for a tap in the Week
     * Brief; "auto" = the coach may auto-apply an adjustment TYPE once it has earned trust
     * (TrustLedger) — never a blanket switch.
     */
    val coachMode: Flow<String> = pref { it[PreferenceKeys.COACH_MODE] ?: "suggest" }
    suspend fun setCoachMode(mode: String) =
        context.forgePreferences.edit { it[PreferenceKeys.COACH_MODE] = mode }

    /** ISO week id of the last Week Brief the user opened/dismissed — gates the Overview banner. */
    val lastSeenCoachWeekId: Flow<String> = pref { it[PreferenceKeys.LAST_SEEN_COACH_WEEK_ID] ?: "" }
    suspend fun setLastSeenCoachWeekId(weekId: String) =
        context.forgePreferences.edit { it[PreferenceKeys.LAST_SEEN_COACH_WEEK_ID] = weekId }

    // ─── Warmup disable (#156) ────────────────────────────────────────────────

    val warmupDisabledUntilMs: Flow<Long> = pref { it[PreferenceKeys.WARMUP_DISABLED_UNTIL_MS] ?: 0L }
    suspend fun setWarmupDisabledUntilMs(untilMs: Long) =
        context.forgePreferences.edit { it[PreferenceKeys.WARMUP_DISABLED_UNTIL_MS] = untilMs }

    // ─── Privacy mode (#152) ──────────────────────────────────────────────────

    /**
     * Everything MainActivity needs before it can draw its first frame, from ONE read of the
     * preferences file.
     *
     * It used to collect five separate flows inside a single runBlocking on the UI thread, and each
     * `.first()` is its own subscription to `forgePreferences.data` — five file reads, plus the
     * file's creation on a first-ever launch, with the main thread parked. StrictMode's disk-read
     * detector is debug-only and penaltyLog, so in release this only ever surfaced as an ANR report.
     */
    suspend fun startupPreferences(): StartupPreferences {
        val prefs = allPreferences.first()
        return StartupPreferences(
            privacyMode = prefs[PreferenceKeys.PRIVACY_MODE] ?: false,
            appLockEnabled = prefs[PreferenceKeys.APP_LOCK_ENABLED] ?: false,
            amoledMode = prefs[PreferenceKeys.AMOLED_MODE] ?: false,
            appIcon = prefs[PreferenceKeys.APP_ICON] ?: "",
            themedLaunchIntro = prefs[PreferenceKeys.THEMED_LAUNCH_INTRO] ?: true
        )
    }

    /** The first-frame settings — see [startupPreferences]. Defaults match each flow's own default. */
    data class StartupPreferences(
        val privacyMode: Boolean,
        val appLockEnabled: Boolean,
        val amoledMode: Boolean,
        val appIcon: String,
        val themedLaunchIntro: Boolean
    )

    val privacyMode: Flow<Boolean> = pref { it[PreferenceKeys.PRIVACY_MODE] ?: false }
    suspend fun setPrivacyMode(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.PRIVACY_MODE] = v }

    // ─── App & gallery lock (GYMAP-69) ────────────────────────────────────────

    val appLockEnabled: Flow<Boolean> = pref { it[PreferenceKeys.APP_LOCK_ENABLED] ?: false }
    suspend fun setAppLockEnabled(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.APP_LOCK_ENABLED] = v }

    val galleryLockEnabled: Flow<Boolean> = pref { it[PreferenceKeys.GALLERY_LOCK_ENABLED] ?: false }
    suspend fun setGalleryLockEnabled(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.GALLERY_LOCK_ENABLED] = v }

    /** Background grace before re-locking, in seconds (0 = immediately). */
    val appLockTimeoutSec: Flow<Int> = pref { it[PreferenceKeys.APP_LOCK_TIMEOUT_SEC] ?: 0 }
    suspend fun setAppLockTimeoutSec(v: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.APP_LOCK_TIMEOUT_SEC] = v }

    // ─── Onboarding (#1) ──────────────────────────────────────────────────────

    val onboardingDone: Flow<Boolean> = pref { it[PreferenceKeys.ONBOARDING_DONE] ?: false }
    /** When the user joined (onboarding finished), epoch ms — 0 if never stamped (pre-existing users
     *  who onboarded before this was tracked). Drives the profile's stable "member since" date. */
    val memberSinceMs: Flow<Long> = pref { it[PreferenceKeys.MEMBER_SINCE_MS] ?: 0L }
    /** Whether the default split has ever been auto-seeded — gates [ensureLoaded] so a deliberately
     *  empty plan (build-your-own / cleared) is never re-seeded after onboarding finishes. */
    val programSeeded: Flow<Boolean> = pref { it[PreferenceKeys.PROGRAM_SEEDED] ?: false }
    suspend fun setProgramSeeded(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.PROGRAM_SEEDED] = v }
    val userName: Flow<String> = pref { it[PreferenceKeys.USER_NAME] ?: "" }
    suspend fun setUserName(name: String) =
        context.forgePreferences.edit { it[PreferenceKeys.USER_NAME] = name }

    // ─── Profile avatar defaults (GYMAP-22) ───────────────────────────────────
    /** Whether a random provided default has ever been auto-assigned — guards the one-shot seed. */
    val avatarDefaultSeeded: Flow<Boolean> = pref { it[PreferenceKeys.AVATAR_DEFAULT_SEEDED] ?: false }
    suspend fun setAvatarDefaultSeeded(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.AVATAR_DEFAULT_SEEDED] = v }
    /** Key of the active provided default ("mountain_1"), or empty when the avatar is the user's own. */
    val avatarDefaultId: Flow<String> = pref { it[PreferenceKeys.AVATAR_DEFAULT_ID] ?: "" }
    suspend fun setAvatarDefaultId(id: String) =
        context.forgePreferences.edit { it[PreferenceKeys.AVATAR_DEFAULT_ID] = id }
    /** Whether the one-time "tap your photo to change it" hint has been shown. */
    val avatarEditHintShown: Flow<Boolean> = pref { it[PreferenceKeys.AVATAR_EDIT_HINT_SHOWN] ?: false }
    suspend fun setAvatarEditHintShown() =
        context.forgePreferences.edit { it[PreferenceKeys.AVATAR_EDIT_HINT_SHOWN] = true }
    val userGoal: Flow<String> = pref { it[PreferenceKeys.USER_GOAL] ?: "" }
    suspend fun setUserGoal(goal: String) =
        context.forgePreferences.edit { it[PreferenceKeys.USER_GOAL] = goal }

    /** User's sex for bodyweight-relative strength standards: "male" | "female" | "" (unspecified). */
    val userSex: Flow<String> = pref { it[PreferenceKeys.USER_SEX] ?: "" }
    suspend fun setUserSex(sex: String) =
        context.forgePreferences.edit { it[PreferenceKeys.USER_SEX] = sex }

    /**
     * Age in years, for the Engine's max-HR estimate (E-A). 0 = not given, and the coach then makes
     * NO zone claims at all rather than assuming an age it was never told.
     */
    val userAgeYears: Flow<Int> = pref { it[PreferenceKeys.USER_AGE_YEARS] ?: 0 }
    suspend fun setUserAgeYears(years: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.USER_AGE_YEARS] = years.coerceIn(0, 120) }

    /** An explicit max heart rate. Beats the age estimate; 0 = not set. */
    val maxHrOverride: Flow<Int> = pref { it[PreferenceKeys.MAX_HR_OVERRIDE] ?: 0 }
    suspend fun setMaxHrOverride(bpm: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.MAX_HR_OVERRIDE] = bpm.coerceIn(0, 240) }

    /** "Go with the flow": no fixed program — the home surfaces freestyle logging instead of day
     *  cards. A seed program still exists; this flag only changes what the UI leads with. */
    val freestyleMode: Flow<Boolean> = pref { it[PreferenceKeys.FREESTYLE_MODE] ?: false }
    suspend fun setFreestyleMode(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.FREESTYLE_MODE] = v }

    /** Whether the Coach is surfaced (tab + banners). Defaults on; declined during onboarding for the
     *  no-plan / make-your-own modes hides it until re-enabled in Settings. */
    val coachEnabled: Flow<Boolean> = pref { it[PreferenceKeys.COACH_ENABLED] ?: true }
    suspend fun setCoachEnabled(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.COACH_ENABLED] = v }

    /**
     * The running rest timer, so it survives process death (GYMAP: the day screen's state is
     * in-memory and `leaveAndResume` stops the foreground service that was keeping the process
     * alive — so leaving mid-rest to check Stats could lose the countdown AND its buzz).
     *
     * Stored as a WALL-CLOCK end instant even though the countdown itself runs on elapsed real time:
     * elapsed-since-boot means nothing to a later process, and a rest that outlives a reboot is not
     * a rest any more.
     */
    suspend fun saveRestTimer(endAtMs: Long, totalSeconds: Int, pausedRemainingSeconds: Int) =
        context.forgePreferences.edit {
            it[PreferenceKeys.REST_TIMER_END_AT] = endAtMs
            it[PreferenceKeys.REST_TIMER_TOTAL] = totalSeconds
            it[PreferenceKeys.REST_TIMER_PAUSED_REMAINING] = pausedRemainingSeconds
        }

    suspend fun clearRestTimer() = context.forgePreferences.edit {
        it.remove(PreferenceKeys.REST_TIMER_END_AT)
        it.remove(PreferenceKeys.REST_TIMER_TOTAL)
        it.remove(PreferenceKeys.REST_TIMER_PAUSED_REMAINING)
    }

    /** (endAtMs, totalSeconds, pausedRemainingSeconds) of a persisted rest, or null if none. */
    suspend fun savedRestTimer(): Triple<Long, Int, Int>? {
        val prefs = allPreferences.firstOrNull() ?: return null
        val total = prefs[PreferenceKeys.REST_TIMER_TOTAL] ?: return null
        if (total <= 0) return null
        return Triple(
            prefs[PreferenceKeys.REST_TIMER_END_AT] ?: 0L,
            total,
            prefs[PreferenceKeys.REST_TIMER_PAUSED_REMAINING] ?: 0
        )
    }

    /** ISO week id of the last weekly coach pass recorded while the coach was OFF (empty = none). */
    val coachOffPassWeekId: Flow<String> = pref { it[PreferenceKeys.COACH_OFF_PASS_WEEK] ?: "" }
    suspend fun setCoachOffPassWeekId(weekId: String) =
        context.forgePreferences.edit { it[PreferenceKeys.COACH_OFF_PASS_WEEK] = weekId }

    /** Training experience drives generation volume + difficulty filter (program-unlock Phase 4 / Phase 2). */
    val programExperience: Flow<String> = pref { it[PreferenceKeys.PROGRAM_EXPERIENCE] ?: "intermediate" }
    suspend fun setProgramExperience(level: String) =
        context.forgePreferences.edit { it[PreferenceKeys.PROGRAM_EXPERIENCE] = level }

    // ─── Personalization & safety (program-unlock Phase 3) ────────────────────
    /** Flagged problem-area codes — generation steers around movements that stress them. */
    val problemAreas: Flow<Set<String>> = pref { it[PreferenceKeys.PROBLEM_AREAS] ?: emptySet() }
    suspend fun toggleProblemArea(code: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.PROBLEM_AREAS] ?: emptySet()
            prefs[PreferenceKeys.PROBLEM_AREAS] = if (on) cur + code else cur - code
        }

    /** Priority muscle codes — granular emphasis (extra volume). */
    val priorityMuscles: Flow<Set<String>> = pref { it[PreferenceKeys.PRIORITY_MUSCLES] ?: emptySet() }
    suspend fun togglePriorityMuscle(code: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.PRIORITY_MUSCLES] ?: emptySet()
            prefs[PreferenceKeys.PRIORITY_MUSCLES] = if (on) cur + code else cur - code
        }

    /** Pinned exercise ids — kept across regenerations when their muscle is trained. */
    val pinnedExercises: Flow<Set<String>> = pref { it[PreferenceKeys.PINNED_EXERCISES] ?: emptySet() }
    suspend fun togglePinned(libId: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.PINNED_EXERCISES] ?: emptySet()
            prefs[PreferenceKeys.PINNED_EXERCISES] = if (on) cur + libId else cur - libId
        }

    /** Bookmarked exercise ids — the exercise browser's Favorites filter + per-card bookmark. */
    val favoriteExercises: Flow<Set<String>> = pref { it[PreferenceKeys.FAVORITE_EXERCISES] ?: emptySet() }
    suspend fun toggleFavorite(libId: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.FAVORITE_EXERCISES] ?: emptySet()
            prefs[PreferenceKeys.FAVORITE_EXERCISES] = if (on) cur + libId else cur - libId
        }

    // ─── Onboarding draft (resume after a full app kill) ─────────────────────

    /** One-shot read of the saved mid-onboarding draft JSON, or null when there is none. */
    suspend fun onboardingDraft(): String? =
        allPreferences.firstOrNull()?.get(PreferenceKeys.ONBOARDING_DRAFT)

    suspend fun saveOnboardingDraft(json: String) =
        context.forgePreferences.edit { it[PreferenceKeys.ONBOARDING_DRAFT] = json }

    // ─── Freestyle draft (resume an unsaved in-progress log) ─────────────────

    /** One-shot read of the saved in-progress freestyle log JSON, or null when there is none. */
    suspend fun freestyleDraft(): String? =
        allPreferences.firstOrNull()?.get(PreferenceKeys.FREESTYLE_DRAFT)

    suspend fun saveFreestyleDraft(json: String) =
        context.forgePreferences.edit { it[PreferenceKeys.FREESTYLE_DRAFT] = json }

    suspend fun clearFreestyleDraft() =
        context.forgePreferences.edit { it.remove(PreferenceKeys.FREESTYLE_DRAFT) }

    /** [useMilesChoice] is null when the user left the distance step untouched — in that case
     *  USE_MILES is deliberately NOT persisted, so [useMiles] keeps deriving from the weight unit. */
    suspend fun completeOnboarding(
        name: String,
        weightUnitChoice: com.forge.app.domain.units.WeightUnit,
        goal: String,
        bodyweightLb: Double?,
        useMilesChoice: Boolean? = null
    ) {
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.ONBOARDING_DONE] = true
            prefs[PreferenceKeys.WELCOMED] = true
            // Stamp the "member since" date the moment setup finishes, so the profile's SINCE line has
            // a stable anchor from day one. putIfAbsent-style: a re-run of onboarding that didn't wipe
            // prefs keeps the original join date.
            if (prefs[PreferenceKeys.MEMBER_SINCE_MS] == null) prefs[PreferenceKeys.MEMBER_SINCE_MS] = clock.nowMs()
            if (name.isNotBlank()) prefs[PreferenceKeys.USER_NAME] = name
            prefs[PreferenceKeys.WEIGHT_UNIT] = weightUnitChoice.label
            // Mirror the legacy boolean (true only for kg) so the derived distance/length defaults hold.
            prefs[PreferenceKeys.USE_KG] = (weightUnitChoice == com.forge.app.domain.units.WeightUnit.KG)
            useMilesChoice?.let { prefs[PreferenceKeys.USE_MILES] = it }
            if (goal.isNotBlank()) prefs[PreferenceKeys.USER_GOAL] = goal
            // Setup is done — drop the resume draft in the same atomic write.
            prefs.remove(PreferenceKeys.ONBOARDING_DRAFT)
        }
    }

    /** Clears all user preferences (not session/trophy data). Used by factory reset. */
    suspend fun resetAll() {
        context.forgePreferences.edit { it.clear() }
    }

    /**
     * Restores user-facing preferences to defaults but PRESERVES identity/onboarding state, so
     * "reset app settings" doesn't kick the user back through onboarding or wipe their name/goal.
     * Use [resetAll] for a full factory wipe.
     */
    suspend fun resetSettingsOnly() {
        context.forgePreferences.edit { prefs ->
            val onboarding = prefs[PreferenceKeys.ONBOARDING_DONE]
            val welcomed = prefs[PreferenceKeys.WELCOMED]
            val memberSince = prefs[PreferenceKeys.MEMBER_SINCE_MS]
            val name = prefs[PreferenceKeys.USER_NAME]
            val goal = prefs[PreferenceKeys.USER_GOAL]
            // Training mode is identity-like too — wiping it would silently flip a "go with the flow"
            // user into follow-a-plan with an empty program and no explanation.
            val freestyle = prefs[PreferenceKeys.FREESTYLE_MODE]
            // "Never ask to dislike after a swap" is an explicit, deliberate opt-out — same rationale as
            // freestyle: a reset shouldn't silently re-surface a dialog the user permanently dismissed.
            val swapDislikePrompt = prefs[PreferenceKeys.SWAP_DISLIKE_PROMPT_ENABLED]
            prefs.clear()
            onboarding?.let { prefs[PreferenceKeys.ONBOARDING_DONE] = it }
            welcomed?.let { prefs[PreferenceKeys.WELCOMED] = it }
            memberSince?.let { prefs[PreferenceKeys.MEMBER_SINCE_MS] = it }
            name?.let { prefs[PreferenceKeys.USER_NAME] = it }
            goal?.let { prefs[PreferenceKeys.USER_GOAL] = it }
            freestyle?.let { prefs[PreferenceKeys.FREESTYLE_MODE] = it }
            swapDislikePrompt?.let { prefs[PreferenceKeys.SWAP_DISLIKE_PROMPT_ENABLED] = it }
        }
    }

    /**
     * Scoped "reset to defaults" for one settings section — removes just that section's keys so their
     * defaults reapply, without touching the rest of the user's setup (#544). Identity/onboarding and
     * program/equipment config are never in scope here. The key list lives on [SettingsSection].
     */
    suspend fun resetSection(section: SettingsSection) {
        if (section.keys.isNotEmpty())
            context.forgePreferences.edit { prefs -> section.keys.forEach { prefs.remove(it) } }
    }

    // ─── Rank-up celebration ──────────────────────────────────────────────────

    /** Ordinal of the [com.forge.app.domain.rank.RankTier] the user last saw the profile at.
     *  -1 = never opened (first ever profile open). Used to detect a tier upgrade and trigger
     *  the one-shot confetti/haptic celebration. */
    val lastSeenRankTierOrdinal: Flow<Int> = pref { it[PreferenceKeys.LAST_SEEN_RANK_TIER_ORDINAL] ?: -1 }
    suspend fun setLastSeenRankTierOrdinal(ordinal: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.LAST_SEEN_RANK_TIER_ORDINAL] = ordinal }

    /** Last Stats sub-tab the user settled on, stored by enum NAME (not ordinal) so adding/reordering
     *  tabs can't restore the wrong one. null = unset → the screen lands on its default. Reopening Stats
     *  deep-links here (S4). */
    val lastStatsTabName: Flow<String?> = pref { it[PreferenceKeys.LAST_STATS_TAB_NAME] }
    suspend fun setLastStatsTabName(name: String) =
        context.forgePreferences.edit { it[PreferenceKeys.LAST_STATS_TAB_NAME] = name }

    /** Returns true if the current wall-clock time falls within the user's quiet hours window for
     *  today (#122; per-day windows GYMAP-75). */
    suspend fun isQuietNow(): Boolean {
        val prefs = allPreferences.firstOrNull() ?: return false
        if (prefs[PreferenceKeys.QUIET_HOURS_ENABLED] != true) return false
        // Through the injected clock, so quiet hours are testable with a FakeClock like the rest of
        // the time-dependent logic (Clock's docstring asks for exactly this).
        val now = java.time.Instant.ofEpochMilli(clock.nowMs())
            .atZone(java.time.ZoneId.systemDefault()).toLocalDateTime()
        return readQuietSchedule(prefs).isQuietAt(now)
    }
}
