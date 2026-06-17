package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.map
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
            PreferenceKeys.ACCENT_COLOR_HEX, PreferenceKeys.ACCENT_EMPHASIS, PreferenceKeys.FONT_CHOICE
        )
    ),
    FORMAT(
        listOf(
            PreferenceKeys.USE_KG, PreferenceKeys.USER_SEX, PreferenceKeys.DATE_FORMAT,
            PreferenceKeys.TIMEZONE, PreferenceKeys.TIME_FORMAT_24H, PreferenceKeys.FIRST_DAY_MONDAY
        )
    ),
    SESSION(
        listOf(
            PreferenceKeys.HAPTIC_STRENGTH, PreferenceKeys.REST_COMPOUND_SECONDS,
            PreferenceKeys.REST_ISOLATION_SECONDS, PreferenceKeys.NOTE_TEMPLATES
        )
    ),
    NOTIFICATIONS(
        listOf(
            PreferenceKeys.QUIET_HOURS_ENABLED, PreferenceKeys.QUIET_HOURS_START, PreferenceKeys.QUIET_HOURS_END,
            PreferenceKeys.TRAINING_REMINDER_ENABLED, PreferenceKeys.TRAINING_REMINDER_HOUR
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
    @ApplicationContext private val context: Context
) {
    val shownMilestones: Flow<Set<String>> = context.forgePreferences.data
        .map { prefs -> prefs[PreferenceKeys.SHOWN_MILESTONES] ?: emptySet() }

    suspend fun markMilestoneShown(milestoneId: String) {
        context.forgePreferences.edit { prefs ->
            val current = prefs[PreferenceKeys.SHOWN_MILESTONES] ?: emptySet()
            prefs[PreferenceKeys.SHOWN_MILESTONES] = current + milestoneId
        }
    }

    // ─── Per-day accent color (#65) ───────────────────────────────────────────

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

    /** Accent emphasis intensity for important text: "off" | "subtle" | "medium" | "strong". */
    val accentEmphasis: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.ACCENT_EMPHASIS] ?: "off" }
    suspend fun setAccentEmphasis(level: String) =
        context.forgePreferences.edit { it[PreferenceKeys.ACCENT_EMPHASIS] = level }

    val fontChoice: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.FONT_CHOICE] ?: "default" }

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

    val timezone: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.TIMEZONE] ?: java.util.TimeZone.getDefault().id }
    suspend fun setTimezone(id: String) =
        context.forgePreferences.edit { it[PreferenceKeys.TIMEZONE] = id }

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

    /** Daily training reminder (engagement) — opt-in, default OFF so a new user is never nagged. */
    val trainingReminderEnabled: Flow<Boolean> = context.forgePreferences.data
        .map { it[PreferenceKeys.TRAINING_REMINDER_ENABLED] ?: false }
    suspend fun setTrainingReminderEnabled(value: Boolean) =
        context.forgePreferences.edit { it[PreferenceKeys.TRAINING_REMINDER_ENABLED] = value }

    /** Hour-of-day (0–23) the reminder fires; default 18 (6pm). */
    val trainingReminderHour: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.TRAINING_REMINDER_HOUR] ?: 18 }
    suspend fun setTrainingReminderHour(hour: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.TRAINING_REMINDER_HOUR] = hour.coerceIn(0, 23) }

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

    /**
     * Curated/frozen exercise pool (a preset such as the Developer's preset). Null/absent = no
     * curation (ordinary equipment filtering). Drives generation, the swap picker and the
     * like/dislike screen so they all show the same locked set.
     */
    val frozenExerciseIds: Flow<Set<String>?> = context.forgePreferences.data
        .map { it[PreferenceKeys.FROZEN_EXERCISE_IDS] }
    suspend fun setFrozenExerciseIds(ids: Set<String>?) =
        context.forgePreferences.edit {
            if (ids == null) it.remove(PreferenceKeys.FROZEN_EXERCISE_IDS)
            else it[PreferenceKeys.FROZEN_EXERCISE_IDS] = ids
        }

    // ─── Program generation (program-unlock) ──────────────────────────────────

    val daysPerWeek: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.DAYS_PER_WEEK] ?: 4 }
    suspend fun setDaysPerWeek(n: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.DAYS_PER_WEEK] = n.coerceIn(1, 7) }

    /** Default rest base (seconds) per movement type — what the rest timer starts at before personal
     *  tuning + the brutal bonus. Defaults to the canonical 180 / 90; clamped to a sane 30s–10min. */
    val restCompoundSeconds: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.REST_COMPOUND_SECONDS] ?: 180 }
    val restIsolationSeconds: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.REST_ISOLATION_SECONDS] ?: 90 }
    suspend fun setRestCompoundSeconds(s: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.REST_COMPOUND_SECONDS] = s.coerceIn(30, 600) }
    suspend fun setRestIsolationSeconds(s: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.REST_ISOLATION_SECONDS] = s.coerceIn(30, 600) }

    val programEmphasis: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.PROGRAM_EMPHASIS] ?: "balanced" }
    suspend fun setProgramEmphasis(v: String) =
        context.forgePreferences.edit { it[PreferenceKeys.PROGRAM_EMPHASIS] = v }

    val likedExercises: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.LIKED_EXERCISES] ?: emptySet() }
    val dislikedExercises: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.DISLIKED_EXERCISES] ?: emptySet() }

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

    /** Rotation cadence: "never" | "every_n" (count = finished sessions). */
    val rotationCadence: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.ROTATION_CADENCE] ?: "never" }
    suspend fun setRotationCadence(v: String) =
        context.forgePreferences.edit { it[PreferenceKeys.ROTATION_CADENCE] = v }

    val rotationEveryN: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.ROTATION_EVERY_N] ?: 4 }
    suspend fun setRotationEveryN(n: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.ROTATION_EVERY_N] = n.coerceAtLeast(1) }

    val rotationCounter: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.ROTATION_COUNTER] ?: 0 }
    suspend fun setRotationCounter(n: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.ROTATION_COUNTER] = n }

    /** When the active deload week began (epoch-ms); 0 = not in a deload week (auto-coach seam, #18). */
    val deloadWeekStartMs: Flow<Long> = context.forgePreferences.data
        .map { it[PreferenceKeys.DELOAD_WEEK_START_MS] ?: 0L }
    suspend fun setDeloadWeekStartMs(ms: Long) =
        context.forgePreferences.edit { it[PreferenceKeys.DELOAD_WEEK_START_MS] = ms }

    // ─── Day-aware scheduling (weekly plan vs legacy sequence) ────────────────
    /** "sequence" (default — day after the last finished) or "weekday" (fixed Mon..Sun plan). */
    val scheduleMode: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.SCHEDULE_MODE] ?: com.forge.app.domain.schedule.WeeklySchedule.MODE_SEQUENCE }
    suspend fun setScheduleMode(v: String) =
        context.forgePreferences.edit { it[PreferenceKeys.SCHEDULE_MODE] = v }

    /** The 7-slot weekly schedule (Mon..Sun; "" = rest). Defaults to program days on the first weekdays. */
    val weeklySchedule: Flow<List<String>> = context.forgePreferences.data
        .map {
            it[PreferenceKeys.SCHEDULE_WEEKLY]
                ?.let { stored -> com.forge.app.domain.schedule.WeeklySchedule.parse(stored) }
                ?: com.forge.app.domain.schedule.WeeklySchedule.defaultFor(com.forge.app.program.Program.dayKeys)
        }
    suspend fun setWeeklySchedule(slots: List<String>) =
        context.forgePreferences.edit {
            it[PreferenceKeys.SCHEDULE_WEEKLY] = com.forge.app.domain.schedule.WeeklySchedule.encode(slots)
        }

    // ─── Cardio weekly-minutes goal (cardio tab — NOT a program day) ──────────
    val cardioWeeklyTargetMin: Flow<Int> = context.forgePreferences.data
        .map { it[PreferenceKeys.CARDIO_WEEKLY_TARGET_MIN] ?: 0 }
    suspend fun setCardioWeeklyTargetMin(min: Int) =
        context.forgePreferences.edit { it[PreferenceKeys.CARDIO_WEEKLY_TARGET_MIN] = min.coerceAtLeast(0) }

    // ─── Plate weight (machine/cable plate-loaded exercises) ──────────────────
    /** Weight of one plate in lb. Plate-loaded exercises are entered/shown as a plate count. */
    val plateWeightLb: Flow<Double> = context.forgePreferences.data
        .map { it[PreferenceKeys.PLATE_WEIGHT_LB] ?: 15.0 }
    suspend fun setPlateWeightLb(lb: Double) =
        context.forgePreferences.edit { it[PreferenceKeys.PLATE_WEIGHT_LB] = lb.coerceIn(1.0, 200.0) }

    /**
     * Heaviest dumbbell the user owns, in lb (adjustable sets max out). Null = no ceiling set.
     * Drives the generator's heavy-slot stack bias and caps the progression chip's DB targets
     * (auto-coach Phase 0).
     */
    val maxDbWeightLb: Flow<Double?> = context.forgePreferences.data
        .map { prefs -> prefs[PreferenceKeys.MAX_DB_WEIGHT_LB]?.takeIf { it > 0.0 } }
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
    val coachMode: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.COACH_MODE] ?: "suggest" }
    suspend fun setCoachMode(mode: String) =
        context.forgePreferences.edit { it[PreferenceKeys.COACH_MODE] = mode }

    /** ISO week id of the last Week Brief the user opened/dismissed — gates the Overview banner. */
    val lastSeenCoachWeekId: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.LAST_SEEN_COACH_WEEK_ID] ?: "" }
    suspend fun setLastSeenCoachWeekId(weekId: String) =
        context.forgePreferences.edit { it[PreferenceKeys.LAST_SEEN_COACH_WEEK_ID] = weekId }

    // ─── Plan tomorrow (#147) ─────────────────────────────────────────────────

    val plannedNextDay: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.PLANNED_NEXT_DAY] ?: "" }
    suspend fun setPlannedNextDay(dayKey: String) =
        context.forgePreferences.edit { it[PreferenceKeys.PLANNED_NEXT_DAY] = dayKey }

    // ─── Warmup disable (#156) ────────────────────────────────────────────────

    val warmupDisabledUntilMs: Flow<Long> = context.forgePreferences.data
        .map { it[PreferenceKeys.WARMUP_DISABLED_UNTIL_MS] ?: 0L }
    suspend fun setWarmupDisabledUntilMs(untilMs: Long) =
        context.forgePreferences.edit { it[PreferenceKeys.WARMUP_DISABLED_UNTIL_MS] = untilMs }

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
    suspend fun setUserGoal(goal: String) =
        context.forgePreferences.edit { it[PreferenceKeys.USER_GOAL] = goal }

    /** User's sex for bodyweight-relative strength standards: "male" | "female" | "" (unspecified). */
    val userSex: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.USER_SEX] ?: "" }
    suspend fun setUserSex(sex: String) =
        context.forgePreferences.edit { it[PreferenceKeys.USER_SEX] = sex }

    /** Training experience drives generation volume + difficulty filter (program-unlock Phase 4 / Phase 2). */
    val programExperience: Flow<String> = context.forgePreferences.data
        .map { it[PreferenceKeys.PROGRAM_EXPERIENCE] ?: "intermediate" }
    suspend fun setProgramExperience(level: String) =
        context.forgePreferences.edit { it[PreferenceKeys.PROGRAM_EXPERIENCE] = level }

    // ─── Personalization & safety (program-unlock Phase 3) ────────────────────
    /** Flagged problem-area codes — generation steers around movements that stress them. */
    val problemAreas: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.PROBLEM_AREAS] ?: emptySet() }
    suspend fun toggleProblemArea(code: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.PROBLEM_AREAS] ?: emptySet()
            prefs[PreferenceKeys.PROBLEM_AREAS] = if (on) cur + code else cur - code
        }

    /** Priority muscle codes — granular emphasis (extra volume). */
    val priorityMuscles: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.PRIORITY_MUSCLES] ?: emptySet() }
    suspend fun togglePriorityMuscle(code: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.PRIORITY_MUSCLES] ?: emptySet()
            prefs[PreferenceKeys.PRIORITY_MUSCLES] = if (on) cur + code else cur - code
        }

    /** Pinned exercise ids — kept across regenerations when their muscle is trained. */
    val pinnedExercises: Flow<Set<String>> = context.forgePreferences.data
        .map { it[PreferenceKeys.PINNED_EXERCISES] ?: emptySet() }
    suspend fun togglePinned(libId: String, on: Boolean) =
        context.forgePreferences.edit { prefs ->
            val cur = prefs[PreferenceKeys.PINNED_EXERCISES] ?: emptySet()
            prefs[PreferenceKeys.PINNED_EXERCISES] = if (on) cur + libId else cur - libId
        }

    suspend fun completeOnboarding(name: String, useKgChoice: Boolean, goal: String, bodyweightLb: Double?) {
        context.forgePreferences.edit { prefs ->
            prefs[PreferenceKeys.ONBOARDING_DONE] = true
            prefs[PreferenceKeys.WELCOMED] = true
            if (name.isNotBlank()) prefs[PreferenceKeys.USER_NAME] = name
            prefs[PreferenceKeys.USE_KG] = useKgChoice
            if (goal.isNotBlank()) prefs[PreferenceKeys.USER_GOAL] = goal
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
            val name = prefs[PreferenceKeys.USER_NAME]
            val goal = prefs[PreferenceKeys.USER_GOAL]
            prefs.clear()
            onboarding?.let { prefs[PreferenceKeys.ONBOARDING_DONE] = it }
            welcomed?.let { prefs[PreferenceKeys.WELCOMED] = it }
            name?.let { prefs[PreferenceKeys.USER_NAME] = it }
            goal?.let { prefs[PreferenceKeys.USER_GOAL] = it }
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
