package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.core.time.mondayStartMs
import com.forge.app.core.time.monthStartMs
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.ExtendedGoalDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.ExtendedGoal
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.goal.GoalProgressMath
import com.forge.app.domain.goal.encodeGoalType
import com.forge.app.domain.goal.parseGoalType
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Custom goals (#137): wires the `extended_goal` table to the data the app already logs so each goal
 * auto-updates. Progress is computed live on read (two-to-three cheap aggregate queries per goal),
 * never stored — the number always reflects the latest logged cardio / bodyweight / sessions.
 *
 * The per-exercise weight-target system ([GoalRepository]) is separate and untouched.
 */
@Singleton
class ExtendedGoalRepository @Inject constructor(
    private val dao: ExtendedGoalDao,
    private val sessionDao: SessionDao,
    private val cardioDao: CardioDao,
    private val bodyweightDao: BodyweightDao,
    private val clock: Clock,
) {
    /** One custom goal joined with its live progress. Values are in each metric's canonical unit. */
    data class Progress(
        val id: Long,
        val metric: GoalMetric,
        val period: GoalPeriod,
        /** km / minutes / sessions / lb-volume / lb-bodyweight. */
        val targetValue: Double,
        val currentValue: Double,
        val label: String,
        val fraction: Float,
        val achieved: Boolean,
        val createdAt: Long,
    )

    /**
     * Create a custom goal. [targetValue] is in the metric's canonical unit (km / minutes / sessions /
     * lb). Bodyweight snapshots the current weigh-in as its progress baseline (stored in the spare
     * `stretch_value` column) so a cut like "get to 80 kg" shows correct progress from where you were.
     */
    suspend fun create(metric: GoalMetric, period: GoalPeriod, targetValue: Double, label: String = "") {
        val baseline = if (metric == GoalMetric.BODYWEIGHT) bodyweightDao.latest()?.weightLb else null
        dao.insert(
            ExtendedGoal(
                goalType = encodeGoalType(metric, period),
                targetValue = targetValue,
                stretchValue = baseline,
                label = label.trim(),
                createdAt = clock.nowMs(),
            )
        )
    }

    suspend fun updateTarget(id: Long, targetValue: Double) = dao.updateTarget(id, targetValue)

    suspend fun delete(id: Long) = dao.deleteById(id)

    /** Every custom goal with its live progress, achieved-first then closest-first. */
    suspend fun goalsWithProgress(): List<Progress> {
        val goals = dao.getAll()
        if (goals.isEmpty()) return emptyList()
        val now = clock.nowMs()
        return goals.mapNotNull { g ->
            val parsed = parseGoalType(g.goalType) ?: return@mapNotNull null
            val current = currentValue(parsed.metric, parsed.period, g, now)
            val fraction: Float
            val achieved: Boolean
            if (parsed.metric == GoalMetric.BODYWEIGHT) {
                val baseline = g.stretchValue ?: current
                fraction = GoalProgressMath.bodyweightFraction(baseline, current, g.targetValue)
                achieved = GoalProgressMath.bodyweightAchieved(baseline, current, g.targetValue)
            } else {
                fraction = GoalProgressMath.cumulativeFraction(current, g.targetValue)
                achieved = GoalProgressMath.cumulativeAchieved(current, g.targetValue)
            }
            Progress(
                id = g.id,
                metric = parsed.metric,
                period = parsed.period,
                targetValue = g.targetValue,
                currentValue = current,
                label = g.label,
                fraction = fraction,
                achieved = achieved,
                createdAt = g.createdAt,
            )
        }.sortedWith(compareByDescending<Progress> { it.achieved }.thenByDescending { it.fraction })
    }

    /** The current tracked value for one goal, in its canonical unit. */
    private suspend fun currentValue(
        metric: GoalMetric,
        period: GoalPeriod,
        g: ExtendedGoal,
        now: Long,
    ): Double {
        val windowStart = when (period) {
            GoalPeriod.WEEK -> mondayStartMs(now)
            GoalPeriod.MONTH -> monthStartMs(now)
            GoalPeriod.ALL -> 0L
        }
        return when (metric) {
            GoalMetric.CARDIO_DISTANCE ->
                if (period == GoalPeriod.ALL) cardioDao.totalDistanceKm() ?: 0.0
                else cardioDao.since(windowStart).filter { it.type != REST }.sumOf { it.distanceKm ?: 0.0 }

            GoalMetric.CARDIO_MINUTES ->
                if (period == GoalPeriod.ALL) (cardioDao.totalMinutes() ?: 0).toDouble()
                else cardioDao.since(windowStart).filter { it.type != REST }.sumOf { it.durationMin.toDouble() }

            GoalMetric.SESSIONS ->
                sessionDao.aggregateInRange(windowStart, now).sessionCount.toDouble()

            GoalMetric.VOLUME ->
                sessionDao.aggregateInRange(windowStart, now).totalVolume ?: 0.0

            GoalMetric.BODYWEIGHT ->
                bodyweightDao.latest()?.weightLb ?: (g.stretchValue ?: 0.0)
        }
    }

    private companion object {
        /** Rest-day cardio rows don't count toward distance/time goals (mirrors CardioDao). */
        const val REST = "rest"
    }
}
