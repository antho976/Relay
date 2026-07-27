package com.forge.app.domain.coach

import com.forge.app.data.db.entities.BodyweightEntry

/**
 * Which direction the athlete's bodyweight is actually going (Coach v3 A2) — the
 * `bodyweight_goal` signal slot, and the first consumer of the bodyweight series A1 plumbed in.
 *
 * Why the coach needs it: a flat e1RM means opposite things in a deficit and in a surplus. Held
 * strength while losing weight is a WIN (you kept the muscle and got relatively stronger), so the
 * plateau ladder must not escalate it into a reset the way it would in maintenance. That single
 * rule is the most counterintuitive thing the coach does, which makes it the most trust-building
 * thing it can explain — lesson `coach.strength_on_a_cut` hangs off exactly this detection.
 *
 * Detection is deliberately dumb and gated: a robust weekly rate over smoothed weigh-ins, needing
 * real span and real movement. Not enough data ⇒ [UNKNOWN] ⇒ nothing changes anywhere.
 */
enum class WeightPhase(val code: String, val displayName: String) {
    CUT("cut", "Cutting"),
    MAINTAIN("maintain", "Maintaining"),
    BULK("bulk", "Gaining"),
    UNKNOWN("unknown", "Not enough weigh-ins");

    val isKnown: Boolean get() = this != UNKNOWN

    companion object {
        private const val DAY_MS = 24L * 60 * 60 * 1000
        private const val WEEK_MS = 7 * DAY_MS

        /** Weigh-ins needed before any phase is claimed. */
        const val MIN_ENTRIES = 6

        /** Calendar span the weigh-ins must cover — six readings in two days say nothing. */
        const val MIN_SPAN_DAYS = 14

        /**
         * Pounds per week past which the trend reads as a real cut/gain rather than noise.
         * ~0.5 lb/wk is the low end of a deliberate deficit and comfortably above water drift.
         */
        const val RATE_LB_PER_WEEK = 0.5

        /** How many recent entries the smoothed "current weight" averages over. */
        private const val SMOOTH_WINDOW = 7

        /**
         * The phase implied by [entries] (any order). [UNKNOWN] whenever the data can't support a
         * claim — the caller then behaves exactly as it did before this existed.
         */
        fun of(entries: List<BodyweightEntry>): WeightPhase {
            val sorted = entries.sortedBy { it.recordedAt }
            if (sorted.size < MIN_ENTRIES) return UNKNOWN
            val spanMs = sorted.last().recordedAt - sorted.first().recordedAt
            if (spanMs < MIN_SPAN_DAYS * DAY_MS) return UNKNOWN
            val rate = ratePerWeek(sorted) ?: return UNKNOWN
            return when {
                rate <= -RATE_LB_PER_WEEK -> CUT
                rate >= RATE_LB_PER_WEEK -> BULK
                else -> MAINTAIN
            }
        }

        /** Least-squares pounds-per-week over the weigh-ins, or null when they don't span enough. */
        fun ratePerWeek(entries: List<BodyweightEntry>): Double? {
            val sorted = entries.sortedBy { it.recordedAt }
            if (sorted.size < 3) return null
            val originMs = sorted.first().recordedAt
            val xs = sorted.map { (it.recordedAt - originMs).toDouble() / WEEK_MS }
            val ys = sorted.map { it.weightLb }
            val meanX = xs.average()
            val meanY = ys.average()
            var num = 0.0
            var den = 0.0
            for (i in xs.indices) {
                num += (xs[i] - meanX) * (ys[i] - meanY)
                den += (xs[i] - meanX) * (xs[i] - meanX)
            }
            return if (den == 0.0) null else num / den
        }

        /**
         * Current weight as the average of the most recent [SMOOTH_WINDOW] weigh-ins — one salty
         * dinner must never move a goal readout or flip a phase.
         */
        fun smoothedLatest(entries: List<BodyweightEntry>): Double? {
            if (entries.isEmpty()) return null
            return entries.sortedByDescending { it.recordedAt }
                .take(SMOOTH_WINDOW)
                .map { it.weightLb }
                .average()
        }
    }
}
