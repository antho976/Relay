package com.forge.app.ui.gym.stats

import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.projections.SetWithExerciseAndSession
import com.forge.app.data.db.projections.SetWithExerciseId
import com.forge.app.data.repo.buildWeeklySetsByMuscle
import com.forge.app.domain.adapt.E1rm
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.BodyweightPoint
import com.forge.app.ui.gym.stats.state.DayLoad
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.TemporalAdjusters
import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.roundToInt

/*
 * The redesigned Stats page's reads (2026-10-21, Relay design 7 "Stats final"). One range per page
 * (4W / 12W / 6M / All) slices every section, and each figure is compared with the same span just
 * before it ("8 more than the 12 before"). Pure functions over the per-set history, so every range
 * comes from one read and each one is testable without a database.
 */

private const val DAY_MS = 24L * 60 * 60 * 1000

/** The page's one range. [days] null is All. */
enum class StatsRange(val label: String, val days: Int?) {
    W4("4W", 28), W12("12W", 84), M6("6M", 182), ALL("All", null);

    /** How the range reads in a sentence: "12 wk", "6 mo", "all time". */
    val span: String
        get() = when (this) {
            W4 -> "4 wk"; W12 -> "12 wk"; M6 -> "6 mo"; ALL -> "all time"
        }

    /** "the 12 before", for the comparison half of a change line. Null for All, which has no before. */
    val before: String?
        get() = when (this) {
            W4 -> "the 4 before"; W12 -> "the 12 before"; M6 -> "the 6 months before"; ALL -> null
        }
}

/** [start, end] of the range and the same span before it ([prevStart, start)); prevStart null for All. */
data class StatsWindow(val start: Long, val end: Long, val prevStart: Long?) {
    val length: Long get() = (end - start).coerceAtLeast(DAY_MS)
    operator fun contains(t: Long) = t in start..end
    fun inPrevious(t: Long) = prevStart != null && t >= prevStart && t < start
    /** Where [t] sits across the window, 0 at its start and 1 now. */
    fun fraction(t: Long): Float = ((t - start).toFloat() / length).coerceIn(0f, 1f)
    /** Where [t] in the previous span sits, lined up under the same point of the current one. */
    fun previousFraction(t: Long): Float = ((t - (prevStart ?: start)).toFloat() / length).coerceIn(0f, 1f)
}

/** The end of today in [zone]: a range runs to it, so a session logged today always counts. */
fun endOfToday(nowMs: Long, zone: ZoneId = ZoneId.systemDefault()): Long =
    Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate().plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli() - 1

fun statsWindow(range: StatsRange, nowMs: Long, earliestMs: Long?): StatsWindow {
    val days = range.days
    return if (days == null) {
        StatsWindow(start = (earliestMs ?: nowMs - 28 * DAY_MS).coerceAtMost(nowMs - DAY_MS), end = nowMs, prevStart = null)
    } else {
        val start = nowMs - days * DAY_MS
        StatsWindow(start = start, end = nowMs, prevStart = start - days * DAY_MS)
    }
}

/**
 * Month names under a graph, placed where each month starts, with "Now" at the right edge. The
 * first label is dropped when it would sit on top of the second.
 */
fun monthTicks(window: StatsWindow, zone: ZoneId = ZoneId.systemDefault()): List<Pair<Float, String>> {
    val startDate = Instant.ofEpochMilli(window.start).atZone(zone).toLocalDate()
    val endDate = Instant.ofEpochMilli(window.end).atZone(zone).toLocalDate()
    val spanDays = (endDate.toEpochDay() - startDate.toEpochDay()).coerceAtLeast(1)
    // A long range would print twelve crowded months: step so about four show.
    val stepMonths = when {
        spanDays > 600 -> 6
        spanDays > 300 -> 3
        spanDays > 150 -> 2
        else -> 1
    }
    val out = mutableListOf<Pair<Float, String>>()
    var m = startDate.withDayOfMonth(1).plusMonths(1)
    if (spanDays <= 40) m = startDate.withDayOfMonth(1).let { if (it < startDate) it.plusMonths(1) else it }
    while (m < endDate) {
        val f = (m.toEpochDay() - startDate.toEpochDay()).toFloat() / spanDays
        if (f < 0.88f) out += f to m.month.getDisplayName(java.time.format.TextStyle.SHORT, java.util.Locale.getDefault())
        m = m.plusMonths(stepMonths.toLong())
    }
    // A short range with no month start inside it still says where it begins.
    if (out.isEmpty()) out += 0f to startDate.format(java.time.format.DateTimeFormatter.ofPattern("MMM d"))
    out += 1f to "Now"
    return out
}

