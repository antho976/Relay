package com.forge.app.domain.coach

import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.adapt.AdaptThresholds

/**
 * Periodization as a state machine (Coach v3 C).
 *
 * The coach stops being purely reactive here. Instead of waiting for a stall or a fatigue score to
 * fire, it runs a block with a stated intent, advances it once a week, and schedules the deload in
 * advance — the fatigue tripwire can still pull that deload EARLIER, but it is no longer the only
 * way rest ever happens.
 *
 * Pure and idempotent: [advance] is keyed by ISO week, so running the weekly pass twice in one week
 * moves nothing.
 */
object BlockPlanner {

    /** Default block shape: three build weeks, one peak-ish week, then a deload. */
    const val DEFAULT_WEEKS = 5

    /** A block shorter than this can't express a phase progression. */
    const val MIN_WEEKS = 3

    /**
     * Start a block. The shape is deliberately simple and explainable: accumulate for most of it,
     * intensify near the end, and finish with a planned deload week.
     */
    fun start(
        nowMs: Long,
        weekId: String,
        plannedWeeks: Int = DEFAULT_WEEKS,
        focusGoalId: Long = 0,
        intent: String = ""
    ): TrainingBlock = TrainingBlock(
        phase = BlockPhase.ACCUMULATE.code,
        weekIndex = 1,
        plannedWeeks = plannedWeeks.coerceAtLeast(MIN_WEEKS),
        focusGoalId = focusGoalId,
        intent = intent,
        startedAt = nowMs,
        lastAdvancedWeek = weekId
    )

    /**
     * Move the block on by one week, or return it untouched when this week already advanced it.
     *
     * @param fatigueScore the deload advisor's current score; a high one pulls the deload forward,
     *   which is the whole point of keeping the tripwire alive alongside the schedule.
     */
    fun advance(
        block: TrainingBlock,
        weekId: String,
        nowMs: Long,
        fatigueScore: Int = 0,
        t: AdaptThresholds = AdaptThresholds()
    ): TrainingBlock {
        if (block.lastAdvancedWeek == weekId) return block
        if (!block.isActive) return block

        val nextIndex = block.weekIndex + 1
        // The block is done: its deload week has been served.
        if (block.phase == BlockPhase.DELOAD.code) {
            return block.copy(endedAt = nowMs, lastAdvancedWeek = weekId)
        }
        // Fatigue can pull the deload forward, but never past the block's own schedule.
        val earlyDeload = fatigueScore >= t.deloadScoreThreshold && nextIndex >= MIN_WEEKS
        val phase = when {
            earlyDeload -> BlockPhase.DELOAD
            nextIndex >= block.plannedWeeks -> BlockPhase.DELOAD
            nextIndex >= block.plannedWeeks - 1 -> BlockPhase.PEAK
            nextIndex >= block.plannedWeeks - 2 -> BlockPhase.INTENSIFY
            else -> BlockPhase.ACCUMULATE
        }
        return block.copy(
            phase = phase.code,
            weekIndex = nextIndex,
            lastAdvancedWeek = weekId
        )
    }

    /** How many weeks until this block's deload week, or 0 when it is the deload week. */
    fun weeksToDeload(block: TrainingBlock): Int =
        if (block.phase == BlockPhase.DELOAD.code) 0
        else (block.plannedWeeks - block.weekIndex).coerceAtLeast(0)

    /** The line the coach screen and the directive both use — one sentence, no jargon. */
    fun describe(block: TrainingBlock): String {
        val phase = BlockPhase.fromCode(block.phase) ?: BlockPhase.ACCUMULATE
        val weeks = weeksToDeload(block)
        return when (phase) {
            BlockPhase.ACCUMULATE ->
                "Week ${block.weekIndex} of ${block.plannedWeeks}, building volume. " +
                    if (weeks > 0) "Deload in $weeks weeks." else ""
            BlockPhase.INTENSIFY ->
                "Week ${block.weekIndex} of ${block.plannedWeeks}, trading volume for load."
            BlockPhase.PEAK ->
                "Week ${block.weekIndex} of ${block.plannedWeeks}, expressing what you built."
            BlockPhase.DELOAD ->
                "Deload week. Loads and volume come down so the work catches up with you."
        }.trim()
    }

    /**
     * Whether this week should test a maximum (Coach v3 C's test protocol). A peak phase that never
     * tests is a promise with no payoff: the block builds strength, then measures it, and the e1RM
     * that comes out is what the strength goals are tracked against.
     */
    fun isTestWeek(block: TrainingBlock): Boolean = block.phase == BlockPhase.PEAK.code
}

/** The four phases a block moves through, in order. */
enum class BlockPhase(val code: String, val displayName: String) {
    /** Build the volume that drives adaptation, at moderate effort. */
    ACCUMULATE("accumulate", "Accumulate"),

    /** Trade volume for load: fewer sets, heavier work. */
    INTENSIFY("intensify", "Intensify"),

    /** Express the strength that was built, and measure it. */
    PEAK("peak", "Peak"),

    /** Planned recovery: the week that turns the work into adaptation. */
    DELOAD("deload", "Deload");

    /**
     * How aggressively progression should behave in this phase, as a multiplier on the coach's
     * usual ambition. Deload holds everything back; peak pushes.
     */
    val progressionScale: Double
        get() = when (this) {
            ACCUMULATE -> 1.0
            INTENSIFY -> 1.05
            PEAK -> 1.05
            DELOAD -> 0.85
        }

    /** Weekly set delta this phase asks of the volume model. */
    val volumeDelta: Int
        get() = when (this) {
            ACCUMULATE -> 1
            INTENSIFY -> 0
            PEAK -> -1
            DELOAD -> -2
        }

    companion object {
        fun fromCode(code: String): BlockPhase? = entries.firstOrNull { it.code == code }
    }
}
