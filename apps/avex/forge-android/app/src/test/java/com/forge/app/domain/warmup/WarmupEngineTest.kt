package com.forge.app.domain.warmup

import com.forge.app.domain.adapt.E1rm
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The warmup engine's contract. These pin the claims the doctrine comments make, so a later tweak
 * to the ramp table has to argue with a failing test rather than quietly change what the app
 * prescribes.
 */
class WarmupEngineTest {

    private fun exercise(
        id: String = "e1",
        name: String = "Goblet Squat",
        muscle: MuscleGroup = MuscleGroup.QUADS,
        unit: ExerciseUnit = ExerciseUnit.WEIGHT,
        isCompound: Boolean = true,
        workingLoad: Double? = 200.0,
        targetReps: Int = 8,
        loadStep: Double? = null
    ) = WarmupExercise(id, name, muscle, unit, isCompound, workingLoad, targetReps, loadStep)

    // ── Intensity ────────────────────────────────────────────────────────────────

    @Test
    fun `intensity is the inverse of the app's own Epley convention`() {
        // A warmup derived from one 1RM model and a plateau call derived from another would
        // disagree about the same set, so this must track E1rm exactly.
        listOf(1, 3, 5, 8, 10, 15, 20).forEach { reps ->
            val viaE1rm = 100.0 / E1rm.epley(100.0, reps)
            assertEquals("reps=$reps", viaE1rm, WarmupEngine.intensityOf(reps), 1e-9)
        }
    }

    @Test
    fun `intensity lands on the standard load-rep anchors`() {
        assertEquals(0.857, WarmupEngine.intensityOf(5), 0.005)
        assertEquals(0.750, WarmupEngine.intensityOf(10), 0.005)
        assertEquals(0.667, WarmupEngine.intensityOf(15), 0.005)
    }

    @Test
    fun `intensity never divides by zero on a nonsense rep target`() {
        assertEquals(WarmupEngine.intensityOf(1), WarmupEngine.intensityOf(0), 1e-9)
        assertEquals(WarmupEngine.intensityOf(1), WarmupEngine.intensityOf(-5), 1e-9)
    }

    // ── Ramp depth ───────────────────────────────────────────────────────────────

    @Test
    fun `ramp depth rises with working intensity`() {
        val counts = listOf(20, 15, 10, 5, 2).map { reps ->
            WarmupEngine.rampSetCount(WarmupEngine.intensityOf(reps), isCompound = true, alreadyWarm = false)
        }
        assertEquals(listOf(1, 2, 3, 4, 5), counts)
    }

    @Test
    fun `a warm muscle collapses the ramp`() {
        // Heavy compound on an already-worked muscle keeps one rung to find the groove.
        assertEquals(
            1,
            WarmupEngine.rampSetCount(WarmupEngine.intensityOf(3), isCompound = true, alreadyWarm = true)
        )
        // Everything lighter, or single-joint, needs nothing.
        assertEquals(
            0,
            WarmupEngine.rampSetCount(WarmupEngine.intensityOf(10), isCompound = true, alreadyWarm = true)
        )
        assertEquals(
            0,
            WarmupEngine.rampSetCount(WarmupEngine.intensityOf(3), isCompound = false, alreadyWarm = true)
        )
    }

    @Test
    fun `isolation work never gets a compound's full ladder`() {
        listOf(3, 8, 12, 20).forEach { reps ->
            val intensity = WarmupEngine.intensityOf(reps)
            val iso = WarmupEngine.rampSetCount(intensity, isCompound = false, alreadyWarm = false)
            val compound = WarmupEngine.rampSetCount(intensity, isCompound = true, alreadyWarm = false)
            assertTrue("reps=$reps", iso <= 2 && iso <= compound)
        }
    }

    // ── Ramp shape ───────────────────────────────────────────────────────────────

