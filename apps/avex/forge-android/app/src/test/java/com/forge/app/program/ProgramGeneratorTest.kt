package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** JVM unit tests for the pure generator engine (program-unlock Phase 2). */
class ProgramGeneratorTest {

    @Test
    fun dayCountDrivesStructure() {
        assertEquals(3, ProgramGenerator.generate(GenerationParams(3), emptySet(), emptySet(), emptySet(), seed = 1L).size)
        assertEquals(7, ProgramGenerator.generate(GenerationParams(7), emptySet(), emptySet(), emptySet(), seed = 1L).size)
        // Clamps out-of-range day counts rather than crashing.
        assertEquals(1, ProgramGenerator.generate(GenerationParams(0), emptySet(), emptySet(), emptySet(), seed = 1L).size)
    }

    @Test
    fun deterministicForSameSeed() {
        val p = GenerationParams(4)
        fun ids() = ProgramGenerator.generate(p, emptySet(), emptySet(), emptySet(), seed = 42L)
            .flatMap { d -> d.exercises.map { it.libId } }
        assertEquals(ids(), ids())
    }

    @Test
    fun dislikedExcluded() {
        val days = ProgramGenerator.generate(GenerationParams(3), emptySet(), emptySet(), setOf("db-bench-press"), seed = 7L)
        assertTrue(days.flatMap { it.exercises }.none { it.libId == "db-bench-press" })
    }