// ── Strength ───────────────────────────────────────────────────────────────────────────────────

/** A lift's best set of one session. [pr] marks a best e1RM no earlier session reached. */
data class LiftSession(val t: Long, val e1rm: Double, val weightLb: Double, val reps: Int, val pr: Boolean)

/** Every lift's sessions, best set each, oldest first, by exercise id. */
fun liftSessions(sets: List<SetWithExerciseAndSession>): Map<String, List<LiftSession>> =
    sets.asSequence()
        .filter { (it.weightLb ?: 0.0) > 0.0 }
        .groupBy { it.exerciseId }
        .mapValues { (_, ss) ->
            var best = 0.0
            ss.groupBy { it.sessionStartedAt }.toSortedMap().map { (t, rows) ->
                val top = rows.maxBy { E1rm.epley(it.weightLb!!, it.reps) }
                val e = E1rm.epley(top.weightLb!!, top.reps)
                val pr = e > best + 1e-6
                if (pr) best = e
                LiftSession(t, e, top.weightLb, top.reps, pr)
            }
        }

/** One lift read over a range. */
data class LiftTrend(
    val exerciseId: String,
    val name: String,
    /** The latest best e1RM. */
    val now: Double,
    /** Best e1RM at the end of the range minus at its start; null with fewer than two sessions in it. */
    val delta: Double?,
    /** The same change over the span before; null when there is none to compare. */
    val previousDelta: Double?,
    val current: List<LiftSession>,
    val previous: List<LiftSession>,
    /** Weeks since the e1RM last rose, when it is flat now. */
    val flatWeeks: Int?,
    val prInRange: Boolean,
)

fun liftTrend(id: String, sessions: List<LiftSession>, window: StatsWindow): LiftTrend? {
    if (sessions.isEmpty()) return null
    val cur = sessions.filter { it.t in window }
    val prev = sessions.filter { window.inPrevious(it.t) }
    // The start of the range is the last session BEFORE it when there is one, so a lift trained once
    // inside the range still has a change.
    val startValue = sessions.lastOrNull { it.t < window.start }?.e1rm ?: cur.firstOrNull()?.e1rm
    val delta = if (cur.isNotEmpty() && startValue != null && (cur.size >= 2 || sessions.any { it.t < window.start })) cur.last().e1rm - startValue else null
    val prevStart = window.prevStart?.let { ps -> sessions.lastOrNull { it.t < ps }?.e1rm ?: prev.firstOrNull()?.e1rm }
    val previousDelta = if (prev.isNotEmpty() && prevStart != null) prev.last().e1rm - prevStart else null
    val lastRise = sessions.zipWithNext().lastOrNull { (a, b) -> b.e1rm > a.e1rm * 1.005 }?.second?.t
    val flatWeeks = if (delta != null && abs(delta) < 1.0) {
        (((window.end - (lastRise ?: sessions.first().t)) / (7 * DAY_MS)).toInt()).coerceAtLeast(1)
    } else null
    return LiftTrend(
        exerciseId = id,
        name = Program.exerciseDisplayName(id, null),
        now = sessions.last().e1rm,
        delta = delta,
        previousDelta = previousDelta,
        current = cur,
        previous = prev,
        flatWeeks = flatWeeks,
        prInRange = cur.any { it.pr },
    )
}

