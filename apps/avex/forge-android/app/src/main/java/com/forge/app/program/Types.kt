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
        fun fromCode(code: String): MuscleGroup =
            entries.first { it.code == code }
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
        fun fromCode(code: String): ExerciseUnit =
            entries.first { it.code == code }
    }
}

enum class Difficulty(val code: String, val displayName: String) {
    BEGINNER("beginner", "Beginner"),
    INTERMEDIATE("intermediate", "Intermediate"),
    ADVANCED("advanced", "Advanced");

    companion object {
        fun fromCode(code: String): Difficulty =
            entries.first { it.code == code }
    }
}
