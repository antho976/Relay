package com.forge.app.program

import kotlin.random.Random

/** Inputs that drive generation (program-unlock Phase 2). */
data class GenerationParams(
    val daysPerWeek: Int,
    val emphasis: String = "balanced",
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
    val deload: Boolean = false,
    /**
     * Heaviest dumbbell the user owns (lb), null = no ceiling. Below [ProgramGenerator.LIGHT_DB_LB]
     * heavy STRENGTH slots steer toward plate-stack movements — the only real progressive-overload
     * path when the DBs max out light (auto-coach Phase 0).
     */
    val dbMaxLb: Double? = null,
    /**
     * Net learned weekly-set adjustment per muscle (CoachGenBias) — folded into the baseline by
     * [VolumeModel.allocate] so a regenerate keeps the coach's applied volume changes.
     */
    val volumeBias: Map<MuscleGroup, Int> = emptyMap(),
    /** Movements the coach tried that didn't land — softly down-weighted, never hard-banned. */
    val avoid: Set<String> = emptySet(),
    /**
     * Curated/frozen exercise pool (a preset such as the Developer's preset). When non-null,
     * generation draws ONLY from these library ids and ignores [available] equipment filtering —
     * locking the preset against any movement added to the library later.
     */
    val frozenIds: Set<String>? = null
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
 * the slot's scheme steers toward the right movement type (STRENGTH→compound, PUMP→isolation),
 * **repeated movement patterns within a day are penalized** so you don't get three of the same row
 * (program-unlock Phase 4 — generator intelligence), and each movement's [ExerciseDef.pickBias]
 * keeps niche accessories from headlining a slot as often as the muscle's default picks. Set counts
 * come from [VolumeModel] (frequency-aware). No Android/DB deps → unit-testable on the JVM.
 */
object ProgramGenerator {

    private const val LIKE_BOOST = 3.0
    private const val RECENT_PENALTY = 0.25
    /** How hard a heavy slot favours compounds / a pump slot favours isolation. */
    private const val ROLE_MATCH = 4.0
    private const val ROLE_MISMATCH = 0.3
    /** A unilateral compound (lunge, single-leg) is a fine accessory but a poor "heavy" lift. */
    private const val ROLE_UNILATERAL = 0.35
    /** AMRAP/timed movements can't express a heavy 6-10 — poor leads for a progressive-overload slot. */
    private const val ROLE_NON_LOADABLE = 0.5
    /** Down-weight a movement whose pattern was already used in this day (variety, not a hard rule). */
    private const val PATTERN_REPEAT_PENALTY = 0.25
    /** Multiplier for a movement that stresses a flagged problem area — strongly avoided, not banned. */
    private const val CONTRA_PENALTY = 0.08
    /** Volume multiplier for a deload week (Phase 4 periodization). */
    private const val DELOAD_FACTOR = 0.55
    /** Down-weight a movement already used *earlier this week* so multi-day splits vary across days. */
    private const val WEEK_REPEAT_PENALTY = 0.15
    /** A dumbbell ceiling below this counts as "light" — heavy slots then prefer the plate stack. */
    const val LIGHT_DB_LB = 50.0
    /** Multiplier on DUMBBELL movements in a STRENGTH slot when light DBs + a stack compound exist. */
    private const val LIGHT_DB_PENALTY = 0.3
    /** Multiplier for a movement the coach tried and the watcher failed (soft, like dislikes aren't). */
    private const val AVOID_PENALTY = 0.2

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
        // A deload lowers the per-slot floor to 1 so already-light slots actually drop (otherwise a
        // beginner's 2-set accessories would floor at 2 and the deload would be a no-op for them).
        val setsByDay = VolumeModel.allocate(
            template, focus, volumeFactor, minSets = if (params.deload) 1 else VolumeModel.MIN_SETS,
            bias = params.volumeBias
        )
        val maxDifficulty = GoalProfiles.maxDifficulty(params.experience)
        // Tracks picks across the WHOLE week so a muscle trained on two days gets different movements.
        val usedInWeek = HashSet<String>()
        val liftDays = template.mapIndexed { di, day ->
            val usedInDay = HashSet<String>()
            val usedPatterns = HashSet<MovementPattern>()
            val exercises = day.targets.mapIndexedNotNull { si, slot ->
                val avail = ExerciseLibrary.availablePool(available, params.frozenIds)
                    .filter { it.muscle == slot.muscle && it.id !in disliked }
                // Last-resort bodyweight fills only enter the pool when nothing the user actually owns
                // can train this muscle — so an equipped user never gets a bodyweight squat, but a
                // bodyweight-only / minimal setup is never starved into an empty day.
                val forMuscle = avail.filterNot { it.fallbackOnly }.ifEmpty { avail }
                // Never place the same exercise twice in one day: if the (equipment-limited) pool is
                // exhausted, DROP the slot (a slightly shorter day) rather than repeat a movement —
                // a duplicate reads as a bug, breaks per-exercise logging, and crashed the session list.
                val base = forMuscle.filterNot { it.id in usedInDay }
                // Experience caps movement difficulty, but never empty the slot — fall back if needed.
                val candidates = base.filter { it.difficulty.ordinal <= maxDifficulty.ordinal }.ifEmpty { base }
                // A pinned exercise for this muscle is forced into the slot when it's a valid candidate —
                // unless it stresses a flagged problem area: don't hard-place a movement the user flagged
                // as harmful, let the weighting (which down-weights it) decide instead.
                val pinned = candidates.firstOrNull {
                    it.id in params.pinned &&
                        ExerciseLibrary.contraindicationsOf(it).none { area -> area in params.problemAreas }
                }
                // A heavy STRENGTH slot must lead with a compound when one is available — an
                // isolation (leg curl, fly) may only headline if it's all the equipment allows.
                val pool = if (slot.scheme == RepScheme.STRENGTH)
                    candidates.filter { ExerciseTag.COMPOUND in it.tags }.ifEmpty { candidates }
                else candidates
                // Light dumbbells can't progressively load a heavy slot — when the user's heaviest
                // DB is below the threshold and the pool offers a loadable non-dumbbell compound
                // (barbell / machine / plate stack), steer the STRENGTH pick toward it (the only real
                // overload path; auto-coach Phase 0, generalized to WEIGHT movements 2026-06-11).
                val preferStack = slot.scheme == RepScheme.STRENGTH &&
                    (params.dbMaxLb ?: Double.MAX_VALUE) < LIGHT_DB_LB &&
                    pool.any {
                        (it.unit == ExerciseUnit.PLATES || it.unit == ExerciseUnit.WEIGHT) &&
                            ExerciseTag.COMPOUND in it.tags
                    }
                val pick = pinned ?: weightedPick(pool, rng) { def ->
                    val likeW = if (def.id in liked) LIKE_BOOST else 1.0
                    val recentW = if (def.id in recent) RECENT_PENALTY else 1.0
                    val weekW = if (def.id in usedInWeek) WEEK_REPEAT_PENALTY else 1.0
                    val pattern = ExerciseLibrary.patternOf(def)
                    val patternW = if (pattern != MovementPattern.ISOLATION && pattern in usedPatterns)
                        PATTERN_REPEAT_PENALTY else 1.0
                    val contraW = if (ExerciseLibrary.contraindicationsOf(def).any { it in params.problemAreas })
                        CONTRA_PENALTY else 1.0
                    val stackW = if (preferStack && def.unit == ExerciseUnit.DUMBBELL) LIGHT_DB_PENALTY else 1.0
                    val avoidW = if (def.id in params.avoid) AVOID_PENALTY else 1.0
                    likeW * recentW * weekW * roleFactor(def, slot.scheme) * patternW * contraW * def.pickBias * stackW * avoidW
                } ?: return@mapIndexedNotNull null
                usedInDay += pick.id
                usedInWeek += pick.id
                ExerciseLibrary.patternOf(pick).takeIf { it != MovementPattern.ISOLATION }
                    ?.let { usedPatterns += it }
                GeneratedExercise(pick.id, setsByDay[di][si], repsFor(pick, slot.scheme, params.goal))
            }
            GeneratedDay(day.key, day.name, day.word, day.accentHex, day.key, exercises)
        }
        return liftDays
    }

    /**
     * Heavy (STRENGTH) slots favour **bilateral** compounds (squat / hinge / press / row) as the lead
     * lift, demote unilateral compounds (lunges, single-leg) to accessory weight, and avoid isolation.
     * PUMP slots favour isolation; HYPERTROPHY is neutral.
     */
    private fun roleFactor(def: ExerciseDef, scheme: RepScheme): Double = when (scheme) {
        RepScheme.STRENGTH -> when {
            ExerciseTag.COMPOUND !in def.tags -> ROLE_MISMATCH
            isUnilateral(def) -> ROLE_UNILATERAL
            !def.defaultReps.matches(NUMERIC_REPS) -> ROLE_NON_LOADABLE
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
