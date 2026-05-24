package com.forge.app.program

enum class TrophyCategory(val code: String, val displayName: String) {
    FIRSTS("firsts", "Firsts"),
    CONSISTENCY("consistency", "Consistency"),
    STRENGTH("strength", "Strength")
}

/**
 * The 11 icon types the trophy catalog draws from. Each is a Compose vector or Canvas
 * shape rendered in Phase 6 — the [code] field matches the React prototype's identifier
 * so design references stay in sync.
 */
enum class TrophyIcon(val code: String) {
    SPARK("spark"),
    STAR("star"),
    FLAME("flame"),
    SWAP("swap"),
    CHECK("check"),
    STACK("stack"),
    CROWN("crown"),
    DOOR("door"),
    FOUR("four"),
    DUMBBELL("dumbbell"),
    BOLT("bolt"),
    CLOCK("clock"),
    CALENDAR("calendar"),
    REPEAT("repeat"),
    HEART("heart")
}

/**
 * Declarative description of when a trophy unlocks. Stored as a sealed hierarchy so
 * the rule is data — pattern-matched by [com.forge.app.domain.trophy.TrophyEvaluator]
 * against a stats snapshot in Phase 6. No function references, no reflection, no JS-style
 * closures: trivial to log, test, and serialize.
 */
sealed interface UnlockRule {
    data class TotalSessionsAtLeast(val n: Int) : UnlockRule
    data class TotalPRsAtLeast(val n: Int) : UnlockRule
    data class BrutalCountAtLeast(val n: Int) : UnlockRule
    data class SwapCountAtLeast(val n: Int) : UnlockRule
    data class FullTargetHitsAtLeast(val n: Int) : UnlockRule
    data class WorkoutsCompletedAtLeast(val n: Int) : UnlockRule
    data class DistinctDaysTrainedAtLeast(val n: Int) : UnlockRule
    data class MaxBenchAtLeast(val lb: Double) : UnlockRule
    data class MaxSquatAtLeast(val lb: Double) : UnlockRule
    data class MaxSessionVolumeAtLeast(val lb: Double) : UnlockRule
    // ─── New rules for #105 ──────────────────────────────────────────────────
    data class MaxStreakAtLeast(val days: Int) : UnlockRule
    data class EarlyBirdSessionsAtLeast(val n: Int) : UnlockRule
    data class NightOwlSessionsAtLeast(val n: Int) : UnlockRule
    data class SundaysTrainedAtLeast(val n: Int) : UnlockRule
    data class SessionDurationAtLeast(val minutes: Int) : UnlockRule
    data class SessionDurationAtMost(val minutes: Int) : UnlockRule
    data class MaxSingleExerciseRepsAtLeast(val n: Int) : UnlockRule
    data object ComebackKidRule : UnlockRule
    data object ConsistencyKingRule : UnlockRule
    data object VarietyPackRule : UnlockRule
}

/** Difficulty tier for a trophy — affects display and cumulative score (#150). */
enum class TrophyTier(val display: String, val points: Int, val color: Long) {
    EASY("Easy", 10, 0xFF4CAF50),
    MEDIUM("Medium", 25, 0xFF2196F3),
    HARD("Hard", 50, 0xFFFF9800),
    LEGENDARY("Legendary", 100, 0xFFFFD700)
}

data class Trophy(
    val id: String,
    val name: String,
    val description: String,
    val category: TrophyCategory,
    val icon: TrophyIcon,
    val unlock: UnlockRule,
    /** Difficulty tier (#150). Defaults to MEDIUM if not specified. */
    val tier: TrophyTier = TrophyTier.MEDIUM
)

object Trophies {