/** "↑ 18 lb in 12 wk", "↓ 4 lb in 4 wk", "Flat 3 weeks", or null with nothing to compare. */
fun liftChangeLine(t: LiftTrend, range: StatsRange, fmt: (Double) -> String): String? {
    t.flatWeeks?.let { return "Flat $it ${if (it == 1) "week" else "weeks"}" }
    val d = t.delta ?: return null
    return "${if (d < 0) "↓" else "↑"} ${fmt(abs(d))} in ${range.span}"
}

/** The comparison half: "8 more than the 12 before", "4 less than the 12 before". */
fun versusBefore(delta: Double?, previous: Double?, range: StatsRange, fmtNumber: (Double) -> String): String? {
    val before = range.before ?: return null
    if (delta == null || previous == null) return null
    val diff = delta - previous
    if (abs(diff) < 0.5) return "same as $before"
    return "${fmtNumber(abs(diff))} ${if (diff > 0) "more" else "less"} than $before"
}

// ── Volume ─────────────────────────────────────────────────────────────────────────────────────

/** Average weekly working sets per muscle over the range, highest first. */
fun weeklySetsPerMuscle(sets: List<SetWithExerciseAndSession>, window: StatsWindow): List<Pair<MuscleGroup, Int>> {
    val rows = sets.filter { it.sessionStartedAt in window }
    // Weeks since training began inside the range, not the range's whole length: seven weeks of
    // history read against a 12-week range would put every muscle under.
    val from = maxOf(window.start, rows.minOfOrNull { it.sessionStartedAt } ?: window.start)
    val weeks = ((window.end - from) / (7.0 * DAY_MS)).coerceAtLeast(1.0)
    val inRange = rows.map { SetWithExerciseId(it.weightLb, it.reps, it.exerciseId) }
    return buildWeeklySetsByMuscle(inRange)
        .map { it.muscle to (it.sets / weeks).roundToInt() }
        .filter { it.second > 0 }
        .sortedByDescending { it.second }
}

/** One Monday-start week's volume. [current] is the week still going. */
data class WeekVolume(val weekStart: LocalDate, val volumeLb: Double, val deload: Boolean, val current: Boolean)

fun weeklyVolumes(
    sessions: List<SessionDao.SessionVolumeDeloadRow>,
    window: StatsWindow,
    zone: ZoneId = ZoneId.systemDefault(),
): List<WeekVolume> {
    fun weekOf(ms: Long) = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate().with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
    val first = weekOf(window.start)
    val thisWeek = weekOf(window.end)
    val byWeek = sessions.filter { it.startedAt in window }.groupBy { weekOf(it.startedAt) }
    val out = mutableListOf<WeekVolume>()
    var w = first
    while (w <= thisWeek) {
        val rows = byWeek[w].orEmpty()
        out += WeekVolume(w, rows.sumOf { it.totalVolumeLb ?: 0.0 }, rows.any { it.deloadMarkedHere }, w == thisWeek)
        w = w.plusWeeks(1)
    }
    return out
}

/** The average finished week over a span of sessions (weeks with nothing count as zero). */
fun averageWeek(sessions: List<SessionDao.SessionVolumeDeloadRow>, from: Long, until: Long): Double? {
    val weeks = ((until - from) / (7.0 * DAY_MS))
    if (weeks < 1) return null
    val total = sessions.filter { it.startedAt in from until until }.sumOf { it.totalVolumeLb ?: 0.0 }
    return total / weeks
}

// ── Effort ─────────────────────────────────────────────────────────────────────────────────────

/** Rated sets by reps left in reserve (10 − RPE): index 0 is "4+", then 3, 2, 1, 0. */
data class EffortRead(val buckets: List<Int>, val rated: Int) {
    /** The share of rated sets in the 1 to 3 reps-left zone, 0 to 100. */
    val inZonePct: Int? get() = if (rated == 0) null else ((buckets[1] + buckets[2] + buckets[3]) * 100.0 / rated).roundToInt()
}

