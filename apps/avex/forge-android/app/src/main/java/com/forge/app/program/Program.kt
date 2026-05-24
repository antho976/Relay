package com.forge.app.program

/**
 * A single exercise slot within a day's plan. The `id` matches the React prototype
 * (ua1, la3, ub7, lb6 — letter day + position). Tutorials live in [Tutorials] keyed
 * by the same id; swap candidates live in [Swaps] keyed by [muscle].
 *
 * [reps] is stored as a display string ("8-10", "10/leg", "12-15") because the
 * prototype uses a mix of ranges and per-side notations. PR detection (Phase 6+)
 * parses this when needed.
 */
/** Equipment required to perform an exercise (#44). */
enum class Equipment(val display: String) {
    DUMBBELLS("Dumbbells"),
    BARBELL("Barbell"),
    CABLE("Cable machine"),
    PULL_UP_BAR("Pull-up bar"),
    BENCH("Bench"),
    BODYWEIGHT_ONLY("Bodyweight only"),
    RESISTANCE_BAND("Resistance bands"),
    KETTLEBELL("Kettlebell"),
    MACHINE("Machine")
}

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

    val dayKeys: List<String> = listOf(UPPER_A, LOWER_A, UPPER_B, LOWER_B)

    val days: List<DayPlan> = listOf(
        DayPlan(
            key = UPPER_A,
            defaultName = "Upper A",
            subtitle = "Push-leaning · Size focus",
            word = "PUSH",
            accentHex = "#E85D4A",
            warmup = listOf(
                "Arm circles — 10 forward, 10 back",
                "Push-ups — 10 slow reps",
                "Light shoulder press with empty hands — 15 reps",
                "Scapular wall slides — 10 reps"
            ),
            exercises = listOf(
                ExercisePlan("ua1", "DB Bench Press", 3, "8-10", ExerciseUnit.DUMBBELL, MuscleGroup.CHEST, Difficulty.BEGINNER, "1-2 reps shy of failure"),
                ExercisePlan("ua2", "Machine Chest Press", 3, "10-12", ExerciseUnit.PLATES, MuscleGroup.CHEST, Difficulty.BEGINNER, "MWM-989 press arm"),
                ExercisePlan("ua3", "Lat Pulldown", 4, "8-12", ExerciseUnit.PLATES, MuscleGroup.BACK, Difficulty.BEGINNER, "Wide grip, pull to upper chest"),
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
                "Bodyweight squats — 15 reps slow",
                "Leg swings — 10 each leg, forward and side",
                "Walking lunges — 10 steps",
                "Hip circles — 10 each direction"
            ),
            exercises = listOf(
                ExercisePlan("la1", "Goblet Squat", 4, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.QUADS, Difficulty.BEGINNER, "Heaviest DB you have"),
                ExercisePlan("la2", "DB Romanian Deadlift", 4, "8-10", ExerciseUnit.DUMBBELL, MuscleGroup.HAMSTRINGS, Difficulty.INTERMEDIATE, "Posture work too"),
                ExercisePlan("la3", "Leg Extension", 3, "12-15", ExerciseUnit.PLATES, MuscleGroup.QUADS, Difficulty.BEGINNER, "MWM-989 leg developer"),
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
                "Dead hangs from bar — 20 seconds",
                "Scapular pull-ups — 10 reps",
                "Cat-cow stretches — 10 reps",
                "Light face pulls on the machine — 15 reps"
            ),
            exercises = listOf(
                ExercisePlan("ub1", "Machine Seated Row", 4, "8-12", ExerciseUnit.PLATES, MuscleGroup.BACK, Difficulty.BEGINNER, "Mid-back thickness"),
                ExercisePlan("ub2", "Incline DB Bench Press", 3, "8-10", ExerciseUnit.DUMBBELL, MuscleGroup.CHEST, Difficulty.BEGINNER, "Fills tee neckline"),
                ExercisePlan("ub3", "Close-Grip Lat Pulldown", 3, "10-12", ExerciseUnit.PLATES, MuscleGroup.BACK, Difficulty.BEGINNER, "Different angle"),
                ExercisePlan("ub4", "DB Lateral Raise", 4, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.SHOULDERS, Difficulty.BEGINNER, "Twice a week, by design"),
                ExercisePlan("ub5", "DB Skull Crusher", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.TRICEPS, Difficulty.INTERMEDIATE, "Tricep mass"),
                ExercisePlan("ub6", "DB Incline Curl", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.BICEPS, Difficulty.BEGINNER, "Stretched bicep = growth"),
                ExercisePlan("ub7", "Face Pull (cable)", 3, "15", ExerciseUnit.PLATES, MuscleGroup.REAR_DELTS, Difficulty.BEGINNER, "Posture fix — non-negotiable")
            )
        ),
        DayPlan(
            key = LOWER_B,
            defaultName = "Lower B",
            subtitle = "Hamstring & glute-leaning",
            word = "HAMS",
            accentHex = "#7B6CB5",
            warmup = listOf(
                "Bodyweight squats — 15 reps",
                "Glute bridges — 15 reps",
                "Leg swings — 10 each leg",
                "Walking knee hugs — 10 each leg"
            ),
            exercises = listOf(
                ExercisePlan("lb1", "DB Bulgarian Split Squat", 4, "8-10/leg", ExerciseUnit.DUMBBELL, MuscleGroup.QUADS, Difficulty.ADVANCED, "Brutal but it works"),
                ExercisePlan("lb2", "DB Stiff-Leg Deadlift", 3, "10-12", ExerciseUnit.DUMBBELL, MuscleGroup.HAMSTRINGS, Difficulty.INTERMEDIATE, "Hamstring stretch"),
                ExercisePlan("lb3", "Leg Curl", 3, "12-15", ExerciseUnit.PLATES, MuscleGroup.HAMSTRINGS, Difficulty.BEGINNER, "MWM-989 leg developer"),
                ExercisePlan("lb4", "Goblet Squat", 3, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.QUADS, Difficulty.BEGINNER, "Higher reps today"),
                ExercisePlan("lb5", "Seated Calf Raise", 4, "12-15", ExerciseUnit.DUMBBELL, MuscleGroup.CALVES, Difficulty.BEGINNER, "Different head"),
                ExercisePlan("lb6", "Cable Crunch", 3, "10-15", ExerciseUnit.PLATES, MuscleGroup.CORE, Difficulty.BEGINNER, "Loaded abs")
            )
        )
    )

    fun day(key: String): DayPlan =
        days.firstOrNull { it.key == key }
            ?: error("Unknown day key: $key. Valid keys are $dayKeys")

    fun exercise(id: String): ExercisePlan? =
        days.flatMap { it.exercises }.firstOrNull { it.id == id }
}
