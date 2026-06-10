package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.datastore.preferences.preferencesDataStore

/**
 * Single DataStore instance for app-wide preferences. Property delegate is the
 * recommended way to declare a DataStore — it manages the file path, schema migration,
 * and prevents accidentally creating multiple stores for the same file.
 */
val Context.forgePreferences: DataStore<Preferences> by preferencesDataStore(name = "forge_settings")

/** Centralised keys so misspellings are caught at compile time. */
object PreferenceKeys {
    val WELCOMED = androidx.datastore.preferences.core.booleanPreferencesKey("welcomed")
    /** IDs of one-shot milestone toasts already shown to the user (#56). */
    val SHOWN_MILESTONES = stringSetPreferencesKey("shown_milestones")

    // ─── Per-day accent color (#65) ───────────────────────────────────────────
    /** Stored as hex e.g. "#FF5733". Key per day: "day_color_upper-a". */
    fun dayColorKey(dayKey: String) = stringPreferencesKey("day_color_$dayKey")

    // ─── Overview tile order (#64) ────────────────────────────────────────────
    /** Comma-separated tile IDs in display order. Default = "gym,cardio,trophies". */
    val OVERVIEW_TILE_ORDER = stringPreferencesKey("overview_tile_order")

    // ─── Custom warmup (#120) — per-day key, e.g. "warmup_upper-a" ───────────
    fun warmupKey(dayKey: String) = stringPreferencesKey("warmup_$dayKey")

    // ─── Encouragement messages (#67) ────────────────────────────────────────
    val SHOW_ENCOURAGEMENT = booleanPreferencesKey("show_encouragement")

    // ─── Compact set logging (#35c) ───────────────────────────────────────────
    val COMPACT_SET_LOGGING = booleanPreferencesKey("compact_set_logging")

    // ─── Overview tile visibility (#121) ─────────────────────────────────────
    /** Set of tile IDs that are HIDDEN. Empty = show all. Tile IDs: "gym", "cardio", "trophies", "streak", "deload". */
    val HIDDEN_OVERVIEW_TILES = stringSetPreferencesKey("hidden_overview_tiles")

    // ─── Note templates (#113) ────────────────────────────────────────────────
    /** User-defined note templates shown as quick-insert chips. Default set is hard-coded in SettingsRepository. */
    val NOTE_TEMPLATES = stringSetPreferencesKey("note_templates")

    // ─── Units (#2) ───────────────────────────────────────────────────────────
    val USE_KG = booleanPreferencesKey("use_kg")

    // ─── Appearance (#35a) ────────────────────────────────────────────────────
    val AMOLED_MODE = booleanPreferencesKey("amoled_mode")
    /** Stored hex string e.g. "#EF4444". Empty = use default. Reserved for UI pass. */
    val ACCENT_COLOR_HEX = stringPreferencesKey("accent_color_hex")
    /** Accent emphasis for important text: "off" | "subtle" | "medium" | "strong". Default off. */
    val ACCENT_EMPHASIS = stringPreferencesKey("accent_emphasis")
    /** Font choice key. Reserved for UI pass. */
    val FONT_CHOICE = stringPreferencesKey("font_choice")

    // ─── Locale (#116) ────────────────────────────────────────────────────────
    /** "MM/dd/yyyy" or "dd/MM/yyyy" */
    val DATE_FORMAT = stringPreferencesKey("date_format")
    val TIMEZONE = stringPreferencesKey("timezone")
    val TIME_FORMAT_24H = booleanPreferencesKey("time_format_24h")
    val FIRST_DAY_MONDAY = booleanPreferencesKey("first_day_monday")

    // ─── Feel (#118) ──────────────────────────────────────────────────────────
    /** "off" | "light" | "medium" | "strong" */
    val HAPTIC_STRENGTH = stringPreferencesKey("haptic_strength")

    // ─── Notifications (#122) ─────────────────────────────────────────────────
    val QUIET_HOURS_ENABLED = booleanPreferencesKey("quiet_hours_enabled")
    val QUIET_HOURS_START = intPreferencesKey("quiet_hours_start")  // 0–23
    val QUIET_HOURS_END = intPreferencesKey("quiet_hours_end")      // 0–23

