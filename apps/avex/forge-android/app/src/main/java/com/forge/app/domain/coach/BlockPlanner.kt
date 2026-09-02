package com.forge.app.domain.coach

import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.adapt.AdaptThresholds
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.ChronoUnit

/**
 * Periodization as a state machine (Coach v3 C).
 *
 * The coach stops being purely reactive here. Instead of waiting for a stall or a fatigue score to
 * fire, it runs a block with a stated intent, advances it once a week, and schedules the deload in
 * advance — the fatigue tripwire can still pull that deload EARLIER, but it is no longer the only
 * way rest ever happens.
 *
 * Pure and idempotent: [advance] is keyed by ISO week, so running the weekly pass twice in one week
 * moves nothing — and because it compares weeks ORDINALLY rather than for equality, a block also
 * can't move backwards, and weeks the user never opened the app in are not lost.
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
     * Move the block on to [weekId], or return it untouched when it has already reached that week.
     *
     * The guard used to be `lastAdvancedWeek == weekId` — equality, not ordering — and the block
     * moved exactly one week per pass regardless of how many had actually gone by. So a user who
     * didn't open the app for three weeks advanced the block by ONE, and "Deload in N weeks" was
     * then wrong by however long they were away. Weeks elapsed are counted from the two week ids
     * and applied in full, so the block's position always reflects real elapsed time.
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
        if (!block.isActive) return block
        // Ordinal, not equal: a week id that is the same as — or earlier than — the last one it
        // advanced on moves nothing, so a clock change or an out-of-order pass can't rewind a block.
        val weeksElapsed = weeksBetween(block.lastAdvancedWeek, weekId)
        if (weeksElapsed <= 0) return block

        // A long absence is caught up rather than lost, bounded by the block's own length: stepping
        // past the deload ends the block, so the loop always terminates inside plannedWeeks + 1.
        var out = block
        repeat(weeksElapsed.coerceAtMost(block.plannedWeeks + 1)) {
            if (!out.isActive) return out
            out = step(out, weekId, nowMs, fatigueScore, t)
        }
        return out
    }

    /** One week of block progression — the state machine's single transition. */
    private fun step(
        block: TrainingBlock,
        weekId: String,
        nowMs: Long,
        fatigueScore: Int,
        t: AdaptThresholds
    ): TrainingBlock {
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

    /**
     * Whole ISO weeks from [from] to [to], both "yyyy-Www". 0 when either is missing or unparseable,
     * so a block with no recorded week (or a malformed one) simply doesn't move until the next pass
     * records a good id.
     */
    private fun weeksBetween(from: String, to: String): Int {
        val a = weekStart(from) ?: return 0
        val b = weekStart(to) ?: return 0
        return ChronoUnit.WEEKS.between(a, b).toInt()
    }

    /** The Monday of an ISO week id, or null when it isn't one. */
    private fun weekStart(weekId: String): LocalDate? {
        val m = WEEK_ID.matchEntire(weekId) ?: return null
        val year = m.groupValues[1].toIntOrNull() ?: return null
        val week = m.groupValues[2].toIntOrNull() ?: return null
        // Jan 4 is always in ISO week 1 of its week-based year.
        return LocalDate.of(year, 1, 4).with(DayOfWeek.MONDAY).plusWeeks((week - 1).toLong())
    }

    private val WEEK_ID = Regex("""(\d{4})-W(\d{2})""")

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

    /**
     * True when [advance] has just carried [before] into its deload week and that week is the LIVE
     * one — the moment the repository serves the scheduled deload through the same regeneration the
     * reactive coach uses. A block that caught up past its deload (a long absence stepping through
     * the deload week and ending) has missed the week, so nothing is generated for it; a block that
     * was already deloading isn't entering anything.
     */
    fun entersDeload(before: TrainingBlock, after: TrainingBlock): Boolean =
        after.isActive &&
            after.phase == BlockPhase.DELOAD.code &&
            before.phase != BlockPhase.DELOAD.code
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
     *
     * Not yet threaded into the per-set load suggestion: the phase currently shapes the week
     * through [volumeDelta] (the weekly pass) and the served deload week (BlockRepository), which
     * is where the audited "phases changed nothing" gap was closed. Scaling the in-session load
     * target by this value is the remaining step and is tracked in docs/AUDIT_DEFERRED.md.
     */
    val progressionScale: Double
        get() = when (this) {
            ACCUMULATE -> 1.0
            INTENSIFY -> 1.05
            PEAK -> 1.05
            DELOAD -> 0.85
        }

    /**
     * Weekly set delta this phase asks of the volume model. The weekly pass ([AutoCoachPlanner])
     * reads its sign: positive lets the pass add a set, zero holds volume where it is, negative
     * trims a set. The deload's cut is served by the deload regeneration itself, so the planner
     * never has to express −2 as decisions.
     */
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
