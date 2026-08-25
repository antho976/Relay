package com.forge.app.data.repo

import com.forge.app.domain.adapt.E1rm
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.HistoryPoint
import com.forge.app.ui.gym.stats.state.OverloadSummary
import com.forge.app.ui.gym.stats.state.PatternAxis
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.PrRecency
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.RepMaxEntry
import com.forge.app.ui.gym.stats.state.RepMaxSet
import com.forge.app.ui.gym.stats.state.TimeToPrEntry
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import kotlin.math.roundToInt

// Pure strength / PR / e1RM aggregation helpers extracted from StatsRepository.
// These operate on already-loaded set projections — no DAO or DI dependencies — so
// they live as top-level functions the repository orchestrator calls.

internal fun buildPrEntries(
    rows: List<com.forge.app.data.db.projections.RecentPrRow>,
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<PrEntry> {
    // Match each PR row to its exact owning LoggedExercise via loggedExerciseId, so an
    // exercise logged twice in one session resolves to the right set (was matched on
    // session date + exercise id, which picked whichever instance weighed more).
    return rows.mapNotNull { row ->
        val candidateSets = allSets.filter { it.loggedExerciseId == row.loggedExerciseId }
        // No matching set in the tracked/non-skipped/unassisted population (e.g. the PR's sets were
        // filtered out) — drop the row rather than render a blank "— / 0 reps" entry.
        val prSet = candidateSets.maxByOrNull { it.weightLb ?: 0.0 } ?: return@mapNotNull null
        val weightLb = prSet.weightLb ?: return@mapNotNull null
        // Never a raw id: swap name → active/library, humanized (incl. seed split) as a last resort (C3).
        val name = Program.exerciseDisplayName(row.exerciseId, row.swappedName)
        PrEntry(
            date = row.sessionStartedAt,
            exerciseName = name,
            weightLb = weightLb,
            reps = prSet.reps
        )
    }
}

internal fun buildHallOfFame(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    bodyweightLb: Double? = null
): List<PrRecord> {
    return allSets
        .filter { it.weightLb != null }
        .groupBy { it.exerciseId }
        .mapNotNull { (exerciseId, sets) ->
            val plan = Program.exercise(exerciseId) ?: return@mapNotNull null
            val bestSet = sets.maxByOrNull { it.weightLb!! } ?: return@mapNotNull null
            // Rounded to one decimal, not truncated: 1.99x bodyweight used to render "1.9x".
            val rel = if (bodyweightLb != null && bodyweightLb > 0)
                (bestSet.weightLb!! / bodyweightLb * 10).roundToInt() / 10.0
            else null
            PrRecord(
                exerciseId = exerciseId,
                exerciseName = plan.name,
                maxWeightLb = bestSet.weightLb!!,
                bestReps = bestSet.reps,
                sessionDate = bestSet.sessionStartedAt,
                muscle = plan.muscle,
                relativeStrength = rel
            )
        }
        .sortedWith(compareBy({ it.muscle.displayName }, { it.exerciseName }))
}

internal fun buildExerciseHistory(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): Map<String, List<HistoryPoint>> {
    return allSets
        .filter { it.weightLb != null }
        .groupBy { it.exerciseId }
        .mapValues { (_, sets) ->
            sets
                .groupBy { it.sessionStartedAt }
                .map { (sessionDate, sessionSets) ->
                    HistoryPoint(
                        sessionDate = sessionDate,
                        maxWeightLb = sessionSets.maxOf { it.weightLb!! }
                    )
                }
                .sortedBy { it.sessionDate }
        }
}

/**
 * Epley estimated 1-rep max — delegates to the canonical [E1rm] so the Stats path and the
 * adaptation engine can never diverge. [E1rm.epley] guards `reps <= 1` (a true single's e1RM is
 * the weight itself); the old private copy here omitted that guard and inflated heavy singles by
 * 3.3% on every absolute e1RM number the Stats screen shows.
 */
private fun e1rm(weightLb: Double, reps: Int): Double = E1rm.epley(weightLb, reps)

/** Per-lift estimated-1RM progression: best e1RM per session, with growth rate + stall flag. */
internal fun buildE1rmLifts(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): List<E1rmLift> {
    return allSets
        .filter { it.weightLb != null && it.weightLb > 0 }
        .groupBy { it.exerciseId }
        .mapNotNull { (id, sets) ->
            val name = Program.exercise(id)?.name ?: return@mapNotNull null
            val perSession = sets.groupBy { it.sessionStartedAt }.toSortedMap()
            val points = perSession.map { (_, ss) -> ss.maxOf { e1rm(it.weightLb!!, it.reps) } }
            if (points.isEmpty()) return@mapNotNull null
            val dates = perSession.keys.toList()
            val first = points.first()
            val current = points.last()
            val monthlyPct = if (points.size >= 2 && first > 0) {
                // Need ≥ ~15 days of span to annualise a rate; below that show no rate rather than a
                // wildly off one (was coerceAtLeast(0.5), which deflated brand-new lifts). (F14.)
                val months = (dates.last() - dates.first()) / (30.44 * 24 * 60 * 60 * 1000)
                if (months >= 0.5) (current - first) / first / months * 100.0 else null
            } else null
            val stalling = points.size >= 3 && run {
                val recent = points.takeLast(3)
                val hi = recent.max()
                hi > 0 && (hi - recent.min()) / hi < 0.01
            }
            E1rmLift(
                exerciseId = id, exerciseName = name, currentE1rm = current,
                history = points, monthlyPct = monthlyPct, stalling = stalling
            )
        }
        .sortedByDescending { it.currentE1rm }
        .take(6)
}

/** Best weight at each rep count for the single most-trained lift. */
internal fun buildRepMaxes(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
): RepMaxSet? {
    val byExercise = allSets.filter { it.weightLb != null && it.weightLb > 0 }.groupBy { it.exerciseId }
    val top = byExercise.maxByOrNull { it.value.size } ?: return null
    val name = Program.exercise(top.key)?.name ?: return null
    val entries = top.value
        .groupBy { it.reps }
        .map { (reps, ss) -> RepMaxEntry(reps = reps, weightLb = ss.maxOf { it.weightLb!! }) }
        .sortedBy { it.reps }
    return if (entries.isEmpty()) null else RepMaxSet(exerciseName = name, entries = entries)
}

/** Average estimated-1RM growth per month across lifts, as a percent. */
internal fun computeProgressiveOverload(lifts: List<E1rmLift>): Double? =
    lifts.mapNotNull { it.monthlyPct }.takeIf { it.isNotEmpty() }?.average()

/**
 * The concrete "progressive overload" series: per ISO week, the average of each tracked
 * lift's best e1RM that week (lifts not trained that week simply don't contribute — a
 * known composition wobble on split weeks, acceptable for a personal trend line).
 * Restricted to the lifts [buildE1rmLifts] tracks so one stray accessory can't move it.
 */
internal fun buildOverloadSummary(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    lifts: List<E1rmLift>,
    zone: ZoneId = ZoneId.systemDefault(),
    maxWeeks: Int = 12
): OverloadSummary? {
    val ids = lifts.map { it.exerciseId }.toSet()
    if (ids.isEmpty()) return null
    val bestPerWeekPerLift = HashMap<LocalDate, HashMap<String, Double>>()
    allSets.forEach { s ->
        if (s.exerciseId !in ids) return@forEach
        val w = s.weightLb ?: return@forEach
        if (w <= 0) return@forEach
        val d = Instant.ofEpochMilli(s.sessionStartedAt).atZone(zone).toLocalDate()
        val week = d.minusDays(d.dayOfWeek.value.toLong() - 1)
        bestPerWeekPerLift.getOrPut(week) { HashMap() }.merge(s.exerciseId, e1rm(w, s.reps), ::maxOf)
    }
    if (bestPerWeekPerLift.isEmpty()) return null
    // Forward-fill each lift's last-known best e1RM into weeks it wasn't trained, so every week's
    // average covers the SAME cumulative set of lifts. Without this, on a split program the weekly
    // average swings with WHICH lifts were trained that week rather than with strength — an
    // 'a'-only week and a 'b'-only week alternate wildly. (Stats audit F12.) Accumulate across ALL
    // weeks in order, then keep the last [maxWeeks] for display.
    val carried = HashMap<String, Double>()
    val weekly = bestPerWeekPerLift.entries
        .sortedBy { it.key }
        .map { (_, perLift) ->
            carried.putAll(perLift)
            carried.values.average()
        }
        .takeLast(maxWeeks)
    return OverloadSummary(current = weekly.last(), weekly = weekly)
}

/** Days since the last PR, overall + per exercise — the Strength tab's drought numbers. */
internal fun buildPrRecency(
    rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.ExercisePrDate>,
    nowMs: Long,
    zone: ZoneId = ZoneId.systemDefault()
): PrRecency? {
    if (rows.isEmpty()) return null
    val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
    fun daysSince(ms: Long): Int =
        ChronoUnit.DAYS.between(Instant.ofEpochMilli(ms).atZone(zone).toLocalDate(), today)
            .toInt().coerceAtLeast(0)
    val byExercise = rows.groupBy { it.exerciseId }
        .mapValues { (_, rs) -> daysSince(rs.maxOf { it.sessionDate }) }
    return PrRecency(daysSinceLast = byExercise.values.minOrNull() ?: 0, byExercise = byExercise)
}

/** Movement-pattern buckets for the radar — derived from muscle groups, not exercise ids. */
private val PATTERN_BUCKETS: List<Pair<String, Set<MuscleGroup>>> = listOf(
    "PUSH" to setOf(MuscleGroup.CHEST, MuscleGroup.SHOULDERS, MuscleGroup.TRICEPS),
    "PULL" to setOf(MuscleGroup.BACK, MuscleGroup.REAR_DELTS, MuscleGroup.BICEPS),
    "QUADS" to setOf(MuscleGroup.QUADS),
    "POSTERIOR" to setOf(MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES),
    "CORE" to setOf(MuscleGroup.CORE),
    "CALVES" to setOf(MuscleGroup.CALVES)
)

/**
 * Movement-pattern radar (replaces the hardcoded compound-id radar, which broke whenever
 * the generated program didn't contain those exact exercises): per pattern, best e1RM in
 * the recent window vs the all-time best. Absolute lb isn't comparable across patterns;
 * current-vs-your-own-peak is. Returns empty below 3 axes — a 2-point radar is a line.
 */
internal fun buildPatternRadar(
    allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
    nowMs: Long,
    windowDays: Int = 42,
    muscleOf: (String) -> MuscleGroup? = { Program.exercise(it)?.muscle }
): List<PatternAxis> {
    val since = nowMs - windowDays * 24L * 60 * 60 * 1000
    val peak = HashMap<String, Double>()
    val current = HashMap<String, Double>()
    allSets.forEach { s ->
        val w = s.weightLb ?: return@forEach
        if (w <= 0) return@forEach
        val muscle = muscleOf(s.exerciseId) ?: return@forEach
        val bucket = PATTERN_BUCKETS.firstOrNull { muscle in it.second }?.first ?: return@forEach
        val e = e1rm(w, s.reps)
        peak.merge(bucket, e, ::maxOf)
        if (s.sessionStartedAt >= since) current.merge(bucket, e, ::maxOf)
    }
    val axes = PATTERN_BUCKETS.mapNotNull { (label, _) ->
        val p = peak[label] ?: return@mapNotNull null
        if (p <= 0) return@mapNotNull null
        PatternAxis(label = label, currentE1rm = current[label] ?: 0.0, peakE1rm = p)
    }
    return if (axes.size >= 3) axes else emptyList()
}

internal fun buildTimeToPr(
    rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.ExercisePrDate>
): List<TimeToPrEntry> {
    return rows
        .groupBy { it.exerciseId }
        .mapNotNull { (exerciseId, dates) ->
            if (dates.size < 2) return@mapNotNull null
            val sorted = dates.sortedBy { it.sessionDate }
            // Averaged over CALENDAR-day gaps, not elapsed milliseconds: the number is labelled and
            // read as a day count ("a PR every ~9 days"), and dividing elapsed ms shortens any gap
            // that crossed a spring-forward while lengthening one that crossed a fall-back.
            val zone = ZoneId.systemDefault()
            fun dayOf(ms: Long) = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
            val avgDays = sorted
                .zipWithNext { a, b -> ChronoUnit.DAYS.between(dayOf(a.sessionDate), dayOf(b.sessionDate)) }
                .average().roundToInt().coerceAtLeast(1)
            val name = Program.exercise(exerciseId)?.name ?: return@mapNotNull null
            TimeToPrEntry(exerciseId = exerciseId, exerciseName = name, avgDaysBetween = avgDays, prCount = dates.size)
        }
        .sortedBy { it.avgDaysBetween }
}

internal fun buildPrsByDayOfWeek(prTimes: List<Long>): List<Int> {
    val zone = ZoneId.systemDefault()
    val counts = IntArray(7)
    prTimes.forEach { ms ->
        val dow = Instant.ofEpochMilli(ms).atZone(zone).dayOfWeek.value - 1 // 0=Mon
        counts[dow]++
    }
    return counts.toList()
}