    @Test
    fun `ramp climbs, stays under the working load, and never repeats a rung`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = 225.0, targetReps = 5))
        assertTrue("expected a real ladder", ramp.size >= 4)
        val loads = ramp.map { it.load!! }
        assertEquals(loads.sorted(), loads)
        assertEquals(loads.distinct(), loads)
        assertTrue(loads.all { it < 225.0 })
    }

    @Test
    fun `ramp reps fall as the load rises so no warmup set is hard`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = 225.0, targetReps = 3))
        val reps = ramp.map { it.reps }
        assertEquals(reps.sortedDescending(), reps)
        assertTrue("top rung should be a single or a double", reps.last() <= 2)
    }

    @Test
    fun `ramp reps follow absolute intensity, not share of the working load`() {
        // Same 90%-of-working rung, two very different sets. Under a heavy triple it is genuinely
        // heavy and earns low reps; under a set of fifteen it is still light work.
        val heavy = WarmupEngine.rampReps(0.90, WarmupEngine.intensityOf(3))
        val light = WarmupEngine.rampReps(0.90, WarmupEngine.intensityOf(15))
        assertTrue("heavy=$heavy light=$light", heavy < light)
    }

    @Test
    fun `a light high-rep lift is never given a near-maximal warmup double`() {
        // The bug this pins: 60 lb x 2 prescribed before a working set of 70 lb x 8.
        val ramp = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.DUMBBELL, workingLoad = 70.0, targetReps = 8)
        )
        assertTrue(ramp.isNotEmpty())
        assertTrue("top rung was ${ramp.last().reps} reps", ramp.last().reps >= 3)
    }

    @Test
    fun `a heavy ramp tapers strictly, never repeating a rep count`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = 315.0, targetReps = 3))
        val reps = ramp.map { it.reps }
        assertEquals("got $reps", reps.distinct(), reps)
        assertEquals("got $reps", reps.sortedDescending(), reps)
    }

    @Test
    fun `the last jump into the working set is the smallest one`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = 300.0, targetReps = 3))
        val loads = ramp.map { it.load!! } + 300.0
        val gaps = loads.zipWithNext { a, b -> b - a }
        assertEquals("final gap should be the tightest", gaps.min(), gaps.last(), 1e-9)
    }

    @Test
    fun `rest lengthens toward the top of the ramp`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = 300.0, targetReps = 3))
        val rests = ramp.map { it.restSeconds }
        assertEquals(rests.sorted(), rests)
    }

    // ── Loadability ──────────────────────────────────────────────────────────────

    @Test
    fun `dumbbell loads land on the 5 lb grid`() {
        val ramp = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.DUMBBELL, workingLoad = 70.0, targetReps = 8)
        )
        assertTrue(ramp.isNotEmpty())
        ramp.forEach { assertEquals("load ${it.load}", 0.0, it.load!! % 5.0, 1e-9) }
    }

    @Test
    fun `plate machines are prescribed in whole plates`() {
        val ramp = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.PLATES, workingLoad = 8.0, targetReps = 10)
        )
        assertTrue(ramp.isNotEmpty())
        ramp.forEach { assertEquals("plates ${it.load}", 0.0, it.load!! % 1.0, 1e-9) }
    }

    @Test
    fun `a kg user gets loads that round to whole kilos`() {
        val kgStep = 2.5 / 0.45359237
        val ramp = WarmupEngine.rampFor(
            exercise(workingLoad = 100.0 / 0.45359237, targetReps = 5, loadStep = kgStep)
        )
        assertTrue(ramp.isNotEmpty())
        ramp.forEach {
            val kg = it.load!! * 0.45359237
            // Compare against the nearest 2.5 step rather than a modulo: the lb round-trip lands on
            // 59.999999 for 60 kg, and modulo turns that rounding dust into a whole step.
            assertEquals("kg $kg", Math.round(kg / 2.5) * 2.5, kg, 1e-6)
        }
    }

    @Test
    fun `a light lift on a coarse grid never repeats the same loadable weight`() {
        // 20 lb working on the 5 lb dumbbell grid: several fractions round together, and the
        // duplicates must be dropped rather than prescribed twice.
        val ramp = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.DUMBBELL, workingLoad = 20.0, targetReps = 5)
        )
        val loads = ramp.map { it.load!! }
        assertEquals(loads.distinct(), loads)
        assertTrue(loads.all { it in 5.0..15.0 })
    }

    @Test
    fun `percent is reported against the rounded load actually prescribed`() {
        val ramp = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.DUMBBELL, workingLoad = 70.0, targetReps = 8)
        )
        ramp.forEach {
            assertEquals(Math.round((it.load!! / 70.0) * 100).toInt(), it.percentOfWorking)
        }
    }

    // ── Degrading honestly ───────────────────────────────────────────────────────

    @Test
    fun `an unknown working load gives a rehearsal set, never an invented number`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = null))
        assertEquals(1, ramp.size)
        assertNull(ramp.single().load)
        assertEquals(0, ramp.single().percentOfWorking)
    }

    @Test
    fun `bodyweight compounds rehearse and bodyweight isolation does not`() {
        val compound = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.BODYWEIGHT, workingLoad = null, isCompound = true)
        )
        assertEquals(1, compound.size)
        val isolation = WarmupEngine.rampFor(
            exercise(unit = ExerciseUnit.BODYWEIGHT, workingLoad = null, isCompound = false)
        )
        assertTrue(isolation.isEmpty())
    }

    @Test
    fun `a warm muscle with no known load gets nothing at all`() {
        val ramp = WarmupEngine.rampFor(exercise(workingLoad = null), alreadyWarm = true)
        assertTrue(ramp.isEmpty())
    }

    // ── Whole protocol ───────────────────────────────────────────────────────────

    @Test
    fun `the protocol runs raise then mobilize then ramp`() {
        val protocol = WarmupEngine.build(
            listOf(
                exercise(id = "a", muscle = MuscleGroup.QUADS, targetReps = 5),
                exercise(id = "b", name = "Leg Curl", muscle = MuscleGroup.HAMSTRINGS, isCompound = false)
            )
        )
        val phases = protocol.steps.map { it.phase }
        assertEquals(WarmupPhase.RAISE, phases.first())
        assertEquals(phases.sortedBy { it.ordinal }, phases)
        assertTrue(phases.contains(WarmupPhase.MOBILIZE))
        assertTrue(phases.contains(WarmupPhase.RAMP))
    }

    @Test
    fun `only the first exercise is ramped, because the effect decays`() {
        val protocol = WarmupEngine.build(
            listOf(
                exercise(id = "a", name = "Squat", targetReps = 5),
                exercise(id = "b", name = "Romanian Deadlift", muscle = MuscleGroup.HAMSTRINGS, targetReps = 5),
                exercise(id = "c", name = "Leg Curl", muscle = MuscleGroup.HAMSTRINGS, targetReps = 12)
            )
        )
        val ramped = protocol.steps.filterIsInstance<WarmupRampSet>().map { it.exerciseName }.distinct()
        assertEquals(listOf("Squat"), ramped)
    }

    @Test
    fun `mobilize drills only cover muscles the session actually trains`() {
        val protocol = WarmupEngine.build(
            listOf(exercise(id = "a", name = "Bench", muscle = MuscleGroup.CHEST, targetReps = 8))
        )
        val drills = protocol.steps.filterIsInstance<WarmupDrill>()
            .filter { it.phase == WarmupPhase.MOBILIZE }
        assertTrue(drills.isNotEmpty())
        // No leg drills crept into a chest-only session.
        assertTrue(drills.none { it.id.contains("squat") || it.id.contains("leg-swing") })
    }

    @Test
    fun `mobilize work is capped so the warmup cannot become a second workout`() {
        val everything = MuscleGroup.entries.mapIndexed { i, m ->
            exercise(id = "e$i", muscle = m, targetReps = 10)
        }
        val drills = WarmupEngine.build(everything).steps
            .filter { it.phase == WarmupPhase.MOBILIZE }
        assertTrue("got ${drills.size}", drills.size <= 2)
    }

    @Test
    fun `a lower-body session gets the longer pulse raiser`() {
        val lower = WarmupEngine.build(listOf(exercise(muscle = MuscleGroup.QUADS)))
            .steps.first { it.phase == WarmupPhase.RAISE }
        val upper = WarmupEngine.build(listOf(exercise(muscle = MuscleGroup.CHEST)))
            .steps.first { it.phase == WarmupPhase.RAISE }
        assertTrue(lower.seconds > upper.seconds)
    }

    @Test
    fun `custom drills replace the generated ones but keep the ramp`() {
        val protocol = WarmupEngine.build(
            exercises = listOf(exercise(name = "Squat", targetReps = 5)),
            customDrills = listOf("Treadmill 5 min", "Hip openers")
        )
        val drills = protocol.steps.filterIsInstance<WarmupDrill>()
        assertEquals(listOf("Treadmill 5 min", "Hip openers"), drills.map { it.name })
        assertTrue(protocol.steps.any { it is WarmupRampSet })
    }

    @Test
    fun `no exercises means no warmup to gate on`() {
        assertTrue(WarmupEngine.build(emptyList()).isEmpty)
        assertTrue(WarmupEngine.build(emptyList(), customDrills = emptyList()).isEmpty)
    }

    @Test
    fun `the whole warmup stays short enough to actually do`() {
        val protocol = WarmupEngine.build(
            listOf(
                exercise(id = "a", name = "Squat", targetReps = 5),
                exercise(id = "b", name = "RDL", muscle = MuscleGroup.HAMSTRINGS, targetReps = 8)
            )
        )
        assertTrue("got ${protocol.totalMinutes}", protocol.totalMinutes in 3..8)
        // The user reads this in one glance on the way to the rack, so the row count is capped too.
        assertTrue("got ${protocol.steps.size} rows", protocol.steps.size <= 8)
    }

    @Test
    fun `every rendered string obeys the copy rules`() {
        val protocol = WarmupEngine.build(
            MuscleGroup.entries.mapIndexed { i, m -> exercise(id = "e$i", muscle = m, targetReps = 8) }
        )
        val strings = protocol.steps.filterIsInstance<WarmupDrill>()
            .flatMap { listOf(it.name, it.prescription, it.why) }
        assertTrue(strings.isNotEmpty())
        strings.forEach { s ->
            assertTrue("em dash in \"$s\"", !s.contains('—'))
            assertTrue("exclamation in \"$s\"", !s.contains('!'))
        }
        // The "why" is a caption, not a paragraph (DESIGN §4.3).
        protocol.steps.filterIsInstance<WarmupDrill>().forEach {
            assertTrue("why too long: \"${it.why}\"", it.why.split(" ").size <= 12)
        }
    }

    @Test
    fun `every mobility drill carries a dose and a reason`() {
        val drills = WarmupEngine.build(
            MuscleGroup.entries.mapIndexed { i, m -> exercise(id = "e$i", muscle = m) }
        ).steps.filterIsInstance<WarmupDrill>()
        drills.forEach {
            assertTrue("no dose on ${it.name}", it.prescription.isNotBlank())
            assertTrue("no reason on ${it.name}", it.why.isNotBlank())
            assertNotNull(it.id)
        }
    }
}
