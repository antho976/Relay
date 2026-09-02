package com.forge.app.ui.gym.freestyle

import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * A freestyle draft stores what was TYPED, in the display unit, as raw text — which is meaningless
 * on its own. Draft "100" while the app is set to pounds, change the setting to kilograms, resume,
 * and the same "100" was saved as 100 kg: a 220 lb set, in the history and in every aggregate built
 * on it, from a number the user never changed.
 *
 * Robolectric because the draft round-trips through `org.json`, which the Android unit-test stub
 * jar leaves unimplemented — a plain JVM test gets "Method put in org.json.JSONObject not mocked"
 * rather than an answer about the draft.
 */
@RunWith(RobolectricTestRunner::class)
class FreestyleDraftUnitTest {

    private fun draft(unit: WeightUnit?, weight: String = "100") = FreestyleDraft(
        openedAtMs = 1_000L,
        exercises = listOf(
            FreestyleDraftExercise(libId = "bench", sets = listOf(FreestyleDraftSet(weight, "8")))
        ),
        unitLabel = unit?.label
    )

    @Test
    fun `the unit survives the json round trip`() {
        val restored = FreestyleDraft.fromJson(draft(WeightUnit.KG).toJson())!!
        assertEquals("kg", restored.unitLabel)
        assertEquals("100", restored.exercises.single().sets.single().weight)
    }

    @Test
    fun `a draft from a build that never recorded the unit reads as unknown`() {
        // The field is additive and optional on purpose: a schema bump would have DISCARDED every
        // in-progress log on upgrade, which is a worse outcome than the bug for anyone mid-workout.
        val withoutUnit = """{"schema":3,"openedAtMs":1000,"exercises":[{"libId":"bench","sets":[{"w":"100","r":"8"}]}]}"""
        val restored = FreestyleDraft.fromJson(withoutUnit)!!
        assertNull(restored.unitLabel)
        // And it restores verbatim, which is exactly the old behaviour — the honest answer for a
        // draft that never said what it meant.
        assertEquals("100", restored.weightTextIn("100", WeightUnit.KG))
    }

    @Test
    fun `text drafted in pounds is re-expressed in kilograms`() {
        // 100 lb ≈ 45.4 kg. The user resumes and sees the weight they lifted, not the digits.
        assertEquals("45.4", draft(WeightUnit.LB).weightTextIn("100", WeightUnit.KG))
    }

    @Test
    fun `text drafted in kilograms is re-expressed in pounds`() {
        assertEquals("220.5", draft(WeightUnit.KG).weightTextIn("100", WeightUnit.LB))
    }

    @Test
    fun `an unchanged unit leaves the text exactly as typed`() {
        assertEquals("137.25", draft(WeightUnit.LB).weightTextIn("137.25", WeightUnit.LB))
    }

    @Test
    fun `blank and unparseable text is left alone`() {
        assertEquals("", draft(WeightUnit.LB).weightTextIn("", WeightUnit.KG))
        assertEquals("BW", draft(WeightUnit.LB).weightTextIn("BW", WeightUnit.KG))
    }

    @Test
    fun `an unrecognised unit label is treated as unknown rather than guessed`() {
        val odd = FreestyleDraft(openedAtMs = 1L, exercises = emptyList(), unitLabel = "stone")
        assertEquals("100", odd.weightTextIn("100", WeightUnit.KG))
    }

    @Test
    fun `a draft carrying the same move twice restores it once`() {
        // The logger keys its lazy list on libId, so a doubled row is a crash on every resume.
        val doubled = """{"schema":3,"openedAtMs":1000,"exercises":[""" +
            """{"libId":"bench","sets":[{"w":"100","r":"8"}]},""" +
            """{"libId":"bench","sets":[{"w":"110","r":"5"}]}]}"""
        val restored = FreestyleDraft.fromJson(doubled)!!
        assertEquals(listOf("bench"), restored.exercises.map { it.libId })
        assertEquals("100", restored.exercises.single().sets.single().weight)
    }
}