    // ─── Monthly PR target (#84) ──────────────────────────────────────────────
    /** Target PRs per calendar month. 0 = no goal set. */
    val MONTHLY_PR_TARGET = intPreferencesKey("monthly_pr_target")

    // ─── Equipment context (#44) ──────────────────────────────────────────────
    /** Set of Equipment code strings the user has available. Empty = all equipment assumed available. */
    val AVAILABLE_EQUIPMENT = stringSetPreferencesKey("available_equipment")

    // ─── Plan tomorrow (#147) ─────────────────────────────────────────────────
    /** Day key the user intends to train next (e.g. "upper-a"). Empty = not set. */
    val PLANNED_NEXT_DAY = stringPreferencesKey("planned_next_day")

    // ─── Privacy mode (#152) ──────────────────────────────────────────────────
    val PRIVACY_MODE = booleanPreferencesKey("privacy_mode")

    // ─── Warmup disable (#156) ────────────────────────────────────────────────
    /** Epoch-ms until which warmup should be auto-skipped. 0 = not disabled. */
    val WARMUP_DISABLED_UNTIL_MS = longPreferencesKey("warmup_disabled_until_ms")

    // ─── Onboarding (#1) ──────────────────────────────────────────────────────
    /** User's display name set during onboarding. Empty = not set. */
    val USER_NAME = stringPreferencesKey("user_name")
    /** Onboarding completed flag — distinct from WELCOMED which just tracks splash. */
    val ONBOARDING_DONE = booleanPreferencesKey("onboarding_done")
    /** User's initial goal: "build_muscle" | "lose_weight" | "get_stronger" | "general_fitness" */
    val USER_GOAL = stringPreferencesKey("user_goal")

    // ─── Program generation (program-unlock) ──────────────────────────────────
    /** Training days/week the program targets — drives the split template (3-day ≠ 7-day). */
    val DAYS_PER_WEEK = intPreferencesKey("days_per_week")
    /** Program focus, e.g. "balanced" | "arms_shoulders" | "legs". Empty = balanced. */
    val PROGRAM_EMPHASIS = stringPreferencesKey("program_emphasis")
    /** Training experience: "beginner" | "intermediate" | "advanced". Drives volume + difficulty filter. */
    val PROGRAM_EXPERIENCE = stringPreferencesKey("program_experience")
    /** Flagged problem areas (ProblemArea codes) — generation avoids movements that stress them. */
    val PROBLEM_AREAS = stringSetPreferencesKey("problem_areas")
    /** Priority muscles (MuscleGroup codes) — granular emphasis, extra volume. */
    val PRIORITY_MUSCLES = stringSetPreferencesKey("priority_muscles")
    /** Pinned exercise library ids — kept across regenerations when their muscle is trained. */
    val PINNED_EXERCISES = stringSetPreferencesKey("pinned_exercises")
    /** Rotation cadence: "never" | "every_n". */
    val ROTATION_CADENCE = stringPreferencesKey("rotation_cadence")
    /** N for the "every_n" cadence. */
    val ROTATION_EVERY_N = intPreferencesKey("rotation_every_n")
    /** Unit for ROTATION_EVERY_N: "sessions" | "weeks". */
    val ROTATION_UNIT = stringPreferencesKey("rotation_unit")
    /** Progress toward the next rotation. */
    val ROTATION_COUNTER = intPreferencesKey("rotation_counter")
    val LAST_ROTATED_AT_MS = longPreferencesKey("last_rotated_at_ms")
    /** ExerciseLibrary ids the user LIKES — weighted up in selection (recurs more). */
    val LIKED_EXERCISES = stringSetPreferencesKey("liked_exercises")
    /** ExerciseLibrary ids the user DISLIKES — excluded from generation + swaps. */
    val DISLIKED_EXERCISES = stringSetPreferencesKey("disliked_exercises")

    // ─── Cardio layer (program-unlock Phase 6) ────────────────────────────────
    /** Weekly cardio goal in minutes. 0 = no goal. */
    val CARDIO_WEEKLY_TARGET_MIN = intPreferencesKey("cardio_weekly_target_min")

    /** Weight of one plate (in lb) for plate-loaded machine/cable exercises. Default 15 (MWM-989). */
    val PLATE_WEIGHT_LB = doublePreferencesKey("plate_weight_lb")
}
