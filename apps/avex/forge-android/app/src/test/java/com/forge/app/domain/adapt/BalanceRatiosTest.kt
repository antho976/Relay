package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The extracted always-on ratio counting (Stats balance bars). The gated insight path
 * shares the same counts, so these also pin the insight's arithmetic.
 */
class BalanceRatiosTest {

    private val now = 1_000_000_000_000L
    private val dayMs = 24L * 60 * 60 * 1000

    private fun slot(id: String, muscle: MuscleGroup) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = muscle, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-12"
    )

    private fun bout(at: Long, sets: Int, skipped: Boolean = false) = ExerciseBout(
        sessionStartedAt = at, effort = null, hitFullTarget = true,
        skipped = skipped, swappedName = null,
        sets = List(sets) {
            LoggedSet(loggedExerciseId = 1, setIndex = it, weightText = "40", weightLb = 40.0, reps = 10, completedAt = at)
        }
    )

    private fun snapshot(history: Map<String, List<ExerciseBout>>) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(
            ProgramDaySnap("d1", "Day 1", listOf(
                slot("bench", MuscleGroup.CHEST),
                slot("row", MuscleGroup.BACK),
                slot("squat", MuscleGroup.QUADS),
                slot("curl-leg", MuscleGroup.HAMSTRINGS)
            ))
        ),
        sessions = emptyList(),
        exerciseHistory = history,
        prefs = PrefsSnap()
    )

    @Test
    fun countsSetsPerSide_insideWindowOnly() {
        val inWindow = now - 5 * dayMs
        val outside = now - 60 * dayMs // beyond the 28-day window
        val s = snapshot(mapOf(
            "bench" to listOf(bout(inWindow, 4), bout(outside, 9)),
            "row" to listOf(bout(inWindow, 3))
        ))
        val pushPull = InsightEngine.balanceRatios(s).first { it.key == "pushpull" }
        assertEquals(4, pushPull.setsA)
        assertEquals(3, pushPull.setsB)
        assertTrue(pushPull.balanced == true) // 4/3 ≈ 1.33 inside 0.6–1.6
    }

    @Test
    fun skippedBoutsDoNotCount() {
        val at = now - dayMs
        val s = snapshot(mapOf("bench" to listOf(bout(at, 5, skipped = true))))
        val pushPull = InsightEngine.balanceRatios(s).first { it.key == "pushpull" }
        assertEquals(0, pushPull.setsA)
    }

    @Test
    fun quadHamRatio_flagsImbalanceOutsideBand() {
        val at = now - dayMs
        val s = snapshot(mapOf(
            "squat" to listOf(bout(at, 30)),
            "curl-leg" to listOf(bout(at, 10))
        ))
        val quadHam = InsightEngine.balanceRatios(s).first { it.key == "quadham" }
        assertEquals(30, quadHam.setsA)
        assertEquals(10, quadHam.setsB)
        assertFalse(quadHam.balanced!!) // 3.0 above the 2.5 ceiling
    }

    @Test
    fun bothRatiosAlwaysPresent_evenAtZeroData() {
        val ratios = InsightEngine.balanceRatios(snapshot(emptyMap()))
        assertEquals(setOf("pushpull", "quadham"), ratios.map { it.key }.toSet())
        assertTrue(ratios.all { it.setsA == 0 && it.setsB == 0 && it.ratio == null })
    }
}