fun effortRead(sets: List<SetWithExerciseAndSession>, from: Long, until: Long): EffortRead {
    val b = IntArray(5)
    var n = 0
    sets.forEach { s ->
        val rpe = s.rpe ?: return@forEach
        if (s.sessionStartedAt < from || s.sessionStartedAt > until) return@forEach
        val rir = (10.0 - rpe).roundToInt().coerceAtLeast(0)
        b[if (rir >= 4) 0 else 4 - rir]++
        n++
    }
    return EffortRead(b.toList(), n)
}

/** Fitness and fatigue per day, the Banister model as the old Fitness card drew it. */
data class Banister(val days: List<Long>, val fitness: List<Float>, val fatigue: List<Float>) {
    val fitnessNow: Float get() = fitness.lastOrNull() ?: 0f
    val formNow: Float get() = (fitness.lastOrNull() ?: 0f) - (fatigue.lastOrNull() ?: 0f)
}

/**
 * The model over [fromDay]..[today] (epoch days). Both averages start cold at the first training
 * day and are corrected for it (dividing by the weight seen so far), so a steady load reads as two
 * lines that meet rather than as fatigue far above fitness for everyone's first months.
 */
fun banister(load: List<DayLoad>, fromDay: Long, today: Long): Banister? {
    if (load.size < 5) return null
    val firstDay = load.minOf { it.epochDay }
    val byDay = load.associate { it.epochDay to it.volumeLb }
    val aFit = exp(-1.0 / 42.0)
    val aFat = exp(-1.0 / 7.0)
    var f = 0.0; var g = 0.0; var fw = 0.0; var gw = 0.0
    val days = ArrayList<Long>(); val fit = ArrayList<Float>(); val fat = ArrayList<Float>()
    val drawFrom = maxOf(fromDay, if (today - firstDay >= 21) firstDay + 7 else firstDay)
    for (day in firstDay..today) {
        val l = byDay[day] ?: 0.0
        f = f * aFit + (1 - aFit) * l; g = g * aFat + (1 - aFat) * l
        fw = fw * aFit + (1 - aFit); gw = gw * aFat + (1 - aFat)
        if (day >= drawFrom) {
            days += day; fit += (f / fw).toFloat(); fat += (g / gw).toFloat()
        }
    }
    return if (days.size < 2) null else Banister(days, fit, fat)
}

/** Today's read in one word, from form as a share of fitness. */
fun formWord(b: Banister): String {
    if (b.fitnessNow <= 0f) return "No load yet"
    val share = b.formNow / b.fitnessNow * 100f
    return when {
        share > 10f -> "Fresh"
        share >= -15f -> "Balanced"
        share >= -35f -> "Building"
        else -> "Deep fatigue"
    }
}

/** "Form −12% of fitness". */
fun formLine(b: Banister): String? {
    if (b.fitnessNow <= 0f) return null
    val pct = (b.formNow / b.fitnessNow * 100f).roundToInt()
    return "Form ${if (pct < 0) "−" else "+"}${abs(pct)}% of fitness"
}

// ── Body ───────────────────────────────────────────────────────────────────────────────────────

/** Body weight over the range: the latest, the change across it, and both spans' points. */
data class WeightTrend(
    val latestLb: Double,
    val deltaLb: Double?,
    val current: List<BodyweightPoint>,
    val previous: List<BodyweightPoint>,
)

fun weightTrend(points: List<BodyweightPoint>, window: StatsWindow): WeightTrend? {
    val latest = points.lastOrNull() ?: return null
    val cur = points.filter { it.recordedAt in window }
    val prev = points.filter { window.inPrevious(it.recordedAt) }
    val start = points.lastOrNull { it.recordedAt < window.start } ?: cur.firstOrNull()
    val delta = if (start != null && start !== latest) latest.weightLb - start.weightLb else null
    return WeightTrend(latest.weightLb, delta, cur, prev)
}
