package com.forge.app.program

/**
 * A single exercise slot within a day's plan. The `id` matches the React prototype
 * (ua1, la3, ub7, lb6 — letter day + position). Swap candidates come from
 * [ExerciseLibrary], filtered by [muscle].
 *
 * [reps] is stored as a display string ("8-10", "10/leg", "12-15") because the
 * prototype uses a mix of ranges and per-side notations. PR detection (Phase 6+)
 * parses this when needed.
 */
/**
 * Equipment required to perform an exercise (#44). Every value must have at least one
 * [ExerciseLibrary] movement that uses it — an equipment chip with no exercises generates a broken
 * (empty) day. The barbell/Smith/trap-bar/EZ-bar/kettlebell/band gear was added (2026-06-11) as part
 * of generalizing the app beyond the owner's MWM-989 home gym, alongside the movements that use it.
 */
enum class Equipment(val display: String) {
    DUMBBELLS("Dumbbells"),
    BARBELL("Barbell"),
    /** A squat / power rack — needed to unrack barbell squats safely. */
    SQUAT_RACK("Squat / power rack"),
    SMITH_MACHINE("Smith machine"),
    /** Trap / hex bar. */
    TRAP_BAR("Trap / hex bar"),
    EZ_BAR("EZ-bar"),
    KETTLEBELL("Kettlebell"),
    RESISTANCE_BAND("Resistance bands"),
    CABLE("Cable machine"),
    PULL_UP_BAR("Pull-up bar"),
    /** A FLAT bench. Movements that need an adjustable back rest require [INCLINE_BENCH]. */
    BENCH("Flat bench"),
    /** Adjustable/incline bench — NOT part of the Developer's preset (that bench is flat-only). */
    INCLINE_BENCH("Incline bench"),
    BODYWEIGHT_ONLY("Bodyweight only"),
    /** Generic selectorized resistance machine (leg press, pulldown, chest press, etc.). */
    MACHINE("Machine")
}

/**
 * A one-tap equipment preset. [equipment] fills the available-equipment set. A non-null [frozenIds]
 * marks a CURATED preset whose exercise pool is locked to exactly those library ids — generation,
 * the swap picker and the like/dislike screen all draw only from them, and movements added to the
 * library later never enter it. Null = ordinary equipment filtering.
 */
data class EquipmentPreset(
    val id: String,
    val label: String,
    val equipment: Set<String>,
    val frozenIds: Set<String>? = null
)

/**
 * One-tap equipment presets shared by onboarding + Settings. The **Home machine gym** preset
 * (id `developer`) is Antho's
 * own MWM-989 home gym = dumbbells + a FLAT bench (the bench-press bench; its bar is benching-only
 * and barbell movements aren't modeled) + the machine's cable (high/low pulley) + machine stations
 * (leg developer, press arm). It is CURATED: its exercise pool is frozen to
 * [ExerciseLibrary.DEVELOPER_FROZEN_IDS] (no incline-bench/pull-up-bar movements), so the equipment
 * generalization can never change what it produces. Every other preset is plain equipment filtering.
 */
val equipmentPresets: List<EquipmentPreset> = listOf(
    EquipmentPreset(
        // Label renamed for sharing; id "developer" + frozenIds kept intact so the curated pool is unchanged.
        "developer", "Home machine gym",
        setOf(Equipment.DUMBBELLS.name, Equipment.BENCH.name, Equipment.CABLE.name, Equipment.MACHINE.name),
        frozenIds = ExerciseLibrary.DEVELOPER_FROZEN_IDS
    ),
    EquipmentPreset("commercial", "Full gym (everything)", Equipment.entries.map { it.name }.toSet()),
    EquipmentPreset("basic-gym", "Basic gym", setOf(
        Equipment.DUMBBELLS.name, Equipment.BARBELL.name, Equipment.SQUAT_RACK.name,
        Equipment.BENCH.name, Equipment.INCLINE_BENCH.name, Equipment.MACHINE.name, Equipment.CABLE.name
    )),
    EquipmentPreset("home-barbell", "Home gym + barbell", setOf(
        Equipment.DUMBBELLS.name, Equipment.BARBELL.name, Equipment.SQUAT_RACK.name,
        Equipment.BENCH.name, Equipment.INCLINE_BENCH.name, Equipment.PULL_UP_BAR.name
    )),
    EquipmentPreset("db-bench", "Dumbbells + bench",
        setOf(Equipment.DUMBBELLS.name, Equipment.BENCH.name)),
    EquipmentPreset("dumbbells", "Dumbbells only", setOf(Equipment.DUMBBELLS.name)),
    EquipmentPreset("bodyweight", "Bodyweight only", setOf(Equipment.BODYWEIGHT_ONLY.name))
)

