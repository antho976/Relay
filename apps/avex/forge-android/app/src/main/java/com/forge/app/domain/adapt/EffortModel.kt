package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating

/**
 * Proximity-to-failure model (Coach v3 A1) — the one place that reads "how hard was that?".
 *
 * V2 answered the question from two inputs: per-set RPE and the coarse per-exercise
 * [EffortRating]. Three logged fields that say the same thing more precisely went unread by
 * every advisor: [LoggedSet.toFailure] (#18), [LoggedSet.difficultyTag] (#68) and
 * [LoggedSet.setType] (#142). Folding them in here means every advisor asks the question the
 * same way instead of re-deriving it — and the answer improves for users who log richly
 * without changing anything for users who don't.
 *
 * Authority order, strongest first:
 *  1. a set taken to failure, or an intensity technique that ends at/past failure by
 *     construction (drop / myo-rep / rest-pause / negative sets),
 *  2. per-set RPE,
 *  3. the per-set difficulty tag ("easy"/"hard"),
 *  4. the per-exercise [EffortRating].
 *
 * Warm-up sets never speak for working effort: a `setType = "warmup"` row is a label only
 * (it still counts toward volume and PRs, GYMAP-46) but a light warm-up must not read as
 * "there's room to progress".
 *
 * **Additive, like every other new signal:** with none of the new fields logged, [read]
 * returns exactly what v2's inlined rules returned. Pure and deterministic.
 */
object EffortModel {

    /** [LoggedSet.setType] code for a warm-up row — excluded from every effort read. */
    const val SET_TYPE_WARMUP = "warmup"

    /**
     * Set types that are taken to (or past) failure by construction, so the row itself is a
     * failure signal even when the user logged no RPE. Codes per [LoggedSet.setType] (#142).
     */
    private val PAST_FAILURE_SET_TYPES = setOf("drop", "myo", "rest_pause", "negative")

    private const val TAG_EASY = "easy"
    private const val TAG_HARD = "hard"

    /**
     * What the last bout says about effort.
     *
     * @param backOff the next prescription should step down.
     * @param backOffReason human sentence for [backOff], already in the app's voice.
     * @param roomToProgress the bout was easy enough to earn more next time.
     * @param highEffort the bout was near-maximal — a stall here means "reset", not "push".
     */
    data class Reading(
        val backOff: Boolean,
        val backOffReason: String,
        val roomToProgress: Boolean,
        val highEffort: Boolean
    )

    /** Working sets for effort purposes: warm-up rows carry no information about the top set. */
    fun workingSets(sets: List<LoggedSet>): List<LoggedSet> = sets.filter { it.setType != SET_TYPE_WARMUP }

    /**
     * Reads [sets] (one bout's sets, warm-ups included — they're filtered here) plus the bout's
     * per-exercise [prevEffort] rating into a single [Reading].
     */
    fun read(
        sets: List<LoggedSet>,
        prevEffort: EffortRating?,
        t: AdaptThresholds = AdaptThresholds()
    ): Reading {
        val working = workingSets(sets)
        val maxRpe = working.mapNotNull { it.rpe }.maxOrNull()
        val toFailure = working.any { it.toFailure }
        val pastFailureTechnique = working.any { it.setType in PAST_FAILURE_SET_TYPES }
        val taggedHard = working.any { it.difficultyTag == TAG_HARD }
        val taggedEasyThroughout = working.isNotEmpty() && working.all { it.difficultyTag == TAG_EASY }

        // ── Back off ───────────────────────────────────────────────────────────────
        // RPE stays the primary dial when logged. Failure is the fallback signal for users who
        // log the checkbox instead of a number — without an RPE it is the strongest thing said.
        val backOff: Boolean
        val backOffReason: String
        when {
            maxRpe != null -> {
                backOff = maxRpe >= t.rpeBrutalMin
                backOffReason = "last RPE hit ${trim(maxRpe)}"
            }
            toFailure -> {
                backOff = true
                backOffReason = "last set taken to failure"
            }
            pastFailureTechnique -> {
                backOff = true
                backOffReason = "last set ran past failure"
            }
            else -> {
                backOff = prevEffort == EffortRating.BRUTAL
                backOffReason = "last rated brutal"
            }
        }

        // ── Room to progress ───────────────────────────────────────────────────────
        val roomToProgress = when {
            toFailure || pastFailureTechnique -> false
            maxRpe != null -> maxRpe <= t.rpeEasyMax
            taggedHard -> false
            taggedEasyThroughout -> true
            else -> prevEffort == null || prevEffort == EffortRating.EASY || prevEffort == EffortRating.JUST_RIGHT
        }

        // ── High effort (stall signature) ──────────────────────────────────────────
        val highEffort = toFailure ||
            pastFailureTechnique ||
            (maxRpe != null && maxRpe >= t.rpeHighEffortMin) ||
            prevEffort == EffortRating.HARD ||
            prevEffort == EffortRating.BRUTAL ||
            taggedHard

        return Reading(
            backOff = backOff,
            backOffReason = backOffReason,
            roomToProgress = roomToProgress,
            highEffort = highEffort
        )
    }

    /** [read] for a stored bout — the snapshot-path callers (plateau ladder, fatigue drivers). */
    fun read(bout: ExerciseBout, t: AdaptThresholds = AdaptThresholds()): Reading =
        read(bout.sets, bout.effort, t)

    private fun trim(v: Double): String =
        if (v % 1.0 == 0.0) v.toInt().toString() else v.toString()
}
