package com.forge.app.ui.gym.stats

import com.forge.app.domain.adapt.HealthSnap

/** One watch signal over the last [BODY_DAYS] days, oldest first; a day with no reading is null. */
data class BodySeries(
    val days: List<Double?>,
    /** Today's value (last night, for sleep), or null when the watch has not reported it yet. */
    val latest: Double?,
    /** The median of the earlier days, needing three of them. */
    val usual: Double?
) {
    val hasData: Boolean get() = days.any { it != null }
}

/** What the Body lens shows. Null series = that signal is not granted or never reported. */
data class StatsBodyUi(
    /** Health Connect is installed and usable on this phone. */
    val available: Boolean,
    /** At least one body read is granted. */
    val connected: Boolean,
    val sleepMin: BodySeries? = null,
    val restingHr: BodySeries? = null,
    val hrvMs: BodySeries? = null,
    val steps: BodySeries? = null
)

const val BODY_DAYS = 14

/**
 * Buckets a Health Connect read into per-day series ending today. [dayStartMs] maps an epoch-ms to
 * its local midnight, so the caller owns the zone.
 */
fun statsBodySeries(
    snap: HealthSnap,
    todayStartMs: Long,
    dayStartMs: (Long) -> Long,
    stepsGranted: Boolean
): StatsBodyUi {
    val dayMs = 24L * 60 * 60 * 1000
    val firstDay = todayStartMs - (BODY_DAYS - 1) * dayMs
    fun index(ms: Long): Int? {
        val day = dayStartMs(ms)
        if (day < firstDay || day > todayStartMs) return null
        return ((day - firstDay + dayMs / 2) / dayMs).toInt().coerceIn(0, BODY_DAYS - 1)
    }
    fun series(points: List<Pair<Long, Double>>, combine: (List<Double>) -> Double): BodySeries? {
        if (points.isEmpty()) return null
        val byDay = arrayOfNulls<MutableList<Double>>(BODY_DAYS)
        points.forEach { (ms, v) -> index(ms)?.let { (byDay[it] ?: mutableListOf<Double>().also { l -> byDay[it] = l }).add(v) } }
        val days = byDay.map { it?.let(combine) }
        val earlier = days.dropLast(1).filterNotNull().sorted()
        val usual = if (earlier.size >= 3) earlier[earlier.size / 2] else null
        return BodySeries(days = days, latest = days.last(), usual = usual)
    }
    return StatsBodyUi(
        available = true,
        connected = true,
        // A night belongs to the morning it ended on.
        sleepMin = series(snap.sleepNights.map { it.endedAtMs to it.durationMin.toDouble() }) { it.sum() },
        restingHr = series(snap.restingHr.map { it.timeMs to it.bpm.toDouble() }) { it.last() },
        hrvMs = series(snap.hrv.map { it.timeMs to it.rmssdMs }) { it.average() },
        steps = if (!stepsGranted) null else series(snap.dailySteps.map { it.dayStartMs to it.steps.toDouble() }) { it.sum() }
    )
}
