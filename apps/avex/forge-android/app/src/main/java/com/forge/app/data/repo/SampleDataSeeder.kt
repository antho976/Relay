package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.program.Program
import java.time.LocalDate
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.max
import kotlin.random.Random

/**
 * Populates the database with realistic fake training data for demo / testing runs (#148).
 * Simulates 8 weeks of the 4-day split with gradual progressive overload.
 * Only runs when the DB is empty (totalSessions == 0).
 */
@Singleton
class SampleDataSeeder @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val clock: Clock
) {

    suspend fun seed() {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val rng = Random(42)

        // 8 weeks of training, Mon/Tue/Thu/Fri each week, rotating through the days the
        // *active* program actually has. Must not assume the 4-day split: a 1/2/3/5/6/7-day
        // program has different day keys and would crash the old `first { it.key == ... }` lookup.
        val programDays = Program.days
        if (programDays.isEmpty()) return
        var sessionIndex = 0
        // All-time best working weight per exercise, accumulated oldest-week-first so PR flags are
        // realistic across the seeded history.
        val maxByExercise = mutableMapOf<String, Double>()

        for (weekOffset in 7 downTo 0) {
            val weekStart = today.minusWeeks(weekOffset.toLong())
            // train Mon, Tue, Thu, Fri each week
            val trainingDays = listOf(0L, 1L, 3L, 4L)
            for (dayOffset in trainingDays) {
                val trainingDate = weekStart.plusDays(dayOffset)
                if (trainingDate.isAfter(today)) continue

                val startMs = trainingDate.atTime(18, 30).atZone(zone).toInstant().toEpochMilli()
                val durationMs = (55 + rng.nextInt(20)) * 60_000L
                val finishedMs = startMs + durationMs

                val day = programDays[sessionIndex % programDays.size]
                val dayKey = day.key
                var totalVol = 0.0
                var prCount = 0

                val sessionId = sessionDao.insert(Session(
                    dayKey = dayKey,
                    startedAt = startMs,
                    finishedAt = null
                ))

                val totalSetsInSession = day.exercises.sumOf { it.sets }
                var setCounter = 0

                day.exercises.forEachIndexed { exIdx, plan ->
                    val baseWeight = when (plan.unit) {
                        com.forge.app.program.ExerciseUnit.BODYWEIGHT -> 0.0
                        com.forge.app.program.ExerciseUnit.PLATES -> 30.0 + exIdx * 5.0
                        else -> 20.0 + exIdx * 5.0
                    }
                    // progressive overload: ~+1.25 lb per week
                    val progressionLb = (7 - weekOffset) * 1.25
                    val workingWeight = max(0.0, baseWeight + progressionLb + rng.nextDouble(-2.0, 2.0))
                    val reps = 8 + rng.nextInt(3) // 8–10 reps

                    // A set counts as a PR the first time an exercise appears, or when it beats its prior
                    // seeded best — so the demo data has realistic PRs (#148) instead of prCount = 0.
                    val prevMax = maxByExercise[plan.id]
                    val isPr = plan.unit != com.forge.app.program.ExerciseUnit.BODYWEIGHT && workingWeight > 0 &&
                        (prevMax == null || workingWeight > prevMax)
                    if (isPr) { maxByExercise[plan.id] = workingWeight; prCount++ }

                    val loggedExId = loggedExerciseDao.insert(LoggedExercise(
                        sessionId = sessionId,
                        exerciseId = plan.id,
                        orderIndex = exIdx,
                        wasPr = isPr
                    ))

                    repeat(plan.sets) { setIdx ->
                        val weightLb = if (plan.unit == com.forge.app.program.ExerciseUnit.BODYWEIGHT) null else workingWeight
                        val setReps = reps + rng.nextInt(2)
                        // Space sets evenly across the session so completedAt is monotonic and stays
                        // within [startMs, finishedMs] (was index*3min, which could overshoot the finish).
                        val completedAt = startMs + durationMs * (setCounter + 1) / (totalSetsInSession + 1)
                        setCounter++
                        loggedSetDao.insert(LoggedSet(
                            loggedExerciseId = loggedExId,
                            setIndex = setIdx,
                            weightText = weightLb?.let { "${it.toInt()}" } ?: "BW",
                            weightLb = weightLb,
                            reps = setReps,
                            completedAt = completedAt
                        ))
                        totalVol += (weightLb ?: 0.0) * setReps
                    }
                }

                sessionDao.update(sessionDao.get(sessionId)!!.copy(
                    finishedAt = finishedMs,
                    totalVolumeLb = totalVol,
                    prCount = prCount,
                    setCount = day.exercises.sumOf { it.sets }
                ))
                sessionIndex++
            }
        }
    }
}
