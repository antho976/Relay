package com.forge.app.domain.engine

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.adapt.RestingHrSample
import kotlin.math.roundToInt

/**
 * Is the aerobic base actually improving? (Engine E-D)
 *
 * No lab, no lactate test — three cheap signals triangulated:
 *  - **pace at a given effort**: same route, same effort, faster is fitter;
 *  - **resting heart rate trend**: the cheapest recovery signal there is;
 *  - **volume consistency**: a base you stopped building is a base that fades.
 *
 * Hard-gated like every estimator in the app, and honest about confounders: sessions logged in
 * heat, cold, rain or wind are excluded from pace comparisons, because weather inflates heart rate
 * and would otherwise read as lost fitness. The Engine's own lesson teaches exactly that, so the
 * estimator had better practice it.
 */
object AerobicBase {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** Paced sessions needed on each side before a pace trend is claimed. */
    const val MIN_PACED_SESSIONS = 4

    /** Percent change in pace that counts as a real move rather than noise. */
    const val PACE_MOVE_PCT = 3.0

    enum class Trend { IMPROVING, HOLDING, DETRAINING, UNKNOWN }

    /**
     * @param trend the verdict.
     * @param reading the one line explaining it, in the athlete's own numbers.
     * @param confident false when only one weak signal supported the read.
     */
    data class BaseRead(val trend: Trend, val reading: String, val confident: Boolean)

    fun assess(
        entries: List<CardioEntry>,
        restingHr: List<RestingHrSample>,
        nowMs: Long
    ): BaseRead {
        val pace = paceTrend(entries, nowMs)
        val rhr = restingHrTrend(restingHr, nowMs)

        return when {
            pace == Trend.IMPROVING && rhr != Trend.DETRAINING ->
                BaseRead(Trend.IMPROVING, "You're covering the same ground faster at the same effort.", true)
            pace == Trend.DETRAINING && rhr != Trend.IMPROVING ->
                BaseRead(Trend.DETRAINING, "Your pace at the same effort has slipped.", true)
            rhr == Trend.IMPROVING ->
                BaseRead(Trend.IMPROVING, "Your resting heart rate has come down.", false)
            rhr == Trend.DETRAINING ->
                BaseRead(Trend.DETRAINING, "Your resting heart rate has drifted up.", false)
            pace == Trend.HOLDING || rhr == Trend.HOLDING ->
                BaseRead(Trend.HOLDING, "Your base is holding steady.", pace == Trend.HOLDING)
            else ->
                BaseRead(Trend.UNKNOWN, "Not enough comparable sessions yet.", false)
        }
    }

    /**
     * Pace at a given effort, recent half against the earlier half. Only same-type, same-effort,
     * clean-weather sessions are compared: a hot 5k and a cool 5k are not the same test.
     */
    fun paceTrend(entries: List<CardioEntry>, nowMs: Long): Trend {
        val usable = entries.filter { entry ->
            entry.restReason == null &&
                entry.distanceKm != null && entry.distanceKm > 0 &&
                entry.durationMin > 0 &&
                entry.conditions.isNullOrBlank() && // weather-confounded sessions are excluded
                entry.date >= nowMs - 90 * DAY_MS
        }.sortedBy { it.date }
        if (usable.size < MIN_PACED_SESSIONS * 2) return Trend.UNKNOWN

        // Group by type + effort so like is compared with like.
        val byKind = usable.groupBy { it.type to (it.effort ?: "moderate") }
        val best = byKind.values.maxByOrNull { it.size } ?: return Trend.UNKNOWN
        if (best.size < MIN_PACED_SESSIONS * 2) return Trend.UNKNOWN

        val half = best.size / 2
        val earlier = best.take(half).map { paceMinPerKm(it) }
        val recent = best.drop(half).map { paceMinPerKm(it) }
        val earlierAvg = earlier.average()
        val recentAvg = recent.average()
        if (earlierAvg <= 0) return Trend.UNKNOWN

        // Lower pace is faster, so a NEGATIVE change is an improvement.
        val changePct = ((recentAvg - earlierAvg) / earlierAvg) * 100
        return when {
            changePct <= -PACE_MOVE_PCT -> Trend.IMPROVING
            changePct >= PACE_MOVE_PCT -> Trend.DETRAINING
            else -> Trend.HOLDING
        }
    }

    /** Resting HR over the last fortnight against the month before it. */
    fun restingHrTrend(samples: List<RestingHrSample>, nowMs: Long): Trend {
        val recent = samples.filter { it.timeMs >= nowMs - 14 * DAY_MS }.map { it.bpm }
        val prior = samples.filter { it.timeMs in (nowMs - 45 * DAY_MS) until (nowMs - 14 * DAY_MS) }.map { it.bpm }
        if (recent.size < 5 || prior.size < 5) return Trend.UNKNOWN
        val delta = recent.average() - prior.average()
        return when {
            delta <= -2 -> Trend.IMPROVING
            delta >= 2 -> Trend.DETRAINING
            else -> Trend.HOLDING
        }
    }

    /** Minutes per kilometre — lower is faster. */
    private fun paceMinPerKm(entry: CardioEntry): Double =
        entry.durationMin / (entry.distanceKm ?: 1.0)

    /** How the base read should change next week's conditioning volume. */
    fun volumeAdvice(read: BaseRead): String = when (read.trend) {
        Trend.IMPROVING -> "Hold this volume while it's still paying off."
        Trend.HOLDING -> "A small increase is worth trying."
        Trend.DETRAINING -> "Get the easy minutes back before adding anything hard."
        Trend.UNKNOWN -> "Keep logging; a few more comparable sessions and this reads properly."
    }
}
