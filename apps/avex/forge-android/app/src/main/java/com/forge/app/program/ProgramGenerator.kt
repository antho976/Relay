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
    /** Flagged problem areas exclude movements with a matching known contraindication. */
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
    /**
     * Per-muscle weekly caps measured from THIS athlete (Coach v3 D's PersonalProfile). Empty means
     * population defaults, which is what every generate used before the learning loop closed.
     */
    val personalCaps: Map<MuscleGroup, Int> = emptyMap(),
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
 * the slot's scheme steers toward the right movement type (STRENGTH→loadable compound, PUMP→isolation),
 * a slot's preferred [MuscleSlot.pattern] steers toward the right lead (a squat, not a trap-bar
 * deadlift, opens a quad day), **repeated movement patterns within a day are penalized as a group**
 * so five hinge candidates can't out-vote one leg curl (program-unlock Phase 4 — generator
 * intelligence), single-joint families (two lateral raises, two curls) count as repeats too, and
 * each movement's [ExerciseDef.pickBias] keeps niche accessories from headlining a slot as often as
 * the muscle's default picks. Pins land in the slot that fits their tags — an isolation never takes
 * the heavy slot. Set counts come from [VolumeModel] (frequency-aware). No Android/DB deps →
 * unit-testable on the JVM.
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
    /**
     * Total weight a *used* pattern's candidates share, relative to one fresh candidate. Applied as a
     * group (divided among the candidates of that pattern), not per candidate: per-candidate 0.25
     * let five stale hinges out-weigh the one fresh leg curl, so 38% of leg days ran two deadlift
     * variants (2026-09-21).
     */
    private const val PATTERN_REPEAT_PENALTY = 0.1
    /** Multiplier on a candidate whose pattern differs from the slot's preferred [MuscleSlot.pattern]. */
    private const val SLOT_PATTERN_MISMATCH = 0.15
    /**
     * Heavy-slot loadability tiers (2026-09-21). A barbell / plate-loaded / stack movement is the
     * progressive-overload path; dumbbells and kettlebells top out; bodyweight can't add load at all.
     * Applied in STRENGTH slots only — a DB-only setup is unaffected (every candidate shares a tier).
     */
    private const val LOAD_TIER_HANDHELD = 0.6
    private const val LOAD_TIER_BODYWEIGHT = 0.3
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
    /** Swap ranking: a candidate of the current movement's own pattern is the "fresh variation" a stall wants. */
    private const val SWAP_SAME_PATTERN = 1.5
    private const val SWAP_ROLE_MISMATCH = 0.3

    /**
     * The sets each day of a [daysPerWeek] split is planned to carry, before any equipment filter —
     * the same [VolumeModel] allocation [generate] runs, so onboarding can draw the week's shape
     * from a day-count alone without duplicating the volume math (or drifting from it). Emphasis,
     * bias and personal caps are onboarding's no-op defaults.
     */
    fun plannedSetsPerDay(daysPerWeek: Int, experience: String, goal: String = "build_muscle"): List<Int> =
        VolumeModel.allocate(
            SplitTemplates.forDays(daysPerWeek),
            volumeFactor = GoalProfiles.volumeFactor(experience),
            goal = goal
        ).map { it.sum() }

    /**
     * How much of [GenerationParams.volumeBias] the weekly cap actually lets through, per muscle.
     *
     * `VolumeModel.allocate` folds the coach's bias in and THEN runs the per-muscle cap trim, which
     * can take every set the bias just added — most easily when `PersonalProfile` has decided the
     * muscle isn't responsive and dropped its ceiling, so the un-biased baseline already sits at the
     * cap. The two never reconciled: the coach went on believing it held a +2 credit on a muscle
     * that received nothing, which permanently spent that muscle's ±2 drift budget (the planner
     * would never propose chest volume again) and had the Coach Lab reporting "+2 set(s) carried
     * forward" against a program where they did not exist.
     *
     * Two cheap pure allocations, no RNG and no exercise selection — this is the same arithmetic
     * [generate] runs, asked twice.
     */
    fun effectiveVolumeBias(params: GenerationParams): Map<MuscleGroup, Int> {
        if (params.volumeBias.isEmpty()) return emptyMap()
        val template = SplitTemplates.forDays(params.daysPerWeek)
        val focus = VolumeModel.emphasisFocus(params.emphasis) + params.priorityMuscles
        val volumeFactor = GoalProfiles.volumeFactor(params.experience) * (if (params.deload) DELOAD_FACTOR else 1.0)
        val minSets = if (params.deload) 1 else VolumeModel.MIN_SETS
        fun totalsPerMuscle(bias: Map<MuscleGroup, Int>): Map<MuscleGroup, Int> {
            val sets = VolumeModel.allocate(template, focus, volumeFactor, minSets, bias, params.goal, params.personalCaps)
            val out = HashMap<MuscleGroup, Int>()
            template.forEachIndexed { di, day ->
                day.targets.forEachIndexed { si, slot ->
                    out[slot.muscle] = (out[slot.muscle] ?: 0) + sets[di][si]
                }
            }
            return out
        }
        val withBias = totalsPerMuscle(params.volumeBias)
        val without = totalsPerMuscle(emptyMap())
        return params.volumeBias.keys.associateWith { m -> (withBias[m] ?: 0) - (without[m] ?: 0) }
    }

    /**
     * @param usedElsewhere library ids the *rest of the program* already carries when only one day is
     *   being re-rolled — seeded into the week-repeat penalty so the fresh day avoids what the
     *   unchanged days use (2026-09-21). Ignored for a whole-program generate.
     * @param onlyDay when non-null, only the day with this key is filled; every other day comes back
     *   with an empty exercise list. A single-day re-roll used to generate a phantom week and
     *   de-duplicate against *that* instead of the real one.
     */
    fun generate(
        params: GenerationParams,
        available: Set<Equipment>,
        liked: Set<String>,
        disliked: Set<String>,
        recent: Set<String> = emptySet(),
        seed: Long = Random.nextLong(),
        usedElsewhere: Set<String> = emptySet(),
        onlyDay: String? = null
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
            bias = params.volumeBias,
            goal = params.goal,
            personalCaps = params.personalCaps
        )
        val maxDifficulty = GoalProfiles.maxDifficulty(params.experience)
        val pool = ExerciseLibrary.availablePool(available, params.frozenIds)
        // Every filter that applies before selection, per muscle — shared by the slots AND the pin
        // pre-pass so a pin can never bypass what a slot honours.
        val candidatesByMuscle = HashMap<MuscleGroup, List<ExerciseDef>>()
        fun candidatesFor(muscle: MuscleGroup): List<ExerciseDef> = candidatesByMuscle.getOrPut(muscle) {
            val avail = pool
                .filter { it.muscle == muscle && it.id !in disliked }
                .filter { ExerciseLibrary.contraindicationsOf(it).none { area -> area in params.problemAreas } }
            // Last-resort bodyweight fills only enter the pool when nothing the user actually owns
            // can train this muscle — so an equipped user never gets a bodyweight squat, but a
            // bodyweight-only / minimal setup is never starved into an empty day.
            val forMuscle = avail.filterNot { it.fallbackOnly }.ifEmpty { avail }
            // Respect the experience ceiling even when it leaves a slot unavailable.
            forMuscle.filter { it.difficulty.ordinal <= maxDifficulty.ordinal }
        }
        // Tracks picks across the WHOLE week so a muscle trained on two days gets different movements.
        val usedInWeek = HashSet<String>(if (onlyDay != null) usedElsewhere else emptySet())
        val liftDays = template.mapIndexed { di, day ->
            if (onlyDay != null && day.key != onlyDay) {
                return@mapIndexed GeneratedDay(day.key, day.name, day.word, day.accentHex, day.key, emptyList())
            }
            val usedInDay = HashSet<String>()
            val usedPatterns = HashSet<MovementPattern>()
            val pinnedFor = placePins(day, setsByDay[di], params.pinned, ::candidatesFor)
            val exercises = day.targets.mapIndexedNotNull { si, slot ->
                if (setsByDay[di][si] == 0) return@mapIndexedNotNull null
                // Never place the same exercise twice in one day: if the (equipment-limited) pool is
                // exhausted, DROP the slot (a slightly shorter day) rather than repeat a movement —
                // a duplicate reads as a bug, breaks per-exercise logging, and crashed the session list.
                val candidates = candidatesFor(slot.muscle).filterNot { it.id in usedInDay }
                // A heavy STRENGTH slot must lead with a compound when one is available — an
                // isolation (leg curl, fly) may only headline if it's all the equipment allows.
                val slotPool = if (slot.scheme == RepScheme.STRENGTH)
                    candidates.filter { ExerciseTag.COMPOUND in it.tags }.ifEmpty { candidates }
                else candidates
                // Light dumbbells can't progressively load a heavy slot — when the user's heaviest
                // DB is below the threshold and the pool offers a loadable non-dumbbell compound
                // (barbell / machine / plate stack), steer the STRENGTH pick toward it (the only real
                // overload path; auto-coach Phase 0, generalized to WEIGHT movements 2026-06-11).
                val preferStack = slot.scheme == RepScheme.STRENGTH &&
                    (params.dbMaxLb ?: Double.MAX_VALUE) < LIGHT_DB_LB &&
                    slotPool.any {
                        (it.unit == ExerciseUnit.PLATES || it.unit == ExerciseUnit.WEIGHT) &&
                            !isHandheld(it) && ExerciseTag.COMPOUND in it.tags
                    }
                // Group size per pattern, so a used pattern's penalty is shared across its candidates.
                val patternCounts = slotPool.groupingBy { ExerciseLibrary.patternOf(it) }.eachCount()
                val pick = pinnedFor[si] ?: weightedPick(slotPool, rng) { def ->
                    val likeW = if (def.id in liked) LIKE_BOOST else 1.0
                    val recentW = if (def.id in recent) RECENT_PENALTY else 1.0
                    val weekW = if (def.id in usedInWeek) WEEK_REPEAT_PENALTY else 1.0
                    val pattern = ExerciseLibrary.patternOf(def)
                    val patternW = if (pattern != MovementPattern.ISOLATION && pattern in usedPatterns)
                        PATTERN_REPEAT_PENALTY / (patternCounts[pattern] ?: 1) else 1.0
                    val slotPatternW = if (slot.pattern != null && pattern != slot.pattern) SLOT_PATTERN_MISMATCH else 1.0
                    val stackW = if (preferStack && isHandheld(def)) LIGHT_DB_PENALTY else 1.0
                    val avoidW = if (def.id in params.avoid) AVOID_PENALTY else 1.0
                    likeW * recentW * weekW * roleFactor(def, slot.scheme) * patternW * slotPatternW *
                        def.pickBias * stackW * avoidW
                } ?: return@mapIndexedNotNull null
                usedInDay += pick.id
                usedInWeek += pick.id
                ExerciseLibrary.patternOf(pick).takeIf { it != MovementPattern.ISOLATION }
                    ?.let { usedPatterns += it }
                GeneratedExercise(pick.id, setsByDay[di][si], repsFor(pick, slot.scheme, params.goal))
            }
            // An unfillable day stays empty, as with an empty frozen pool. Never bypass a
            // restriction or borrow an unrelated muscle just to make a day look populated.
            GeneratedDay(day.key, day.name, day.word, day.accentHex, day.key, exercises)
        }
        return liftDays
    }

    /**
     * Decide which slot each pinned movement takes on [day], before any slot is filled (2026-09-21).
     *
     * A pin used to be honoured on the FIRST slot of its muscle — the STRENGTH slot — whatever the
     * movement was, so a pinned fly led the push day at 6-10 and the day lost its press. Now a
     * compound pin prefers the muscle's STRENGTH slot (then its first slot); an isolation pin takes
     * the muscle's last PUMP slot, then its last HYPERTROPHY slot, and never a STRENGTH slot — on a
     * day where the muscle's only slot is the heavy one, the pin simply isn't placed there.
     */
    private fun placePins(
        day: DayArchetype,
        sets: List<Int>,
        pinned: Set<String>,
        candidatesFor: (MuscleGroup) -> List<ExerciseDef>
    ): Map<Int, ExerciseDef> {
        if (pinned.isEmpty()) return emptyMap()
        val taken = HashMap<Int, ExerciseDef>()
        // Sorted for determinism: a pin's placement can't depend on Set iteration order.
        pinned.sorted().forEach { id ->
            val def = ExerciseLibrary.byId(id) ?: return@forEach
            // Pins are honored only after equipment, recovery-related restrictions and skill filters.
            if (candidatesFor(def.muscle).none { it.id == id }) return@forEach
            if (taken.values.any { it.id == id }) return@forEach
            val open = day.targets.indices.filter {
                day.targets[it].muscle == def.muscle && sets[it] > 0 && it !in taken
            }
            val slot = if (ExerciseTag.COMPOUND in def.tags) {
                open.firstOrNull { day.targets[it].scheme == RepScheme.STRENGTH } ?: open.firstOrNull()
            } else {
                open.lastOrNull { day.targets[it].scheme == RepScheme.PUMP }
                    ?: open.lastOrNull { day.targets[it].scheme == RepScheme.HYPERTROPHY }
            } ?: return@forEach
            taken[slot] = def
        }
        return taken
    }

    /**
     * Rank the swap pool for a slot currently holding [current] — the order the coach's stall-swap
     * and the pain re-route should propose from (2026-09-21). Library order used to be the ranking,
     * so a stalled back squat was told to rotate to a goblet squat and a stalled fly to the bench
     * press the day already opened with. The ranking is deterministic (stable sort, library order on
     * ties): same role as [current] (compound ↔ compound), the same movement pattern first (a fresh
     * *variation* is what restarts a stall), loadable movements ahead of handheld / bodyweight for a
     * compound, [ExerciseDef.pickBias] last. [excludeIds] — the day's other movements — never appear.
     */
    fun rankSwapCandidates(
        current: ExerciseDef,
        pool: List<ExerciseDef>,
        excludeIds: Set<String> = emptySet()
    ): List<ExerciseDef> {
        val currentCompound = ExerciseTag.COMPOUND in current.tags
        val currentPattern = ExerciseLibrary.patternOf(current)
        val eligible = pool.filter { it.id != current.id && it.id !in excludeIds }
        val ranked = eligible.filterNot { it.fallbackOnly }.ifEmpty { eligible }
        return ranked.sortedByDescending { def ->
            val role = if ((ExerciseTag.COMPOUND in def.tags) == currentCompound) 1.0 else SWAP_ROLE_MISMATCH
            val load = if (currentCompound) loadTier(def) * (if (def.defaultReps.matches(NUMERIC_REPS)) 1.0 else ROLE_NON_LOADABLE) else 1.0
            val pattern = if (ExerciseLibrary.patternOf(def) == currentPattern) SWAP_SAME_PATTERN else 1.0
            role * load * pattern * def.pickBias
        }
    }

    /**
     * Heavy (STRENGTH) slots favour **bilateral, loadable** compounds (squat / hinge / press / row) as
     * the lead lift, demote unilateral compounds (lunges, single-leg) to accessory weight, prefer a
     * bar or stack over handheld and bodyweight loads, and avoid isolation. PUMP slots favour
     * isolation; HYPERTROPHY is neutral.
     */
    private fun roleFactor(def: ExerciseDef, scheme: RepScheme): Double = when (scheme) {
        RepScheme.STRENGTH -> when {
            ExerciseTag.COMPOUND !in def.tags -> ROLE_MISMATCH
            isUnilateral(def) -> ROLE_UNILATERAL
            !def.defaultReps.matches(NUMERIC_REPS) -> ROLE_NON_LOADABLE
            else -> ROLE_MATCH * loadTier(def)
        }
        RepScheme.PUMP -> if (ExerciseTag.ISOLATION in def.tags) 2.0 else 0.5
        RepScheme.HYPERTROPHY -> 1.0
    }

    /** Barbell / plate-loaded / stack = 1.0; dumbbells and kettlebells top out; bodyweight can't add load. */
    private fun loadTier(def: ExerciseDef): Double = when {
        def.unit == ExerciseUnit.BODYWEIGHT -> LOAD_TIER_BODYWEIGHT
        isHandheld(def) -> LOAD_TIER_HANDHELD
        else -> 1.0
    }

    /** Dumbbell or kettlebell — loads that top out at the heaviest one the user owns. */
    private fun isHandheld(def: ExerciseDef): Boolean =
        def.unit == ExerciseUnit.DUMBBELL || Equipment.KETTLEBELL in def.equipment

    /** Per-side movements (lunges, step-ups, single-leg work) — flagged by per-leg reps or LUNGE pattern. */
    private fun isUnilateral(def: ExerciseDef): Boolean =
        def.defaultReps.contains("/") || ExerciseLibrary.patternOf(def) == MovementPattern.LUNGE

    /** Numeric ranges like "8-10" / "15" take the goal-adjusted scheme reps; "AMRAP"/"30-60s"/"10/leg" stay. */
    private val NUMERIC_REPS = Regex("""^\d+(-\d+)?$""")

    /**
     * Reps for [def] in a [scheme] slot. Non-numeric and [ExerciseDef.fixedReps] movements keep their
     * own range. An isolation that ends up in a STRENGTH slot (nothing compound was available) is
     * prescribed the HYPERTROPHY range, never a heavy 6-10 wall sit or leg curl (2026-09-21).
     */
    private fun repsFor(def: ExerciseDef, scheme: RepScheme, goal: String): String =
        if (!def.defaultReps.matches(NUMERIC_REPS) || def.fixedReps) def.defaultReps
        else if (ExerciseTag.COMPOUND !in def.tags && goal == "get_stronger") {
            if (scheme == RepScheme.PUMP) "12-15" else "8-12"
        } else if (ExerciseTag.COMPOUND !in def.tags && scheme == RepScheme.STRENGTH) {
            GoalProfiles.reps(goal, RepScheme.HYPERTROPHY)
        } else GoalProfiles.reps(goal, scheme)

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