/** Equipment/movement tag for an exercise (#37). */
enum class ExerciseTag(val display: String) {
    COMPOUND("Compound"),
    ISOLATION("Isolation"),
    MACHINE("Machine"),
    FREE_WEIGHT("Free Weight"),
    BODYWEIGHT("Bodyweight")
}

data class ExercisePlan(
    val id: String,
    val name: String,
    val sets: Int,
    val reps: String,
    val unit: ExerciseUnit,
    val muscle: MuscleGroup,
    val difficulty: Difficulty,
    val note: String,
    /** Movement tags for swap-picker filtering (#37). */
    val tags: List<ExerciseTag> = emptyList(),
    /** Short form cue shown as a chip during session (#8). Null = no cue. */
    val formCue: String? = null,
    /** Equipment required for this exercise (#44). Empty = no specific equipment needed. */
    val equipment: List<Equipment> = emptyList()
)

/**
 * One day of the 4-day Upper/Lower split. [accentHex] is the day's identity colour,
 * used for the rotated spine word and accents on the day card. [defaultName] is the
 * built-in label; user customisation (Phase 2 DayNameOverride table) takes precedence
 * at the UI layer.
 */
data class DayPlan(
    val key: String,
    val defaultName: String,
    val subtitle: String,
    val word: String,
    val accentHex: String,
    val warmup: List<String>,
    val exercises: List<ExercisePlan>
)

/**
 * The hard-coded 4-day Upper/Lower split tuned for Antho's home equipment
 * (MWM-989 home gym, adjustable bench, adjustable DBs up to ~30 lb, pull-up bar).
 */
object Program {

    const val UPPER_A = "upper-a"
    const val LOWER_A = "lower-a"
    const val UPPER_B = "upper-b"
    const val LOWER_B = "lower-b"

    /** Sentinel day key for freestyle ("go with the flow") sessions — not part of any program split.
     *  Resolved to a friendly label by [dayDisplayName] wherever a session's day name is shown. */
    const val FREESTYLE_DAY_KEY = "freestyle"

    /** Human label for a session's day key: the program day's name, "Open workout" for a freestyle
     *  session, or the raw key as a last resort. Use this instead of `day(key)?.defaultName ?: key`
     *  on session-display surfaces so a freestyle log never shows the raw "freestyle" slug. */
    fun dayDisplayName(dayKey: String): String =
        if (dayKey == FREESTYLE_DAY_KEY) "Open workout"
        else dayOrNull(dayKey)?.defaultName ?: dayKey

