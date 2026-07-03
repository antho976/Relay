package com.forge.app.domain.goal

/**
 * Custom goals (#137). A custom goal is assembled from parameters — a [GoalMetric] to measure, a
 * numeric target, and (for cumulative metrics) a [GoalPeriod] window — and then auto-updates from
 * data the app already logs. There is no free-text / manual check-off: every custom goal tracks a
 * real quantity, so progress moves on its own as you train.
 *
 * These live in the `extended_goal` table (goal_type = "<metric>:<period>"). Per-exercise weight
 * targets are a separate, older system in `exercise_goal` and are NOT [GoalMetric]s.
 */
enum class GoalMetric(val code: String) {
    /** Total cardio distance (canonical unit: km). */
    CARDIO_DISTANCE("cardio_distance"),
    /** Total cardio time (minutes). */
    CARDIO_MINUTES("cardio_minutes"),
    /** Number of finished gym sessions. */
    SESSIONS("sessions"),
    /** Training volume (canonical unit: lb). */
    VOLUME("volume"),
    /** A bodyweight target — reach a weight, up or down (canonical unit: lb). Uses a baseline. */
    BODYWEIGHT("bodyweight");

    /** Bodyweight is a level target tracked from a baseline; the rest accumulate over a [GoalPeriod]. */
    val isCumulative: Boolean get() = this != BODYWEIGHT

    companion object {
        fun fromCode(code: String): GoalMetric? = entries.firstOrNull { it.code == code }
    }
}

/**
 * The window a cumulative goal accumulates over. This is NOT a rolling / trailing window: [WEEK] and
 * [MONTH] measure the CURRENT calendar week/month and reset at its boundary, while [ALL] is the
 * lifetime running total. Ignored for [GoalMetric.BODYWEIGHT].
 */
enum class GoalPeriod(val code: String) {
    WEEK("week"), MONTH("month"), ALL("all");

    companion object {
        fun fromCode(code: String): GoalPeriod? = entries.firstOrNull { it.code == code }
    }
}

/** The parsed pieces of an [encodeGoalType] string. */
data class ParsedGoalType(val metric: GoalMetric, val period: GoalPeriod)

/** The `goal_type` value stored in `extended_goal`: "<metric>:<period>". */
fun encodeGoalType(metric: GoalMetric, period: GoalPeriod): String = "${metric.code}:${period.code}"

/** Inverse of [encodeGoalType]; null if the metric is unknown. Missing period defaults to [GoalPeriod.ALL]. */
fun parseGoalType(raw: String): ParsedGoalType? {
    val parts = raw.split(":")
    val metric = GoalMetric.fromCode(parts.getOrNull(0)?.trim() ?: return null) ?: return null
    val period = GoalPeriod.fromCode(parts.getOrNull(1)?.trim() ?: "") ?: GoalPeriod.ALL
    return ParsedGoalType(metric, period)
}

/**
 * Pure progress math for custom goals — kept free of Room so it is unit-testable. All values are in
 * each metric's canonical unit (km / minutes / sessions / lb).
 */
object GoalProgressMath {

    /** 0f–1f toward a cumulative target (distance / minutes / sessions / volume). */
    fun cumulativeFraction(current: Double, target: Double): Float =
        if (target <= 0) 0f else (current / target).toFloat().coerceIn(0f, 1f)

    fun cumulativeAchieved(current: Double, target: Double): Boolean =
        target > 0 && current >= target

    /**
     * Direction-aware bodyweight progress. [baseline] is the weigh-in when the goal was set, [current]
     * the latest weigh-in, [target] the goal weight. Handles both a cut (target < baseline) and a bulk
     * (target > baseline): 0f while moving the wrong way, 1f once the target is met or passed.
     */
    fun bodyweightFraction(baseline: Double, current: Double, target: Double): Float {
        val span = target - baseline
        if (span == 0.0) return if (bodyweightAchieved(baseline, current, target)) 1f else 0f
        return ((current - baseline) / span).toFloat().coerceIn(0f, 1f)
    }

    fun bodyweightAchieved(baseline: Double, current: Double, target: Double): Boolean =
        if (target >= baseline) current >= target else current <= target
}
