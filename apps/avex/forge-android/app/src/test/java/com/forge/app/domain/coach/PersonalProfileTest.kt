package com.forge.app.domain.coach

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgramDaySnap
import com.forge.app.domain.adapt.ProgramSlotSnap
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.program.VolumeModel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import kotlin.math.roundToInt

/**
 * D's learned volume caps. The cap may only move on a WITHIN-lift strength response to volume
 * (H-06): which lifts happened to be in a week is exercise selection, not physiology.
 * Fixtures: UTC, one bout per lift per ISO week, weeks 7 days apart.
 */
class PersonalProfileTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 200 * day
    private val chestDefault = VolumeModel.weeklyCap.getValue(MuscleGroup.CHEST)

    private fun sets(weight: Double, n: Int) = List(n) { i ->
        LoggedSet(
            loggedExerciseId = 1, setIndex = i, weightText = "$weight",
            weightLb = weight, reps = 10, completedAt = 0
        )
    }

    private fun bout(week: Int, weight: Double, setCount: Int) = ExerciseBout(
        sessionStartedAt = (30 + week * 7) * day, effort = null, hitFullTarget = true,
        skipped = false, swappedName = null, sets = sets(weight, setCount)
    )

    private fun slot(id: String) = ProgramSlotSnap(
        exerciseId = id, name = id, muscle = MuscleGroup.CHEST, unit = ExerciseUnit.DUMBBELL,
        tags = emptyList(), targetSets = 3, repsText = "8-10"
    )

    private fun snapshot(history: Map<String, List<ExerciseBout>>) = AdaptationSnapshot(
        nowMs = now,
        program = listOf(ProgramDaySnap("upper-a", "Upper A", listOf(slot("bench"), slot("fly")))),
        sessions = emptyList(), exerciseHistory = history, prefs = PrefsSnap()
    )

    private fun capFor(history: Map<String, List<ExerciseBout>>) =
        PersonalProfile.build(snapshot(history))

    // ── H-06 regression: constant strength, varying exercise mix ───────────────

    @Test
    fun mixedLiftWeeks_withNeitherLiftMoving_leaveTheCapAtDefault() {
        // Four high-volume weeks (flat 300 lb bench + flat 50 lb fly) alternating with four
        // low-volume weeks (the same flat fly alone). The old per-muscle max-e1RM read saw ~250 lb
        // swings landing entirely in the high tier and cut the chest cap by 35%.
        val bench = (0 until 8 step 2).map { bout(it, 300.0, 4) }
        val fly = (0 until 8).map { bout(it, 50.0, 3) }
        val profile = capFor(mapOf("bench" to bench, "fly" to fly))
        assertNull(profile.volumeCaps[MuscleGroup.CHEST])
        assertEquals(chestDefault, profile.capFor(MuscleGroup.CHEST))
    }

    @Test
    fun reversedLiftMix_withNeitherLiftMoving_leavesTheCapAtDefault() {
        // The heavy lift in the LOW-volume weeks instead — the mix the old read raised the cap on.
        val fly = (0 until 8).map { bout(it, 50.0, if (it % 2 == 0) 8 else 1) }
        val bench = (1 until 8 step 2).map { bout(it, 300.0, 2) }
        val profile = capFor(mapOf("bench" to bench, "fly" to fly))
        assertNull(profile.volumeCaps[MuscleGroup.CHEST])
    }

    // ── A genuine within-lift response still moves the cap ─────────────────────

    /** Both lifts ~+9% after every high-volume week, flat after every low-volume one. */
    private val benchRising = listOf(300.0, 330.0, 330.0, 360.0, 360.0, 390.0, 390.0, 420.0, 420.0)
    private val flyRising = listOf(50.0, 55.0, 55.0, 60.0, 60.0, 65.0, 65.0, 70.0, 70.0)

    @Test
    fun strengthThatFollowsHighVolumeWeeks_raisesTheCapByTheBand() {
        // Even weeks are high volume (4 + 4 sets), odd weeks low (2 + 2).
        val bench = benchRising.mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 4 else 2) }
        val fly = flyRising.mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 4 else 2) }
        val profile = capFor(mapOf("bench" to bench, "fly" to fly))
        assertEquals((chestDefault * (1 + PersonalProfile.CAP_BAND)).roundToInt(), profile.capFor(MuscleGroup.CHEST))
    }

    @Test
    fun strengthThatFollowsLowVolumeWeeks_lowersTheCapByTheBand() {
        // Same gains, but now the even (gain-preceding) weeks are the LOW-volume ones.
        val bench = benchRising.mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 2 else 4) }
        val fly = flyRising.mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 2 else 4) }
        val profile = capFor(mapOf("bench" to bench, "fly" to fly))
        assertEquals((chestDefault * (1 - PersonalProfile.CAP_BAND)).roundToInt(), profile.capFor(MuscleGroup.CHEST))
    }

    @Test
    fun belowTheWeekGate_theCapStaysDefault() {
        val bench = benchRising.take(7).mapIndexed { i, w -> bout(i, w, if (i % 2 == 0) 4 else 2) }
        assertNull(capFor(mapOf("bench" to bench)).volumeCaps[MuscleGroup.CHEST])
    }
}
