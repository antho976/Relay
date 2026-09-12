package com.forge.app.program

import com.forge.app.domain.schedule.TrainingRecovery
import com.forge.app.domain.schedule.WeeklySchedule
import org.junit.Assert.*
import org.junit.Test
import java.time.LocalDate

class CoachingPolicyTest {
    private fun plans(days: List<GeneratedDay>): List<DayPlan> = days.map { day ->
        DayPlan(day.key, day.name, "", day.word, day.accentHex, emptyList(), day.exercises.map { ex ->
            val def = ExerciseLibrary.byId(ex.libId)!!
            ExercisePlan(def.id, def.name, ex.sets, ex.reps, def.unit, def.muscle, def.difficulty, "", tags = def.tags)
        })
    }

    @Test fun generatedWeeksAllowInterveningDayIncludingSundayToMonday() {
        val equipment = listOf(Equipment.entries.toSet(), setOf(Equipment.BODYWEIGHT_ONLY),
            setOf(Equipment.DUMBBELLS, Equipment.BENCH, Equipment.MACHINE, Equipment.CABLE))
        for (gear in equipment) for (count in 1..7) for (seed in 0L..19L) {
            val days = plans(ProgramGenerator.generate(GenerationParams(count), gear, emptySet(), emptySet(), seed = seed))
            val schedule = WeeklySchedule.defaultFor(days.map { it.key })
            val muscles = days.associate { it.key to TrainingRecovery.muscles(it) }
            for (i in 0..6) {
                val overlap = (muscles[schedule[i]] ?: emptySet()).intersect(muscles[schedule[(i + 1) % 7]] ?: emptySet())
                assertTrue("$count days seed $seed weekday $i overlap $overlap", overlap.isEmpty())
            }
        }
    }

    @Test fun threeDayPlansTrainMajorMusclesThreeTimesWithoutMarathonSessions() {
        for (experience in listOf("beginner", "intermediate", "advanced")) {
            val days = ProgramGenerator.generate(GenerationParams(3, experience = experience),
                Equipment.entries.toSet(), emptySet(), emptySet(), seed = 15L)
            assertEquals(3, days.size)
            days.forEach { day ->
                val muscles = day.exercises.map { ExerciseLibrary.byId(it.libId)!!.muscle }.toSet()
                assertTrue(muscles.containsAll(setOf(MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS)))
                assertTrue(day.exercises.sumOf { it.sets } <= 24)
            }
        }
    }

    @Test fun everyProfileHasBoundedSessionsAndIsolationNeverGetsHeavyStrengthReps() {
        for (count in 1..7) for (experience in listOf("beginner", "intermediate", "advanced")) {
            for (goal in listOf("build_muscle", "get_stronger", "lose_weight", "general_fitness")) {
                val days = ProgramGenerator.generate(GenerationParams(count, experience = experience, goal = goal,
                    priorityMuscles = MuscleGroup.entries.toSet()), Equipment.entries.toSet(), emptySet(), emptySet(), seed = 7L)
                days.forEach { day ->
                    assertTrue("${day.key} $experience $goal", day.exercises.sumOf { it.sets } <= 24)
                    day.exercises.forEach { ex ->
                        assertTrue(ex.sets in 2..5)
                        if (ExerciseTag.COMPOUND !in ExerciseLibrary.byId(ex.libId)!!.tags) {
                            assertNotEquals("${ex.libId} is isolation", "4-6", ex.reps)
                            assertNotEquals("${ex.libId} is isolation", "6-8", ex.reps)
                        }
                    }
                }
            }
        }
    }

    @Test fun actualSessionCompletionDefersOverlappingWorkoutAcrossWeekBoundary() {
        val today = LocalDate.of(2026, 9, 14) // Monday
        val days = plans(ProgramGenerator.generate(GenerationParams(3), Equipment.entries.toSet(), emptySet(), emptySet(), seed = 1L))
        val recovery = TrainingRecovery.daysUntilRecovered(days, listOf(days.last().key to today.minusDays(1)), today)
        val next = WeeklySchedule.resolveNextUpWithOffset(WeeklySchedule.MODE_SEQUENCE, 0, emptyList(),
            days.map { it.key }, days.last().key, emptySet(), recovery)!!
        assertEquals(1, next.daysAhead)
        assertNull(WeeklySchedule.trainTodayKey(next))
        assertEquals(days.first().key, WeeklySchedule.upcomingKey(next))
        assertTrue(TrainingRecovery.daysUntilRecovered(days, listOf(days.last().key to today.minusDays(2)), today).values.all { it == 0 })
    }

    @Test fun weeklyScheduleSkipsUnsafeCustomAssignmentWithoutSchedulingCatchup() {
        val next = WeeklySchedule.resolveNextUpWithOffset(WeeklySchedule.MODE_WEEKDAY, 0,
            listOf("a", "", "b", "", "", "", ""), listOf("a", "b"), "b", emptySet(), mapOf("a" to 1))!!
        assertEquals("b", next.dayKey)
        assertEquals(2, next.daysAhead)
        assertNull(WeeklySchedule.trainTodayKey(next))
    }
    @Test fun personalVolumeCeilingsAreHonoredEvenBelowTheUsualSlotMinimum() {
        for (cap in 0..5) {
            val days = ProgramGenerator.generate(GenerationParams(3,
                personalCaps = mapOf(MuscleGroup.CHEST to cap), priorityMuscles = setOf(MuscleGroup.CHEST)),
                Equipment.entries.toSet(), emptySet(), emptySet(), seed = 3L)
            assertTrue(days.flatMap { it.exercises }.filter { ExerciseLibrary.byId(it.libId)!!.muscle == MuscleGroup.CHEST }
                .sumOf { it.sets } <= cap)
        }
    }

    @Test fun restrictionsAndDislikesAreNeverOverriddenByPinsOrFallbacks() {
        val days = ProgramGenerator.generate(GenerationParams(3,
            problemAreas = setOf(ProblemArea.LOWER_BACK), pinned = setOf("db-romanian-deadlift")),
            Equipment.entries.toSet(), emptySet(), setOf("db-bench-press"), seed = 12L)
        days.flatMap { it.exercises }.forEach { ex ->
            assertNotEquals("db-bench-press", ex.libId)
            assertFalse(ProblemArea.LOWER_BACK in ExerciseLibrary.contraindicationsOf(ExerciseLibrary.byId(ex.libId)!!))
        }
        val result = runCatching {
            ProgramGenerator.generate(GenerationParams(3), Equipment.entries.toSet(), emptySet(),
                ExerciseLibrary.all.map { it.id }.toSet(), seed = 1L)
        }
        assertTrue("Impossible preferences must not crash the onboarding preview", result.isSuccess)
        assertTrue("An impossible plan must not invent allowed exercises", result.getOrThrow().all { it.exercises.isEmpty() })
    }

}