    private val defaultDays: List<DayPlan> = listOf(
        DayPlan(
            key = UPPER_A,
            defaultName = "Upper A",
            subtitle = "Push-leaning · Size focus",
            word = "PUSH",
            accentHex = "#E85D4A",
            warmup = listOf(
                "Jumping jacks — 20 reps",
                "Arm circles — 10 forward, 10 back",
                "Shoulder rolls — 10 each direction",
                "Push-ups — 10 slow reps"
            ),
            exercises = listOf(
                ExercisePlan("ua1", "DB Bench Press", 3, "8-10", ExerciseUnit.DUMBBELL, MuscleGroup.CHEST, Difficulty.BEGINNER, "1-2 reps shy of failure"),
                ExercisePlan("ua2", "Seated Bench Press", 3, "10-12", ExerciseUnit.PLATES, MuscleGroup.CHEST, Difficulty.BEGINNER, "Seated chest press machine"),
                ExercisePlan("ua3", "Wide Lat Pulldown", 4, "8-12", ExerciseUnit.PLATES, MuscleGroup.BACK, Difficulty.BEGINNER, "Wide grip, pull to upper chest"),
                ExercisePlan("ua4", "DB Lateral Raise", 4, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.SHOULDERS, Difficulty.BEGINNER, "Priority — slow eccentric"),
                ExercisePlan("ua5", "DB Overhead Tricep Ext.", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.TRICEPS, Difficulty.BEGINNER, "Long head — biggest visual lever"),
                ExercisePlan("ua6", "DB Hammer Curl", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.BICEPS, Difficulty.BEGINNER, "Bicep + forearm")
            )
        ),
        DayPlan(
            key = LOWER_A,
            defaultName = "Lower A",
            subtitle = "Quad-leaning",
            word = "QUADS",
            accentHex = "#D4A017",
            warmup = listOf(
                "Jumping jacks — 20 reps",
                "Bodyweight squats — 15 reps slow",
                "Walking lunges — 10 steps",
                "Leg swings — 10 each leg"
            ),
            exercises = listOf(
                ExercisePlan("la1", "Goblet Squat", 4, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.QUADS, Difficulty.BEGINNER, "Heaviest DB you have"),
                ExercisePlan("la2", "DB Romanian Deadlift", 4, "8-10", ExerciseUnit.DUMBBELL, MuscleGroup.HAMSTRINGS, Difficulty.INTERMEDIATE, "Posture work too"),
                ExercisePlan("la3", "Leg Extension", 3, "12-15", ExerciseUnit.PLATES, MuscleGroup.QUADS, Difficulty.BEGINNER, "Leg extension machine"),
                ExercisePlan("la4", "DB Walking Lunge", 3, "10/leg", ExerciseUnit.DUMBBELL, MuscleGroup.GLUTES, Difficulty.BEGINNER, "Unilateral balance"),
                ExercisePlan("la5", "Standing Calf Raise", 4, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.CALVES, Difficulty.BEGINNER, "DB in hand"),
                ExercisePlan("la6", "Hanging Knee Raise", 3, "10-15", ExerciseUnit.BODYWEIGHT, MuscleGroup.CORE, Difficulty.INTERMEDIATE, "Or plank 30-60s")
            )
        ),
        DayPlan(
            key = UPPER_B,
            defaultName = "Upper B",
            subtitle = "Pull-leaning · Arm emphasis",
            word = "PULL",
            accentHex = "#5B9279",
            warmup = listOf(
                "Jumping jacks — 20 reps",
                "Arm circles — 10 forward, 10 back",
                "Shoulder rolls — 10 each direction",
                "Standing torso twists — 10 each side"
            ),
            exercises = listOf(
                ExercisePlan("ub1", "DB Row (1-arm)", 4, "8-12", ExerciseUnit.DUMBBELL, MuscleGroup.BACK, Difficulty.BEGINNER, "Mid-back thickness"),
                ExercisePlan("ub2", "Incline DB Bench Press", 3, "8-10", ExerciseUnit.DUMBBELL, MuscleGroup.CHEST, Difficulty.BEGINNER, "Fills tee neckline"),
                ExercisePlan("ub3", "Chest-Supported DB Row", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.BACK, Difficulty.BEGINNER, "Chest down on the bench"),
                ExercisePlan("ub4", "DB Lateral Raise", 4, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.SHOULDERS, Difficulty.BEGINNER, "Twice a week, by design"),
                ExercisePlan("ub5", "DB Skull Crusher", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.TRICEPS, Difficulty.INTERMEDIATE, "Tricep mass"),
                ExercisePlan("ub6", "DB Incline Curl", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.BICEPS, Difficulty.BEGINNER, "Stretched bicep = growth"),
                ExercisePlan("ub7", "Rear Delt DB Fly", 3, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.REAR_DELTS, Difficulty.BEGINNER, "Posture fix — non-negotiable")
            )
        ),
        DayPlan(
            key = LOWER_B,
            defaultName = "Lower B",
            subtitle = "Hamstring & glute-leaning",
            word = "HAMS",
            accentHex = "#7B6CB5",
            warmup = listOf(
                "Jumping jacks — 20 reps",
                "Glute bridges — 15 reps",
                "Bodyweight squats — 15 reps",
                "Leg swings — 10 each leg"
            ),
            exercises = listOf(
                ExercisePlan("lb1", "DB Bulgarian Split Squat", 4, "8-10/leg", ExerciseUnit.DUMBBELL, MuscleGroup.QUADS, Difficulty.ADVANCED, "Brutal but it works"),
                ExercisePlan("lb2", "DB Stiff-Leg Deadlift", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.HAMSTRINGS, Difficulty.INTERMEDIATE, "Hamstring stretch"),
                ExercisePlan("lb3", "Leg Curl", 3, "12-15", ExerciseUnit.PLATES, MuscleGroup.HAMSTRINGS, Difficulty.BEGINNER, "Leg curl machine"),
                ExercisePlan("lb4", "Goblet Squat", 3, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.QUADS, Difficulty.BEGINNER, "Higher reps today"),
                ExercisePlan("lb5", "Seated Calf Raise", 4, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.CALVES, Difficulty.BEGINNER, "Different head"),
                ExercisePlan("lb6", "High Pulley Ab Crunch", 3, "10-15", ExerciseUnit.PLATES, MuscleGroup.CORE, Difficulty.BEGINNER, "Loaded abs")
            )
        )
    )

