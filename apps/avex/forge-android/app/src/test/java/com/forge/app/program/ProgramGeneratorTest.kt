package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** JVM unit tests for the pure generator engine (program-unlock Phase 2). */
class ProgramGeneratorTest {

    /** The MWM-989 home-gym preset (dumbbells + bench + cable + machine stations). */
    private val mwm = setOf(Equipment.DUMBBELLS, Equipment.BENCH, Equipment.CABLE, Equipment.MACHINE)

    @Test
    fun mwmPresetCoversEveryMuscleEveryWeek() {
        // The MWM-989 preset must train ALL muscle groups weekly on every standard split — no
        // starved slots, no missing muscle.
        (3..6).forEach { d ->
            listOf(1L, 2L, 3L).forEach { seed ->
                val muscles = ProgramGenerator.generate(GenerationParams(d), mwm, emptySet(), emptySet(), seed = seed)
                    .flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId)?.muscle }.toSet()
                assertEquals("$d-day MWM plan (seed $seed) should cover every muscle",
                    MuscleGroup.entries.toSet(), muscles)
            }
        }
    }

    @Test
    fun noSingleMuscleDominatesADay() {
        // A day must never stack 4+ exercises of the same muscle (3 is the pull day's back, by design).
        (1..7).forEach { d ->
            ProgramGenerator.generate(GenerationParams(d), mwm, emptySet(), emptySet(), seed = 17L)
                .forEach { day ->
                    val worst = day.exercises
                        .groupingBy { ExerciseLibrary.byId(it.libId)?.muscle }.eachCount()
                        .maxByOrNull { it.value }
                    assertTrue("${day.key} ($d-day) has ${worst?.value}× ${worst?.key}",
                        (worst?.value ?: 0) <= 3)
                }
        }
    }

    @Test
    fun twoDayPlanCoversGlutesRearDeltsAndCalves() {
        // Regression: both full-body days used the same template, so a 2-day plan trained glutes,
        // rear delts and calves ZERO times a week. Full Body B now carries them.
        val muscles = ProgramGenerator.generate(GenerationParams(2), mwm, emptySet(), emptySet(), seed = 6L)
            .flatMap { it.exercises }.mapNotNull { ExerciseLibrary.byId(it.libId)?.muscle }.toSet()
        listOf(MuscleGroup.GLUTES, MuscleGroup.REAR_DELTS, MuscleGroup.CALVES).forEach { m ->
            assertTrue("2-day plan should train $m", m in muscles)
        }
    }

    @Test
    fun equippedHeavySlotsRarelyLeadWithBodyweightAmrap() {
        // An equipped user's heavy chest slot should be a loadable press, not AMRAP push-ups —
        // pickBias + the non-loadable STRENGTH penalty keep them out. Statistical over many seeds.
        val n = 80
        val amrapLeads = (0 until n).count { s ->
            val lead = ProgramGenerator.generate(GenerationParams(3), mwm, emptySet(), emptySet(), seed = s.toLong())
                .first { it.key == "push" }.exercises.first()
            ExerciseLibrary.byId(lead.libId)!!.defaultReps == "AMRAP"
        }
        assertTrue("push day led with AMRAP $amrapLeads/$n times", amrapLeads <= n / 10)
    }

    @Test
    fun lightDumbbellCeilingSteersHeavySlotsToTheStack() {
        // With light DBs (25 lb max), the heavy chest slot should land on a plate-stack movement
        // (machine press) far more often than without a ceiling — the only real overload path.
        fun stackLeads(dbMaxLb: Double?): Int = (0 until 80).count { s ->
            val lead = ProgramGenerator.generate(
                GenerationParams(3, dbMaxLb = dbMaxLb), mwm, emptySet(), emptySet(), seed = s.toLong()
            ).first { it.key == "push" }.exercises.first()
            ExerciseLibrary.byId(lead.libId)!!.unit == ExerciseUnit.PLATES
        }
        val light = stackLeads(25.0)
        val none = stackLeads(null)
        assertTrue("light-DB plans should lead with the stack more often ($light vs $none)", light > none)
    }

    @Test
    fun coachAvoidedMovementsArePickedLessOften() {
        // A movement the coach tried that failed (CoachGenBias.avoid) is softly steered around.
        fun benchLeads(avoid: Set<String>): Int = (0 until 80).count { s ->
            ProgramGenerator.generate(
                GenerationParams(3, avoid = avoid), mwm, emptySet(), emptySet(), seed = s.toLong()
            ).first { it.key == "push" }.exercises.any { it.libId == "incline-db-bench-press" }
        }
        val avoided = benchLeads(setOf("incline-db-bench-press"))
        val normal = benchLeads(emptySet())
        assertTrue("avoided movement should appear less ($avoided vs $normal)", avoided < normal)
    }

    @Test
    fun volumeBiasFlowsThroughToGeneratedSets() {
        fun chestSets(bias: Map<MuscleGroup, Int>) = ProgramGenerator.generate(
            GenerationParams(3, volumeBias = bias), mwm, emptySet(), emptySet(), seed = 4L
        ).flatMap { it.exercises }
            .filter { ExerciseLibrary.byId(it.libId)?.muscle == MuscleGroup.CHEST }
            .sumOf { it.sets }
        assertEquals(chestSets(emptyMap()) + 1, chestSets(mapOf(MuscleGroup.CHEST to 1)))
    }

    @Test
    fun nicheAccessoriesArePickedLessOftenThanDefaults() {
        // pickBias: the inner-thigh adductor cross (0.35) must headline quad slots clearly less
        // often than the leg extension (1.0). Statistical over many seeds.
        var innerThigh = 0
        var legExtension = 0
        repeat(100) { s ->
            ProgramGenerator.generate(GenerationParams(3), mwm, emptySet(), emptySet(), seed = s.toLong())
                .flatMap { it.exercises }.forEach {
                    when (it.libId) {
                        "mwm-inner-thigh" -> innerThigh++
                        "leg-extension" -> legExtension++
                    }
                }
        }
        assertTrue("inner thigh ($innerThigh) should appear less than leg extension ($legExtension)",
            innerThigh < legExtension)
    }

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
    fun liftDaysHaveFiveToSevenExercises() {
        // Phase 4 tuning: each session should be a "standard" 5-7 movements (with full equipment).
        val days = ProgramGenerator.generate(GenerationParams(3), emptySet(), emptySet(), emptySet(), seed = 11L)
        days.forEach { d ->
            assertTrue("${d.key} has ${d.exercises.size} exercises", d.exercises.size in 5..7)
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
    fun multiDaySplitVariesMovementsAcrossDays() {
        // A muscle trained on two days (e.g. 4-day Upper/Lower) should mostly get DIFFERENT movements
        // across the week, not the same lift twice.
        listOf(11L, 22L, 33L).forEach { seed ->
            val ids = ProgramGenerator.generate(GenerationParams(4), emptySet(), emptySet(), emptySet(), seed = seed)
                .flatMap { it.exercises }.map { it.libId }
            val distinctRatio = ids.toSet().size.toDouble() / ids.size
            assertTrue("too many repeated movements across the week ($distinctRatio @ seed $seed)", distinctRatio >= 0.75)
        }
    }

    @Test
    fun bodyweightOnlyNeverProducesAnEmptyLiftDay() {
        // Regression: the "Bodyweight only" preset used to leave whole days (e.g. Pull) empty because
        // the library had no bodyweight back/legs/arms movements. Every muscle now has a bodyweight
        // fallback, so no lift day is ever starved — for any day-count.
        val bodyweight = setOf(Equipment.BODYWEIGHT_ONLY)
        (1..7).forEach { d ->
            val days = ProgramGenerator.generate(GenerationParams(d), bodyweight, emptySet(), emptySet(), seed = 3L)
            days.filterNot { it.key.startsWith("cardio") }.forEach { day ->
                assertTrue("${day.key} is empty on a bodyweight-only $d-day plan", day.exercises.isNotEmpty())
            }
        }
    }

    @Test
    fun equippedUserNeverGetsBodyweightFallbacks() {
        // fallbackOnly movements must stay out of an equipped user's picks (no bodyweight squat for a
        // full-gym lifter) — they only rescue starved muscles.
        val full = Equipment.entries.toSet()
        val ids = ProgramGenerator.generate(GenerationParams(4), full, emptySet(), emptySet(), seed = 9L)
            .flatMap { it.exercises }.map { it.libId }
        assertTrue("a fully-equipped plan should never use a fallback movement",
            ids.none { ExerciseLibrary.byId(it)?.fallbackOnly == true })
    }

    @Test
    fun defaultFourDaySplitTrainsRearDelts() {
        // Regression: the generated default 4-day Upper/Lower used to allocate ZERO rear-delt volume.
        val rearDeltSets = ProgramGenerator.generate(GenerationParams(4), emptySet(), emptySet(), emptySet(), seed = 5L)
            .flatMap { it.exercises }
            .filter { ExerciseLibrary.byId(it.libId)?.muscle == MuscleGroup.REAR_DELTS }
            .sumOf { it.sets }
        assertTrue("default 4-day should train rear delts", rearDeltSets > 0)
    }

    @Test
    fun lowDayCountFullBodyCoversArmsAndCore() {
        // Regression: 1- and 2-day full-body plans used to give biceps/triceps/core zero weekly volume.
        listOf(1, 2).forEach { d ->
            val byMuscle = ProgramGenerator.generate(GenerationParams(d), emptySet(), emptySet(), emptySet(), seed = 2L)
                .flatMap { it.exercises }
                .groupBy { ExerciseLibrary.byId(it.libId)?.muscle }
                .mapValues { (_, exs) -> exs.sumOf { it.sets } }
            listOf(MuscleGroup.BICEPS, MuscleGroup.TRICEPS, MuscleGroup.CORE).forEach { m ->
                assertTrue("$d-day full body should train $m", (byMuscle[m] ?: 0) > 0)
            }
        }
    }

    @Test
    fun pinnedExerciseNotForcedWhenItStressesAFlaggedArea() {
        // Phase 3 safety: a pin must not hard-place a movement that stresses a flagged problem area
        // when a safer alternative exists. db-overhead-press stresses SHOULDERS; lateral raises don't.
        fun appearances(flagShoulders: Boolean): Int = (0 until 20).count { s ->
            ProgramGenerator.generate(
                GenerationParams(
                    3, pinned = setOf("db-overhead-press"),
                    problemAreas = if (flagShoulders) setOf(ProblemArea.SHOULDERS) else emptySet()
                ),
                emptySet(), emptySet(), emptySet(), seed = s.toLong()
            ).flatMap { it.exercises }.any { it.libId == "db-overhead-press" }
        }
        val withoutFlag = appearances(false)
        val withFlag = appearances(flagShoulders = true)
        assertTrue("pin should be honored when not contraindicated ($withoutFlag/20)", withoutFlag >= 18)
        assertTrue("flagging shoulders should stop the pin being forced ($withFlag vs $withoutFlag)",
            withFlag < withoutFlag)
    }

    @Test
    fun beginnerDeloadLightensEvenLowVolumeMuscles() {
        // Regression: deload floored at MIN_SETS, so a beginner's already-light accessory muscles
        // (e.g. calves, one PUMP slot) didn't drop at all. They should now.
        fun calfSets(deload: Boolean) = ProgramGenerator.generate(
            GenerationParams(3, experience = "beginner", deload = deload),
            emptySet(), emptySet(), emptySet(), seed = 8L
        ).flatMap { it.exercises }
            .filter { ExerciseLibrary.byId(it.libId)?.muscle == MuscleGroup.CALVES }
            .sumOf { it.sets }
        assertTrue("beginner deload should reduce light accessory volume", calfSets(true) < calfSets(false))
    }
}
