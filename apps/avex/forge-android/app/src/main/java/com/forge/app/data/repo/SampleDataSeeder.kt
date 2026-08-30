package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.mood.Mood
import com.forge.app.program.Program
import java.time.LocalDate
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.max
import kotlin.random.Random

/**
 * Populates the database with realistic fake training data for demo / testing runs (#148).
 * Simulates 8 weeks of the 4-day split with gradual progressive overload — including RPE,
 * effort ratings, moods, and a weekly bodyweight, so every Stats section has data to show.
 * Only runs when the DB is empty (totalSessions == 0).
 */
@Singleton
class SampleDataSeeder @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val bodyweightDao: BodyweightDao,
    private val moodDao: MoodDao,
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
        // Anchor the seeded bodyweight trend on the user's own logged weight (entered in onboarding).
        // If they have none, skip bodyweight seeding entirely rather than invent a personal number.
        val anchorBwLb = bodyweightDao.latest()?.weightLb
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

                    // Effort ratings skew harder as the weeks accumulate — gives the effort
                    // distribution and fatigue signals something honest to chew on.
                    val effortRoll = rng.nextInt(10) + (7 - weekOffset) / 3
                    val effort = when {
                        effortRoll <= 2 -> EffortRating.EASY
                        effortRoll <= 6 -> EffortRating.JUST_RIGHT
                        effortRoll <= 8 -> EffortRating.HARD
                        else -> EffortRating.BRUTAL
                    }

                    val loggedExId = loggedExerciseDao.insert(LoggedExercise(
                        sessionId = sessionId,
                        exerciseId = plan.id,
                        orderIndex = exIdx,
                        wasPr = isPr,
                        difficulty = effort
                    ))

                    repeat(plan.sets) { setIdx ->
                        val weightLb = if (plan.unit == com.forge.app.program.ExerciseUnit.BODYWEIGHT) null else workingWeight
                        val setReps = reps + rng.nextInt(2)
                        // Space sets evenly across the session so completedAt is monotonic and stays
                        // within [startMs, finishedMs] (was index*3min, which could overshoot the finish).
                        val completedAt = startMs + durationMs * (setCounter + 1) / (totalSetsInSession + 1)
                        setCounter++
                        // RPE on roughly half the sets, clustered 6.5–9.5.
                        val rpe = if (rng.nextBoolean()) (13 + rng.nextInt(7)) / 2.0 else null
                        loggedSetDao.insert(LoggedSet(
                            loggedExerciseId = loggedExId,
                            setIndex = setIdx,
                            weightText = weightLb?.let { "${it.toInt()}" } ?: "BW",
                            weightLb = weightLb,
                            // Funnel through the same reps clamp as the live write path so no insert
                            // route can poison downstream stats (the values here are already sane).
                            reps = sanitizeReps(setReps),
                            completedAt = completedAt,
                            rpe = rpe
                        ))
                        totalVol += (weightLb ?: 0.0) * setReps
                    }
                }

                sessionDao.get(sessionId)?.let { row ->
                    sessionDao.update(row.copy(
                        finishedAt = finishedMs,
                        totalVolumeLb = totalVol,
                        prCount = prCount,
                        setCount = day.exercises.sumOf { it.sets }
                    ))
                }

                // Post-workout mood, loosely tracking how hard the sessions roll.
                val mood = when (rng.nextInt(6)) {
                    0 -> Mood.DRAINED; 1 -> Mood.OFF; 2 -> Mood.FINE; 3, 4 -> Mood.GOOD; else -> Mood.STRONG
                }
                moodDao.insert(MoodEntry(
                    sessionId = sessionId, dayKey = dayKey,
                    mood = mood.code, recordedAt = finishedMs + 60_000
                ))
                sessionIndex++
            }

            // One bodyweight log per seeded week, drifting up to the user's own current weight
            // (~0.4 lb/week). Skipped entirely when the user has no logged bodyweight to anchor on.
            //
            // And never over a day the user has already weighed in on. `upsert` is INSERT OR
            // REPLACE, which DELETES the conflicting row — so demo data written blindly across eight
            // Wednesdays destroyed a real reading and its note on each of them. The seeder's own
            // gate ("no finished sessions") does not protect a bodyweight-only user, which is
            // exactly who reaches for demo data: they have months of weigh-ins and no workouts.
            // Skipping an occupied date matches the importer's merge behaviour — real data wins.
            val bwDate = weekStart.plusDays(2)
            if (anchorBwLb != null && !bwDate.isAfter(today) &&
                bodyweightDao.byDateKey(bwDate.toString()) == null
            ) {
                val bwMs = bwDate.atTime(8, 0).atZone(zone).toInstant().toEpochMilli()
                bodyweightDao.upsert(BodyweightEntry(
                    dateKey = bwDate.toString(),
                    weightLb = anchorBwLb - weekOffset * 0.4 + rng.nextDouble(-0.6, 0.6),
                    recordedAt = bwMs
                ))
            }
        }
    }
}
