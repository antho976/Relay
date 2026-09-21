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
 * - DUMBBELL: dumbbells, entered as total lb; progression is subject to the user's heaviest-dumbbell
 *   ceiling (adjustable sets max out).
 * - WEIGHT: any other lb-entered external load that is NOT a dumbbell — barbell / Smith / trap-bar /
 *   EZ-bar lifts, kettlebells, and selectorized machine/cable stacks read in lb. Entered as total lb
 *   and NOT capped by the dumbbell ceiling (you keep adding load).
 * - PLATES: a plate-loaded stack entered as a plate count (× the user's plate weight) — the owner's
 *   MWM-989 machine stations.
 * - BODYWEIGHT: no external load (also used for band work, where tension isn't tracked in lb).
 */
enum class ExerciseUnit(val code: String, val display: String) {
    DUMBBELL("db", "lb"),
    WEIGHT("wt", "lb"),
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
 * day/week from stacking near-duplicate movements (e.g. three horizontal pulls, or a dumbbell AND a
 * cable lateral raise). Compound patterns come first; the single-joint families below them let the
 * generator tell two flavours of the same isolation apart. [ISOLATION] is the catch-all for the odd
 * accessory that belongs to no family and is *not* penalized for repeating.
 */
enum class MovementPattern {
    HORIZONTAL_PUSH, VERTICAL_PUSH,
    HORIZONTAL_PULL, VERTICAL_PULL,
    SQUAT, HINGE, LUNGE,
    CORE,
    // Single-joint families (2026-09-21): a day that already holds one member of a family is
    // steered away from a second (fly + pec deck, DB + cable lateral raise, two skull crushers).
    FLY, LATERAL_RAISE, REAR_DELT, PULLOVER, CURL, TRICEP_EXTENSION, PUSHDOWN,
    LEG_EXTENSION, LEG_CURL, HIP_THRUST, KICKBACK, CALF_RAISE,
    ISOLATION;

    /** A single-joint family or the catch-all — never a compound pattern. */
    val isIsolation: Boolean get() = ordinal >= FLY.ordinal
}

/**
 * A joint/area a user can flag as a problem — a sore joint OR an injury (program-unlock Phase 3).
 * Movements with a known contraindication for a flagged area are EXCLUDED from generation and from
 * the swap pool (a hard filter since the 2026-09-11 review — never a soft down-weight). When the
 * exclusion empties a muscle's pool the slot is dropped rather than filled with a flagged movement.
 * [code] is the persisted value; entries are ordered top-to-bottom anatomically for the chip UI.
 */
enum class ProblemArea(val code: String, val displayName: String) {
    NECK("neck", "Neck"),
    SHOULDERS("shoulders", "Shoulders"),
    ELBOWS("elbows", "Elbows"),
    WRISTS("wrists", "Wrists"),
    LOWER_BACK("lower_back", "Lower back"),
    HIPS("hips", "Hips"),
    KNEES("knees", "Knees"),
    ANKLES("ankles", "Ankles");

    companion object {
        fun fromCode(code: String): ProblemArea? = entries.firstOrNull { it.code == code }
    }
}
