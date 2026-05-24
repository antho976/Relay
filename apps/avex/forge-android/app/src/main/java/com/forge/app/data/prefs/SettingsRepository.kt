package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.firstOrNull
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

    val shownMilestones: Flow<Set<String>> = context.forgePreferences.data
        .map { prefs -> prefs[PreferenceKeys.SHOWN_MILESTONES] ?: emptySet() }

    suspend fun markMilestoneShown(milestoneId: String) {
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.SHOWN_MILESTONES] ?: emptySet()
            prefs[PreferenceKeys.SHOWN_MILESTONES] = current + milestoneId
        }
    }

    // ─── Per-day accent color (#65) ───────────────────────────────────────────

    fun getDayColor(dayKey: String): kotlinx.coroutines.flow.Flow<String?> =
        context.forgePreferences.data.map { it[PreferenceKeys.dayColorKey(dayKey)] }

    suspend fun setDayColor(dayKey: String, hex: String?) =
        context.forgePreferences.edit { prefs ->
            if (hex == null) prefs.remove(PreferenceKeys.dayColorKey(dayKey))
            else prefs[PreferenceKeys.dayColorKey(dayKey)] = hex
        }

    fun observeAllDayColors(): kotlinx.coroutines.flow.Flow<Map<String, String>> =
        context.forgePreferences.data.map { prefs ->
            com.forge.app.program.Program.dayKeys.mapNotNull { key ->
                prefs[PreferenceKeys.dayColorKey(key)]?.let { color -> key to color }
            }.toMap()
        }

    // ─── Overview tile order (#64) ────────────────────────────────────────────

    val overviewTileOrder: Flow<List<String>> = context.forgePreferences.data
        .map { (it[PreferenceKeys.OVERVIEW_TILE_ORDER] ?: "gym,cardio,trophies").split(",") }
    suspend fun setOverviewTileOrder(order: List<String>) =
        context.forgePreferences.edit { it[PreferenceKeys.OVERVIEW_TILE_ORDER] = order.joinToString(",") }

    // ─── Custom warmup (#120) ────────────────────────────────────────────────

    /** Returns the custom warmup list for [dayKey], or null if the user hasn't overridden it. */
    fun getCustomWarmup(dayKey: String): kotlinx.coroutines.flow.Flow<List<String>?> =
        context.forgePreferences.data.map { prefs ->
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

    // ─── Encouragement messages (#67) ────────────────────────────────────────

    val showEncouragement: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.SHOW_ENCOURAGEMENT] ?: true }
    suspend fun setShowEncouragement(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.SHOW_ENCOURAGEMENT] = v }

    // ─── Compact set logging (#35c) ───────────────────────────────────────────

    val compactSetLogging: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.COMPACT_SET_LOGGING] ?: false }
    suspend fun setCompactSetLogging(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.COMPACT_SET_LOGGING] = v }

    // ─── Overview tile visibility (#121) ─────────────────────────────────────

    val hiddenOverviewTiles: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.HIDDEN_OVERVIEW_TILES] ?: emptySet() }
    suspend fun setTileHidden(tileId: String, hidden: Boolean) {
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.HIDDEN_OVERVIEW_TILES] ?: emptySet()
            prefs[PreferenceKeys.HIDDEN_OVERVIEW_TILES] = if (hidden) current + tileId else current - tileId
        }
    }

    // ─── Note templates (#113) ────────────────────────────────────────────────

    private val defaultNoteTemplates = setOf("form felt: ", "energy: ", "pain/discomfort: ", "focus cue: ")

    val noteTemplates: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.NOTE_TEMPLATES] ?: defaultNoteTemplates }
    suspend fun setNoteTemplates(templates: Set<String>) =
        context.forgePreferences.edit { it[PreferenceKeys.NOTE_TEMPLATES] = templates }

    // ─── Units (#2) ───────────────────────────────────────────────────────────

    val useKg: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.USE_KG] ?: false }
    suspend fun setUseKg(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.USE_KG] = value }

    // ─── Appearance (#35a) ────────────────────────────────────────────────────

    val amoledMode: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.AMOLED_MODE] ?: false }
    suspend fun setAmoledMode(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.AMOLED_MODE] = value }

    val accentColorHex: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.ACCENT_COLOR_HEX] ?: "" }
    suspend fun setAccentColorHex(hex: String) =
        context.forgePreferences.edit { it[PreferenceKeys.ACCENT_COLOR_HEX] = hex }

    val fontChoice: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.FONT_CHOICE] ?: "default" }
    suspend fun setFontChoice(key: String) =
        context.forgePreferences.edit { it[PreferenceKeys.FONT_CHOICE] = key }

    // ─── Locale (#116) ────────────────────────────────────────────────────────

    val dateFormat: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.DATE_FORMAT] ?: "MMM d, yyyy" }
    suspend fun setDateFormat(pattern: String) =
        context.forgePreferences.edit { it[PreferenceKeys.DATE_FORMAT] = pattern }

    val timeFormat24h: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.TIME_FORMAT_24H] ?: false }
    suspend fun setTimeFormat24h(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.TIME_FORMAT_24H] = value }

    val firstDayMonday: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.FIRST_DAY_MONDAY] ?: true }
    suspend fun setFirstDayMonday(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.FIRST_DAY_MONDAY] = value }

    // ─── Feel (#118) ──────────────────────────────────────────────────────────

    val hapticStrength: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.HAPTIC_STRENGTH] ?: "strong" }
    suspend fun setHapticStrength(value: String) =
        context.forgePreferences.edit { it[PreferenceKeys.HAPTIC_STRENGTH] = value }

    // ─── Notifications (#122) ─────────────────────────────────────────────────

    val quietHoursEnabled: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.QUIET_HOURS_ENABLED] ?: false }
    suspend fun setQuietHoursEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.QUIET_HOURS_ENABLED] = value }

    val quietHoursStart: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.QUIET_HOURS_START] ?: 22 }
    suspend fun setQuietHoursStart(hour: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.QUIET_HOURS_START] = hour }

    val quietHoursEnd: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.QUIET_HOURS_END] ?: 7 }
    suspend fun setQuietHoursEnd(hour: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.QUIET_HOURS_END] = hour }

    // ─── Monthly PR target (#84) ──────────────────────────────────────────────

    val monthlyPrTarget: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.MONTHLY_PR_TARGET] ?: 0 }
    suspend fun setMonthlyPrTarget(target: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.MONTHLY_PR_TARGET] = target }

    // ─── Equipment context (#44) ──────────────────────────────────────────────

    val availableEquipment: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.AVAILABLE_EQUIPMENT] ?: emptySet() }
    suspend fun setAvailableEquipment(codes: Set<String>) =
        context.forgePreferences.edit { it[PreferenceKeys.AVAILABLE_EQUIPMENT] = codes }

    // ─── Plan tomorrow (#147) ─────────────────────────────────────────────────

    val plannedNextDay: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.PLANNED_NEXT_DAY] ?: "" }
    suspend fun setPlannedNextDay(dayKey: String) =
        context.forgePreferences.edit { it[PreferenceKeys.PLANNED_NEXT_DAY] = dayKey }
    suspend fun clearPlannedNextDay() =
        context.forgePreferences.edit { it.remove(PreferenceKeys.PLANNED_NEXT_DAY) }

    // ─── Privacy mode (#152) ──────────────────────────────────────────────────

    val privacyMode: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.PRIVACY_MODE] ?: false }
    suspend fun setPrivacyMode(v: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.PRIVACY_MODE] = v }

    // ─── Onboarding (#1) ──────────────────────────────────────────────────────

    val onboardingDone: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.ONBOARDING_DONE] ?: false }
    val userName: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.USER_NAME] ?: "" }
    val userGoal: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.USER_GOAL] ?: "" }

    suspend fun completeOnboarding(name: String, useKgChoice: Boolean, goal: String, bodyweightLb: Double?) {
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.ONBOARDING_DONE] = true
            prefs[PreferenceKeys.WELCOMED] = true
            if (name.isNotBlank()) prefs[PreferenceKeys.USER_NAME] = name
            prefs[PreferenceKeys.USE_KG] = useKgChoice
            if (goal.isNotBlank()) prefs[PreferenceKeys.USER_GOAL] = goal
        }
    }

    /** Clears all user preferences (not session/trophy data). */
    suspend fun resetAll() {
        context.forgePreferences.edit { it.clear() }
    }

    /** Returns true if the current wall-clock time falls within the user's quiet hours window (#122). */
    suspend fun isQuietNow(): Boolean {
        val prefs = context.forgePreferences.data.firstOrNull() ?: return false
        val enabled = prefs[PreferenceKeys.QUIET_HOURS_ENABLED] ?: false
        if (!enabled) return false
        val start = prefs[PreferenceKeys.QUIET_HOURS_START] ?: 22
        val end = prefs[PreferenceKeys.QUIET_HOURS_END] ?: 7
        val now = java.time.LocalTime.now().hour
        return if (start <= end) now in start until end
               else now >= start || now < end
    }
}
