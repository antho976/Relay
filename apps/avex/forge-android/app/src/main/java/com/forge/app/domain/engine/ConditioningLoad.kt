package com.forge.app.domain.engine

import com.forge.app.data.db.entities.CardioEntry
import kotlin.math.roundToInt

/**
 * TRIMP-lite: what a piece of cardio actually cost (Engine E-A).
 *
 * This is the number that finally makes `effort` and `hrZone` consumed data — both were logged
 * since v1 and read by nothing. Duration alone can't distinguish a walk from intervals, so load is
 * duration weighted by intensity, taking the best evidence available:
 *   1. a measured HR zone, 2. the manually picked zone, 3. the effort word as a proxy.
 *
 * **This is the one interference formula in the product.** Readiness consumes THIS function rather
 * than reimplementing effort × zone × minutes on the coach side — two formulas would disagree, and
 * the disagreement would show up as a coach that contradicts its own cardio hub.
 */
object ConditioningLoad {

    private const val DAY_MS = 24L * 60 * 60 * 1000
    private const val WEEK_MS = 7 * DAY_MS

    /** Intensity weight per zone. Zone 5 costs roughly three times what zone 1 does, per minute. */
    fun zoneWeight(zone: Int): Double = when (zone) {
        1 -> 1.0
        2 -> 1.3
        3 -> 1.8
        4 -> 2.4
        5 -> 3.0
        else -> 1.3
    }

    /**
     * One entry's load, or 0 for a rest day. Rest rows carry a reason, not a workout, and counting
     * them would make "I rested because I was sore" read as training stress.
     */
    fun of(entry: CardioEntry): Double {
        if (entry.restReason != null) return 0.0
        val minutes = entry.durationMin.coerceAtLeast(0)
        if (minutes == 0) return 0.0
        return minutes * zoneWeight(zoneOf(entry))
    }

    /**
     * The zone to price an entry at, best evidence first: the logged HR zone, else the effort word
     * as a proxy, else a moderate default so an entry with neither still counts as something.
     */
    fun zoneOf(entry: CardioEntry): Int =
        entry.hrZone?.toIntOrNull()?.takeIf { it in 1..5 }
            ?: EffortZones.zoneForEffort(entry.effort)
            ?: 2

    /** Total load across the last 7 days — the acute conditioning cost the coach reasons about. */
    fun weekly(entries: List<CardioEntry>, nowMs: Long): Double =
        entries.filter { it.date >= nowMs - WEEK_MS }.sumOf { of(it) }

    /**
     * Ramp rate: this week's load against the average of the prior three, as a fraction. Above
     * ~1.1 means conditioning is climbing faster than the usual guidance, which is the number the
     * planner caps against.
     */
    fun rampRate(entries: List<CardioEntry>, nowMs: Long): Double? {
        val thisWeek = weekly(entries, nowMs)
        val prior = (1..3).map { w ->
            val end = nowMs - w * WEEK_MS
            entries.filter { it.date in (end - WEEK_MS) until end }.sumOf { of(it) }
        }.filter { it > 0 }
        if (prior.isEmpty()) return null
        val baseline = prior.average()
        if (baseline <= 0) return null
        return thisWeek / baseline
    }

    /**
     * The interference term readiness subtracts (Engine E-A → Coach B1). Bounded on purpose: cardio
     * competes with lifting recovery, but it is not a reason for the coach to shut a session down.
     */
    fun interferencePenalty(entries: List<CardioEntry>, nowMs: Long): Int {
        val yesterday = entries.filter { it.date >= nowMs - DAY_MS }.sumOf { of(it) }
        return when {
            yesterday >= HARD_DAY_LOAD -> 2
            yesterday >= MODERATE_DAY_LOAD -> 1
            else -> 0
        }
    }

    /** Load in a day past which yesterday's cardio meaningfully costs today's lifting. */
    const val MODERATE_DAY_LOAD = 60.0
    const val HARD_DAY_LOAD = 110.0

    /** A plain-language summary of the week, for the cardio hub. */
    fun describeWeek(entries: List<CardioEntry>, nowMs: Long): String {
        val load = weekly(entries, nowMs)
        if (load <= 0) return "No conditioning logged this week."
        val minutes = entries.filter { it.date >= nowMs - WEEK_MS && it.restReason == null }
            .sumOf { it.durationMin }
        return "$minutes minutes this week, load ${load.roundToInt()}."
    }
}
