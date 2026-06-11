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
 * (3-day ≠ 7-day). Tuned (Phase 4) for **5–7 exercises per session**, ordered heavy-compound
 * first → isolation last. Per-slot set counts come from [VolumeModel] (frequency-aware), not from
 * here — so the same template gives ~10 chest sets/wk at 1× frequency and ~14 at 2×.
 *
 * Slots can repeat a muscle (e.g. CHEST ×2) — the generator picks a *distinct* exercise for each,
 * so you get a press + a second chest movement rather than the same lift twice.
 *
 * Repeated-day splits use **complementary A/B shapes** (push-lean/pull-lean upper days, quad-lean/
 * ham-lean lower days, squat-led/hinge-led full-body days) instead of running the same template
 * twice — that's what covers every muscle across the week and gives each day its own identity.
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
            // Hamstrings get a second slot (squat + RDL + leg ext + leg curl is the classic shape) —
            // with one slot they sat at ~half the weekly quad volume on the 3-day split. Glutes run
            // HYP, not PUMP, so the hip thrust (a compound) isn't down-weighted out of its best slot.
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.HAMSTRINGS, STR),
            MuscleSlot(MuscleGroup.QUADS, HYP),
            MuscleSlot(MuscleGroup.HAMSTRINGS, HYP),
            MuscleSlot(MuscleGroup.GLUTES, HYP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    /** Balanced upper day — used once in the 5-day split (PPL already provides the push/pull lean). */
    private fun upper(key: String, name: String, word: String, accent: String = RED) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.CHEST, STR),
            MuscleSlot(MuscleGroup.BACK, STR),
            MuscleSlot(MuscleGroup.SHOULDERS, HYP),
            MuscleSlot(MuscleGroup.REAR_DELTS, PUMP),
            MuscleSlot(MuscleGroup.BICEPS, PUMP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP)
        ))

    /**
     * 4-day Upper A — push-leaning: two chest slots + laterals (the seed split's "PUSH" identity).
     * Running the same balanced upper template twice left chest AND back at just 1 movement/day.
     * Chest AND back both open STR on each upper day — a muscle's only slot of the day must be a
     * heavy compound (a press/row), never an isolation like a fly or straight-arm pulldown.
     */
    private fun upperA(key: String, name: String, word: String, accent: String = RED) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.CHEST, STR),
            MuscleSlot(MuscleGroup.BACK, STR),
            MuscleSlot(MuscleGroup.CHEST, HYP),
            MuscleSlot(MuscleGroup.SHOULDERS, HYP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP),
            MuscleSlot(MuscleGroup.BICEPS, PUMP)
        ))

    /** 4-day Upper B — pull-leaning: two back slots + the week's rear-delt posture work ("PULL"). */
    private fun upperB(key: String, name: String, word: String, accent: String = GREEN) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.BACK, STR),
            MuscleSlot(MuscleGroup.CHEST, STR),
            MuscleSlot(MuscleGroup.BACK, HYP),
            MuscleSlot(MuscleGroup.SHOULDERS, PUMP),
            MuscleSlot(MuscleGroup.REAR_DELTS, PUMP),
            MuscleSlot(MuscleGroup.BICEPS, HYP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP)
        ))

    /** 5-day balanced lower day. The 4-day split uses the leaning [lowerA]/[lowerB] pair instead. */
    private fun lower(key: String, name: String, word: String, accent: String = GOLD) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.HAMSTRINGS, STR),
            MuscleSlot(MuscleGroup.GLUTES, HYP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    /** 4-day Lower A — quad-leaning: squat lead + a quad accessory (the seed split's "QUADS" day). */
    private fun lowerA(key: String, name: String, word: String, accent: String = GOLD) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.HAMSTRINGS, HYP),
            MuscleSlot(MuscleGroup.QUADS, HYP),
            MuscleSlot(MuscleGroup.GLUTES, HYP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    /** 4-day Lower B — hamstring/glute-leaning: hinge lead + a ham accessory ("HAMS"). */
    private fun lowerB(key: String, name: String, word: String, accent: String = PURPLE) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.HAMSTRINGS, STR),
            MuscleSlot(MuscleGroup.QUADS, HYP),
            MuscleSlot(MuscleGroup.HAMSTRINGS, HYP),
            MuscleSlot(MuscleGroup.GLUTES, HYP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    /**
     * Full body A — squat-led, with direct arm work. 1- and 2-day plans use only the full-body
     * templates, so A + B together must cover the whole body: A carries arms/shoulders, B carries
     * glutes/rear delts/calves (which used to get ZERO weekly volume on a 2-day plan).
     */
    private fun fullBodyA(key: String, name: String, word: String, accent: String = BLUE) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.QUADS, STR),
            MuscleSlot(MuscleGroup.CHEST, HYP),
            MuscleSlot(MuscleGroup.BACK, HYP),
            MuscleSlot(MuscleGroup.HAMSTRINGS, HYP),
            MuscleSlot(MuscleGroup.SHOULDERS, PUMP),
            MuscleSlot(MuscleGroup.BICEPS, PUMP),
            MuscleSlot(MuscleGroup.TRICEPS, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    /** Full body B — hinge-led complement to [fullBodyA]: glutes, rear delts and calves live here. */
    private fun fullBodyB(key: String, name: String, word: String, accent: String = BLUE) =
        DayArchetype(key, name, word, accent, listOf(
            MuscleSlot(MuscleGroup.HAMSTRINGS, STR),
            MuscleSlot(MuscleGroup.CHEST, HYP),
            MuscleSlot(MuscleGroup.BACK, HYP),
            MuscleSlot(MuscleGroup.QUADS, HYP),
            MuscleSlot(MuscleGroup.GLUTES, HYP),
            MuscleSlot(MuscleGroup.REAR_DELTS, PUMP),
            MuscleSlot(MuscleGroup.CALVES, PUMP),
            MuscleSlot(MuscleGroup.CORE, PUMP)
        ))

    private fun arms(key: String, name: String = "Arms & Delts", word: String = "ARMS", accent: String = PURPLE) =
        DayArchetype(key, name, word, accent, listOf(
            // Leads with the triceps press — the library has no biceps compound, and a STRENGTH curl
            // slot prescribed 4-6 rep heaving under the get_stronger goal. Curls run HYP/PUMP.
            MuscleSlot(MuscleGroup.TRICEPS, STR),
            MuscleSlot(MuscleGroup.BICEPS, HYP),
            MuscleSlot(MuscleGroup.TRICEPS, HYP),
            MuscleSlot(MuscleGroup.BICEPS, PUMP),
            MuscleSlot(MuscleGroup.SHOULDERS, PUMP),
            MuscleSlot(MuscleGroup.REAR_DELTS, PUMP)
        ))

    fun forDays(daysPerWeek: Int): List<DayArchetype> = when (daysPerWeek.coerceIn(1, 7)) {
        1 -> listOf(fullBodyA("fb", "Full Body", "FULL"))
        2 -> listOf(fullBodyA("fb-a", "Full Body A", "FULL"), fullBodyB("fb-b", "Full Body B", "BODY"))
        3 -> listOf(push("push"), pull("pull"), legs("legs"))
        4 -> listOf(
            upperA("upper-a", "Upper A", "PUSH"), lowerA("lower-a", "Lower A", "QUADS"),
            upperB("upper-b", "Upper B", "PULL"), lowerB("lower-b", "Lower B", "HAMS")
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
