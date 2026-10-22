package com.forge.app.ui.goals

import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.trimDecimal
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.roundToInt

/*
 * The Set-the-target screen's arithmetic ("pg-goals gt"): where the slider starts and ends, what the
 * target is worth at your pace, and the done-and-projected bars. Every value here is in the DISPLAY
 * unit; the screen converts to the stored unit once, on Set goal.
 */

/** The slider's span in the display unit: [lo] is where you are, [hi] the far named end. */
internal data class TargetScale(val lo: Double, val hi: Double, val step: Double) {
    val mid: Double get() = snap((lo + hi) / 2.0)
    fun snap(v: Double): Double {
        val s = lo + ((v - lo) / step).roundToInt() * step
        return s.coerceIn(lo, hi)
    }
    fun fraction(v: Double): Float = if (hi <= lo) 0f else ((v - lo) / (hi - lo)).toFloat().coerceIn(0f, 1f)
    fun at(f: Float): Double = snap(lo + (hi - lo) * f.coerceIn(0f, 1f))
}

private fun floorTo(v: Double, step: Double) = floor(v / step + 1e-9) * step
private fun ceilTo(v: Double, step: Double) = ceil(v / step - 1e-9) * step

/** A lift target's slider: plate steps, from your best (or an empty bar) to a long way past it. */
internal fun liftScale(nowDisplay: Double, targetDisplay: Double?, unit: WeightUnit): TargetScale {
    val step = when (unit) { WeightUnit.KG -> 2.5; WeightUnit.ST -> 0.5; WeightUnit.LB -> 5.0 }
    val span = when (unit) { WeightUnit.KG -> 30.0; WeightUnit.ST -> 4.5; WeightUnit.LB -> 60.0 }
    val lo = if (nowDisplay > 0) floorTo(nowDisplay, step) else step
    val far = if (nowDisplay > 0) lo + span else lo + span * 5
    val hi = maxOf(far, targetDisplay?.let { ceilTo(it, step) } ?: 0.0)
    return TargetScale(lo, hi, step)
}

/** Where a new lift target starts: a third of the way along, a real next step past your best. */
internal fun liftDefaultTarget(scale: TargetScale): Double = scale.snap(scale.lo + (scale.hi - scale.lo) / 3.0)

/** A bodyweight target's slider: either side of today's weight. */
internal fun bodyweightScale(nowDisplay: Double, targetDisplay: Double?, unit: WeightUnit): TargetScale {
    val step = when (unit) { WeightUnit.ST -> 0.1; else -> 0.5 }
    val span = when (unit) { WeightUnit.KG -> 10.0; WeightUnit.ST -> 1.5; WeightUnit.LB -> 20.0 }
    val center = if (nowDisplay > 0) nowDisplay else when (unit) { WeightUnit.KG -> 80.0; WeightUnit.ST -> 12.5; WeightUnit.LB -> 175.0 }
    var lo = floorTo(center - span, step)
    var hi = ceilTo(center + span, step)
    targetDisplay?.let { t -> lo = minOf(lo, floorTo(t, step)); hi = maxOf(hi, ceilTo(t, step)) }
    return TargetScale(lo, hi, step)
}

/** A cumulative target's slider, in the metric's display unit. [recent] is your recent windows. */
internal fun cumulativeScale(metric: GoalMetric, period: GoalPeriod, recent: List<Double>, targetDisplay: Double?, unit: WeightUnit): TargetScale {
    val x = when (period) { GoalPeriod.WEEK -> 1.0; GoalPeriod.MONTH -> 4.0; GoalPeriod.ALL -> 20.0 }
    val (step, floorHi) = when (metric) {
        GoalMetric.SESSIONS -> when (period) {
            GoalPeriod.WEEK -> 1.0 to 7.0
            GoalPeriod.MONTH -> 1.0 to 31.0
            GoalPeriod.ALL -> 10.0 to 500.0
        }
        GoalMetric.VOLUME -> (if (unit == WeightUnit.LB) 1000.0 else 500.0) * (if (period == GoalPeriod.ALL) 10 else 1) to
            (if (unit == WeightUnit.LB) 100_000.0 else 45_000.0) * x
        GoalMetric.CARDIO_DISTANCE -> when (period) {
            GoalPeriod.WEEK -> 0.5 to 30.0
            GoalPeriod.MONTH -> 1.0 to 120.0
            GoalPeriod.ALL -> 10.0 to 1000.0
        }
        GoalMetric.CARDIO_MINUTES -> when (period) {
            GoalPeriod.WEEK -> 5.0 to 300.0
            GoalPeriod.MONTH -> 15.0 to 1200.0
            GoalPeriod.ALL -> 50.0 to 6000.0
        }
        GoalMetric.BODYWEIGHT -> return bodyweightScale(recent.lastOrNull() ?: 0.0, targetDisplay, unit)
    }
    // Sessions in a week can't pass seven, whatever the history says.
    val capped = metric == GoalMetric.SESSIONS && period != GoalPeriod.ALL
    val best = recent.maxOrNull() ?: 0.0
    val hi = if (capped) floorHi else maxOf(floorHi, ceilTo(best * 2, step), targetDisplay?.let { ceilTo(it, step) } ?: 0.0)
    return TargetScale(step, hi, step)
}

