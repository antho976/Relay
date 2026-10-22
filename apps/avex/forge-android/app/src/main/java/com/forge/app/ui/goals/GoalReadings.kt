package com.forge.app.ui.goals

import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.goal.customPinKey
import com.forge.app.domain.goal.liftPinKey
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.trimDecimal
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

/*
 * The Goals page's readings (2026-10-21 redesign, "pg-goals gl"): each goal as a reading with its
 * pace. Pure, so the arithmetic behind "~6 weeks" and "On plan" is unit-tested rather than eyeballed
 * in a screenshot.
 */

private const val DAY_MS = 86_400_000L

/** Which editor a goal opens. */
internal sealed interface GoalRef {
    data class Lift(val exerciseId: String) : GoalRef
    data class Custom(val id: Long) : GoalRef
}

/**
 * One goal as the page draws it: name, where it stands ([figure] + [denominator], the latter muted
 * at half size), the meter, and the pace column on the right: [paceTop] (when, at this pace) over
 * [paceBottom] (what moved lately).
 */
internal data class GoalLine(
    val ref: GoalRef,
    val pinKey: String,
    val title: String,
    val figure: String,
    val denominator: String,
    val fraction: Float,
    val paceTop: String,
    /** Ink when the pace is a real, near answer; muted when it is slow or there is none. */
    val paceTopMoved: Boolean,
    val paceBottom: String?,
    val paceBottomMoved: Boolean = false,
    val achieved: Boolean,
    /** The folded row's name once reached ("Squat 275 lb"). */
    val reachedTitle: String,
    /** When it was reached, or what it was ("Oct 16", "This week"). */
    val reachedWhen: String?,
    /** Newest-first order for the reached fold. */
    val reachedAtMs: Long,
)

/** Weeks to cover [remaining] at [perWeek], or null when the pace does not head there. */
internal fun etaWeeks(remaining: Double, perWeek: Double): Double? = when {
    remaining <= 0.0 -> 0.0
    perWeek <= 0.0 -> null
    else -> remaining / perWeek
}

/** "~6 weeks", "~1 week", "~4 mo". */
internal fun etaText(weeks: Double): String {
    val w = weeks.roundToInt().coerceAtLeast(1)
    return when {
        w <= 1 -> "~1 week"
        w <= 12 -> "~$w weeks"
        else -> "~${(weeks / 4.345).roundToInt().coerceAtLeast(3)} mo"
    }
}

/** A pace answer near enough to name plainly; past it the page says Slow. */
private const val NEAR_WEEKS = 12.0

/** A signed change as the page's change line: "↑ 10 in 30d", "↓ 1.4 in 30d", "Steady over 30d". */
internal fun changeLine(deltaDisplay: Double): String =
    if (abs(deltaDisplay) < 0.05) "Steady over 30d"
    else "${if (deltaDisplay < 0) "↓" else "↑"} ${trimDecimal(abs(deltaDisplay))} in 30d"

/**
 * Change over the last 30 days from weigh-ins (oldest first, lb): the latest against the one
 * closest to 30 days before it, at least 21 days back. Null with too short a history.
 */
internal fun weighInChange30d(weighIns: List<Pair<Long, Double>>): Double? {
    val latest = weighIns.maxByOrNull { it.first } ?: return null
    val target = latest.first - 30 * DAY_MS
    val then = weighIns.filter { it.first <= latest.first - 21 * DAY_MS }
        .minByOrNull { abs(it.first - target) } ?: return null
    return latest.second - then.second
}

/** How far into its window a WEEK/MONTH goal is today, 0..1, today counted as lived. */
internal fun periodElapsed(period: GoalPeriod, today: LocalDate): Float = when (period) {
    GoalPeriod.WEEK -> today.dayOfWeek.value / 7f
    GoalPeriod.MONTH -> today.dayOfMonth / today.lengthOfMonth().toFloat()
    GoalPeriod.ALL -> 1f
}

