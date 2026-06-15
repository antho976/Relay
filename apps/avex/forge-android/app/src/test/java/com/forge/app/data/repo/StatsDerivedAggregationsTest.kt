package com.forge.app.data.repo

import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.projections.SetWithExerciseAndSession
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.VolumeDeloadPoint
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneId

/**
 * The Stats revamp's new pure aggregations: overload series, PR recency, the
 * movement-pattern radar, weekly tonnage/durations, and training times.
 */
class StatsDerivedAggregationsTest {

    private val zone = ZoneId.of("UTC")

    /** Monday of an ISO week n weeks before a fixed anchor Monday, at [hour]. */
    private fun dayMs(date: LocalDate, hour: Int = 18) =
        date.atTime(hour, 0).atZone(zone).toInstant().toEpochMilli()

    // Anchor on a known Monday so ISO-week bucketing is deterministic.
    private val monday: LocalDate = LocalDate.of(2026, 6, 1)

    private fun set(exerciseId: String, weight: Double, reps: Int, at: Long) =
        SetWithExerciseAndSession(
            weightLb = weight, reps = reps, exerciseId = exerciseId,
            sessionStartedAt = at, loggedExerciseId = 0L
        )

    private fun lift(id: String) = E1rmLift(
        exerciseId = id, exerciseName = id, currentE1rm = 0.0, history = emptyList()
    )

    // ── buildOverloadSummary ───────────────────────────────────────────────────

    @Test
    fun overload_averagesBestE1rmPerLiftPerWeek() {
        val wk1 = dayMs(monday)
        val wk2 = dayMs(monday.plusWeeks(1))
        // reps = 15 → e1RM = weight × 1.5, keeping expectations round.
        val sets = listOf(
            set("a", 100.0, 15, wk1),            // e1RM 150
            set("a", 90.0, 15, wk1),             // ignored — not the week's best for "a"
            set("b", 200.0, 15, wk1),            // e1RM 300 → wk1 avg 225
            set("a", 110.0, 15, wk2),            // 165
            set("b", 210.0, 15, wk2)             // 315 → wk2 avg 240
        )
        val summary = buildOverloadSummary(sets, listOf(lift("a"), lift("b")), zone)!!
        assertEquals(2, summary.weekly.size)
        assertEquals(225.0, summary.weekly[0], 0.001)
        assertEquals(240.0, summary.weekly[1], 0.001)
        assertEquals(240.0, summary.current, 0.001)
        assertEquals(15.0, summary.deltaVsPrevWeek!!, 0.001)
        assertEquals(225.0, summary.prevWeek!!, 0.001)
    }

    @Test
    fun overload_ignoresUntrackedLifts_andEmptyIsNull() {
        val sets = listOf(set("stray", 100.0, 5, dayMs(monday)))
        assertNull(buildOverloadSummary(sets, listOf(lift("a")), zone))
        assertNull(buildOverloadSummary(emptyList(), emptyList(), zone))
    }

    @Test
    fun overload_forwardFillsUntrainedLiftsAcrossWeeks() {
        // Split program: each later week trains only one of the two tracked lifts. Forward-filling
        // the untrained lift's last value keeps every week's average over BOTH lifts, so the series
        // tracks strength, not which lift happened to be trained that week. reps=15 → e1RM = w×1.5.
        val sets = listOf(
            set("a", 100.0, 15, dayMs(monday)),                 // a → 150
            set("b", 200.0, 15, dayMs(monday)),                 // b → 300  ⇒ wk1 avg 225
            set("a", 110.0, 15, dayMs(monday.plusWeeks(1))),    // a → 165; b carried 300 ⇒ wk2 avg 232.5
            set("b", 210.0, 15, dayMs(monday.plusWeeks(2)))     // b → 315; a carried 165 ⇒ wk3 avg 240
        )
        val summary = buildOverloadSummary(sets, listOf(lift("a"), lift("b")), zone)!!
        assertEquals(3, summary.weekly.size)
        assertEquals(225.0, summary.weekly[0], 0.001)
        assertEquals(232.5, summary.weekly[1], 0.001)
        assertEquals(240.0, summary.weekly[2], 0.001)
    }

    // ── e1RM single-rep guard (Stats audit 2026-06-15) ─────────────────────────
    // A true 1-rep max's e1RM IS the weight (315), NOT 315 × (1 + 1/30) = 325.5. The Stats e1rm()
    // must stay delegated to the engine's E1rm.epley, which guards reps <= 1; a naive Epley copy
    // here was inflating every heavy single shown on the Strength tab by 3.3%.
    @Test
    fun e1rm_trueSingleIsNotInflated() {
        val ex = "db-bench-press" // a real library id so Program.exercise(id) resolves a name
        val sets = listOf(set(ex, 315.0, 1, dayMs(monday)))

        val lifts = buildE1rmLifts(sets)
        assertEquals(1, lifts.size)
        assertEquals(315.0, lifts[0].currentE1rm, 0.001)

        val summary = buildOverloadSummary(sets, listOf(lift(ex)), zone)!!
        assertEquals(315.0, summary.current, 0.001)
    }

    // ── buildPrRecency ─────────────────────────────────────────────────────────

    @Test
    fun prRecency_overallIsMostRecentAcrossExercises() {
        val now = dayMs(monday.plusWeeks(2))
        val rows = listOf(
            LoggedExerciseDao.ExercisePrDate("a", dayMs(monday)),                // 14 days ago
            LoggedExerciseDao.ExercisePrDate("a", dayMs(monday.plusWeeks(1))),   // 7 days ago
            LoggedExerciseDao.ExercisePrDate("b", dayMs(monday.plusWeeks(2)))    // today
        )
        val recency = buildPrRecency(rows, now, zone)!!
        assertEquals(0, recency.daysSinceLast)
        assertEquals(7, recency.byExercise["a"])
        assertEquals(0, recency.byExercise["b"])
    }

