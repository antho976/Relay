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
    BOLT("bolt")
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
}

data class Trophy(
    val id: String,
    val name: String,
    val description: String,
    val category: TrophyCategory,
    val icon: TrophyIcon,
    val unlock: UnlockRule
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
        )
    )

    fun byCategory(): Map<TrophyCategory, List<Trophy>> = all.groupBy { it.category }

    fun byId(id: String): Trophy? = all.firstOrNull { it.id == id }
}
