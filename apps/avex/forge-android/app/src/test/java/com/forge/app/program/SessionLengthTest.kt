package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The onboarding "about how long per session?" answer, as the generator applies it. */
class SessionLengthTest {

    /** A generated day priced the way the day card prices it (canonical rest, warmup allowance). */
    private fun minutesOf(day: GeneratedDay): Double {
        val seconds = day.exercises.sumOf { ex ->
            val def = ExerciseLibrary.byId(ex.libId)!!
            val compound = ExerciseTag.COMPOUND in def.tags
            val rest = if (compound) SessionEstimate.COMPOUND_REST +
                (if (SessionEstimate.isHeavy(ex.reps)) SessionEstimate.HEAVY_REST_BONUS else 0)
            else SessionEstimate.ISOLATION_REST
            SessionEstimate.exerciseSeconds(ex.sets, rest)
        } + SessionEstimate.WARMUP_ALLOWANCE_SECONDS
        return seconds / 60.0
    }

    private fun week(days: Int, minutes: Int?, goal: String = "build_muscle", experience: String = "intermediate") =
        ProgramGenerator.generate(
            GenerationParams(days, goal = goal, experience = experience, sessionMinutes = minutes),
            emptySet(), emptySet(), emptySet(), seed = 11L
        )

    @Test
    fun noAnswerChangesNothing() {
        (1..7).forEach { d ->
            val days = SplitTemplates.forDays(d)
            assertEquals(VolumeModel.allocate(days), VolumeModel.allocate(days, sessionMinutes = null))
            assertEquals(VolumeModel.allocate(days), VolumeModel.allocate(days, sessionMinutes = 0))
        }
    }

    @Test
    fun everyDayFitsTheAnswer() {
        // A HYPERTROPHY slot is priced between compound and isolation rest before its movement is
        // picked, so the real day can land a few minutes either side; the day card rounds to 5 anyway.
        val slack = 6.0
        for (d in 1..7) for (minutes in listOf(45, 60)) for (goal in listOf("build_muscle", "get_stronger")) {
            week(d, minutes, goal, "advanced").forEach { day ->
                val m = minutesOf(day)
                assertTrue("$d-day ${day.key} ($goal) runs ${"%.1f".format(m)} min against a $minutes-min answer",
                    m <= minutes + slack)
            }
        }
    }

    @Test
    fun shorterAnswersNeverAddVolume() {
        (1..7).forEach { d ->
            val days = SplitTemplates.forDays(d)
            val none = VolumeModel.allocate(days, volumeFactor = 1.2).map { it.sum() }
            val hour = VolumeModel.allocate(days, volumeFactor = 1.2, sessionMinutes = 60).map { it.sum() }
            val half = VolumeModel.allocate(days, volumeFactor = 1.2, sessionMinutes = 30).map { it.sum() }
            none.indices.forEach { i ->
                assertTrue("$d-day day $i: 60 min (${hour[i]}) > no limit (${none[i]})", hour[i] <= none[i])
                assertTrue("$d-day day $i: 30 min (${half[i]}) > 60 min (${hour[i]})", half[i] <= hour[i])
            }
        }
    }

    @Test
    fun theMainLiftAndASessionSurviveEvenAVeryShortAnswer() {
        (1..7).forEach { d ->
            val days = SplitTemplates.forDays(d)
            val sets = VolumeModel.allocate(days, sessionMinutes = 15)
            days.forEachIndexed { di, day ->
                day.targets.forEachIndexed { si, slot ->
                    if (slot.scheme == RepScheme.STRENGTH) {
                        assertTrue("$d-day ${day.key}: heavy slot $si was dropped", sets[di][si] > 0)
                    }
                }
                val kept = sets[di].count { it > 0 }
                assertTrue("$d-day ${day.key} kept $kept slots", kept >= minOf(VolumeModel.MIN_SLOTS_PER_SESSION, day.targets.size))
            }
        }
    }

    @Test
    fun aGenerousAnswerLeavesAnOrdinaryWeekAlone() {
        (1..7).forEach { d ->
            assertEquals(week(d, null), week(d, 180))
        }
    }

    @Test
    fun onboardingLedgerFollowsTheAnswer() {
        val none = ProgramGenerator.plannedSetsPerDay(4, "intermediate")
        val short = ProgramGenerator.plannedSetsPerDay(4, "intermediate", sessionMinutes = 30)
        assertTrue("a 30-min week ($short) should plan fewer sets than no limit ($none)", short.sum() < none.sum())
    }
}
