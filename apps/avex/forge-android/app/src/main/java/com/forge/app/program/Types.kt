package com.forge.app.program

/**
 * Muscle groups used by the program and the swap catalog. The `code` field matches the
 * literal string used in the React prototype (e.g. "rear-delts"), so any migration or
 * data import can map back to the original schema unambiguously.
 */
enum class MuscleGroup(val code: String, val displayName: String) {
    CHEST("chest", "Chest"),
    BACK("back", "Back"),
    SHOULDERS("shoulders", "Shoulders"),
    REAR_DELTS("rear-delts", "Rear Delts"),
    BICEPS("biceps", "Biceps"),
    TRICEPS("triceps", "Triceps"),
    QUADS("quads", "Quads"),
    HAMSTRINGS("hamstrings", "Hamstrings"),
    GLUTES("glutes", "Glutes"),
    CALVES("calves", "Calves"),
    CORE("core", "Core");

    companion object {
        fun fromCode(code: String): MuscleGroup? =
            entries.firstOrNull { it.code == code }
    }
}

/**
 * How the load on an exercise is measured.
 * - DUMBBELL: free weights, displayed in lb.
 * - PLATES: MWM-989 plate stack (15 lb per plate).
 * - BODYWEIGHT: no external load.
 */
enum class ExerciseUnit(val code: String, val display: String) {
    DUMBBELL("db", "lb"),
    PLATES("plates", "plates"),
    BODYWEIGHT("bw", "BW");

    companion object {
        fun fromCode(code: String): ExerciseUnit? =
            entries.firstOrNull { it.code == code }
    }
}

enum class Difficulty(val code: String, val displayName: String) {
    BEGINNER("beginner", "Beginner"),
    INTERMEDIATE("intermediate", "Intermediate"),
    ADVANCED("advanced", "Advanced");

    companion object {
        fun fromCode(code: String): Difficulty? =
            entries.firstOrNull { it.code == code }
    }
}

/**
 * Movement pattern of an exercise (program-unlock Phase 4 — generator intelligence). Used to keep a
 * day/week from stacking near-duplicate movements (e.g. three horizontal pulls). [ISOLATION] is the
 * catch-all for single-joint accessory work and is *not* penalized for repeating (you can do several
 * different isolations in a day); the compound patterns are.
 */
enum class MovementPattern {
    HORIZONTAL_PUSH, VERTICAL_PUSH,
    HORIZONTAL_PULL, VERTICAL_PULL,
    SQUAT, HINGE, LUNGE,
    CORE, ISOLATION
}

/**
 * A joint/area a user can flag as a problem (program-unlock Phase 3). Movements that stress a flagged
 * area are strongly down-weighted in generation so the plan steers around it (soft, not a hard ban —
 * if it's the only option for a muscle it can still appear). [code] is the persisted value.
 */
enum class ProblemArea(val code: String, val displayName: String) {
    KNEES("knees", "Knees"),
    SHOULDERS("shoulders", "Shoulders"),
    LOWER_BACK("lower_back", "Lower back"),
    WRISTS("wrists", "Wrists");

    companion object {
        fun fromCode(code: String): ProblemArea? = entries.firstOrNull { it.code == code }
    }
}
