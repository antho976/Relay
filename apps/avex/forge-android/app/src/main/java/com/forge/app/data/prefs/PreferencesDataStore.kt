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
    /** Cardio distance/pace in miles. ABSENT = derive from [USE_KG] (lb→miles, kg→km); only an
     *  explicit pick in onboarding or Settings persists this and breaks the tie to the weight unit. */
    val USE_MILES = booleanPreferencesKey("use_miles")

    // ─── Health Connect bodyweight sync (HC-3) ────────────────────────────────
    /** When ON (and the WeightRecord write permission is granted), Avex mirrors each weigh-in to
     *  Health Connect. Off by default — write-back is strictly opt-in. */
    val HC_WRITE_BODYWEIGHT = booleanPreferencesKey("hc_write_bodyweight")
    /** When ON (and the ActiveCaloriesBurned write permission is granted), Avex writes each finished
     *  session's estimated active calories to Health Connect (HC-4). Off by default — strictly opt-in. */
    val HC_WRITE_CALORIES = booleanPreferencesKey("hc_write_calories")

    // ─── Appearance (#35a) ────────────────────────────────────────────────────
    val AMOLED_MODE = booleanPreferencesKey("amoled_mode")
    /** Stored hex string e.g. "#EF4444". Empty = use default. Reserved for UI pass. */
    val ACCENT_COLOR_HEX = stringPreferencesKey("accent_color_hex")
    /** When false the app goes monochrome — highlights use a neutral tone instead of the accent. */
    val ACCENT_ENABLED = booleanPreferencesKey("accent_enabled")
    /** Font choice key. Reserved for UI pass. */
    val FONT_CHOICE = stringPreferencesKey("font_choice")

    // ─── Locale (#116) ────────────────────────────────────────────────────────
    /** "MM/dd/yyyy" or "dd/MM/yyyy" */
    val DATE_FORMAT = stringPreferencesKey("date_format")
    val TIMEZONE = stringPreferencesKey("timezone")
    /** IANA zone ids the user has starred — pinned to the top of the timezone picker. */
    val FAVORITE_TIMEZONES = stringSetPreferencesKey("favorite_timezones")
    val TIME_FORMAT_24H = booleanPreferencesKey("time_format_24h")
    val FIRST_DAY_MONDAY = booleanPreferencesKey("first_day_monday")

    // ─── Feel (#118) ──────────────────────────────────────────────────────────
    /** "off" | "light" | "medium" | "strong" */
    val HAPTIC_STRENGTH = stringPreferencesKey("haptic_strength")

    // ─── Notifications (#122) ─────────────────────────────────────────────────
    val QUIET_HOURS_ENABLED = booleanPreferencesKey("quiet_hours_enabled")
    val QUIET_HOURS_START = intPreferencesKey("quiet_hours_start")  // 0–23
    val QUIET_HOURS_END = intPreferencesKey("quiet_hours_end")      // 0–23
    /** Daily "train today" reminder (engagement). Off by default; hour is 0–23, default 18 (6pm). */
    val TRAINING_REMINDER_ENABLED = booleanPreferencesKey("training_reminder_enabled")
    val TRAINING_REMINDER_HOUR = intPreferencesKey("training_reminder_hour")
    /** Per-type opt-outs (N2). Recap + rest-timer alert default ON; the reminder defaults OFF above. */
    val WEEKLY_RECAP_ENABLED = booleanPreferencesKey("weekly_recap_enabled")
    val REST_TIMER_ALERT_ENABLED = booleanPreferencesKey("rest_timer_alert_enabled")
    /** Whether the one-time notification-permission rationale has been shown (N1). */
    val NOTIF_PERM_ASKED = booleanPreferencesKey("notif_perm_asked")
    /** Set true once the first workout is finished. First-touch onboarding cards gate on this so they
     *  never reappear for a returning user — it lives in DataStore, so it survives a DB wipe/restore. */
    val FIRST_WORKOUT_DONE = booleanPreferencesKey("first_workout_done")

    // ─── Monthly PR target (#84) ──────────────────────────────────────────────
    /** Target PRs per calendar month. 0 = no goal set. */
    val MONTHLY_PR_TARGET = intPreferencesKey("monthly_pr_target")

    // ─── Equipment context (#44) ──────────────────────────────────────────────
    /** Set of Equipment code strings the user has available. Empty = all equipment assumed available. */
    val AVAILABLE_EQUIPMENT = stringSetPreferencesKey("available_equipment")
    /**
     * Curated/frozen exercise-pool ids (a preset such as the Developer's preset). Absent = no
     * curation (ordinary equipment filtering). When present, generation, swaps and the like/dislike
     * screen all draw only from these ids.
     */
    val FROZEN_EXERCISE_IDS = stringSetPreferencesKey("frozen_exercise_ids")

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
    /** Set true the first (and only) time the default split is auto-seeded, so a deliberately empty
     *  plan (build-your-own / cleared) is never silently re-seeded on a later relaunch. */
    val PROGRAM_SEEDED = booleanPreferencesKey("program_seeded")
    /** User's initial goal: "build_muscle" | "lose_weight" | "get_stronger" | "general_fitness" */
    val USER_GOAL = stringPreferencesKey("user_goal")
    /** User's sex for bodyweight-relative strength standards: "male" | "female" | "" (unspecified). */
    val USER_SEX = stringPreferencesKey("user_sex")
    /** "Go with the flow" — no fixed program; the home leads with freestyle logging instead of day
     *  cards. A seed program still exists under the hood; this only changes what the UI surfaces. */
    val FREESTYLE_MODE = booleanPreferencesKey("freestyle_mode")
    /** Whether the Coach feature is surfaced at all (tab + banners). Off → coach hidden until the user
     *  re-enables it in Settings. Declined during onboarding for the no-plan / make-your-own modes. */
    val COACH_ENABLED = booleanPreferencesKey("coach_enabled")

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
    /** Day-aware scheduling: "sequence" (legacy day-after-last) | "weekday" (fixed weekly plan). */
    val SCHEDULE_MODE = stringPreferencesKey("schedule_mode")
    /** Weekly schedule: comma-joined 7 slots, Mon..Sun, each a program day key or "" for rest. */
    val SCHEDULE_WEEKLY = stringPreferencesKey("schedule_weekly")
    /** N for the "every_n" cadence. */
    val ROTATION_EVERY_N = intPreferencesKey("rotation_every_n")
    /** Unit for ROTATION_EVERY_N: "sessions" | "weeks". */
    val ROTATION_UNIT = stringPreferencesKey("rotation_unit")
    /** Progress toward the next rotation. */
    val ROTATION_COUNTER = intPreferencesKey("rotation_counter")
    val LAST_ROTATED_AT_MS = longPreferencesKey("last_rotated_at_ms")
    /**
     * When the current deload week was applied (epoch-ms). 0 = not in a deload week. Persists the
     * deload so auto-rotation pauses and DeloadAdvisor suppression fires (auto-coach seam, finding 18).
     */
    val DELOAD_WEEK_START_MS = longPreferencesKey("deload_week_start_ms")
    /** ExerciseLibrary ids the user LIKES — weighted up in selection (recurs more). */
    val LIKED_EXERCISES = stringSetPreferencesKey("liked_exercises")
    /** ExerciseLibrary ids the user DISLIKES — excluded from generation + swaps. */
    val DISLIKED_EXERCISES = stringSetPreferencesKey("disliked_exercises")
    /** Whether, after a "Make default" swap, Avex offers to dislike the swapped-out exercise.
     *  Default ON; the prompt's "Never ask" button or the Settings toggle turns it off. */
    val SWAP_DISLIKE_PROMPT_ENABLED = booleanPreferencesKey("swap_dislike_prompt_enabled")

    // ─── Cardio layer (program-unlock Phase 6) ────────────────────────────────
    /** Weekly cardio goal in minutes. 0 = no goal. */
    val CARDIO_WEEKLY_TARGET_MIN = intPreferencesKey("cardio_weekly_target_min")

    /** Set true once the user dismisses the "connect a watch/ring" hint on the cardio screen, so it
     *  never reappears. Lives in DataStore so it survives a DB wipe/restore. */
    val CARDIO_WEARABLE_HINT_DISMISSED = booleanPreferencesKey("cardio_wearable_hint_dismissed")

    /** Default rest between sets (seconds), per movement type — the base the RestAdvisor builds on.
     *  Absent ⇒ the canonical 180 (compound) / 90 (isolation). */
    val REST_COMPOUND_SECONDS = intPreferencesKey("rest_compound_seconds")
    val REST_ISOLATION_SECONDS = intPreferencesKey("rest_isolation_seconds")

    /** Weight of one plate (in lb) for plate-loaded machine/cable exercises. Default 15 (MWM-989). */
    val PLATE_WEIGHT_LB = doublePreferencesKey("plate_weight_lb")

    /** Heaviest dumbbell owned (lb). Absent/0 = no ceiling (auto-coach Phase 0). */
    val MAX_DB_WEIGHT_LB = doublePreferencesKey("max_db_weight_lb")

    /** Coach mode (auto-coach Phase 4): "suggest" (default) | "auto" (earned auto-apply). */
    val COACH_MODE = stringPreferencesKey("coach_mode")

    /** Whether the one-time "how your coach learns" intro card has been dismissed (CO6). */
    val COACH_BRIEF_INTRO_SEEN = booleanPreferencesKey("coach_brief_intro_seen")

    /** ISO week id of the most recent Week Brief the user has seen — drives the "new report" banner. */
    val LAST_SEEN_COACH_WEEK_ID = stringPreferencesKey("last_seen_coach_week_id")

    // ─── Rank-up celebration (#rank_up_celebration) ───────────────────────────
    /** Ordinal of the RankTier the user last saw the profile at — used to detect a tier cross
     *  and trigger the one-shot confetti/haptic celebration. -1 = never seen (first open). */
    val LAST_SEEN_RANK_TIER_ORDINAL = intPreferencesKey("last_seen_rank_tier_ordinal")

    /** Last Stats sub-tab the user settled on, stored by enum NAME so inserting/reordering tabs can't
     *  silently restore the wrong one (an ordinal Int would shift). Reopening Stats deep-links here (S4).
     *  (The legacy ordinal key `last_stats_tab` is intentionally abandoned — unset here just lands on Overview.) */
    val LAST_STATS_TAB_NAME = stringPreferencesKey("last_stats_tab_name")
}
