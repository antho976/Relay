package com.forge.app.program

/**
 * Canonical exercise — the un-hardlocked replacement for slot-bound [ExercisePlan]s.
 *
 * The library is the single pool the generator and swap picker draw from. Each entry has a
 * **stable** id (kebab-case, e.g. "db-bench-press") that is independent of which day/slot it
 * lands in, and an explicit [equipment] list that drives availability filtering (#44).
 *
 * Authored in code (like [Swaps]) — no DB seeding migration, easy to expand. `defaultSets` /
 * `defaultReps` are starting points; a [ExercisePlan] (a placed slot) may override them.
 *
 * Phase 0 of the program-unlock plan (.claude/program-unlock-plan.md): this seeds the model and
 * ports today's program movements. Broader variants (from [Swaps]) and selection/weighting come
 * with the generator phase.
 */
data class ExerciseDef(
    val id: String,
    val name: String,
    val muscle: MuscleGroup,
    /** Equipment required. Empty = no equipment (always available). */
    val equipment: List<Equipment>,
    val unit: ExerciseUnit,
    val tags: List<ExerciseTag> = emptyList(),
    val difficulty: Difficulty = Difficulty.BEGINNER,
    val defaultSets: Int = 3,
    val defaultReps: String = "8-12",
    val formCue: String? = null,
    val note: String = ""
)

object ExerciseLibrary {

    private val FW = ExerciseTag.FREE_WEIGHT
    private val MC = ExerciseTag.MACHINE
    private val BW = ExerciseTag.BODYWEIGHT
    private val COMP = ExerciseTag.COMPOUND
    private val ISO = ExerciseTag.ISOLATION

    val all: List<ExerciseDef> = listOf(
        // ── Chest ────────────────────────────────────────────────────────────────
        ExerciseDef("db-bench-press", "DB Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "8-10", "1-2 reps shy of failure"),
        ExerciseDef("incline-db-bench-press", "Incline DB Bench Press", MuscleGroup.CHEST,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "8-10", "Fills tee neckline"),
        ExerciseDef("machine-chest-press", "Machine Chest Press", MuscleGroup.CHEST,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-12", "MWM-989 press arm"),

        // ── Back ─────────────────────────────────────────────────────────────────
        ExerciseDef("lat-pulldown", "Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 4, "8-12", "Wide grip, pull to upper chest"),
        ExerciseDef("close-grip-lat-pulldown", "Close-Grip Lat Pulldown", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 3, "10-12", "Different angle"),
        ExerciseDef("machine-seated-row", "Machine Seated Row", MuscleGroup.BACK,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(COMP, MC), Difficulty.BEGINNER, 4, "8-12", "Mid-back thickness"),

        // ── Shoulders / rear delts ─────────────────────────────────────────────────
        ExerciseDef("db-lateral-raise", "DB Lateral Raise", MuscleGroup.SHOULDERS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "Priority — slow eccentric"),
        ExerciseDef("face-pull", "Face Pull (cable)", MuscleGroup.REAR_DELTS,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "15", "Posture fix — non-negotiable"),

        // ── Arms ──────────────────────────────────────────────────────────────────
        ExerciseDef("db-hammer-curl", "DB Hammer Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Bicep + forearm"),
        ExerciseDef("db-incline-curl", "DB Incline Curl", MuscleGroup.BICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Stretched bicep = growth"),
        ExerciseDef("db-overhead-tricep-ext", "DB Overhead Tricep Ext.", MuscleGroup.TRICEPS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 3, "10-12", "Long head — biggest visual lever"),
        ExerciseDef("db-skull-crusher", "DB Skull Crusher", MuscleGroup.TRICEPS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.INTERMEDIATE, 3, "10-12", "Tricep mass"),

        // ── Quads / glutes ──────────────────────────────────────────────────────────
        ExerciseDef("goblet-squat", "Goblet Squat", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 4, "10-12", "Heaviest DB you have"),
        ExerciseDef("db-bulgarian-split-squat", "DB Bulgarian Split Squat", MuscleGroup.QUADS,
            listOf(Equipment.DUMBBELLS, Equipment.BENCH), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.ADVANCED, 4, "8-10/leg", "Brutal but it works"),
        ExerciseDef("leg-extension", "Leg Extension", MuscleGroup.QUADS,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "MWM-989 leg developer"),
        ExerciseDef("db-walking-lunge", "DB Walking Lunge", MuscleGroup.GLUTES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.BEGINNER, 3, "10/leg", "Unilateral balance"),

        // ── Hamstrings ──────────────────────────────────────────────────────────────
        ExerciseDef("db-romanian-deadlift", "DB Romanian Deadlift", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 4, "8-10", "Posture work too"),
        ExerciseDef("db-stiff-leg-deadlift", "DB Stiff-Leg Deadlift", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(COMP, FW), Difficulty.INTERMEDIATE, 3, "10-12", "Hamstring stretch"),
        ExerciseDef("leg-curl", "Leg Curl", MuscleGroup.HAMSTRINGS,
            listOf(Equipment.MACHINE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "12-15", "MWM-989 leg developer"),

        // ── Calves ────────────────────────────────────────────────────────────────
        ExerciseDef("standing-calf-raise", "Standing Calf Raise", MuscleGroup.CALVES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "DB in hand"),
        ExerciseDef("seated-calf-raise", "Seated Calf Raise", MuscleGroup.CALVES,
            listOf(Equipment.DUMBBELLS), ExerciseUnit.DUMBBELL,
            listOf(ISO, FW), Difficulty.BEGINNER, 4, "12-15", "Different head"),

        // ── Core ──────────────────────────────────────────────────────────────────
        ExerciseDef("hanging-knee-raise", "Hanging Knee Raise", MuscleGroup.CORE,
            listOf(Equipment.PULL_UP_BAR), ExerciseUnit.BODYWEIGHT,
            listOf(BW), Difficulty.INTERMEDIATE, 3, "10-15", "Or plank 30-60s"),
        ExerciseDef("cable-crunch", "Cable Crunch", MuscleGroup.CORE,
            listOf(Equipment.CABLE), ExerciseUnit.PLATES,
            listOf(ISO, MC), Difficulty.BEGINNER, 3, "10-15", "Loaded abs")
    )

    private val byId: Map<String, ExerciseDef> = all.associateBy { it.id }

    fun byId(id: String): ExerciseDef? = byId[id]

    fun forMuscle(muscle: MuscleGroup): List<ExerciseDef> = all.filter { it.muscle == muscle }
}
