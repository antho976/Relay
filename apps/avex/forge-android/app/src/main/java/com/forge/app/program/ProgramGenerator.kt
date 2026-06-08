package com.forge.app.program

import kotlin.random.Random

/** Inputs that drive generation (program-unlock Phase 2). */
data class GenerationParams(
    val daysPerWeek: Int,
    val emphasis: String = "balanced",
    /** Dedicated standalone cardio days appended to the week (Phase 6 days mode). */
    val cardioDays: Int = 0,
    /** Onboarding goal (`USER_GOAL`) — reshapes rep ranges via [GoalProfiles]. */
    val goal: String = "build_muscle",
    /** Training experience — scales volume + filters movement difficulty via [GoalProfiles]. */
    val experience: String = "intermediate",
    /** Flagged problem areas — movements stressing these are strongly avoided (Phase 3). */
    val problemAreas: Set<ProblemArea> = emptySet(),
    /** Muscles to bias extra volume toward (granular emphasis, Phase 3). */
    val priorityMuscles: Set<MuscleGroup> = emptySet(),
    /** Library ids the user pinned — forced into a slot of their muscle when possible (Phase 3). */
    val pinned: Set<String> = emptySet(),
    /** Deload week — cuts volume for recovery (Phase 4 periodization). */
    val deload: Boolean = false
)

data class GeneratedExercise(val libId: String, val sets: Int, val reps: String)

data class GeneratedDay(
    val key: String,
    val name: String,
    val word: String,
    val accentHex: String,
    val archetype: String,
    val exercises: List<GeneratedExercise>
)

/**
 * Pure, deterministic-by-seed program generator. Picks the split for the requested day-count, then
 * fills each day's muscle slots from the equipment-filtered library. Selection is weighted so:
 * **dislikes are excluded, likes weighted up, [recent] picks down-weighted** (rotation variety),
 * the slot's scheme steers toward the right movement type (STRENGTH→compound, PUMP→isolation), and
 * **repeated movement patterns within a day are penalized** so you don't get three of the same row
 * (program-unlock Phase 4 — generator intelligence). Set counts come from [VolumeModel] (frequency-
 * aware). No Android/DB deps → unit-testable on the JVM.
 */
object ProgramGenerator {

    private const val LIKE_BOOST = 3.0
    private const val RECENT_PENALTY = 0.25
    private const val CARDIO_ACCENT = "#3FA7B5"
    /** How hard a heavy slot favours compounds / a pump slot favours isolation. */
    private const val ROLE_MATCH = 4.0
    private const val ROLE_MISMATCH = 0.3
    /** A unilateral compound (lunge, single-leg) is a fine accessory but a poor "heavy" lift. */
    private const val ROLE_UNILATERAL = 0.5
    /** Down-weight a movement whose pattern was already used in this day (variety, not a hard rule). */
    private const val PATTERN_REPEAT_PENALTY = 0.35
    /** Multiplier for a movement that stresses a flagged problem area — strongly avoided, not banned. */
    private const val CONTRA_PENALTY = 0.08
    /** Volume multiplier for a deload week (Phase 4 periodization). */
    private const val DELOAD_FACTOR = 0.55
    /** Down-weight a movement already used *earlier this week* so multi-day splits vary across days. */
    private const val WEEK_REPEAT_PENALTY = 0.15

