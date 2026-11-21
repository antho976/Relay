package com.forge.app.program

/** How much of the plan one muscle gets. Stored by [code]; an absent muscle is [NORMAL]. */
enum class BodyPartLevel(val code: String, val displayName: String) {
    /** Out of the plan entirely: no slots, no coach proposals, no balance warnings. */
    SKIP("skip", "Skip"),

    /** One slot a week at two sets: enough to hold what is there. */
    LIGHT("light", "Light"),

    NORMAL("normal", "Normal"),

    /** Specialized: trained more often, first in its days, with the rest eased to make room. */
    FOCUS("focus", "Focus");

    companion object {
        fun fromCode(code: String?): BodyPartLevel = entries.firstOrNull { it.code == code } ?: NORMAL
    }
}

/**
 * Which muscles a plan leaves out, holds, or specializes in.
 *
 * Until this existed the generator could only ADD (an emphasis lean, priority muscles) or steer
 * around a sore joint. There was no way to say "no legs", and because the split is chosen by day
 * count alone, even deleting the leg exercises by hand lasted only until the next rotation, deload
 * or rebuild put them back. This is the input that makes the choice survive all three.
 *
 * [focus] is the old priority-muscle set under its new name: the same stored preference, now with
 * real specialization behind it ([SplitTemplates.forDays], [VolumeModel.allocate]).
 */
data class BodyParts(
    val skip: Set<MuscleGroup> = emptySet(),
    val light: Set<MuscleGroup> = emptySet(),
    val focus: Set<MuscleGroup> = emptySet()
) {
    fun levelOf(muscle: MuscleGroup): BodyPartLevel = when (muscle) {
        in skip -> BodyPartLevel.SKIP
        in light -> BodyPartLevel.LIGHT
        in focus -> BodyPartLevel.FOCUS
        else -> BodyPartLevel.NORMAL
    }

    /** Every leg muscle is out: the week is upper body (and core) only. */
    val noLegs: Boolean get() = skip.containsAll(LEGS)

    /** Every upper-body muscle is out: the week is legs (and core) only. */
    val noUpper: Boolean get() = skip.containsAll(UPPER)

    /** Muscles the coach must not propose more work for: out of the plan, or held on purpose. */
    val held: Set<MuscleGroup> get() = skip + light

    val isDefault: Boolean get() = skip.isEmpty() && light.isEmpty() && focus.isEmpty()

    /** [muscle] set to [level], leaving every other muscle as it is. */
    fun with(muscle: MuscleGroup, level: BodyPartLevel): BodyParts = BodyParts(
        skip = if (level == BodyPartLevel.SKIP) skip + muscle else skip - muscle,
        light = if (level == BodyPartLevel.LIGHT) light + muscle else light - muscle,
        focus = if (level == BodyPartLevel.FOCUS) focus + muscle else focus - muscle
    )

    companion object {
        val NONE = BodyParts()

        /** More than this many specializations at once is no specialization at all. */
        const val MAX_FOCUS = 2

        /** How long a Focus runs before the plan returns to normal. */
        const val FOCUS_WEEKS = 6

        val LEGS: Set<MuscleGroup> = setOf(
            MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES, MuscleGroup.CALVES
        )
        val UPPER: Set<MuscleGroup> = setOf(
            MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.SHOULDERS, MuscleGroup.REAR_DELTS,
            MuscleGroup.BICEPS, MuscleGroup.TRICEPS
        )

        /** The region a muscle belongs to, for deciding which days can take an extra slot of it. */
        fun regionOf(muscle: MuscleGroup): Set<MuscleGroup> = when (muscle) {
            in LEGS -> LEGS
            in UPPER -> UPPER
            else -> setOf(muscle)
        }

        /** Stored codes back to a set, dropping anything this build does not know. */
        fun muscles(codes: Collection<String>): Set<MuscleGroup> =
            codes.mapNotNull { MuscleGroup.fromCode(it) }.toSet()
    }
}