    val all: List<Trophy> = listOf(
        // ─── Firsts ───────────────────────────────────────────────────────────────
        Trophy(
            id = "first_session",
            name = "First Session",
            description = "Logged your first exercise",
            category = TrophyCategory.FIRSTS,
            icon = TrophyIcon.SPARK,
            unlock = UnlockRule.TotalSessionsAtLeast(1)
        ),
        Trophy(
            id = "first_pr",
            name = "First PR",
            description = "Set your first personal record",
            category = TrophyCategory.FIRSTS,
            icon = TrophyIcon.STAR,
            unlock = UnlockRule.TotalPRsAtLeast(1)
        ),
        Trophy(
            id = "first_brutal",
            name = "Hit the Wall",
            description = "Rated a session \"Brutal\"",
            category = TrophyCategory.FIRSTS,
            icon = TrophyIcon.FLAME,
            unlock = UnlockRule.BrutalCountAtLeast(1)
        ),
        Trophy(
            id = "first_swap",
            name = "Made It Mine",
            description = "Used the swap feature",
            category = TrophyCategory.FIRSTS,
            icon = TrophyIcon.SWAP,
            unlock = UnlockRule.SwapCountAtLeast(1)
        ),
        Trophy(
            id = "all_sets_target",
            name = "Full Slate",
            description = "Hit all target sets at target reps in one exercise",
            category = TrophyCategory.FIRSTS,
            icon = TrophyIcon.CHECK,
            unlock = UnlockRule.FullTargetHitsAtLeast(1)
        ),

        // ─── Consistency ──────────────────────────────────────────────────────────
        Trophy(
            id = "sessions_5",
            name = "Five Strong",
            description = "5 exercises logged",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.STACK,
            unlock = UnlockRule.TotalSessionsAtLeast(5)
        ),
        Trophy(
            id = "sessions_10",
            name = "Double Digits",
            description = "10 exercises logged",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.STACK,
            unlock = UnlockRule.TotalSessionsAtLeast(10)
        ),
        Trophy(
            id = "sessions_25",
            name = "Quarter Hundred",
            description = "25 exercises logged",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.STACK,
            unlock = UnlockRule.TotalSessionsAtLeast(25)
        ),
        Trophy(
            id = "sessions_50",
            name = "Halfway to a Hundred",
            description = "50 exercises logged",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.STACK,
            unlock = UnlockRule.TotalSessionsAtLeast(50)
        ),
        Trophy(
            id = "sessions_100",
            name = "Century",
            description = "100 exercises logged",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.CROWN,
            unlock = UnlockRule.TotalSessionsAtLeast(100)
        ),
        Trophy(
            id = "first_workout_complete",
            name = "Through the Door",
            description = "Finished a full workout",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.DOOR,
            unlock = UnlockRule.WorkoutsCompletedAtLeast(1)
        ),
        Trophy(
            id = "workouts_10",
            name = "Showing Up",
            description = "Completed 10 full workouts",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.DOOR,
            unlock = UnlockRule.WorkoutsCompletedAtLeast(10)
        ),
        Trophy(
            id = "all_4_days",
            name = "Full Week",
            description = "Trained all 4 days at least once",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.FOUR,
            unlock = UnlockRule.DistinctDaysTrainedAtLeast(4)
        ),

        // ─── Strength ─────────────────────────────────────────────────────────────
        Trophy(
            id = "pr_5",
            name = "PR Hunter",
            description = "Set 5 personal records",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.STAR,
            unlock = UnlockRule.TotalPRsAtLeast(5)
        ),
        Trophy(
            id = "pr_10",
            name = "On a Roll",
            description = "Set 10 personal records",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.STAR,
            unlock = UnlockRule.TotalPRsAtLeast(10)
        ),
        Trophy(
            id = "pr_25",
            name = "Forged Strength",
            description = "Set 25 personal records",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.CROWN,
            unlock = UnlockRule.TotalPRsAtLeast(25)
        ),
        Trophy(
            id = "bench_25",
            name = "25lb Bench Club",
            description = "Bench pressed 25lb DBs for any reps",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.DUMBBELL,
            unlock = UnlockRule.MaxBenchAtLeast(25.0)
        ),
        Trophy(
            id = "bench_30",
            name = "30lb Bench Club",
            description = "Bench pressed 30lb DBs for any reps",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.DUMBBELL,
            unlock = UnlockRule.MaxBenchAtLeast(30.0)
        ),
        Trophy(
            id = "squat_30",
            name = "30lb Goblet",
            description = "Goblet squatted 30lb for any reps",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.DUMBBELL,
            unlock = UnlockRule.MaxSquatAtLeast(30.0)
        ),
        Trophy(
            id = "volume_3000",
            name = "Volume King",
            description = "Hit 3000+ lb total volume in one session",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.BOLT,
            unlock = UnlockRule.MaxSessionVolumeAtLeast(3000.0)
        ),
        Trophy(
            id = "volume_5000",
            name = "Volume Beast",
            description = "Hit 5000+ lb total volume in one session",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.BOLT,
            unlock = UnlockRule.MaxSessionVolumeAtLeast(5000.0)
        ),
        Trophy(
            id = "volume_10000",
            name = "Volume Mountain",
            description = "Hit 10,000+ lb total volume in one session",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.BOLT,
            unlock = UnlockRule.MaxSessionVolumeAtLeast(10_000.0)
        ),
        Trophy(
            id = "rep_machine",
            name = "Rep Machine",
            description = "Hit 100+ reps in a single exercise",
            category = TrophyCategory.STRENGTH,
            icon = TrophyIcon.REPEAT,
            unlock = UnlockRule.MaxSingleExerciseRepsAtLeast(100)
        ),

        // ─── Consistency (new) ────────────────────────────────────────────────
        Trophy(
            id = "streak_3",
            name = "Three Days in a Row",
            description = "Trained 3 consecutive days",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.FLAME,
            unlock = UnlockRule.MaxStreakAtLeast(3)
        ),
        Trophy(
            id = "early_bird",
            name = "Early Bird",
            description = "Completed 5 sessions before 7 am",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.CLOCK,
            unlock = UnlockRule.EarlyBirdSessionsAtLeast(5)
        ),
        Trophy(
            id = "night_owl",
            name = "Night Owl",
            description = "Completed 5 sessions after 9 pm",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.CLOCK,
            unlock = UnlockRule.NightOwlSessionsAtLeast(5)
        ),
        Trophy(
            id = "iron_sunday",
            name = "Iron Sunday",
            description = "Trained on 4 different Sundays",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.CALENDAR,
            unlock = UnlockRule.SundaysTrainedAtLeast(4)
        ),
        Trophy(
            id = "comeback_kid",
            name = "Comeback Kid",
            description = "Set a PR within 2 weeks of a 5+ day training gap",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.HEART,
            unlock = UnlockRule.ComebackKidRule
        ),
        Trophy(
            id = "consistency_king",
            name = "Consistency King",
            description = "No missed week in 3 months",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.CROWN,
            unlock = UnlockRule.ConsistencyKingRule
        ),
        Trophy(
            id = "variety_pack",
            name = "Variety Pack",
            description = "Trained all 4 muscle groups in one week",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.FOUR,
            unlock = UnlockRule.VarietyPackRule
        ),
        Trophy(
            id = "slow_burn",
            name = "Slow Burn",
            description = "Completed a 90+ minute session",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.CLOCK,
            unlock = UnlockRule.SessionDurationAtLeast(90)
        ),
        Trophy(
            id = "speed_demon",
            name = "Speed Demon",
            description = "Finished a full session in under 45 minutes",
            category = TrophyCategory.CONSISTENCY,
            icon = TrophyIcon.BOLT,
            unlock = UnlockRule.SessionDurationAtMost(45)
        )
    )

    fun byCategory(): Map<TrophyCategory, List<Trophy>> = all.groupBy { it.category }

    fun byId(id: String): Trophy? = all.firstOrNull { it.id == id }
}