    @Test
    fun prRecency_emptyIsNull() {
        assertNull(buildPrRecency(emptyList(), dayMs(monday), zone))
    }

    // ── buildPatternRadar ──────────────────────────────────────────────────────

    private val muscles = mapOf(
        "bench" to MuscleGroup.CHEST,    // PUSH
        "row" to MuscleGroup.BACK,       // PULL
        "squat" to MuscleGroup.QUADS,    // QUADS
        "rdl" to MuscleGroup.HAMSTRINGS  // POSTERIOR
    )

    @Test
    fun radar_currentWindowVsAllTimePeak() {
        val now = dayMs(monday.plusWeeks(8))
        val old = dayMs(monday)                       // outside the 42-day window
        val recent = dayMs(monday.plusWeeks(7))       // inside
        val sets = listOf(
            set("bench", 100.0, 1, old),   // all-time peak 100
            set("bench", 90.0, 1, recent), // current 90 → fraction 0.9
            set("row", 80.0, 1, recent),
            set("squat", 120.0, 1, recent)
        )
        val axes = buildPatternRadar(sets, now, muscleOf = { muscles[it] })
        assertEquals(3, axes.size)
        val push = axes.first { it.label == "PUSH" }
        assertEquals(0.9, push.fraction, 0.001)
        assertEquals(1.0, axes.first { it.label == "PULL" }.fraction, 0.001)
    }

    @Test
    fun radar_underThreeAxes_staysEmpty() {
        val now = dayMs(monday.plusWeeks(1))
        val sets = listOf(
            set("bench", 100.0, 1, dayMs(monday)),
            set("row", 80.0, 1, dayMs(monday))
        )
        assertTrue(buildPatternRadar(sets, now, muscleOf = { muscles[it] }).isEmpty())
    }

    // ── buildWeeklyTonnage ─────────────────────────────────────────────────────

    @Test
    fun weeklyTonnage_bucketsByIsoWeek_andMarksDeload() {
        val points = listOf(
            VolumeDeloadPoint(dayMs(monday), "d", 1000.0, isDeload = false),
            VolumeDeloadPoint(dayMs(monday.plusDays(2)), "d", 500.0, isDeload = false),
            VolumeDeloadPoint(dayMs(monday.plusWeeks(1)), "d", 700.0, isDeload = true)
        )
        val weeks = buildWeeklyTonnage(points, zone)
        assertEquals(2, weeks.size)
        assertEquals(1500.0, weeks[0].volumeLb, 0.001)
        assertTrue(!weeks[0].isDeload)
        assertEquals(700.0, weeks[1].volumeLb, 0.001)
        assertTrue(weeks[1].isDeload)
    }

    // ── buildTrainingTimes ─────────────────────────────────────────────────────

    @Test
    fun trainingTimes_countsSessionsPerWeekday_andLabelsBestHour() {
        val sets = listOf(
            set("a", 50.0, 8, dayMs(monday, hour = 18)),
            set("a", 50.0, 8, dayMs(monday, hour = 18)),               // same session
            set("a", 50.0, 8, dayMs(monday.plusDays(3), hour = 18))    // Thursday
        )
        val t = buildTrainingTimes(sets, zone, minSetsForHour = 1)!!
        assertEquals(1, t.sessionsByDayOfWeek[0]) // Monday
        assertEquals(1, t.sessionsByDayOfWeek[3]) // Thursday
        assertEquals("evening (18:00)", t.bestHourLabel)
    }

    @Test
    fun trainingTimes_hourLabelGatedBySampleSize() {
        val sets = listOf(set("a", 50.0, 8, dayMs(monday)))
        assertNull(buildTrainingTimes(sets, zone, minSetsForHour = 30)!!.bestHourLabel)
        assertNull(buildTrainingTimes(emptyList(), zone))
    }

    // ── buildWeeklyDurations ───────────────────────────────────────────────────

    @Test
    fun weeklyDurations_medianPerWeek_insaneDurationsDropped() {
        fun session(start: Long, minutes: Long?) = Session(
            dayKey = "d", startedAt = start,
            finishedAt = minutes?.let { start + it * 60_000 }
        )
        val sessions = listOf(
            session(dayMs(monday), 50),
            session(dayMs(monday.plusDays(1)), 60),
            session(dayMs(monday.plusDays(2)), 70),
            session(dayMs(monday.plusDays(3)), 500),  // > 240 min → dropped
            session(dayMs(monday.plusDays(4)), null), // unfinished → dropped
            session(dayMs(monday.plusWeeks(1)), 40)
        )
        val weeks = buildWeeklyDurations(sessions, zone)
        assertEquals(2, weeks.size)
        assertEquals(60, weeks[0].medianMin)
        assertEquals(40, weeks[1].medianMin)
    }

    @Test
    fun weeklyDurations_evenCount_averagesTheTwoMiddleElements() {
        fun session(start: Long, minutes: Long) = Session(
            dayKey = "d", startedAt = start, finishedAt = start + minutes * 60_000
        )
        // One ISO week, two sessions: 45 and 75 → median 60, NOT the upper element 75.
        val sessions = listOf(
            session(dayMs(monday), 45),
            session(dayMs(monday.plusDays(1)), 75)
        )
        val weeks = buildWeeklyDurations(sessions, zone)
        assertEquals(1, weeks.size)
        assertEquals(60, weeks[0].medianMin)
    }
}
