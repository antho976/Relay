package com.forge.app.domain.schedule

import com.forge.app.program.DayPlan
import com.forge.app.program.MuscleGroup

/**
 * One week, three kinds of day: lift, cardio or rest, laid out by the app.
 *
 * Lifting and cardio used to be planned in two places that never met. The weekly schedule was seven
 * slots that held a lifting day or nothing, and cardio lived in its own tab with its own target, so
 * the lifter decided every week when to do it, how much and how hard, and what an empty slot was for.
 * This puts them in one week. The lifter gives two numbers (lifting days and weekly cardio minutes)
 * and the rules below decide the rest.
 *
 * The rules, in order:
 *  1. **Lifting days stay where they are.** The schedule's weekdays are the lifter's own and are
 *     never moved.
 *  2. **One day stays fully off.** However much cardio is asked for, it is never laid over the last
 *     free day.
 *  3. **Cardio goes on the free days that are left,** spread apart rather than bunched.
 *  4. **One hard session a week at most,** never the day before a lower-body lifting day, and only
 *     once there is a base of easy minutes to stand on ([Inputs.allowHard]).
 *  5. **What does not fit becomes a short easy finisher** after a lifting day that is not a
 *     lower-body day. Never after legs.
 *
 * Only a fixed-weekday schedule can be laid out: in sequence mode the app cannot see the week ahead,
 * so the caller answers day by day instead and never asks this for a layout.
 *
 * Pure. The caller knows the calendar, the program and what was logged; this decides the shape.
 */
object WeekPlan {

    enum class Kind { LIFT, CARDIO, REST }

    /**
     * One weekday of the plan.
     *
     * @param weekday 0 = Monday … 6 = Sunday, the schedule's own order.
     * @param dayKey the program day, on a [Kind.LIFT] day.
     * @param cardioMinutes the cardio planned for the day: the whole session on a [Kind.CARDIO] day,
     *   the easy finisher on a lifting day, 0 for none.
     * @param hard the week's one interval session.
     */
    data class Day(
        val weekday: Int,
        val kind: Kind,
        val dayKey: String? = null,
        val cardioMinutes: Int = 0,
        val hard: Boolean = false
    )

    /**
     * @param schedule the stored weekly schedule: [WeeklySchedule.SLOTS] entries, "" = no lifting.
     * @param validKeys the program's day keys; a slot naming anything else is not a lifting day.
     * @param lowerBodyKeys the program days that train the legs hard ([isLowerBody]).
     * @param targetMinutes the week's cardio minutes; 0 or less plans none.
     * @param allowHard whether the week may hold an interval session at all.
     */
    data class Inputs(
        val schedule: List<String>,
        val validKeys: Set<String>,
        val lowerBodyKeys: Set<String>,
        val targetMinutes: Int,
        val allowHard: Boolean
    )

    /** A session this long is the usual one; the count of cardio days is the target over this. */
    const val TYPICAL_SESSION = 35

    /** Below this a session is not worth planning; above [MAX_SESSION] it stops being "easy". */
    const val MIN_SESSION = 15
    const val MAX_SESSION = 60

    /** Never more cardio days than this, whatever the target. */
    const val MAX_CARDIO_DAYS = 4

    /** An interval session's whole length, warm-up and cool-down included. */
    const val HARD_SESSION = 29

    /** An easy finisher after lifting: long enough to count, short enough not to cost the session. */
    const val MIN_FINISHER = 10
    const val MAX_FINISHER = 15

    private const val DAYS = WeeklySchedule.SLOTS