/** Where a new cumulative target starts: a step past your recent average, or the kind's example. */
internal fun cumulativeDefaultTarget(metric: GoalMetric, period: GoalPeriod, recent: List<Double>, scale: TargetScale, unit: WeightUnit): Double {
    val done = recent.dropLast(1).filter { it > 0 }
    if (done.isNotEmpty()) return scale.snap(done.average() + scale.step)
    val x = when (period) { GoalPeriod.WEEK -> 1.0; GoalPeriod.MONTH -> 4.0; GoalPeriod.ALL -> 20.0 }
    val example = when (metric) {
        GoalMetric.SESSIONS -> 4.0
        GoalMetric.VOLUME -> if (unit == WeightUnit.LB) 60_000.0 else 27_000.0
        GoalMetric.CARDIO_DISTANCE -> 10.0
        GoalMetric.CARDIO_MINUTES -> 150.0
        GoalMetric.BODYWEIGHT -> scale.mid
    }
    return scale.snap(if (metric == GoalMetric.BODYWEIGHT) example else example * x)
}

/** A slider value as its label: whole where whole, one decimal otherwise. */
internal fun scaleLabel(v: Double): String = trimDecimal(v)

/** "Early December", "Mid March 2027": when [weeks] from [today] lands, at month resolution. */
internal fun landingText(today: LocalDate, weeks: Double, locale: Locale): String {
    val day = today.plusDays((weeks * 7).roundToInt().toLong())
    val part = when {
        day.dayOfMonth <= 10 -> "Early"
        day.dayOfMonth <= 20 -> "Mid"
        else -> "Late"
    }
    val month = day.month.getDisplayName(TextStyle.FULL, locale)
    return if (day.year == today.year) "$part $month" else "$part $month ${day.year}"
}

/** One bar of the pace chart: the best top set by a fortnight's end, or where your pace puts it. */
internal data class PaceBar(val valueLb: Double, val projected: Boolean, val label: String)

private const val FORTNIGHT_MS = 14L * 86_400_000L

/**
 * Six fortnights of your best top set (done, red) and up to three more at [perWeekLb] (your pace,
 * grey) until the target. Fortnights before your first logged set are left out rather than drawn
 * as zero, so a new lift does not read as a cliff.
 */
internal fun paceBars(
    topSets: List<GoalRepository.TopSet>,
    nowMs: Long,
    targetLb: Double,
    perWeekLb: Double,
    locale: Locale,
    zone: ZoneId = ZoneId.systemDefault(),
): List<PaceBar> {
    fun monthOf(ms: Long) = Instant.ofEpochMilli(ms).atZone(zone).month
    fun monthLabel(ms: Long) = monthOf(ms).getDisplayName(TextStyle.SHORT, locale)
    val done = (5 downTo 0).mapNotNull { back ->
        val end = nowMs - back * FORTNIGHT_MS
        val best = topSets.filter { it.atMs <= end }.maxOfOrNull { it.weightLb } ?: return@mapNotNull null
        end to best
    }
    if (done.isEmpty()) return emptyList()
    val bars = mutableListOf<Pair<Long, PaceBar>>()
    var lastMonth: java.time.Month? = null
    done.forEachIndexed { i, (end, best) ->
        val isNow = i == done.lastIndex
        val m = monthOf(end)
        val label = when {
            isNow -> "Now"
            m != lastMonth -> monthLabel(end)
            else -> ""
        }
        lastMonth = m
        bars += end to PaceBar(best, projected = false, label = label)
    }
    val current = done.last().second
    if (perWeekLb > 0 && current < targetLb) {
        for (k in 1..3) {
            val end = nowMs + k * FORTNIGHT_MS
            val v = minOf(targetLb, current + perWeekLb * 2 * k)
            val m = monthOf(end)
            bars += end to PaceBar(v, projected = true, label = if (m != lastMonth) monthLabel(end) else "")
            lastMonth = m
            if (v >= targetLb) break
        }
    }
    return bars.map { it.second }
}

/** A lift's pace in lb a week: its best now against its best 30 days ago, never negative. */
internal fun liftPerWeek(topSets: List<GoalRepository.TopSet>, nowMs: Long): Double {
    val nowBest = topSets.maxOfOrNull { it.weightLb } ?: return 0.0
    val thenBest = topSets.filter { it.atMs <= nowMs - 30 * 86_400_000L }.maxOfOrNull { it.weightLb } ?: return 0.0
    return ((nowBest - thenBest) / 30.0 * 7.0).coerceAtLeast(0.0)
}
