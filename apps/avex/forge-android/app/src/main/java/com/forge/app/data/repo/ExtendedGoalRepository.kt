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
    /** Raw goals stream — a cheap change signal (add/edit/delete) callers recompute progress off. */
    fun observeAll(): kotlinx.coroutines.flow.Flow<List<ExtendedGoal>> = dao.observeAll()

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
        // Repository-level guards so no entry point can store a degenerate goal: a non-positive
        // target breaks the fraction math, and BODYWEIGHT is point-in-time — a WEEK/MONTH window is
        // meaningless for it (currentValue ignores the period), so it normalizes to ALL.
        if (targetValue <= 0) return
        val normalizedPeriod = if (metric == GoalMetric.BODYWEIGHT) GoalPeriod.ALL else period
        val baseline = if (metric == GoalMetric.BODYWEIGHT) bodyweightDao.latest()?.weightLb else null
        dao.insert(
            ExtendedGoal(
                goalType = encodeGoalType(metric, normalizedPeriod),
                targetValue = targetValue,
                stretchValue = baseline,
                label = label.trim(),
                createdAt = clock.nowMs(),
            )
        )
    }

    /** Guarded like [create]: a non-positive target is ignored rather than stored. */
    suspend fun updateTarget(id: Long, targetValue: Double) {
        if (targetValue <= 0) return
        dao.updateTarget(id, targetValue)
    }

    /** Delete a custom goal, returning the removed row so a caller can offer an Undo (§13). */
    suspend fun delete(id: Long): ExtendedGoal? {
        val removed = dao.getById(id)
        dao.deleteById(id)
        return removed
    }

    /** Re-insert a previously deleted goal — its original id is preserved (insert REPLACEs by id), so
     *  live progress reads resume exactly, baseline (stretch_value) and all. */
    suspend fun restore(goal: ExtendedGoal) { dao.insert(goal) }

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
            // between() bounds the window to [windowStart, now) and drops rest-day rows, so a
            // future-dated entry can't inflate a this-week/month total (which the sessions path,
            // aggregateInRange(windowStart, now), already guards against).
            GoalMetric.CARDIO_DISTANCE ->
                if (period == GoalPeriod.ALL) cardioDao.totalDistanceKm() ?: 0.0
                else cardioDao.between(windowStart, now).sumOf { it.distanceKm ?: 0.0 }

            GoalMetric.CARDIO_MINUTES ->
                if (period == GoalPeriod.ALL) (cardioDao.totalMinutes() ?: 0).toDouble()
                else cardioDao.between(windowStart, now).sumOf { it.durationMin.toDouble() }

            GoalMetric.SESSIONS ->
                sessionDao.aggregateInRange(windowStart, now).sessionCount.toDouble()

            GoalMetric.VOLUME ->
                sessionDao.aggregateInRange(windowStart, now).totalVolume ?: 0.0

            GoalMetric.BODYWEIGHT ->
                bodyweightDao.latest()?.weightLb ?: (g.stretchValue ?: 0.0)
        }
    }
}