    /** The week's seven days, Monday first. */
    fun layout(inputs: Inputs): List<Day> {
        val schedule = WeeklySchedule.parse(WeeklySchedule.encode(inputs.schedule))
        val liftKey = (0 until DAYS).associateWith { schedule[it].takeIf { key -> key in inputs.validKeys } }
        val lift = (0 until DAYS).filter { liftKey[it] != null }
        val free = (0 until DAYS).filter { liftKey[it] == null }
        val lower = lift.filter { liftKey[it] in inputs.lowerBodyKeys }.toSet()

        val target = inputs.targetMinutes
        val wanted = if (target < MIN_SESSION) 0 else ((target + TYPICAL_SESSION - 1) / TYPICAL_SESSION).coerceIn(1, MAX_CARDIO_DAYS)
        // Rule 2: the last free day is never taken.
        val count = minOf(wanted, (free.size - 1).coerceAtLeast(0))
        val cardioDays = spread(free, count, lower)

        // Rule 4: the hard day is one whose NEXT day is not a lower-body lift; of those, one whose
        // previous day is not either, so it is not run on yesterday's legs.
        val hardDay = if (!inputs.allowHard) null else cardioDays
            .filter { (it + 1) % DAYS !in lower }
            .minWithOrNull(compareBy({ if ((it + DAYS - 1) % DAYS in lower) 1 else 0 }, { it }))

        val perSession = if (wanted == 0) 0 else (target / wanted).coerceIn(MIN_SESSION, MAX_SESSION)
        val minutes = cardioDays.associateWith { if (it == hardDay) HARD_SESSION else perSession }

        // Rule 5: what the cardio days could not hold, as finishers on the lifting days that spare the legs.
        var left = target - minutes.values.sum()
        val finishers = mutableMapOf<Int, Int>()
        for (day in lift.filter { it !in lower }) {
            if (left < MIN_FINISHER) break
            val m = minOf(left, MAX_FINISHER)
            finishers[day] = m
            left -= m
        }

        return (0 until DAYS).map { d ->
            when {
                liftKey[d] != null -> Day(d, Kind.LIFT, dayKey = liftKey[d], cardioMinutes = finishers[d] ?: 0)
                d in minutes -> Day(d, Kind.CARDIO, cardioMinutes = minutes.getValue(d), hard = d == hardDay)
                else -> Day(d, Kind.REST)
            }
        }
    }

    /**
     * [count] of the [free] days, as far apart as the week allows. Of the equally spread choices it
     * takes the one that leaves a rest day right after a lower-body day, where rest does most, and
     * then the earliest, so the same inputs always give the same week.
     */
    private fun spread(free: List<Int>, count: Int, lower: Set<Int>): List<Int> {
        if (count <= 0 || free.isEmpty()) return emptyList()
        val best = combinations(free, count).maxWithOrNull(
            compareBy<List<Int>>(
                { minGap(it) },
                { chosen -> free.count { it !in chosen && (it + DAYS - 1) % DAYS in lower } },
                { chosen -> -chosen.sum() }
            )
        )
        return best.orEmpty()
    }

    /** The smallest gap, around the week, between any two chosen days; a lone day has the whole week. */
    private fun minGap(days: List<Int>): Int {
        if (days.size < 2) return DAYS
        val sorted = days.sorted()
        return sorted.indices.minOf { i ->
            val next = sorted[(i + 1) % sorted.size]
            (next - sorted[i] + DAYS) % DAYS
        }
    }

    private fun combinations(items: List<Int>, k: Int): List<List<Int>> {
        if (k == 0) return listOf(emptyList())
        if (items.size < k) return emptyList()
        val head = items.first()
        val rest = items.drop(1)
        return combinations(rest, k - 1).map { listOf(head) + it } + combinations(rest, k)
    }

    /**
     * What today's cardio comes to, once the week so far is counted.
     *
     * A cardio day asks for its own minutes plus its share of what the days before it planned and
     * did not get, so a missed day rolls into the ones left instead of being lost. A week already at
     * its target asks for nothing, and nothing is ever asked for past the target.
     *
     * @param doneMinutes cardio logged since Monday.
     * @param doneToday cardio already logged today: the day has had its session.
     * @return the minutes to do today, or 0 for none.
     */
    fun minutesToday(week: List<Day>, todayIndex: Int, targetMinutes: Int, doneMinutes: Int, doneToday: Boolean): Int {
        val today = week.getOrNull(todayIndex) ?: return 0
        if (doneToday || today.cardioMinutes <= 0) return 0
        val owed = targetMinutes - doneMinutes
        return when (today.kind) {
            Kind.LIFT -> if (owed >= MIN_FINISHER) minOf(today.cardioMinutes, owed) else 0
            Kind.CARDIO -> {
                if (owed < MIN_SESSION) return 0
                // The hard session is a fixed piece of work, not a share of the minutes.
                if (today.hard) return HARD_SESSION
                val plannedBefore = week.filter { it.weekday < todayIndex }.sumOf { it.cardioMinutes }
                val missed = (plannedBefore - doneMinutes).coerceAtLeast(0)
                val daysLeft = week.count { it.weekday >= todayIndex && it.kind == Kind.CARDIO }.coerceAtLeast(1)
                minOf(today.cardioMinutes + missed / daysLeft, owed).coerceIn(MIN_SESSION, MAX_SESSION)
            }
            Kind.REST -> 0
        }
    }

    /**
     * Whether a program day trains the legs hard enough that cardio should keep clear of it: two or
     * more movements whose prime mover is the quads, hamstrings or glutes. A full-body day counts; a
     * single calf or leg-curl accessory on an upper day does not.
     */
    fun isLowerBody(day: DayPlan): Boolean =
        day.exercises.count { it.muscle in LEG_MUSCLES } >= 2

    private val LEG_MUSCLES = setOf(MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES)
}
