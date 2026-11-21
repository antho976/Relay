package com.forge.app.domain.adapt

import kotlin.math.roundToInt

/**
 * Resting-HR trend read off the Health Connect samples in an [AdaptationSnapshot] — the same
 * window-vs-prior-month comparison [DeloadAdvisor] folds into its fatigue score, surfaced on its
 * own so the Overview "recovery snapshot" card can show the numbers when resting HR spikes.
 *
 * Compared against each user's OWN baseline (the prior month's average), never an absolute bpm.
 * Pure + deterministic, and gated exactly like the DeloadAdvisor driver so the two never disagree:
 * no Health Connect data ⇒ no samples ⇒ no spike.
 */
object RestingHrTrend {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** Current window vs baseline resting HR, both rounded to whole bpm. [deltaBpm] = window − baseline. */
    data class Readout(val windowBpm: Int, val baselineBpm: Int, val deltaBpm: Int)

    /**
     * A resting-HR spike, or null when there isn't one: returns the readout ONLY when both windows
     * clear the sample gate AND the recent-window average is at least
     * [AdaptThresholds.deloadRestingHrDeltaBpm] above the prior-month baseline. Mirrors the
     * elevated-resting-HR driver in [DeloadAdvisor.fatigue].
     */
    fun spike(
        samples: List<RestingHrSample>,
        nowMs: Long,
        t: AdaptThresholds = AdaptThresholds()
    ): Readout? {
        val windowStart = nowMs - t.deloadWindowDays * DAY_MS
        val priorStart = windowStart - t.deloadPriorBaselineDays * DAY_MS
        val windowHr = samples.filter { it.timeMs >= windowStart }
        val priorHr = samples.filter { it.timeMs in priorStart until windowStart }
        if (windowHr.size < t.deloadMinRestingHrSamples || priorHr.size < t.deloadMinRestingHrSamples) return null
        val windowAvg = windowHr.map { it.bpm }.average()
        val priorAvg = priorHr.map { it.bpm }.average()
        if (priorAvg <= 0 || windowAvg < priorAvg + t.deloadRestingHrDeltaBpm) return null
        return Readout(
            windowBpm = windowAvg.roundToInt(),
            baselineBpm = priorAvg.roundToInt(),
            deltaBpm = (windowAvg - priorAvg).roundToInt()
        )
    }
}