    @Test
    fun equipmentFilter_dumbbellsAndBench_neverPicksMachineOrCable() {
        val available = setOf(Equipment.DUMBBELLS, Equipment.BENCH)
        val days = ProgramGenerator.generate(GenerationParams(4), available, emptySet(), emptySet(), seed = 3L)
        val defs = days.flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId) }
        assertTrue(defs.isNotEmpty())
        assertTrue(defs.none { Equipment.MACHINE in it.equipment || Equipment.CABLE in it.equipment })
    }

    @Test
    fun noDuplicateExerciseWithinADay() {
        val days = ProgramGenerator.generate(GenerationParams(5), emptySet(), emptySet(), emptySet(), seed = 99L)
        days.forEach { d ->
            val ids = d.exercises.map { it.libId }
            assertEquals("no dup within ${d.key}", ids.size, ids.toSet().size)
        }
    }

    @Test
    fun dumbbellsAndBench_fillsBackAndRearDelts() {
        val available = setOf(Equipment.DUMBBELLS, Equipment.BENCH)
        val days = ProgramGenerator.generate(GenerationParams(3), available, emptySet(), emptySet(), seed = 5L)
        val muscles = days.flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId)?.muscle }.toSet()
        assertTrue("BACK should be fillable with dumbbells+bench", MuscleGroup.BACK in muscles)
        assertTrue("REAR_DELTS should be fillable with dumbbells+bench", MuscleGroup.REAR_DELTS in muscles)
    }

    @Test
    fun liftDaysHaveFiveToSixExercises() {
        // Phase 4 tuning: each session should be a "standard" 5-6 movements (with full equipment).
        val days = ProgramGenerator.generate(GenerationParams(3), emptySet(), emptySet(), emptySet(), seed = 11L)
        days.forEach { d ->
            assertTrue("${d.key} has ${d.exercises.size} exercises", d.exercises.size in 5..6)
        }
    }

    @Test
    fun repsAreEitherASchemeValueOrTheMovementsNaturalReps() {
        // Phase 4 tuning: numeric ranges adopt the slot scheme; AMRAP / timed / per-leg stay intact.
        val schemeValues = setOf("6-10", "8-12", "12-15")
        val numeric = Regex("""^\d+(-\d+)?$""")
        val days = ProgramGenerator.generate(GenerationParams(5), emptySet(), emptySet(), emptySet(), seed = 21L)
        days.flatMap { it.exercises }.forEach { ex ->
            val natural = ExerciseLibrary.byId(ex.libId)!!.defaultReps
            val ok = ex.reps in schemeValues || (!numeric.matches(natural) && ex.reps == natural)
            assertTrue("reps '${ex.reps}' (natural '$natural') for ${ex.libId}", ok)
        }
    }

    @Test
    fun strengthSlotsLeanCompound() {
        // Phase 4: the day's heavy STRENGTH slot (first slot of Push = chest) should overwhelmingly
        // pick a COMPOUND, not an isolation like a cable fly. Statistical over many seeds.
        val n = 60
        var compound = 0
        repeat(n) { s ->
            val days = ProgramGenerator.generate(GenerationParams(3), emptySet(), emptySet(), emptySet(), seed = s.toLong())
            val lead = days.first { it.key == "push" }.exercises.first()
            if (ExerciseTag.COMPOUND in ExerciseLibrary.byId(lead.libId)!!.tags) compound++
        }
        assertTrue("heavy slot should usually be a compound ($compound/$n)", compound >= (n * 0.8).toInt())
    }

    @Test
    fun heavyLegSlotPrefersBilateral() {
        // Phase 4 tweak: the QUADS STRENGTH slot (Legs lead) should usually be a bilateral lift
        // (goblet squat), not a per-leg movement like a reverse lunge.
        val n = 60
        var bilateral = 0
        repeat(n) { s ->
            val days = ProgramGenerator.generate(GenerationParams(3), emptySet(), emptySet(), emptySet(), seed = s.toLong())
            val lead = days.first { it.key == "legs" }.exercises.first()
            if (!ExerciseLibrary.byId(lead.libId)!!.defaultReps.contains("/")) bilateral++
        }
        assertTrue("legs lead should usually be bilateral ($bilateral/$n)", bilateral >= (n * 0.55).toInt())
    }

    @Test
    fun emphasisAddsVolumeToTheFocusedMuscles() {
        // Phase 4: emphasis was a no-op before. Arms+shoulders emphasis should raise weekly bicep volume.
        fun weeklyBiceps(emphasis: String): Int =
            ProgramGenerator.generate(GenerationParams(3, emphasis = emphasis), emptySet(), emptySet(), emptySet(), seed = 4L)
                .flatMap { it.exercises }
                .filter { ExerciseLibrary.byId(it.libId)?.muscle == MuscleGroup.BICEPS }
                .sumOf { it.sets }
        assertTrue("arms emphasis should add bicep volume", weeklyBiceps("arms-shoulders") > weeklyBiceps("balanced"))
    }

    @Test
    fun goalShiftsRepRanges() {
        // Phase 2: same seed → same exercises, but the goal reshapes numeric rep ranges.
        fun reps(goal: String) = ProgramGenerator.generate(
            GenerationParams(3, goal = goal), emptySet(), emptySet(), emptySet(), seed = 8L
        ).flatMap { it.exercises }.map { it.reps }.toSet()
        val strong = reps("get_stronger")
        val build = reps("build_muscle")
        assertTrue("get_stronger should produce a heavy 4-6 range", "4-6" in strong)
        assertTrue("build_muscle should not use the 4-6 strength range", "4-6" !in build)
    }

    @Test
    fun experienceScalesVolume() {
        // Phase 2: beginners train less total volume than advanced lifters.
        fun totalSets(level: String) = ProgramGenerator.generate(
            GenerationParams(3, experience = level), emptySet(), emptySet(), emptySet(), seed = 8L
        ).flatMap { it.exercises }.sumOf { it.sets }
        assertTrue("beginner volume should be below advanced", totalSets("beginner") < totalSets("advanced"))
    }

    @Test
    fun beginnerAvoidsAdvancedMovements() {
        // Phase 2: with alternatives available, a beginner is never handed an ADVANCED lift.
        val days = ProgramGenerator.generate(
            GenerationParams(3, experience = "beginner"), emptySet(), emptySet(), emptySet(), seed = 8L
        )
        val anyAdvanced = days.flatMap { it.exercises }
            .any { ExerciseLibrary.byId(it.libId)?.difficulty == Difficulty.ADVANCED }
        assertTrue("beginner plan should contain no ADVANCED movements", !anyAdvanced)
    }

    @Test
    fun problemAreaSteersAwayFromStressfulMovements() {
        // Phase 3: flagging LOWER_BACK should make the generator avoid RDL/deadlift/bent rows when
        // a back-friendlier alternative exists for the muscle. Statistical over many seeds.
        val backLoaders = setOf("db-romanian-deadlift", "db-stiff-leg-deadlift", "db-single-leg-rdl", "db-row")
        var avoidedCount = 0
        val n = 40
        repeat(n) { s ->
            val flagged = ProgramGenerator.generate(
                GenerationParams(3, problemAreas = setOf(ProblemArea.LOWER_BACK)),
                emptySet(), emptySet(), emptySet(), seed = s.toLong()
            ).flatMap { it.exercises }.count { it.libId in backLoaders }
            val unflagged = ProgramGenerator.generate(
                GenerationParams(3), emptySet(), emptySet(), emptySet(), seed = s.toLong()
            ).flatMap { it.exercises }.count { it.libId in backLoaders }
            if (flagged <= unflagged) avoidedCount++
        }
        assertTrue("flagging lower back should not increase back-loading picks ($avoidedCount/$n)", avoidedCount >= (n * 0.9).toInt())
    }

    @Test
    fun priorityMuscleAddsVolume() {
        // Phase 3: granular priority adds volume to the chosen muscle.
        fun chestSets(priority: Set<MuscleGroup>) = ProgramGenerator.generate(
            GenerationParams(3, priorityMuscles = priority), emptySet(), emptySet(), emptySet(), seed = 4L
        ).flatMap { it.exercises }.filter { ExerciseLibrary.byId(it.libId)?.muscle == MuscleGroup.CHEST }.sumOf { it.sets }
        assertTrue("chest priority should raise chest volume",
            chestSets(setOf(MuscleGroup.CHEST)) > chestSets(emptySet()))
    }

    @Test
    fun pinnedExerciseAlwaysAppears() {
        // Phase 3: a pinned exercise is kept across seeds when its muscle is trained + equipment allows.
        repeat(20) { s ->
            val days = ProgramGenerator.generate(
                GenerationParams(3, pinned = setOf("goblet-squat")),
                emptySet(), emptySet(), emptySet(), seed = s.toLong()
            )
            assertTrue("goblet-squat should be pinned in (seed $s)",
                days.flatMap { it.exercises }.any { it.libId == "goblet-squat" })
        }
    }

    @Test
    fun cardioDaysAppendedAfterLiftDays() {
        val days = ProgramGenerator.generate(
            GenerationParams(3, cardioDays = 2), emptySet(), emptySet(), emptySet(), seed = 1L
        )
        assertEquals(5, days.size) // 3 lift + 2 cardio
        val cardio = days.filter { it.key.startsWith("cardio") }
        assertEquals(2, cardio.size)
        assertTrue(cardio.all { it.exercises.isEmpty() && it.archetype == "cardio" })
    }
}