    fun generate(
        params: GenerationParams,
        available: Set<Equipment>,
        liked: Set<String>,
        disliked: Set<String>,
        recent: Set<String> = emptySet(),
        seed: Long = Random.nextLong()
    ): List<GeneratedDay> {
        val rng = Random(seed)
        val template = SplitTemplates.forDays(params.daysPerWeek)
        // Granular priority muscles + the coarse emphasis preset both feed extra volume.
        val focus = VolumeModel.emphasisFocus(params.emphasis) + params.priorityMuscles
        val volumeFactor = GoalProfiles.volumeFactor(params.experience) * (if (params.deload) DELOAD_FACTOR else 1.0)
        val setsByDay = VolumeModel.allocate(template, focus, volumeFactor)
        val maxDifficulty = GoalProfiles.maxDifficulty(params.experience)
        // Tracks picks across the WHOLE week so a muscle trained on two days gets different movements.
        val usedInWeek = HashSet<String>()
        val liftDays = template.mapIndexed { di, day ->
            val usedInDay = HashSet<String>()
            val usedPatterns = HashSet<MovementPattern>()
            val exercises = day.targets.mapIndexedNotNull { si, slot ->
                val forMuscle = ExerciseLibrary.forMuscle(slot.muscle).filter { def ->
                    def.id !in disliked && isAvailable(def, available)
                }
                // Prefer movements not already used today; but if the (equipment-limited) pool is
                // exhausted, allow a repeat rather than dropping the slot and silently undersizing the
                // day (e.g. 3 BACK slots with only 2 available back lifts on a dumbbells+bench setup).
                val base = forMuscle.filterNot { it.id in usedInDay }.ifEmpty { forMuscle }
                // Experience caps movement difficulty, but never empty the slot — fall back if needed.
                val candidates = base.filter { it.difficulty.ordinal <= maxDifficulty.ordinal }.ifEmpty { base }
                // A pinned exercise for this muscle is forced into the slot when it's a valid candidate.
                val pinned = candidates.firstOrNull { it.id in params.pinned }
                val pick = pinned ?: weightedPick(candidates, rng) { def ->
                    val likeW = if (def.id in liked) LIKE_BOOST else 1.0
                    val recentW = if (def.id in recent) RECENT_PENALTY else 1.0
                    val weekW = if (def.id in usedInWeek) WEEK_REPEAT_PENALTY else 1.0
                    val pattern = ExerciseLibrary.patternOf(def)
                    val patternW = if (pattern != MovementPattern.ISOLATION && pattern in usedPatterns)
                        PATTERN_REPEAT_PENALTY else 1.0
                    val contraW = if (ExerciseLibrary.contraindicationsOf(def).any { it in params.problemAreas })
                        CONTRA_PENALTY else 1.0
                    likeW * recentW * weekW * roleFactor(def, slot.scheme) * patternW * contraW
                } ?: return@mapIndexedNotNull null
                usedInDay += pick.id
                usedInWeek += pick.id
                ExerciseLibrary.patternOf(pick).takeIf { it != MovementPattern.ISOLATION }
                    ?.let { usedPatterns += it }
                GeneratedExercise(pick.id, setsByDay[di][si], repsFor(pick, slot.scheme, params.goal))
            }
            GeneratedDay(day.key, day.name, day.word, day.accentHex, day.key, exercises)
        }
        // Dedicated cardio days (no lift slots) appended after the lift days (Phase 6 days mode).
        val cardioDays = (0 until params.cardioDays).map { i ->
            GeneratedDay("cardio-$i", "Cardio", "CARDIO", CARDIO_ACCENT, "cardio", emptyList())
        }
        return liftDays + cardioDays
    }

    /** Available if no equipment is configured (empty = all) or every required item is on hand. */
    private fun isAvailable(def: ExerciseDef, available: Set<Equipment>): Boolean =
        available.isEmpty() || def.equipment.all { it == Equipment.BODYWEIGHT_ONLY || it in available }

    /**
     * Heavy (STRENGTH) slots favour **bilateral** compounds (squat / hinge / press / row) as the lead
     * lift, demote unilateral compounds (lunges, single-leg) to accessory weight, and avoid isolation.
     * PUMP slots favour isolation; HYPERTROPHY is neutral.
     */
    private fun roleFactor(def: ExerciseDef, scheme: RepScheme): Double = when (scheme) {
        RepScheme.STRENGTH -> when {
            ExerciseTag.COMPOUND !in def.tags -> ROLE_MISMATCH
            isUnilateral(def) -> ROLE_UNILATERAL
            else -> ROLE_MATCH
        }
        RepScheme.PUMP -> if (ExerciseTag.ISOLATION in def.tags) 2.0 else 0.5
        RepScheme.HYPERTROPHY -> 1.0
    }

    /** Per-side movements (lunges, step-ups, single-leg work) — flagged by per-leg reps or LUNGE pattern. */
    private fun isUnilateral(def: ExerciseDef): Boolean =
        def.defaultReps.contains("/") || ExerciseLibrary.patternOf(def) == MovementPattern.LUNGE

    /** Numeric ranges like "8-10" / "15" take the goal-adjusted scheme reps; "AMRAP"/"30-60s"/"10/leg" stay. */
    private val NUMERIC_REPS = Regex("""^\d+(-\d+)?$""")
    private fun repsFor(def: ExerciseDef, scheme: RepScheme, goal: String): String =
        if (def.defaultReps.matches(NUMERIC_REPS)) GoalProfiles.reps(goal, scheme) else def.defaultReps

    private inline fun weightedPick(
        items: List<ExerciseDef>,
        rng: Random,
        weight: (ExerciseDef) -> Double
    ): ExerciseDef? {
        if (items.isEmpty()) return null
        val weights = items.map { weight(it).coerceAtLeast(0.0001) }
        val total = weights.sum()
        var r = rng.nextDouble() * total
        for (i in items.indices) {
            r -= weights[i]
            if (r <= 0.0) return items[i]
        }
        return items.last()
    }
}
