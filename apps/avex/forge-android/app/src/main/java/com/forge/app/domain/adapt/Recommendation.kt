package com.forge.app.domain.adapt

/**
 * How sure the engine is about a recommendation. Modules stay SILENT below their data
 * thresholds (see [AdaptThresholds]) — LOW exists for surfaces that opt into showing
 * tentative advice, and the UI may hide LOW entirely.
 */
enum class Confidence { LOW, MEDIUM, HIGH }

/** Which advisor produced a recommendation. One value per adaptation-engine system. */
enum class AdviceSystem { PROGRESSION, REST, ORDERING, INSIGHT, DELOAD, READINESS }

/**
 * One piece of advisory output from the adaptation engine. Recommendations are *proposals
 * only* — nothing in the engine writes to the DB; applying one always goes through an
 * existing user-confirmed write path (suggestion chip tap, persistent swap, deload
 * generation, …).
 *
 * [id] is stable across runs for the same subject (e.g. "progression.weight.ua1") so later
 * phases can dedupe, cooldown, and log accept/dismiss feedback against it. [reason] is
 * always a complete human-readable sentence fragment — every suggestion must be explainable.
 */
sealed interface Recommendation {
    val id: String
    val reason: String
    val confidence: Confidence
    val system: AdviceSystem

    /**
     * Change the working weight on one exercise. [inputText] is expressed in the exercise's
     * *input unit* and is always parseable by [com.forge.app.domain.parser.WeightParser] —
     * a plate count like "3 plates" on PLATES exercises, a plain lb number on DUMBBELL.
     * (The old suggestion path emitted raw lb on plate exercises, which the weight field
     * would have parsed as a plate count — ~15× too heavy.)
     */
    data class WeightChange(
        val exerciseId: String,
        val exerciseName: String,
        val previousMaxLb: Double,
        val targetWeightLb: Double,
        val deltaLb: Double,
        val inputText: String,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "progression.weight.$exerciseId"
        override val system: AdviceSystem get() = AdviceSystem.PROGRESSION
    }

    /** Plateau escalation: same movement, different stimulus (e.g. "8-12" → "6-10"). */
    data class RepRangeShift(
        val exerciseId: String,
        val exerciseName: String,
        val fromReps: String,
        val toReps: String,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "progression.reprange.$exerciseId"
        override val system: AdviceSystem get() = AdviceSystem.PROGRESSION
    }

    /**
     * Bodyweight double-progression (CO5): there's no weight to add, so the next target is more reps.
     * Emitted only when every working set cleared the top of the rep range at acceptable effort.
     * [targetReps] is the rep count to aim for next time; surfaced as a hint on the day-screen chip
     * (pull-ups / push-ups had no progression cue before this), never auto-applied.
     */
    data class RepProgression(
        val exerciseId: String,
        val exerciseName: String,
        val fromReps: String,
        val targetReps: Int,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "progression.reps.$exerciseId"
        override val system: AdviceSystem get() = AdviceSystem.PROGRESSION
    }

    /**
     * Plateau escalation: swap the movement. [candidateIds] are [com.forge.app.program.ExerciseLibrary]
     * ids already filtered by the user's equipment and dislikes (the snapshot builder does
     * that filtering so this stays a pure value).
     */
    data class VariationSwap(
        val exerciseId: String,
        val exerciseName: String,
        val candidateIds: List<String>,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "progression.swap.$exerciseId"
        override val system: AdviceSystem get() = AdviceSystem.PROGRESSION
    }

    /**
     * Accumulated-fatigue deload call (System 5) — replaces the fixed 24-session counter.
     * [drivers] are the human-readable signals that fired, verbatim; [score] is their
     * transparent additive total. Apply path = the existing user-confirmed
     * `generateDeloadWeek()` — never automatic.
     */
    data class DeloadSuggestion(
        val score: Int,
        val drivers: List<String>,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "deload.suggest"
        override val system: AdviceSystem get() = AdviceSystem.DELOAD
    }

    /**
     * Today's readiness autoregulation (System 6): scale weight suggestions by [percent]
     * (bounded ±[com.forge.app.domain.adapt.AdaptThresholds.readinessMaxPercent]). Consumed
     * by ProgressionAdvisor's chip path; an explicit pre-session intensity pick outranks it.
     */
    data class ReadinessScale(
        val percent: Int,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "readiness.scale"
        override val system: AdviceSystem get() = AdviceSystem.READINESS
    }

    /**
     * Fatigue-aware session order (System 3): the day's exercises rearranged so no two
     * consecutive slots share a primary muscle, with compounds kept early, supersets never
     * split, and priority muscles given the freshest eligible slot. Applied via the
     * existing in-session reorder mechanism — never written to the program.
     */
    data class SessionOrder(
        val dayKey: String,
        val orderedExerciseIds: List<String>,
        val adjacenciesBefore: Int,
        val adjacenciesAfter: Int,
        override val reason: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "ordering.order.$dayKey"
        override val system: AdviceSystem get() = AdviceSystem.ORDERING
    }

    /**
     * A pure observation (System 4) — no apply action, rendered as inline rows on Stats.
     * [key] is stable per rule+subject so dismissals/cooldowns work ("insight.skip.ua3").
     */
    data class Insight(
        val key: String,
        val title: String,
        val body: String,
        override val confidence: Confidence
    ) : Recommendation {
        override val id: String get() = "insight.$key"
        override val reason: String get() = body
        override val system: AdviceSystem get() = AdviceSystem.INSIGHT
    }
}