    @Volatile
    private var active: List<DayPlan> = defaultDays

    @Volatile
    private var loaded = false

    /** The hard-coded split, used to seed the DB the first time (program-unlock Phase 1). */
    val seedDays: List<DayPlan> get() = defaultDays

    /** The live program — DB-backed once [setActive] runs; defaults to the seed split. */
    val days: List<DayPlan> get() = active
    val dayKeys: List<String> get() = active.map { it.key }

    /**
     * True once the DB-backed program has been loaded into the facade (via [setActive]). Until then
     * [days]/[dayKeys] report the hard-coded seed split, which is NON-empty — so a "program loaded yet?"
     * check must gate on this, not on a non-empty key set. Anything that would mutate data based on the
     * active day list (e.g. orphan-session resolution) must wait for this to avoid acting on seed keys.
     */
    val isLoaded: Boolean get() = loaded

    /** Swap in a new active program (ProgramRepository, after load / generate). */
    fun setActive(newDays: List<DayPlan>) { active = newDays; loaded = true }

    fun day(key: String): DayPlan =
        dayOrNull(key)
            // Defensive fallback: a stale/zombie day key (e.g. a session left over from a
            // since-regenerated program) resolves to a real day instead of crashing the screen.
            ?: active.firstOrNull()
            ?: defaultDays.first()

    /** Like [day] but null (no crash) when the key isn't in the active or seed program. */
    fun dayOrNull(key: String): DayPlan? =
        active.firstOrNull { it.key == key }
            ?: defaultDays.firstOrNull { it.key == key }

    fun exercise(id: String): ExercisePlan? =
        active.flatMap { it.exercises }.firstOrNull { it.id == id }
            ?: ExerciseLibrary.byId(id)?.toPlan()

    /**
     * The single resolution point every display surface (history, PR list, goals, notes search,
     * stats, export) should use instead of `exercise(id)?.name ?: rawId`. Prefers a session/persistent
     * swap name, then the active program, the seed split, and the full library; only an id that matches
     * NONE of those (e.g. a library movement removed in a later version) falls back to a humanized form
     * of the id rather than the raw kebab slug — so a generated or post-regenerate program can never
     * surface a raw key like "cable-crunch" to a stranger (C3).
     */
    fun exerciseDisplayName(id: String, swappedName: String? = null): String =
        swappedName?.takeIf { it.isNotBlank() }
            ?: exercise(id)?.name
            // Seed-split ids (ua1..lb6) logged before the user regenerated to a data-driven program
            // aren't in [active] or the library, but they're real exercises — resolve their NAME from
            // the seed so old history/PRs read right. Kept on the DISPLAY path only (not in [exercise])
            // so volume/muscle aggregation and the adaptation engine still treat a rotated-out id as
            // absent, exactly as before — this fix is about names, not about counting old sets (C3).
            ?: defaultDays.flatMap { it.exercises }.firstOrNull { it.id == id }?.name
            ?: humanizeExerciseId(id)

    /** Last-ditch readable label from an id ("cable-crunch" / "cable_crunch" → "Cable Crunch"). Only
     *  reached for an id that resolves nowhere above; a resolvable id always returns its real name. */
    private fun humanizeExerciseId(id: String): String {
        val words = id.trim().removePrefix("ex-").split('-', '_', ' ').filter { it.isNotBlank() }
        if (words.isEmpty()) return "Exercise"
        return words.joinToString(" ") { w -> w.replaceFirstChar { it.uppercase() } }
    }
}
