package com.forge.app.program

/**
 * Rep scheme for a slot (program-unlock Phase 4 — subjective tuning). Anchored on Antho's
 * "moderate 8-12" preference, spread to serve a **size + strength** goal: the day's heavy
 * compound runs [STRENGTH], accessories run [PUMP], everything else sits on the [HYPERTROPHY]
 * anchor. The generator keeps a movement's *natural* reps instead when they're non-numeric
 * (AMRAP, timed, per-leg) — see [ProgramGenerator].
 */
enum class RepScheme(val reps: String) {
    STRENGTH("6-10"),
    HYPERTROPHY("8-12"),
    PUMP("12-15")
}

/**
 * A day's target slot: a muscle group + the rep scheme to use. Set counts are **not** hardcoded
 * here — [VolumeModel] computes them from weekly per-muscle targets and how often the muscle is
 * trained, so volume scales with frequency (program-unlock Phase 4). The [scheme] doubles as the
 * slot's priority: STRENGTH slots are the day's heavy compounds and get a slightly larger share.
 */
data class MuscleSlot(
    val muscle: MuscleGroup,
    val scheme: RepScheme = RepScheme.HYPERTROPHY
)

/** A day's shape within a split (program-unlock Phase 2). The generator fills it with concrete exercises. */
data class DayArchetype(
    val key: String,
    val name: String,
    val word: String,
    val accentHex: String,
    val targets: List<MuscleSlot>
)

/**
 * Maps days/week (1..7) to a split structure, so the plan SHAPE scales with day-count
 * (3-day ≠ 7-day). Tuned (Phase 4) for **5–6 exercises per session**, ordered heavy-compound
 * first → isolation last. Per-slot set counts come from [VolumeModel] (frequency-aware), not from
 * here — so the same template gives ~10 chest sets/wk at 1× frequency and ~14 at 2×.
 *
 * Slots can repeat a muscle (e.g. CHEST ×2) — the generator picks a *distinct* exercise for each,
 * so you get a press + a second chest movement rather than the same lift twice.
 */
object SplitTemplates {

    private const val RED = "#E85D4A"
    private const val GOLD = "#D4A017"
    private const val GREEN = "#5B9279"
    private const val PURPLE = "#7B6CB5"
    private const val BLUE = "#4A78E8"

    private val STR = RepScheme.STRENGTH
    private val HYP = RepScheme.HYPERTROPHY
    private val PUMP = RepScheme.PUMP

    private fun push(key: String, name: String = "Push", word: String = "PUSH", accent: String = RED) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.CHEST, STR),
            MuscleSlot(MuscleGroup.SHOULDERS, HYP),
            MuscleSlot(MuscleGroup.CHEST, HYP),
            MuscleSlot(MuscleGroup.SHOULDERS, PUMP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP)
        ))

    private fun pull(key: String, name: String = "Pull", word: String = "PULL", accent: String = GREEN) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.BACK, STR),
            MuscleSlot(MuscleGroup.BACK, HYP),
            MuscleSlot(MuscleGroup.BACK, HYP),
            MuscleSlot(MuscleGroup.REAR_DELTS, PUMP),
            MuscleSlot(MuscleGroup.BICEPS, HYP),
            MuscleSlot(MuscleGroup.BICEPS, PUMP)
        ))

    private fun legs(key: String, name: String = "Legs", word: String = "LEGS", accent: String = GOLD) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.HAMSTRINGS, STR),
            MuscleSlot(MuscleGroup.QUADS, HYP),
            MuscleSlot(MuscleGroup.GLUTES, PUMP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    private fun upper(key: String, name: String, word: String, accent: String = RED) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.CHEST, STR),
            MuscleSlot(MuscleGroup.BACK, STR),
            MuscleSlot(MuscleGroup.SHOULDERS, HYP),
            MuscleSlot(MuscleGroup.BICEPS, PUMP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP)
        ))

    private fun lower(key: String, name: String, word: String, accent: String = GOLD) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.HAMSTRINGS, STR),
            MuscleSlot(MuscleGroup.GLUTES, HYP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    private fun fullBody(key: String, name: String, word: String, accent: String = BLUE) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.CHEST, HYP),
            MuscleSlot(MuscleGroup.BACK, HYP),
            MuscleSlot(MuscleGroup.SHOULDERS, PUMP),
            MuscleSlot(MuscleGroup.HAMSTRINGS, HYP)
        ))

    private fun arms(key: String, name: String = "Arms & Delts", word: String = "ARMS", accent: String = PURPLE) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.BICEPS, STR),
            MuscleSlot(MuscleGroup.TRICEPS, STR),
            MuscleSlot(MuscleGroup.BICEPS, HYP),
            MuscleSlot(MuscleGroup.TRICEPS, HYP),
            MuscleSlot(MuscleGroup.REAR_DELTS, PUMP),
            MuscleSlot(MuscleGroup.SHOULDERS, PUMP)
        ))

    fun forDays(daysPerWeek: Int): List<DayArchetype> = when (daysPerWeek.coerceIn(1, 7)) {
        1 -> listOf(fullBody("fb", "Full Body", "FULL"))
        2 -> listOf(fullBody("fb-a", "Full Body A", "FULL"), fullBody("fb-b", "Full Body B", "BODY"))
        3 -> listOf(push("push"), pull("pull"), legs("legs"))
        4 -> listOf(
            upper("upper-a", "Upper A", "PUSH"), lower("lower-a", "Lower A", "QUADS"),
            upper("upper-b", "Upper B", "PULL", GREEN), lower("lower-b", "Lower B", "HAMS", PURPLE)
        )
        5 -> listOf(
            push("push"), pull("pull"), legs("legs"),
            upper("upper", "Upper", "UPPER", BLUE), lower("lower", "Lower", "LOWER", GOLD)
        )
        6 -> listOf(
            push("push-a"), pull("pull-a"), legs("legs-a"),
            push("push-b", "Push B", "PUSH"), pull("pull-b", "Pull B", "PULL"), legs("legs-b", "Legs B", "LEGS")
        )
        else -> listOf(
            push("push-a"), pull("pull-a"), legs("legs-a"),
            push("push-b", "Push B", "PUSH"), pull("pull-b", "Pull B", "PULL"),
            legs("legs-b", "Legs B", "LEGS"), arms("arms")
        )
    }
}