/** Whether a windowed goal is keeping up with the share of its window already gone. */
internal fun onPlan(current: Double, target: Double, elapsed: Float): Boolean =
    target <= 0.0 || current >= target * elapsed - 1e-9

/** A lift target's line. [change30d] is lb gained on its best set over 30 days, when known. */
internal fun liftLine(
    g: GoalRepository.GoalProgress,
    change30d: Double?,
    reachedAtMs: Long?,
    unit: WeightUnit,
    locale: Locale,
    zone: ZoneId = ZoneId.systemDefault(),
): GoalLine {
    val remainingLb = g.targetLb - g.currentBestLb
    val perWeekLb = (change30d ?: 0.0) / 30.0 * 7.0
    val eta = etaWeeks(remainingLb, perWeekLb)
    val (top, moved) = paceTop(eta, remainingDisplay = toDisplayWeight(remainingLb, unit), unitWord = unitLabel(unit))
    return GoalLine(
        ref = GoalRef.Lift(g.exerciseId),
        pinKey = liftPinKey(g.exerciseId),
        title = g.name,
        figure = weightInputValue(g.currentBestLb, unit),
        denominator = " / ${weightInputValue(g.targetLb, unit)} ${unitLabel(unit)}",
        fraction = g.fraction,
        paceTop = top,
        paceTopMoved = moved,
        paceBottom = change30d?.let { changeLine(toDisplayWeight(it, unit)) },
        achieved = g.achieved,
        reachedTitle = "${g.name} ${formatWeight(g.targetLb, unit)}",
        reachedWhen = reachedAtMs?.let { shortDate(it, locale, zone) },
        reachedAtMs = reachedAtMs ?: g.createdAt,
    )
}

/** The top pace line: a near ETA in ink, a far one as Slow, none as what is left. */
private fun paceTop(eta: Double?, remainingDisplay: Double, unitWord: String): Pair<String, Boolean> = when {
    eta == null -> "${trimDecimal(remainingDisplay)} $unitWord to go" to false
    eta <= NEAR_WEEKS -> etaText(eta) to true
    else -> "Slow · ${etaText(eta)}" to false
}

/** A custom goal's line. [weighIns] (oldest first, lb) feed a bodyweight goal's pace. */
internal fun customLine(
    g: ExtendedGoalRepository.Progress,
    weighIns: List<Pair<Long, Double>>,
    unit: WeightUnit,
    useMiles: Boolean,
    today: LocalDate,
): GoalLine {
    val title = customGoalTitle(g)
    val (figure, denominator) = customFigure(g.metric, g.currentValue, g.targetValue, unit, useMiles)
    val base = GoalLine(
        ref = GoalRef.Custom(g.id),
        pinKey = customPinKey(g.id),
        title = title,
        figure = figure,
        denominator = denominator,
        fraction = g.fraction,
        paceTop = "",
        paceTopMoved = false,
        paceBottom = null,
        achieved = g.achieved,
        reachedTitle = title,
        reachedWhen = when {
            g.metric == GoalMetric.BODYWEIGHT -> customTargetText(g.metric, g.targetValue, unit, useMiles)
            g.period == GoalPeriod.WEEK -> "This week"
            g.period == GoalPeriod.MONTH -> "This month"
            else -> customTargetText(g.metric, g.targetValue, unit, useMiles)
        },
        reachedAtMs = g.createdAt,
    )
    return when {
        g.metric == GoalMetric.BODYWEIGHT -> {
            val change = weighInChange30d(weighIns)
            val from = g.baselineValue ?: g.currentValue
            val losing = g.targetValue < from
            val remaining = abs(g.targetValue - g.currentValue)
            val towardPerWeek = (change ?: 0.0).let { if (losing) -it else it } / 30.0 * 7.0
            val eta = etaWeeks(remaining, towardPerWeek)
            val (top, moved) = paceTop(eta, toDisplayWeight(remaining, unit), unitLabel(unit))
            base.copy(
                paceTop = top,
                paceTopMoved = moved,
                paceBottom = change?.let { changeLine(toDisplayWeight(it, unit)) },
            )
        }
        g.period == GoalPeriod.ALL -> {
            val left = (g.targetValue - g.currentValue).coerceAtLeast(0.0)
            base.copy(paceTop = "${customTargetText(g.metric, left, unit, useMiles)} to go")
        }
        else -> {
            val days = periodDaysLeftOn(g.period, today) ?: 0
            val keeping = onPlan(g.currentValue, g.targetValue, periodElapsed(g.period, today))
            base.copy(
                paceTop = if (days <= 1) "Last day" else "$days days left",
                paceTopMoved = true,
                paceBottom = if (keeping) "On plan" else "Behind",
                paceBottomMoved = !keeping,
            )
        }
    }
}

/** [periodDaysLeft] for a date rather than an instant. */
private fun periodDaysLeftOn(period: GoalPeriod, today: LocalDate): Int? {
    val end = when (period) {
        GoalPeriod.WEEK -> today.with(java.time.temporal.TemporalAdjusters.nextOrSame(java.time.DayOfWeek.SUNDAY))
        GoalPeriod.MONTH -> today.withDayOfMonth(today.lengthOfMonth())
        GoalPeriod.ALL -> return null
    }
    return ChronoUnit.DAYS.between(today, end).toInt() + 1
}

/** The figure and its muted denominator for a custom metric ("42.5k", " / 60k lb"). */
internal fun customFigure(metric: GoalMetric, current: Double, target: Double, unit: WeightUnit, useMiles: Boolean): Pair<String, String> =
    when (metric) {
        GoalMetric.CARDIO_DISTANCE ->
            distanceInputValue(current, useMiles) to " / ${distanceInputValue(target, useMiles)} ${distanceUnitLabel(useMiles)}"
        GoalMetric.CARDIO_MINUTES -> "${current.toInt()}" to " / ${target.toInt()} min"
        GoalMetric.SESSIONS -> "${current.toInt()}" to " / ${target.toInt()} ${if (target.toInt() == 1) "workout" else "workouts"}"
        GoalMetric.VOLUME ->
            formatVolumeCompact(current, unit, withUnit = false) to " / ${formatVolumeCompact(target, unit)}"
        GoalMetric.BODYWEIGHT -> weightInputValue(current, unit) to " / ${weightInputValue(target, unit)} ${unitLabel(unit)}"
    }

/** A custom value with its unit ("10 mi", "150 min", "5 workouts", "60k lb"). */
internal fun customTargetText(metric: GoalMetric, value: Double, unit: WeightUnit, useMiles: Boolean): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> "${distanceInputValue(value, useMiles)} ${distanceUnitLabel(useMiles)}"
    GoalMetric.CARDIO_MINUTES -> "${value.roundToInt()} min"
    GoalMetric.SESSIONS -> "${value.roundToInt()} ${if (value.roundToInt() == 1) "workout" else "workouts"}"
    GoalMetric.VOLUME -> formatVolumeCompact(value, unit)
    GoalMetric.BODYWEIGHT -> formatWeight(value, unit)
}

/** The first session whose heaviest set met [targetLb], or null when none has. */
internal fun firstReachedAt(topSets: List<GoalRepository.TopSet>, targetLb: Double): Long? =
    topSets.sortedBy { it.atMs }.firstOrNull { it.weightLb >= targetLb - 1e-6 }?.atMs

/** "Oct 16". */
internal fun shortDate(ms: Long, locale: Locale, zone: ZoneId = ZoneId.systemDefault()): String =
    Instant.ofEpochMilli(ms).atZone(zone).format(DateTimeFormatter.ofPattern("MMM d", locale))

/** The page's mono subtitle: "4 active · 3 pinned to Home · 2 reached", honest at zero. */
internal fun goalsMetaLine(active: Int, pinned: Int, reached: Int): String = buildList {
    add("$active active")
    if (pinned > 0) add("$pinned pinned to Home")
    if (reached > 0) add("$reached reached")
}.joinToString(" · ")
